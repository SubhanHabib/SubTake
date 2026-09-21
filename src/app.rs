use crate::{AppTray, EditorWindow, Field, RecordingLauncher, RecordingOptions, Region, Wallpaper};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use subtake_native::{
    export::{self, ExportSettings},
    media::{self, MediaInfo},
    platform::{self, Recording},
    project::{History, Project, parse_srt},
    render::Scene,
    timeline::{self, n},
};

thread_local! {static STATE:RefCell<Option<(Rc<RefCell<App>>,slint::Weak<EditorWindow>)>>=const{RefCell::new(None)};}
fn with_app(f: impl FnOnce(&mut App, &EditorWindow)) {
    STATE.with(|slot| {
        if let Some((state, weak)) = slot.borrow().as_ref() {
            if let Some(ui) = weak.upgrade() {
                let mut app = state.borrow_mut();
                f(&mut app, &ui);
                app.sync_launcher(&ui);
            }
        }
    });
}
// Native modal dialogs pump timers while their caller still holds App's borrow.
// Background refreshes may skip a tick; user actions must not be silently dropped.
fn when_idle<T>(state: &RefCell<T>, refresh: impl FnOnce(&T)) {
    if let Ok(state) = state.try_borrow() {
        refresh(&state);
    }
}

fn post(f: impl FnOnce(&mut App, &EditorWindow) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}
fn report(ui: &EditorWindow, result: Result<()>) {
    if let Err(error) = result {
        ui.set_status(format!("{error:#}").into());
    }
}

// Winit creates the AppKit host asynchronously on a cold accessory launch.
// Retry native panel configuration briefly instead of showing a generic window
// first or emitting a spurious handle error.
fn position_launcher_when_ready(attempt: u8) {
    Timer::single_shot(Duration::from_millis(20), move || {
        with_app(|s, _| {
            if let Some(launcher) = &s.launcher {
                if platform::position_launcher(launcher.window()).is_err() && attempt < 9 {
                    position_launcher_when_ready(attempt + 1);
                }
            }
        });
    });
}

struct FrameRequest {
    project: Project,
    path: PathBuf,
    info: MediaInfo,
    time: f64,
    epoch: u64,
    selected: Option<(String, String)>,
    source_revision: u64,
    panel: String,
    width: u32,
    height: u32,
}
struct Preview {
    slot: Arc<(Mutex<Option<FrameRequest>>, Condvar)>,
}
impl Preview {
    fn new() -> Self {
        let slot: Arc<(Mutex<Option<FrameRequest>>, Condvar)> =
            Arc::new((Mutex::new(None), Condvar::new()));
        let worker = slot.clone();
        std::thread::spawn(move || {
            let mut scene: Option<(PathBuf, u32, u32, u64, Scene)> = None;
            loop {
                let request = {
                    let (lock, wake) = &*worker;
                    let mut pending = lock.lock().unwrap();
                    while pending.is_none() {
                        pending = wake.wait(pending).unwrap()
                    }
                    pending.take().unwrap()
                };
                let result = (|| -> Result<(Vec<u8>, Option<[f32; 5]>)> {
                    if scene.as_ref().is_none_or(|(path, w, h, revision, _)| {
                        *path != request.path
                            || *w != request.width
                            || *h != request.height
                            || *revision != request.source_revision
                    }) {
                        scene = Some((
                            request.path.clone(),
                            request.width,
                            request.height,
                            request.source_revision,
                            Scene::new(
                                request.path.clone(),
                                request.info.clone(),
                                request.width,
                                request.height,
                            )?,
                        ));
                    }
                    let scene = &mut scene.as_mut().unwrap().4;
                    let mut drawing = request.project.clone();
                    if request.panel == "Crop" {
                        drawing.set("cropRegion", json!({"x":0,"y":0,"width":1,"height":1}));
                        drawing.set("padding", json!(0));
                        drawing.set("borderRadius", json!(0));
                        drawing.set("shadowIntensity", json!(0));
                        drawing.set("wallpaper", json!("#000000"));
                        drawing.set("showCursor", json!(false));
                        for key in ["zoomRegions", "annotationRegions", "autoCaptions"] {
                            drawing.set(key, json!([]));
                        }
                        drawing.set("webcam", json!({"enabled":false}));
                    }
                    let pixels = scene.render(&drawing, request.time)?;
                    let bounds = scene.edit_bounds(
                        &request.project,
                        request.time,
                        request.selected.as_ref(),
                        &request.panel,
                    );
                    Ok((pixels, bounds))
                })();
                post(move |s, ui| {
                    if s.epoch != request.epoch {
                        return;
                    }
                    match result {
                        Ok((pixels, bounds)) => {
                            ui.set_edit_visible(bounds.is_some());
                            if let Some([x, y, w, h, scale]) = bounds {
                                ui.set_edit_x(x);
                                ui.set_edit_y(y);
                                ui.set_edit_width(w);
                                ui.set_edit_height(h);
                                ui.set_edit_scale(scale);
                            }
                            let buffer =
                                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                    &pixels,
                                    request.width,
                                    request.height,
                                );
                            ui.set_preview(slint::Image::from_rgba8(buffer));
                        }
                        Err(e) => {
                            s.stop(ui);
                            ui.set_status(format!("Preview: {e:#}").into());
                        }
                    }
                });
            }
        });
        Self { slot }
    }
    fn request(&self, r: FrameRequest) {
        let (lock, wake) = &*self.slot;
        *lock.lock().unwrap() = Some(r);
        wake.notify_one();
    }
}

pub struct App {
    preferences: subtake_native::preferences::Preferences,
    history: Option<History>,
    document: Option<PathBuf>,
    source: Option<PathBuf>,
    info: Option<MediaInfo>,
    selected: Option<(String, String)>,
    clipboard: Vec<(String, Value, Option<Value>)>,
    extra_selection: Vec<(String, String)>,
    recovery: subtake_native::recovery::Store,
    recovery_timer: Timer,
    recovery_origin: Option<PathBuf>,
    recoveries: Vec<PathBuf>,
    wallpapers: Vec<PathBuf>,
    presets: Vec<PathBuf>,
    library: Vec<PathBuf>,
    library_query: String,
    fresh_recording: Option<PathBuf>,
    preview: Preview,
    epoch: u64,
    source_time: f64,
    source_revision: u64,
    playback: Timer,
    started: Option<(Arc<AtomicU64>, f64)>,
    audio_cancel: Arc<AtomicBool>,
    job_cancel: Arc<AtomicBool>,
    sources: Vec<Value>,
    devices: Value,
    launcher: Option<RecordingLauncher>,
    launcher_options: Option<RecordingOptions>,
    tray: Option<AppTray>,
    editor_shown: bool,
    capture_started: Option<std::time::Instant>,
    pause_started: Option<std::time::Instant>,
    paused_total: Duration,
    recording_watch: Timer,
    recording: Option<Recording>,
    hotkeys: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey_ids: Vec<(u32, String)>,
    last_export: Option<PathBuf>,
}
impl App {
    #[cfg(target_os = "macos")]
    fn ensure_tray_visible(&self) -> Result<()> {
        // macOS uses the retained AppKit status item installed after the event
        // loop starts. Keeping the Slint tray hidden avoids a second menu-bar
        // item while preserving the cross-platform tray object.
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn ensure_tray_visible(&self) -> Result<()> {
        if let Some(tray) = &self.tray {
            tray.show()?;
        }
        Ok(())
    }

    fn apply_launcher_option(&mut self, ui: &EditorWindow, key: &str, value: &str) {
        if ui.get_busy() || ui.get_recording() {
            return;
        }
        match key {
            "source" => {
                if let Ok(index) = value.parse::<i32>() {
                    if index >= 0 && (index as usize) < self.sources.len() {
                        ui.set_source_index(index);
                    }
                }
            }
            "camera" => ui.set_capture_camera(value == "true"),
            "microphone" => ui.set_capture_mic(value == "true"),
            "system-audio" => ui.set_capture_system(value == "true"),
            "camera-device" => ui.set_camera_index(value.parse().unwrap_or(0)),
            "microphone-device" => ui.set_microphone_index(value.parse().unwrap_or(0)),
            "countdown" => {
                if let Ok(seconds) = value.parse::<u32>() {
                    self.preferences.countdown_seconds = seconds.min(10);
                    report(ui, self.preferences.save());
                }
            }
            _ => {}
        }
    }

    fn sync_launcher_options(&self, ui: &EditorWindow) {
        let (Some(launcher), Some(options)) = (&self.launcher, &self.launcher_options) else {
            return;
        };
        options.set_panel(launcher.get_panel());
        options.set_appearance(self.preferences.appearance.as_str().into());
        options.set_source_names(ui.get_source_names());
        options.set_source_index(ui.get_source_index());
        options.set_camera_names(ui.get_camera_names());
        options.set_camera_index(ui.get_camera_index());
        options.set_microphone_names(ui.get_microphone_names());
        options.set_microphone_index(ui.get_microphone_index());
        options.set_camera(ui.get_capture_camera());
        options.set_microphone(ui.get_capture_mic());
        options.set_system_audio(ui.get_capture_system());
        options.set_busy(ui.get_busy());
        options.set_has_project(self.history.is_some());
        options.set_countdown(self.preferences.countdown_seconds as i32);
        options.set_directory(self.recording_directory().map(|p| p.display().to_string()).unwrap_or_default().into());
    }

    fn sync_launcher(&self, ui: &EditorWindow) {
        let Some(launcher) = &self.launcher else {
            return;
        };
        launcher.set_source_names(ui.get_source_names());
        launcher.set_appearance(self.preferences.appearance.as_str().into());
        launcher.set_source_index(ui.get_source_index());
        launcher.set_camera_names(ui.get_camera_names());
        launcher.set_camera_index(ui.get_camera_index());
        launcher.set_microphone_names(ui.get_microphone_names());
        launcher.set_microphone_index(ui.get_microphone_index());
        launcher.set_camera(ui.get_capture_camera());
        launcher.set_microphone(ui.get_capture_mic());
        launcher.set_system_audio(ui.get_capture_system());
        launcher.set_busy(ui.get_busy());
        launcher.set_recording(ui.get_recording());
        launcher.set_paused(ui.get_recording_paused());
        launcher.set_has_project(self.history.is_some());
        launcher.set_cancellable(ui.get_busy() && (ui.get_sources_loading() || self.capture_started.is_none()));
        launcher.set_countdown(self.preferences.countdown_seconds as i32);
        launcher.set_directory(self.recording_directory().map(|p| p.display().to_string()).unwrap_or_default().into());
        launcher.set_status(ui.get_status());
        let seconds = self.capture_started.map(|start| {
            self.pause_started.unwrap_or_else(std::time::Instant::now).duration_since(start).saturating_sub(self.paused_total).as_secs()
        }).unwrap_or(0);
        launcher.set_elapsed(format!("{:02}:{:02}", seconds / 60, seconds % 60).into());
        // The native glass belongs only to the fixed bar. The option menu owns a separate window.
        platform::update_recorder_glass(launcher.window(), launcher.get_bar_width(), 0., 0., false);
        self.sync_launcher_options(ui);
    }

    fn set_launcher_options_panel(&mut self, ui: &EditorWindow, panel: &str) -> Result<()> {
        let Some(launcher) = &self.launcher else { return Ok(()); };
        launcher.set_panel(panel.into());
        self.sync_launcher(ui);
        let Some(options) = &self.launcher_options else { return Ok(()); };
        if panel.is_empty() {
            options.hide()?;
            return Ok(());
        }
        options.show()?;
        platform::update_options_glass(options.window());
        use slint::winit_030::WinitWindowAccessor;
        options.window().with_winit_window(|window| {
            window.set_blur(false);
            window.set_transparent(true);
        });
        // The menu is positioned synchronously. A deferred retry
        // only covers the first-show case where Winit has not exposed its AppKit
        // view until the next event-loop tick.
        if platform::position_launcher_options(options.window(), launcher.window()).is_err() {
            let options = options.as_weak();
            let launcher = launcher.as_weak();
            Timer::single_shot(Duration::from_millis(20), move || {
                if let (Some(options), Some(launcher)) = (options.upgrade(), launcher.upgrade()) {
                    if let Err(error) = platform::position_launcher_options(options.window(), launcher.window()) {
                        eprintln!("Recorder options position: {error:#}");
                    }
                }
            });
        }
        Ok(())
    }

    fn recording_directory(&self) -> Result<PathBuf> {
        if let Some(directory) = &self.preferences.recording_directory {
            return Ok(directory.clone());
        }
        if let Some(directory) = directories::UserDirs::new()
            .and_then(|dirs| dirs.video_dir().map(|path| path.join("SubTake")))
        {
            return Ok(directory);
        }
        Ok(subtake_native::preferences::Preferences::directory()?.join("recordings"))
    }
    fn show_launcher(&mut self, ui: &EditorWindow) -> Result<()> {
        self.ensure_tray_visible()?;
        let first_show = self.launcher.is_none();
        if self.launcher.is_none() {
            let launcher = RecordingLauncher::new()?;
            let options = RecordingOptions::new()?;
            launcher.on_action(|action| {
                post(move |s, ui| report(ui, s.action(ui, &action)));
            });
            launcher.on_panel_change(|panel| {
                let panel = panel.to_string();
                post(move |s, ui| report(ui, s.set_launcher_options_panel(ui, &panel)));
            });
            options.on_action(|action| {
                post(move |s, ui| report(ui, s.action(ui, &action)));
            });
            options.on_option(|key, value| {
                let key = key.to_string();
                let value = value.to_string();
                post(move |s, ui| s.apply_launcher_option(ui, &key, &value));
            });
            options.on_panel_change(|_| {
                post(move |s, ui| report(ui, s.set_launcher_options_panel(ui, "")));
            });
            launcher.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            options.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            self.launcher = Some(launcher);
            self.launcher_options = Some(options);
        }
        self.sync_launcher(ui);
        if !ui.window().is_visible() {
            platform::set_editor_active(false);
        }
        let launcher = self.launcher.as_ref().unwrap();
        launcher.show()?;
        platform::activate_launcher();
        use slint::winit_030::WinitWindowAccessor;
        launcher.window().with_winit_window(|window| {
            // The recorder uses masked card-level frosting. Window-wide blur
            // would blur the whole transparent envelope when a menu resizes it.
            window.set_blur(false);
            window.set_transparent(true);
            window.focus_window();
        });
        // A launch-time Slint window has no native handle until the event loop starts.
        // Positioning must not prevent source discovery or opening the recorder.
        // Configure immediately when possible; the bounded retry handles a
        // cold launch before Winit has exposed the backing NSView.
        let _ = platform::configure_recording_hud(launcher.window(), true);
        if first_show {
            position_launcher_when_ready(0);
        }
        if self.sources.is_empty() && !ui.get_busy() && !ui.get_recording() {
            self.action(ui, "sources-passive")?;
        }
        Ok(())
    }
    fn show_editor(&mut self, ui: &EditorWindow) -> Result<()> {
        self.ensure_tray_visible()?;
        if ui.get_panel() == "Recording" {
            ui.set_panel("Frame".into());
            self.refresh(ui);
        }
        if let Some(launcher) = &self.launcher {
            launcher.hide()?;
        }
        if let Some(options) = &self.launcher_options {
            options.hide()?;
        }
        platform::set_editor_active(true);
        if !self.editor_shown {
            ui.window().set_size(slint::LogicalSize::new(1360., 880.));
            self.editor_shown = true;
        }
        ui.show()?;
        // Cold-start show may precede the native window. Activate editor glass
        // from the running event loop, using its specific handle and no App borrow.
        let editor = ui.as_weak();
        Timer::single_shot(Duration::from_millis(100), move || {
            if let Some(ui) = editor.upgrade() {
                use slint::winit_030::WinitWindowAccessor;
                ui.window().with_winit_window(|window| window.set_blur(true));
            }
        });
        use slint::winit_030::WinitWindowAccessor;
        ui.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        Ok(())
    }
    fn finish_sources(&mut self, ui: &EditorWindow, result: Result<Vec<Value>>, cancelled: bool) {
        ui.set_busy(false);
        ui.set_sources_loading(false);
        let message = if cancelled {
            "Source refresh cancelled. Press Refresh displays and windows to try again.".to_owned()
        } else {
            match result {
                Ok(sources) => {
                    let selected = self
                        .sources
                        .get(ui.get_source_index().max(0) as usize)
                        .cloned();
                    let index = sources
                        .iter()
                        .position(|source| {
                            selected.as_ref().is_some_and(|old| {
                                old["nativeId"] == source["nativeId"]
                                    && old["kind"] == source["kind"]
                            })
                        })
                        .unwrap_or(0);
                    ui.set_source_names(ModelRc::new(VecModel::from(
                        sources
                            .iter()
                            .map(|source| {
                                SharedString::from(source["name"].as_str().unwrap_or("Source"))
                            })
                            .collect::<Vec<_>>(),
                    )));
                    ui.set_source_index(index as i32);
                    self.sources = sources;
                    if self.sources.is_empty() {
                        "No sources found. Allow SubTake in System Settings → Privacy & Security → Screen & System Audio Recording, then refresh.".to_owned()
                    } else {
                        "Choose a source, then press the red Record button. Your video saves to the recordings folder.".to_owned()
                    }
                }
                Err(error) => format!(
                    "Could not find recording sources: {error:#}. Check Screen & System Audio Recording permission, then refresh."
                ),
            }
        };
        ui.set_recording_hint(message.clone().into());
        ui.set_status(message.into());
    }
    fn new() -> Self {
        Self {
            preferences: subtake_native::preferences::Preferences::load().unwrap_or_else(|e| {
                eprintln!("Preferences: {e}");
                Default::default()
            }),
            history: None,
            document: None,
            source: None,
            info: None,
            selected: None,
            clipboard: vec![],
            extra_selection: vec![],
            recovery: subtake_native::recovery::Store::new(|message| {
                post(move |_, ui| ui.set_status(message.into()))
            }),
            recovery_timer: Timer::default(),
            recovery_origin: None,
            recoveries: subtake_native::recovery::list().unwrap_or_default(),
            wallpapers: media::wallpapers(),
            presets: subtake_native::presets::list(),
            library: vec![],
            library_query: String::new(),
            fresh_recording: None,
            preview: Preview::new(),
            epoch: 0,
            source_time: 0.,
            source_revision: 0,
            playback: Timer::default(),
            started: None,
            audio_cancel: Arc::new(AtomicBool::new(false)),
            job_cancel: Arc::new(AtomicBool::new(false)),
            sources: vec![],
            devices: Value::Null,
            launcher: None,
            launcher_options: None,
            tray: None,
            editor_shown: false,
            capture_started: None,
            pause_started: None,
            paused_total: Duration::ZERO,
            recording_watch: Timer::default(),
            recording: None,
            hotkeys: None,
            hotkey_ids: vec![],
            last_export: None,
        }
    }
    fn register_hotkeys(&mut self) -> Result<()> {
        use global_hotkey::hotkey::HotKey;
        use std::str::FromStr;
        let keys = [
            (
                HotKey::from_str(&self.preferences.record_shortcut)?,
                "record",
            ),
            (
                HotKey::from_str(&self.preferences.pause_shortcut)?,
                "pause-recording",
            ),
        ];
        ensure!(
            keys[0].0.id() != keys[1].0.id(),
            "Recording shortcuts must be different"
        );
        self.hotkeys.take();
        self.hotkey_ids.clear();
        let manager = global_hotkey::GlobalHotKeyManager::new()?;
        for (key, action) in keys {
            manager.register(key)?;
            self.hotkey_ids.push((key.id(), action.into()));
        }
        self.hotkeys = Some(manager);
        Ok(())
    }
    fn reload_library(&mut self) -> Result<()> {
        self.library = subtake_native::library::entries(
            self.preferences.library_directory.as_deref(),
            &self.preferences.recent_projects,
            &self.library_query,
        )?;
        Ok(())
    }
    fn selected_keys(&self) -> Vec<(String, String)> {
        let mut keys = self.extra_selection.clone();
        if let Some(key) = &self.selected {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        keys.retain(|(kind, id)| {
            self.history
                .as_ref()
                .is_some_and(|h| h.project.regions(kind).iter().any(|r| r["id"] == *id))
        });
        keys
    }
    fn project(&self) -> Result<&Project> {
        self.history
            .as_ref()
            .map(|h| &h.project)
            .context("Open a video first")
    }
    fn edit(
        &mut self,
        ui: &EditorWindow,
        f: impl FnOnce(&mut Project) -> Result<()>,
    ) -> Result<()> {
        self.stop(ui);
        self.history
            .as_mut()
            .context("Open a video first")?
            .edit(f)?;
        self.epoch += 1;
        self.schedule_recovery();
        self.refresh(ui);
        self.request();
        Ok(())
    }
    fn schedule_recovery(&self) {
        self.recovery_timer
            .start(TimerMode::SingleShot, Duration::from_secs(2), || {
                with_app(|s, _| {
                    if let Some(h) = &s.history {
                        if h.dirty() {
                            s.recovery.save(h.project.clone(), s.document.clone());
                        } else {
                            s.recovery.clear(s.recovery_origin.take());
                        }
                    }
                })
            });
    }
    fn discard_recovery(&mut self) {
        self.recovery_timer.stop();
        self.recovery.clear(self.recovery_origin.take());
    }
    fn request(&self) {
        if let (Some(h), Some(source), Some(info)) = (&self.history, &self.source, &self.info) {
            let panel = STATE.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .and_then(|(_, ui)| ui.upgrade())
                    .map(|ui| ui.get_panel().to_string())
                    .unwrap_or_default()
            });
            let aspect = if panel == "Crop" {
                info.width as f64 / info.height as f64
            } else {
                aspect_ratio(&h.project, info)
            };
            let physical_width = STATE.with(|slot| {
                slot.borrow().as_ref().and_then(|(_, ui)| ui.upgrade()).map(|ui| {
                    ui.get_preview_pixel_width() as f64 * ui.window().scale_factor() as f64
                }).unwrap_or(info.width as f64)
            });
            // Match display pixels, retaining only the renderer's allocation safety bound.
            let width = physical_width.ceil().max(2.).min(8192.).min(8192. * aspect) as u32;
            let height = (width as f64 / aspect).round().max(2.) as u32;
            self.preview.request(FrameRequest {
                project: h.project.clone(),
                path: source.clone(),
                info: info.clone(),
                time: self.source_time.min((info.duration - 0.001).max(0.)),
                epoch: self.epoch,
                selected: self.selected.clone(),
                source_revision: self.source_revision,
                panel,
                width,
                height,
            });
        }
    }
    fn stop(&mut self, ui: &EditorWindow) {
        self.playback.stop();
        self.started = None;
        self.audio_cancel.store(true, Ordering::Relaxed);
        ui.set_playing(false);
    }
    fn seek(&mut self, ui: &EditorWindow, time: f64) {
        self.stop(ui);
        self.epoch += 1;
        self.source_time = time.clamp(0., self.info.as_ref().map(|i| i.duration).unwrap_or(0.));
        self.update_time(ui);
        self.request();
    }
    fn update_time(&self, ui: &EditorWindow) {
        ui.set_playhead(self.source_time as f32);
        let visible = ui.get_timeline_visible();
        let offset = ui.get_timeline_offset();
        if (self.source_time as f32) < offset || (self.source_time as f32) > offset + visible {
            ui.set_timeline_offset(
                ((self.source_time as f32 - visible * 0.1).max(0.))
                    .min((ui.get_duration() - visible).max(0.)),
            );
        }
        let t = self.source_time;
        ui.set_time_label(
            format!(
                "{:02}:{:06.3} / {:02}:{:06.3}",
                (t / 60.) as u64,
                t % 60.,
                (ui.get_duration() / 60.) as u64,
                ui.get_duration() % 60.
            )
            .into(),
        );
    }
    fn refresh(&self, ui: &EditorWindow) {
        ui.set_language(self.preferences.language.as_str().into());
        ui.set_appearance(self.preferences.appearance.as_str().into());
        ui.set_auto_apply_zooms(self.preferences.auto_apply_zooms);
        ui.set_look_choice(
            self.history
                .as_ref()
                .map(|h| subtake_native::presets::appearance_choice(&h.project))
                .unwrap_or("")
                .into(),
        );
        ui.set_connect_zooms(
            self.history
                .as_ref()
                .map(|h| {
                    h.project
                        .editor
                        .get("connectZooms")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .unwrap_or(true),
        );
        ui.set_motion_choice(
            self.history
                .as_ref()
                .map(|h| subtake_native::presets::motion_choice(&h.project))
                .unwrap_or("")
                .into(),
        );
        ui.set_can_undo(self.history.as_ref().is_some_and(|h| h.can_undo()));
        ui.set_can_redo(self.history.as_ref().is_some_and(|h| h.can_redo()));
        let aspect = self
            .history
            .as_ref()
            .map(|h| h.project.text("aspectRatio", "native"))
            .unwrap_or("native");
        ui.set_aspect_index(
            ["native", "16:9", "9:16", "1:1", "4:3", "3:2"]
                .iter()
                .position(|a| *a == aspect)
                .unwrap_or(0) as i32,
        );
        ui.set_background_value(
            self.history
                .as_ref()
                .map(|h| h.project.text("wallpaper", "#17171c"))
                .unwrap_or("#17171c")
                .into(),
        );
        ui.set_panel_index(
            [
                "Frame",
                "Cursor",
                "Webcam",
                "Captions",
                "Selection",
                "Recording",
                "Export",
                "Audio",
                "Preferences",
                "Recent",
                "Wallpapers",
                "Crop",
                "Presets",
                "Shortcuts",
            ]
            .iter()
            .position(|p| *p == ui.get_panel().as_str())
            .unwrap_or(0) as i32,
        );
        ui.set_has_video(self.history.is_some());
        if let Some(history) = &self.history {
            ui.set_dirty(history.dirty());
            let title = self
                .document
                .as_deref()
                .or(self.source.as_deref())
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy();
            ui.set_document_title(title.as_ref().into());
            if let Some(info) = &self.info {
                ui.set_preview_aspect(if ui.get_panel() == "Crop" {
                    info.width as f32 / info.height as f32
                } else {
                    aspect_ratio(&history.project, info) as f32
                });
            }
            ui.set_duration(self.info.as_ref().map(|i| i.duration as f32).unwrap_or(1.));
            let mut regions = vec![];
            let selected_keys = self.selected_keys();
            for (key, row, tint, title) in [
                ("zoomRegions", 0, "#397afa", "Zoom"),
                ("trimRegions", 1, "#ee5261", "Trim"),
                ("speedRegions", 1, "#dc922d", "Speed"),
                ("clipRegions", 1, "#357c65", "Clip"),
                ("annotationRegions", 2, "#cbb44f", "Annotation"),
                ("audioRegions", 3, "#a468e9", "Audio"),
                ("autoCaptions", 4, "#6396dc", "Caption"),
                ("nativeMarkers", 0, "#f5bb6b", "◆"),
            ] {
                for r in history.project.regions(key) {
                    let [red, green, blue, _] = subtake_native::project::parse_color(tint);
                    regions.push(Region {
                        id: r["id"].as_str().unwrap_or("").into(),
                        kind: key.into(),
                        label: r["text"]
                            .as_str()
                            .or(r["textContent"].as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(title)
                            .into(),
                        start: (n(r, "startMs", 0.) / 1000.) as f32,
                        end: ((n(r, "startMs", 0.)
                            + (n(r, "endMs", 0.) - n(r, "startMs", 0.))
                                * if key == "clipRegions" {
                                    n(r, "speed", 1.)
                                } else {
                                    1.
                                })
                            / 1000.) as f32,
                        row,
                        tint: slint::Color::from_rgb_u8(red, green, blue),
                        selected: selected_keys
                            .iter()
                            .any(|(kind, id)| kind == key && r["id"] == *id),
                    });
                }
            }
            // Put overlapping overlays on separate visible lanes while preserving source timing.
            let mut labels = Vec::new();
            let base_rows = regions.iter().map(|r| r.row).collect::<Vec<_>>();
            for (base, label) in ["Zoom", "Clip", "Annotation", "Audio", "Caption"]
                .into_iter()
                .enumerate()
            {
                let mut indices = regions
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| base_rows[*i] == base as i32)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                indices.sort_by(|a, b| regions[*a].start.total_cmp(&regions[*b].start));
                let mut ends = vec![f32::NEG_INFINITY];
                for index in indices {
                    let lane = if base == 2 || base == 3 {
                        ends.iter()
                            .position(|end| *end <= regions[index].start)
                            .unwrap_or(ends.len())
                    } else {
                        0
                    };
                    if lane == ends.len() {
                        ends.push(f32::NEG_INFINITY);
                    }
                    ends[lane] = regions[index].end;
                    regions[index].row = (labels.len() + lane) as i32;
                }
                labels.extend((0..ends.len()).map(|_| SharedString::from(label)));
            }
            ui.set_audio_row(labels.iter().position(|l| l == "Audio").unwrap_or(3) as i32);
            ui.set_track_labels(ModelRc::new(VecModel::from(labels)));
            ui.set_regions(ModelRc::new(VecModel::from(regions)));
            ui.set_selected_id(
                self.selected
                    .as_ref()
                    .map(|s| s.1.as_str())
                    .unwrap_or("")
                    .into(),
            );
        }
        if self.history.is_none() {
            ui.set_dirty(false);
            ui.set_document_title("Untitled".into());
            ui.set_duration(0.);
            ui.set_regions(ModelRc::default());
            ui.set_edit_visible(false);
        }
        ui.set_fields(ModelRc::new(VecModel::from(self.fields(&ui.get_panel()))));
        self.update_time(ui);
    }
    fn fields(&self, panel: &str) -> Vec<Field> {
        let mut raw = self.raw_fields(if panel == "Presets" || panel == "Wallpapers" {
            "Frame"
        } else {
            panel
        });
        if panel == "Frame" {
            let padding = self
                .history
                .as_ref()
                .and_then(|h| h.project.editor.get("padding"))
                .cloned()
                .unwrap_or(json!(20));
            let linked = padding
                .get("linked")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            for field in &mut raw {
                if let Some(side) = field.key.strip_prefix("padding.") {
                    field.value = padding
                        .as_f64()
                        .unwrap_or_else(|| n(&padding, side, 20.))
                        .to_string()
                        .into();
                }
            }
            if linked {
                raw.retain(|f| !f.key.starts_with("padding.") || f.key == "padding.top");
                if let Some(f) = raw.iter_mut().find(|f| f.key == "padding.top") {
                    f.key = "padding.all".into();
                    f.label = "Padding".into();
                }
            }
            raw.push(Field {
                key: "padding.linked".into(),
                label: "Link all sides".into(),
                kind: 2,
                value: linked.to_string().into(),
                ..Default::default()
            });
        }
        crate::inspector::present(raw, panel, &self.preferences.language)
    }
    fn raw_fields(&self, panel: &str) -> Vec<Field> {
        let p = self.history.as_ref().map(|h| &h.project);
        let settings = p
            .map(|p| {
                self.info
                    .as_ref()
                    .map(|i| ExportSettings::for_media(p, i))
                    .unwrap_or_else(|| ExportSettings::from_project(p))
            })
            .unwrap_or_default();
        let mut fields = vec![];
        if panel == "Preferences" || panel == "Shortcuts" {
            return [
                (
                    "prefs.language",
                    "Language (en, es, fr, de, it, nl, ko, pt-BR, zh-CN, zh-TW)",
                    json!(self.preferences.language),
                ),
                (
                    "prefs.record_shortcut",
                    "Record / stop global shortcut",
                    json!(self.preferences.record_shortcut),
                ),
                (
                    "prefs.pause_shortcut",
                    "Pause / resume global shortcut",
                    json!(self.preferences.pause_shortcut),
                ),
                (
                    "prefs.countdown_seconds",
                    "Recording countdown (seconds)",
                    json!(self.preferences.countdown_seconds),
                ),
            ]
            .into_iter()
            .map(|(key, label, value)| Field {
                key: key.into(),
                label: label.into(),
                value: value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
                    .into(),
                kind: 0,
                minimum: 0.,
                maximum: 0.,
                ..Default::default()
            })
            .chain(subtake_native::shortcuts::ACTIONS.into_iter().map(
                |(action, label, default)| {
                    Field {
                        key: format!("shortcut.{action}").into(),
                        label: label.into(),
                        value: self
                            .preferences
                            .editor_shortcuts
                            .get(action)
                            .map(String::as_str)
                            .unwrap_or(default)
                            .into(),
                        kind: 0,
                        minimum: 0.,
                        maximum: 0.,
                        ..Default::default()
                    }
                },
            ))
            .collect();
        }
        if panel == "Wallpapers" {
            return self
                .wallpapers
                .iter()
                .enumerate()
                .map(|(i, path)| Field {
                    key: format!("wallpaper-{i}").into(),
                    label: "Built-in wallpaper".into(),
                    value: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                        .into(),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                })
                .collect();
        }
        if panel == "Recent" {
            return vec![
                Field {
                    key: "choose-library".into(),
                    label: "Project folder".into(),
                    value: self
                        .preferences
                        .library_directory
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Choose folder…".into())
                        .into(),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
                Field {
                    key: "library.query".into(),
                    label: "Search projects and recordings".into(),
                    value: self.library_query.as_str().into(),
                    kind: 0,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
                Field {
                    key: "refresh-library".into(),
                    label: "".into(),
                    value: "Refresh".into(),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
            ]
            .into_iter()
            .chain(
                self.recoveries
                    .iter()
                    .enumerate()
                    .map(|(i, path)| Field {
                        key: format!("recovery-{i}").into(),
                        label: "Unsaved project recovery".into(),
                        value: Project::load(path)
                            .ok()
                            .map(|p| {
                                format!(
                                    "Recover {}",
                                    Path::new(&p.video_path)
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                )
                            })
                            .unwrap_or_else(|| "Recovery file".into())
                            .into(),
                        kind: 3,
                        minimum: 0.,
                        maximum: 0.,
                        ..Default::default()
                    })
                    .chain(self.library.iter().enumerate().map(|(i, path)| {
                        Field {
                            key: format!("library-open-{i}").into(),
                            label: path
                                .parent()
                                .unwrap_or(Path::new(""))
                                .display()
                                .to_string()
                                .into(),
                            value: path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                                .into(),
                            kind: 3,
                            minimum: 0.,
                            maximum: 0.,
                            ..Default::default()
                        }
                    })),
            )
            .collect();
        }
        let mut add = |key: &str, label: &str, kind: i32, min: f32, max: f32, default: Value| {
            let value = if key.starts_with("export.") {
                match key {
                    "export.width" => json!(settings.width),
                    "export.height" => json!(settings.height),
                    "export.fps" => json!(settings.fps),
                    "export.gif" => json!(settings.gif),
                    "export.loop" => json!(settings.gif_loop),
                    "export.hardware" => json!(settings.hardware),
                    _ => json!(settings.quality),
                }
            } else if let Some(rest) = key.strip_prefix("region.") {
                self.selected
                    .as_ref()
                    .and_then(|(kind, id)| p?.regions(kind).iter().find(|r| r["id"] == *id))
                    .map(|r| get_nested(r, rest))
                    .filter(|v| !v.is_null())
                    .unwrap_or(default)
            } else {
                p.and_then(|p| {
                    let (a, b) = key.split_once('.').unwrap_or((key, ""));
                    p.editor.get(a).map(|v| {
                        if b.is_empty() {
                            v.clone()
                        } else {
                            get_nested(v, b)
                        }
                    })
                })
                .filter(|v| !v.is_null())
                .unwrap_or(default)
            };
            let value = if let Some(s) = value.as_str() {
                s.into()
            } else {
                value.to_string()
            };
            fields.push(Field {
                key: key.into(),
                label: label.into(),
                value: value.into(),
                kind,
                minimum: min,
                maximum: max,
                ..Default::default()
            });
        };
        match panel {
            "Crop" => {
                add("finish-crop", "Crop editing", 3, 0., 0., json!("Done"));
                for key in ["x", "y", "width", "height"] {
                    add(
                        &format!("cropRegion.{key}"),
                        key,
                        1,
                        0.,
                        1.,
                        json!(if key == "width" || key == "height" {
                            1.
                        } else {
                            0.
                        }),
                    );
                }
            }
            "Frame" => {
                add(
                    "visual-crop",
                    "Crop",
                    3,
                    0.,
                    0.,
                    json!("Edit crop visually"),
                );
                add(
                    "save-preset",
                    "Appearance presets",
                    3,
                    0.,
                    0.,
                    json!("Save current appearance…"),
                );
                add("load-preset", "", 3, 0., 0., json!("Load preset file…"));
                for (index, path) in self.presets.iter().enumerate() {
                    add(
                        &format!("apply-preset-{index}"),
                        "Saved preset",
                        3,
                        0.,
                        0.,
                        json!(path.file_stem().unwrap_or_default().to_string_lossy()),
                    );
                    add(
                        &format!("remove-preset-{index}"),
                        "",
                        3,
                        0.,
                        0.,
                        json!("Delete preset"),
                    );
                }
                add(
                    "wallpapers",
                    "Built-in backgrounds",
                    3,
                    0.,
                    0.,
                    json!("Browse wallpapers…"),
                );
                add("zoomSmoothness", "Camera smoothness", 1, 0., 1., json!(0.5));
                add(
                    "connectZooms",
                    "Connect nearby zooms",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add(
                    "zoomClassicMode",
                    "Classic zoom motion",
                    2,
                    0.,
                    0.,
                    json!(false),
                );
                add(
                    "wallpaper",
                    "Background color, gradient or media path",
                    0,
                    0.,
                    0.,
                    json!("#171c35"),
                );
                add(
                    "choose-background",
                    "Background image or video",
                    3,
                    0.,
                    0.,
                    json!("Choose media…"),
                );
                add(
                    "aspectRatio",
                    "Aspect ratio (16:9, 9:16, 1:1)",
                    0,
                    0.,
                    0.,
                    json!("16:9"),
                );
                for side in ["top", "bottom", "left", "right"] {
                    add(
                        &format!("padding.{side}"),
                        &format!("Padding · {side}"),
                        1,
                        0.,
                        250.,
                        json!(20),
                    );
                }
                add("borderRadius", "Rounded corners", 1, 0., 100., json!(8));
                add("shadowIntensity", "Shadow", 1, 0., 1., json!(0.3));
                add("backgroundBlur", "Background blur", 1, 0., 100., json!(0));
                for (key, default) in [("x", 0.), ("y", 0.), ("width", 1.), ("height", 1.)] {
                    add(
                        &format!("cropRegion.{key}"),
                        &format!("Crop · {key}"),
                        1,
                        0.,
                        1.,
                        json!(default),
                    );
                }
            }
            "Cursor" => {
                add(
                    "motion-focused",
                    "Motion presets",
                    3,
                    0.,
                    0.,
                    json!("Focused"),
                );
                add("motion-smooth", "", 3, 0., 0., json!("Smooth"));
                add(
                    "zoomMotionBlur",
                    "Camera motion blur",
                    1,
                    0.,
                    2.,
                    json!(0.35),
                );
                add("showCursor", "Rendered cursor", 2, 0., 0., json!(true));
                add(
                    "cursorStyle",
                    "Style (tahoe, macos, windows11, dot, figma)",
                    0,
                    0.,
                    0.,
                    json!("tahoe"),
                );
                add("cursorSize", "Size", 1, 0.5, 8., json!(3));
                add("cursorSmoothing", "Smoothing", 1, 0., 2., json!(0.67));
                add("cursorSway", "Sway", 1, 0., 2., json!(0.4));
                add("cursorMotionBlur", "Motion blur", 1, 0., 2., json!(0.6));
                add(
                    "cursorClickEffectScale",
                    "Click effect size",
                    1,
                    0.25,
                    4.,
                    json!(1),
                );
                add(
                    "cursorClickEffectOpacity",
                    "Click effect opacity",
                    1,
                    0.,
                    1.,
                    json!(1),
                );
                add(
                    "cursorClickEffectDurationMs",
                    "Click effect duration (ms)",
                    1,
                    60.,
                    1200.,
                    json!(600),
                );
                add("loopCursor", "Loop cursor", 2, 0., 0., json!(false));
                add("cursorClickBounce", "Click bounce", 1, 0., 4., json!(2));
                add(
                    "cursorClickBounceDuration",
                    "Bounce duration (ms)",
                    1,
                    50.,
                    1000.,
                    json!(350),
                );
                add(
                    "cursorClickEffect",
                    "Click effect (none, ripple, spotlight, echo)",
                    0,
                    0.,
                    0.,
                    json!("ripple"),
                );
                add(
                    "cursorClickEffectColor",
                    "Click effect color",
                    0,
                    0.,
                    0.,
                    json!("#2563eb"),
                );
            }
            "Webcam" => {
                add(
                    "choose-webcam",
                    "Webcam footage",
                    3,
                    0.,
                    0.,
                    json!("Choose video…"),
                );
                add("webcam.enabled", "Show webcam", 2, 0., 0., json!(false));
                add("webcam.mirror", "Mirror", 2, 0., 0., json!(true));
                for key in ["width", "height"] {
                    add(&format!("webcam.{key}"), key, 1, 5., 100., json!(40));
                }
                for key in ["positionX", "positionY"] {
                    add(&format!("webcam.{key}"), key, 1, 0., 1., json!(1));
                }
                add(
                    "webcam.positionPreset",
                    "Position preset (custom, bottom-right…)",
                    0,
                    0.,
                    0.,
                    json!("custom"),
                );
                add(
                    "webcam.reactToZoom",
                    "React to zoom",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add("webcam.shadow", "Shadow", 1, 0., 1., json!(0.3));
                for key in ["x", "y", "width", "height"] {
                    add(
                        &format!("webcam.cropRegion.{key}"),
                        &format!("Crop · {key}"),
                        1,
                        0.,
                        1.,
                        json!(if key == "width" || key == "height" {
                            1.
                        } else {
                            0.
                        }),
                    );
                }
                add("webcam.roundness", "Roundness", 1, 0., 100., json!(100));
                add("webcam.margin", "Margin", 1, 0., 150., json!(24));
                add(
                    "webcam.timeOffsetMs",
                    "Time offset (ms)",
                    0,
                    0.,
                    0.,
                    json!(0),
                );
            }
            "Captions" => {
                add(
                    "nativeCaptionLanguage",
                    "Speech language (auto, en, fr, …)",
                    0,
                    0.,
                    0.,
                    json!("auto"),
                );
                add(
                    "download-model",
                    "Local caption model",
                    3,
                    0.,
                    0.,
                    json!("Download Whisper Small"),
                );
                add(
                    "choose-model",
                    "Use an existing model",
                    3,
                    0.,
                    0.,
                    json!("Choose model…"),
                );
                add(
                    "import-font",
                    "Custom font",
                    3,
                    0.,
                    0.,
                    json!("Import font…"),
                );
                add(
                    "autoCaptionSettings.fontFamily",
                    "Font family",
                    0,
                    0.,
                    0.,
                    json!("Helvetica"),
                );
                add(
                    "autoCaptionSettings.animationStyle",
                    "Animation (none, fade, rise, pop)",
                    0,
                    0.,
                    0.,
                    json!("fade"),
                );
                add(
                    "autoCaptionSettings.enabled",
                    "Show captions",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add(
                    "autoCaptionSettings.fontSize",
                    "Font size",
                    1,
                    12.,
                    120.,
                    json!(30),
                );
                add(
                    "autoCaptionSettings.bottomOffset",
                    "Bottom offset (%)",
                    1,
                    0.,
                    45.,
                    json!(3),
                );
                add(
                    "autoCaptionSettings.maxWidth",
                    "Maximum width (%)",
                    1,
                    20.,
                    100.,
                    json!(62),
                );
                add(
                    "autoCaptionSettings.maxRows",
                    "Maximum rows",
                    1,
                    1.,
                    6.,
                    json!(2),
                );
                add(
                    "autoCaptionSettings.textColor",
                    "Text color",
                    0,
                    0.,
                    0.,
                    json!("#ffffff"),
                );
                add(
                    "autoCaptionSettings.backgroundOpacity",
                    "Box opacity",
                    1,
                    0.,
                    1.,
                    json!(0.9),
                );
            }
            "Selection" => {
                if let Some((kind, _)) = &self.selected {
                    add("region.startMs", "Start (ms)", 0, 0., 0., json!(0));
                    add("region.endMs", "End (ms)", 0, 0., 0., json!(1000));
                    match kind.as_str() {
                        "zoomRegions" => {
                            add(
                                "region.mode",
                                "Focus tracking (manual or auto)",
                                0,
                                0.,
                                0.,
                                json!("manual"),
                            );
                            add("region.depth", "Zoom depth", 1, 1., 6., json!(3));
                            add("region.focus.cx", "Focus X", 1, 0., 1., json!(0.5));
                            add("region.focus.cy", "Focus Y", 1, 0., 1., json!(0.5));
                        }
                        "speedRegions" | "clipRegions" => {
                            if kind == "clipRegions" {
                                add("region.muted", "Mute clip", 2, 0., 0., json!(false));
                            }
                            add("region.speed", "Playback speed", 1, 0.25, 4., json!(1.5));
                        }
                        "autoCaptions" => {
                            add("region.text", "Caption", 0, 0., 0., json!(""));
                            add(
                                "split-caption",
                                "Phrase editing",
                                3,
                                0.,
                                0.,
                                json!("Split near playhead"),
                            );
                            add(
                                "merge-caption",
                                "",
                                3,
                                0.,
                                0.,
                                json!("Merge with following caption"),
                            );
                            if let Some(cue) = p.and_then(|p| {
                                p.regions("autoCaptions").iter().find(|c| {
                                    self.selected.as_ref().is_some_and(|s| c["id"] == s.1)
                                })
                            }) {
                                for (index, word) in
                                    subtake_native::caption_editing::normalized_words(cue)
                                        .iter()
                                        .enumerate()
                                {
                                    for (key, label) in [
                                        ("text", "Text"),
                                        ("startMs", "Start (ms)"),
                                        ("endMs", "End (ms)"),
                                    ] {
                                        add(
                                            &format!("word.{index}.{key}"),
                                            &format!("Word {} · {label}", index + 1),
                                            0,
                                            0.,
                                            0.,
                                            word[key].clone(),
                                        );
                                    }
                                }
                            }
                        }
                        "audioRegions" => {
                            add("region.volume", "Volume", 1, 0., 3., json!(1));
                            add(
                                "region.normalize",
                                "Normalize audio",
                                2,
                                0.,
                                0.,
                                json!(false),
                            );
                        }
                        "annotationRegions" => {
                            for (key, label, default) in [
                                ("textContent", "Text", json!("")),
                                ("style.fontFamily", "Font family", json!("Helvetica")),
                                ("style.color", "Text color", json!("#ffffff")),
                                ("style.backgroundColor", "Box color", json!("transparent")),
                                (
                                    "figureData.arrowDirection",
                                    "Arrow direction",
                                    json!("right"),
                                ),
                                ("figureData.color", "Arrow color", json!("#2563eb")),
                            ] {
                                add(&format!("region.{key}"), label, 0, 0., 0., default);
                            }
                            for (key, label, default) in [
                                ("position.x", "Position X", 50.),
                                ("position.y", "Position Y", 50.),
                                ("size.width", "Width", 30.),
                                ("size.height", "Height", 20.),
                                ("style.fontSize", "Font size", 32.),
                                ("blurIntensity", "Blur strength", 20.),
                            ] {
                                add(&format!("region.{key}"), label, 1, 0., 120., json!(default));
                            }
                        }
                        _ => (),
                    }
                }
            }
            "Audio" => {
                for track in ["system", "mic", "mixed"] {
                    add(
                        &format!("defaultSourceAudioTrackSettings.{track}.volume"),
                        &format!("{track} volume"),
                        1,
                        0.,
                        3.,
                        json!(1),
                    );
                    add(
                        &format!("defaultSourceAudioTrackSettings.{track}.normalize"),
                        &format!("{track} normalization"),
                        2,
                        0.,
                        0.,
                        json!(false),
                    );
                }
            }
            "Export" => {
                add(
                    "export-preset-720",
                    "Output size presets",
                    3,
                    0.,
                    0.,
                    json!("720p"),
                );
                add("export-preset-1080", "", 3, 0., 0., json!("1080p"));
                add("export-preset-source", "", 3, 0., 0., json!("Source size"));
                add(
                    "nativeCaptionSidecars",
                    "Save SRT and VTT subtitles",
                    2,
                    0.,
                    0.,
                    json!(false),
                );
                add("export.width", "Width (pixels)", 0, 0., 0., json!(1920));
                add("export.height", "Height (pixels)", 0, 0., 0., json!(1080));
                add("export.fps", "Frames per second", 0, 0., 0., json!(30));
                add(
                    "export.quality",
                    "Quality (low, medium, high)",
                    0,
                    0.,
                    0.,
                    json!("high"),
                );
                add("export.gif", "Export GIF", 2, 0., 0., json!(false));
                add("export.loop", "Loop GIF", 2, 0., 0., json!(true));
                add(
                    "export.hardware",
                    "Hardware H.264 encoding",
                    2,
                    0.,
                    0.,
                    json!(false),
                );
                add(
                    "reveal-export",
                    "Last exported file",
                    3,
                    0.,
                    0.,
                    json!("Reveal in Finder"),
                );
            }
            _ => (),
        }
        fields
    }
    fn field(&mut self, ui: &EditorWindow, key: &str, value: &str) -> Result<()> {
        if key == "padding.all" || key == "padding.linked" {
            return self.edit(ui, |p| {
                let old = p.editor.get("padding").cloned().unwrap_or(json!(20));
                let amount = if key == "padding.all" {
                    value.parse::<f64>()?
                } else {
                    old.as_f64().unwrap_or_else(|| n(&old, "top", 20.))
                };
                ensure!(
                    amount.is_finite() && (0.0..=250.0).contains(&amount),
                    "Padding must be between 0 and 250"
                );
                let linked = key == "padding.all" || value == "true";
                let mut padding = if old.is_object() {
                    old
                } else {
                    json!({"top":amount,"bottom":amount,"left":amount,"right":amount})
                };
                if linked {
                    for side in ["top", "bottom", "left", "right"] {
                        padding[side] = json!(amount);
                    }
                }
                padding["linked"] = json!(linked);
                p.set("padding", padding);
                Ok(())
            });
        }

        if key == "library.query" {
            self.library_query = value.into();
            self.reload_library()?;
            self.refresh(ui);
            return Ok(());
        }
        if let Some(rest) = key.strip_prefix("word.") {
            let (index, field) = rest.split_once('.').context("Invalid word edit")?;
            let index: usize = index.parse()?;
            let (kind, id) = self.selected.clone().context("Select a caption")?;
            ensure!(kind == "autoCaptions", "Select a caption");
            return self.edit(ui, |p| {
                subtake_native::caption_editing::edit_word(p, &id, index, field, value)
            });
        }
        if let Some(action) = key.strip_prefix("shortcut.") {
            subtake_native::shortcuts::validate(action, value, &self.preferences.editor_shortcuts)?;
            self.preferences
                .editor_shortcuts
                .insert(action.into(), value.into());
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key == "prefs.language" {
            ensure!(
                subtake_native::localization::LOCALES.contains(&value),
                "Unsupported interface language"
            );
            self.preferences.language = value.into();
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key == "prefs.appearance" || key == "prefs.auto_apply_zooms" {
            if key == "prefs.appearance" {
                ensure!(
                    ["light", "dark", "system"].contains(&value),
                    "Unknown appearance"
                );
                self.preferences.appearance = value.into();
            } else {
                self.preferences.auto_apply_zooms = value.parse()?;
            }
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key.starts_with("prefs.") {
            let previous = self.preferences.clone();
            match key {
                "prefs.record_shortcut" => self.preferences.record_shortcut = value.into(),
                "prefs.pause_shortcut" => self.preferences.pause_shortcut = value.into(),
                "prefs.countdown_seconds" => {
                    self.preferences.countdown_seconds = value.parse::<u32>()?.min(10)
                }
                _ => (),
            }
            if let Err(e) = self.register_hotkeys() {
                self.preferences = previous;
                let _ = self.register_hotkeys();
                return Err(e);
            }
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        let text_field = matches!(
            key,
            "wallpaper"
                | "aspectRatio"
                | "cursorStyle"
                | "cursorClickEffect"
                | "cursorClickEffectColor"
                | "nativeCaptionLanguage"
                | "webcam.positionPreset"
                | "autoCaptionSettings.fontFamily"
                | "autoCaptionSettings.animationStyle"
                | "autoCaptionSettings.textColor"
                | "region.text"
                | "region.textContent"
                | "region.style.fontFamily"
                | "region.style.color"
                | "region.style.backgroundColor"
                | "region.style.fontWeight"
                | "region.style.fontStyle"
                | "region.style.textAlign"
                | "region.figureData.arrowDirection"
                | "region.figureData.color"
                | "region.mode"
        );
        let v = if text_field {
            json!(value)
        } else {
            serde_json::from_str::<Value>(value).unwrap_or(json!(value))
        };
        if key.starts_with("export.") {
            let mut settings = ExportSettings::for_media(
                self.project()?,
                self.info.as_ref().context("Open a video first")?,
            );
            match key {
                "export.width" => settings.width = value.parse()?,
                "export.height" => settings.height = value.parse()?,
                "export.fps" => settings.fps = value.parse()?,
                "export.gif" => settings.gif = value == "true",
                "export.loop" => settings.gif_loop = value == "true",
                "export.hardware" => settings.hardware = value == "true",
                "export.quality" => settings.quality = value.into(),
                _ => (),
            }
            return self.edit(ui, |p| {
                settings.store(p);
                Ok(())
            });
        }
        if let Some(key) = key.strip_prefix("region.") {
            let (kind, id) = self.selected.clone().context("Select a timeline region")?;
            self.edit(ui, |p| {
                let mut r = p
                    .regions(&kind)
                    .iter()
                    .find(|r| r["id"] == id)
                    .context("Region not found")?
                    .clone();
                set_nested(&mut r, key, v);
                p.change_region(&kind, &id, r)
            })
        } else {
            self.edit(ui, |p| {
                let (base, rest) = key.split_once('.').unwrap_or((key, ""));
                if rest.is_empty() {
                    p.set(base, v)
                } else {
                    let mut object = p.editor.get(base).cloned().unwrap_or(json!({}));
                    set_nested(&mut object, rest, v);
                    if base == "cropRegion" {
                        normalize_crop(&mut object);
                    }
                    if base == "webcam" {
                        if rest == "positionX" || rest == "positionY" {
                            object["positionPreset"] = json!("custom");
                        }
                        if let Some(crop) = object.get_mut("cropRegion") {
                            normalize_crop(crop);
                        }
                    }
                    p.set(base, object)
                }
                Ok(())
            })
        }
    }
    fn load(&mut self, ui: &EditorWindow, path: PathBuf) -> Result<()> {
        self.stop(ui);
        let project = if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("recordly" | "openscreen" | "json")
        ) {
            Project::load(&path)?
        } else {
            Project::new(&path)
        };
        let is_project = matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("recordly" | "openscreen" | "json")
        );
        let recovery_origin = subtake_native::recovery::is_snapshot(&path).then(|| path.clone());
        let mut project = project;
        if is_project && recovery_origin.is_none() {
            project.resolve_assets(&path);
        }
        let document = if recovery_origin.is_some() {
            project
                .extra
                .remove("nativeRecoveryDocument")
                .and_then(|v| v.as_str().map(PathBuf::from))
        } else {
            is_project.then_some(path)
        };
        let mut source = project.source_path(document.as_deref());
        if !source.is_file() {
            source = rfd::FileDialog::new()
                .set_title("Locate the missing source video")
                .pick_file()
                .context("Source video was not located")?;
        }
        ui.set_status("Opening video…".into());
        self.epoch += 1;
        let epoch = self.epoch;
        let auto_zoom =
            self.preferences.auto_apply_zooms && self.fresh_recording.as_ref() == Some(&source);
        self.fresh_recording = None;
        std::thread::spawn(move || {
            let result = media::probe(&source);
            post(move |s, ui| {
                if epoch != s.epoch {
                    return;
                }
                match result {
                    Ok(info) => {
                        let mut project = project;
                        project.video_path = source.to_string_lossy().into();
                        if !is_project {
                            let webcam = source.with_extension("webcam.mp4");
                            if webcam.is_file() {
                                let mut settings =
                                    project.editor.get("webcam").cloned().unwrap_or(json!({}));
                                settings["enabled"] = json!(true);
                                settings["sourcePath"] = json!(webcam);
                                project.set("webcam", settings);
                            }
                        }
                        // Cursor positions are normalized; square and portrait captures
                        // use the same suggestion and camera pipeline as landscape ones.
                        let mut zoom_warning = None;
                        if auto_zoom {
                            match subtake_native::autozoom::from_source(
                                &project,
                                &source,
                                info.duration * 1000.,
                            ) {
                                Ok(regions) => {
                                    for region in regions {
                                        if let Err(error) = project.add("zoomRegions", region) {
                                            zoom_warning =
                                                Some(format!("Automatic zooms: {error:#}"));
                                        }
                                    }
                                }
                                Err(error) => {
                                    zoom_warning = Some(format!("Automatic zooms: {error:#}"))
                                }
                            }
                        }
                        let artwork_source = source.clone();
                        let artwork_info = info.clone();
                        ui.set_thumbnails(slint::Image::default());
                        ui.set_frosted_thumbnails(slint::Image::default());
                        ui.set_waveform(slint::Image::default());
                        std::thread::spawn(move || {
                            let result = media::timeline_artwork(&artwork_source, &artwork_info);
                            // Blur the real filmstrip once on the artwork worker, never capture the desktop.
                            let frosted = result.as_ref().ok().and_then(|(path, _)| image::open(path).ok())
                                .map(|image| image.blur(10.).to_rgba8());
                            post(move |s, ui| {
                                if s.source.as_ref() != Some(&artwork_source) {
                                    return;
                                }
                                match result {
                                    Ok((thumbs, wave)) => {
                                        if let Ok(image) = slint::Image::load_from_path(&thumbs) {
                                            ui.set_thumbnails(image);
                                            if let Some(ref pixels) = frosted {
                                                ui.set_frosted_thumbnails(slint::Image::from_rgba8(
                                                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                                        pixels.as_raw(), pixels.width(), pixels.height())));
                                            }
                                        }
                                        if let Some(wave) = wave {
                                            if let Ok(image) = slint::Image::load_from_path(&wave) {
                                                ui.set_waveform(image);
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("Timeline artwork: {e:#}"),
                                }
                            });
                        });
                        s.source_revision = s.source_revision.wrapping_add(1);
                        s.history = Some(History::new(project));
                        if recovery_origin.is_some() {
                            s.history.as_mut().unwrap().mark_unsaved();
                        }
                        s.recovery_origin = recovery_origin;
                        s.schedule_recovery();
                        s.preferences
                            .opened(document.clone().unwrap_or_else(|| source.clone()));
                        if let Err(e) = s.preferences.save() {
                            eprintln!("Recent projects: {e}");
                        }
                        s.document = document;
                        s.source = Some(source);
                        s.info = Some(info);
                        s.source_time = 0.;
                        ui.invoke_reset_preview();
                        ui.set_timeline_zoom(1.);
                        ui.set_timeline_offset(0.);
                        s.selected = None;
                        s.extra_selection.clear();
                        if auto_zoom {
                            ui.set_panel("Frame".into());
                        }
                        s.refresh(ui);
                        s.request();
                        ui.set_status(zoom_warning.unwrap_or_else(|| "Ready".into()).into());
                        if let Err(error) = s.show_editor(ui) {
                            ui.set_status(format!("Open editor: {error:#}").into());
                        }
                    }
                    Err(e) => ui.set_status(format!("Open failed: {e:#}").into()),
                }
            });
        });
        Ok(())
    }
    fn save(&mut self, ui: &EditorWindow, save_as: bool) -> Result<()> {
        let path = if !save_as && self.document.is_some() {
            self.document.clone().unwrap()
        } else {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("SubTake project", &["recordly"])
                .set_file_name("Untitled.recordly")
                .save_file()
            else {
                return Ok(());
            };
            path
        };
        self.project()?.save(&path)?;
        self.preferences.opened(path.clone());
        self.preferences.save()?;
        self.document = Some(path);
        self.history.as_mut().unwrap().mark_saved();
        self.discard_recovery();
        self.refresh(ui);
        ui.set_status("Project saved".into());
        Ok(())
    }
    fn can_replace(&mut self, ui: &EditorWindow) -> bool {
        if self.history.as_ref().is_some_and(History::dirty) {
            match rfd::MessageDialog::new()
                .set_title("Save your changes?")
                .set_description("Save this project before opening another video.")
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show()
            {
                rfd::MessageDialogResult::Yes => {
                    if self.save(ui, false).is_err() {
                        return false;
                    }
                    !self.history.as_ref().unwrap().dirty()
                }
                rfd::MessageDialogResult::No => {
                    self.discard_recovery();
                    true
                }
                _ => false,
            }
        } else {
            true
        }
    }
    fn action(&mut self, ui: &EditorWindow, action: &str) -> Result<()> {
        ensure!(
            !ui.get_busy()
                || matches!(
                    action,
                    "cancel" | "show" | "drag-window" | "drag-launcher" | "hide-launcher"
                ),
            "Wait for the current operation or cancel it first"
        );
        if let Some(index) = action.strip_prefix("apply-preset-") {
            let path = self
                .presets
                .get(index.parse::<usize>()?)
                .context("Preset no longer exists")?;
            let data = subtake_native::presets::load(path)?;
            return self.edit(ui, |p| subtake_native::presets::apply(p, &data));
        }
        if let Some(index) = action.strip_prefix("library-open-") {
            let path = self
                .library
                .get(index.parse::<usize>()?)
                .context("Library item no longer exists")?
                .clone();
            if self.can_replace(ui) {
                self.load(ui, path)?;
            }
            return Ok(());
        }
        if let Some(index) = action.strip_prefix("remove-preset-") {
            let path = self
                .presets
                .get(index.parse::<usize>()?)
                .context("Preset no longer exists")?;
            let retained = subtake_native::presets::remove(path)?;
            self.presets = subtake_native::presets::list();
            self.refresh(ui);
            ui.set_status(
                format!("Preset removed. Recoverable copy: {}", retained.display()).into(),
            );
            return Ok(());
        }
        match action {
            "visual-crop" | "finish-crop" => {
                self.stop(ui);
                ui.set_panel(
                    if action == "visual-crop" {
                        "Crop"
                    } else {
                        "Frame"
                    }
                    .into(),
                );
                self.refresh(ui);
                self.epoch += 1;
                self.request();
            }
            "choose-library" => {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose project and recording folder")
                    .pick_folder()
                {
                    self.preferences.library_directory = Some(folder);
                    self.preferences.save()?;
                    self.reload_library()?;
                    self.refresh(ui);
                }
            }
            "refresh-library" => {
                self.reload_library()?;
                self.refresh(ui);
            }

            "next-annotation" | "previous-annotation" => {
                let mut items = self
                    .project()?
                    .regions("annotationRegions")
                    .iter()
                    .filter(|r| {
                        n(r, "startMs", 0.) <= self.source_time * 1000.
                            && n(r, "endMs", 0.) > self.source_time * 1000.
                    })
                    .collect::<Vec<_>>();
                items.sort_by(|a, b| n(a, "zIndex", 0.).total_cmp(&n(b, "zIndex", 0.)));
                ensure!(!items.is_empty(), "No annotations at the playhead");
                let current = items
                    .iter()
                    .position(|r| self.selected.as_ref().is_some_and(|s| r["id"] == s.1));
                let index = match current {
                    Some(i) if action == "previous-annotation" => {
                        (i + items.len() - 1) % items.len()
                    }
                    Some(i) => (i + 1) % items.len(),
                    None => 0,
                };
                let id = items[index]["id"]
                    .as_str()
                    .context("Annotation id missing")?
                    .to_owned();
                self.extra_selection.clear();
                self.selected = Some(("annotationRegions".into(), id));
                ui.set_panel("Selection".into());
                self.refresh(ui);
                self.epoch += 1;
                self.request();
            }
            "look-studio" | "look-minimal" | "look-bold" => {
                self.edit(ui, |p| {
                    subtake_native::presets::appearance(p, action.trim_start_matches("look-"))
                })?;
            }
            "motion-focused" | "motion-smooth" => {
                self.edit(ui, |p| {
                    subtake_native::presets::motion(p, action == "motion-smooth");
                    Ok(())
                })?;
            }
            "save-preset" => {
                let dir = subtake_native::presets::directory()?;
                std::fs::create_dir_all(&dir)?;
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Save appearance preset")
                    .set_directory(dir)
                    .set_file_name("My preset.json")
                    .add_filter("SubTake preset", &["json"])
                    .save_file()
                {
                    subtake_native::presets::save(&path, self.project()?)?;
                    self.presets = subtake_native::presets::list();
                    self.refresh(ui);
                }
            }
            "load-preset" => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Load appearance preset")
                    .add_filter("SubTake preset", &["json"])
                    .pick_file()
                {
                    let data = subtake_native::presets::load(&path)?;
                    self.edit(ui, |p| subtake_native::presets::apply(p, &data))?;
                }
            }

            "add-marker" => {
                let time = self.source_time * 1000.;
                let mut id = None;
                self.edit(ui, |p| {
                    id = Some(p.add("nativeMarkers", json!({"startMs":time,"endMs":time+1.}))?);
                    Ok(())
                })?;
                self.extra_selection.clear();
                self.selected = id.map(|id| ("nativeMarkers".into(), id));
                self.refresh(ui);
            }
            "previous-marker" | "next-marker" => {
                let time = self.source_time * 1000.;
                let mut markers = self
                    .project()?
                    .regions("nativeMarkers")
                    .iter()
                    .map(|m| n(m, "startMs", 0.))
                    .collect::<Vec<_>>();
                markers.sort_by(f64::total_cmp);
                let next = if action == "previous-marker" {
                    markers.into_iter().rev().find(|&t| t < time - 1.)
                } else {
                    markers.into_iter().find(|&t| t > time + 1.)
                };
                if let Some(time) = next {
                    self.seek(ui, time / 1000.);
                }
            }
            "feedback" => {
                platform::open_feedback()?;
            }
            "wallpapers" => {
                self.wallpapers = media::wallpapers();
                ui.set_panel("Wallpapers".into());
                self.refresh(ui);
            }
            action if action.starts_with("wallpaper-") => {
                let index = action[10..].parse::<usize>()?;
                let path = self
                    .wallpapers
                    .get(index)
                    .context("Wallpaper is missing")?
                    .clone();
                self.edit(ui, |p| {
                    p.set(
                        "wallpaper",
                        json!(
                            path.strip_prefix(media::resources().join("public"))
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| path.to_string_lossy().to_string())
                        ),
                    );
                    Ok(())
                })?;
            }
            "shortcut-reference" => {
                let editable = subtake_native::shortcuts::ACTIONS
                    .iter()
                    .map(|(action, label, default)| {
                        format!(
                            "{}: {}",
                            label,
                            self.preferences
                                .editor_shortcuts
                                .get(*action)
                                .map(String::as_str)
                                .unwrap_or(default)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                rfd::MessageDialog::new().set_title("SubTake keyboard shortcuts").set_description(format!("Primary means Cmd on Mac and Ctrl on Windows.\n\nOpen: Primary+O\nSave: Primary+S\nSave as: Primary+Shift+S\nUndo / redo: Primary+Z / Shift+Z\nCopy / cut / paste: Primary+C / X / V\nDuplicate: Primary+Shift+D\nSplit clip: Primary+B\nDelete selected region: Delete\n\n{editable}\n\nGlobal record/stop: {}\nGlobal pause/resume: {}",self.preferences.record_shortcut,self.preferences.pause_shortcut)).set_buttons(rfd::MessageButtons::Ok).show();
            }
            action if action.starts_with("export-preset-") => {
                let info = self.info.as_ref().context("Open a video first")?;
                let mut settings = ExportSettings::for_media(self.project()?, info);
                let max = match &action[14..] {
                    "720" => 720.,
                    "1080" => 1080.,
                    _ => info.height as f64,
                };
                let aspect = aspect_ratio(self.project()?, info);
                let h = max.min(info.height as f64);
                settings.height = ((h / 2.).floor().max(1.) * 2.) as u32;
                settings.width = ((h * aspect / 2.).floor().max(1.) * 2.) as u32;
                self.edit(ui, |p| {
                    settings.store(p);
                    Ok(())
                })?;
            }
            action if action.starts_with("recovery-") => {
                let index = action[9..].parse::<usize>()?;
                let path = self
                    .recoveries
                    .get(index)
                    .context("Recovery no longer exists")?
                    .clone();
                if self.can_replace(ui) {
                    self.load(ui, path)?;
                }
            }
            action if action.starts_with("recent-") => {
                let index = action[7..].parse::<usize>()?;
                let path = self
                    .preferences
                    .recent_projects
                    .get(index)
                    .cloned()
                    .context("Recent project is missing")?;
                if self.can_replace(ui) {
                    self.load(ui, path)?;
                }
            }
            "download-model" => {
                ui.set_busy(true);
                ui.set_progress(0.);
                ui.set_status("Downloading Whisper Small for local captions…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    let result = subtake_native::models::download(&cancel, |progress| {
                        post(move |_, ui| ui.set_progress(progress))
                    });
                    post(move |s, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(path) => {
                                s.preferences.whisper_model = Some(path);
                                let result = s.preferences.save();
                                report(ui, result);
                                ui.set_status(
                                    "Whisper Small is ready. Transcription runs locally.".into(),
                                );
                            }
                            Err(e) => ui.set_status(format!("{e:#}").into()),
                        }
                    });
                });
            }
            "choose-model" => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose Whisper model")
                    .add_filter("Whisper model", &["bin"])
                    .pick_file()
                {
                    self.preferences.whisper_model = Some(path);
                    self.preferences.save()?;
                    ui.set_status("Caption model selected".into());
                }
            }
            "import-font" => {
                use base64::Engine;
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fonts", &["ttf", "otf", "ttc"])
                    .pick_file()
                else {
                    return Ok(());
                };
                let bytes = std::fs::read(path)?;
                ensure!(bytes.len() <= 16 * 1024 * 1024, "Font exceeds 16 MiB");
                let face = skia_safe::FontMgr::new()
                    .new_from_data(&bytes, None)
                    .context("Invalid font file")?;
                let family = face.family_name();
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                let captions = ui.get_panel() == "Captions";
                self.edit(ui, |p| {
                    let mut fonts = p
                        .editor
                        .get("nativeFonts")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    fonts.retain(|f| f["family"] != family);
                    fonts.push(json!({"family":family,"data":data}));
                    p.set("nativeFonts", json!(fonts));
                    if captions {
                        let mut settings = p
                            .editor
                            .get("autoCaptionSettings")
                            .cloned()
                            .unwrap_or(json!({}));
                        settings["fontFamily"] = json!(family);
                        p.set("autoCaptionSettings", settings);
                    }
                    Ok(())
                })?;
                ui.set_status(
                    format!("Imported {family}; use this family in text annotations or captions.")
                        .into(),
                );
            }
            "show" => self.show_launcher(ui)?,
            "show-editor" => self.show_editor(ui)?,
            "storyboard-spike" => {
                let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("spikes/storyboard/storyboard.py");
                ensure!(
                    script.is_file(),
                    "Storyboard spike requires the local checkout"
                );
                let weak = ui.as_weak();
                std::thread::spawn(move || {
                    let result = (|| -> anyhow::Result<String> {
                        let output = std::process::Command::new("python3")
                            .arg(script).args(["launch", "--no-open"]).output()?;
                        ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                        let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                        Ok(result["url"].as_str().context("Missing workspace URL")?.to_owned())
                    })();
                    let _ = slint::invoke_from_event_loop(move || {
                        let result = result.and_then(|url| platform::open_agent_workspace(&url));
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status(match result {
                                Ok(()) => "Agent video workspace opened".into(),
                                Err(error) => format!("Agent workspace: {error}").into(),
                            });
                        }
                    });
                });
                ui.set_status("Opening the agent video workspace…".into());
            }
            "projects" => {
                ui.set_panel("Recent".into());
                self.recoveries = subtake_native::recovery::list().unwrap_or_default();
                self.reload_library()?;
                self.refresh(ui);
                self.show_editor(ui)?;
            }
            "hide-launcher" => {
                if let Some(launcher) = &self.launcher {
                    launcher.hide()?;
                }
                if let Some(options) = &self.launcher_options {
                    options.hide()?;
                }
            }
            "drag-launcher" => {
                use slint::winit_030::WinitWindowAccessor;
                if let Some(launcher) = &self.launcher {
                    launcher.window().with_winit_window(|window| {
                        let _ = window.drag_window();
                    });
                }
            }
            "recording-folder" => {
                if let Some(directory) = rfd::FileDialog::new()
                    .set_directory(self.recording_directory()?)
                    .pick_folder()
                {
                    self.preferences.recording_directory = Some(directory);
                    self.preferences.save()?;
                }
            }
            "quit" => {
                ensure!(self.recording.is_none(), "Stop recording before quitting");
                if self.can_replace(ui) {
                    self.stop(ui);
                    self.job_cancel.store(true, Ordering::Relaxed);
                    slint::quit_event_loop()?;
                }
            }
            "open" => {
                if self.can_replace(ui) {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "Videos and projects",
                            &[
                                "mp4",
                                "mov",
                                "mkv",
                                "webm",
                                "avi",
                                "recordly",
                                "openscreen",
                                "json",
                            ],
                        )
                        .pick_file()
                    {
                        self.load(ui, path)?;
                    }
                }
            }
            "save" => self.save(ui, false)?,
            "save-as" => self.save(ui, true)?,
            "undo" | "redo" => {
                self.stop(ui);
                if let Some(h) = &mut self.history {
                    if action == "undo" { h.undo() } else { h.redo() }
                }
                self.epoch += 1;
                self.schedule_recovery();
                self.refresh(ui);
                self.request();
            }
            "select-all" => {
                self.extra_selection.clear();
                for kind in [
                    "zoomRegions",
                    "trimRegions",
                    "speedRegions",
                    "clipRegions",
                    "annotationRegions",
                    "audioRegions",
                    "autoCaptions",
                    "nativeMarkers",
                ] {
                    let ids = self
                        .project()?
                        .regions(kind)
                        .iter()
                        .filter_map(|r| r["id"].as_str().map(|id| (kind.to_owned(), id.to_owned())))
                        .collect::<Vec<_>>();
                    self.extra_selection.extend(ids);
                }
                self.selected = self.extra_selection.last().cloned();
                self.refresh(ui);
            }
            "copy" | "cut" => {
                let keys = self.selected_keys();
                ensure!(!keys.is_empty(), "Select a region first");
                self.clipboard = keys
                    .into_iter()
                    .map(|(kind, id)| {
                        let p = self.project().unwrap();
                        let region = p
                            .regions(&kind)
                            .iter()
                            .find(|r| r["id"] == id)
                            .unwrap()
                            .clone();
                        let audio = (kind == "clipRegions")
                            .then(|| {
                                p.editor
                                    .get("sourceAudioTrackSettingsByClip")
                                    .and_then(|m| m.get(&id))
                                    .cloned()
                            })
                            .flatten();
                        (kind, id_to_null(region), audio)
                    })
                    .collect();
                if action == "cut" {
                    self.action(ui, "delete")?;
                }
            }
            "paste" | "duplicate" => {
                if action == "duplicate" {
                    self.action(ui, "copy")?;
                }
                ensure!(!self.clipboard.is_empty(), "Copy a region first");
                let copied = self.clipboard.clone();
                let earliest = copied
                    .iter()
                    .map(|(_, r, _)| n(r, "startMs", 0.))
                    .fold(f64::INFINITY, f64::min);
                let last = copied
                    .iter()
                    .map(|(kind, r, _)| subtake_native::editing::source_end(kind, r))
                    .fold(0., f64::max);
                let duration = self.info.as_ref().unwrap().duration * 1000.;
                ensure!(
                    last - earliest <= duration,
                    "Copied regions exceed the source duration"
                );
                let delta = (self.source_time * 1000.)
                    .min(duration - (last - earliest))
                    .max(0.)
                    - earliest;
                let mut selection = vec![];
                self.edit(ui, |p| {
                    for (kind, mut region, audio) in copied {
                        let a = n(&region, "startMs", 0.);
                        let b = n(&region, "endMs", 0.);
                        region["startMs"] = json!(a + delta);
                        region["endMs"] = json!(b + delta);
                        if kind == "autoCaptions" {
                            subtake_native::project::retime_caption_words(&mut region, a, b);
                        }
                        let id = p.add(&kind, region)?;
                        if let Some(audio) = audio {
                            let map = p
                                .editor
                                .entry("sourceAudioTrackSettingsByClip")
                                .or_insert_with(|| json!({}));
                            map[&id] = audio;
                        }
                        selection.push((kind, id));
                    }
                    Ok(())
                })?;
                self.selected = selection.last().cloned();
                self.extra_selection = selection;
                self.refresh(ui);
            }
            "split-caption" | "merge-caption" => {
                let (kind, id) = self.selected.clone().context("Select a caption first")?;
                ensure!(kind == "autoCaptions", "Select a caption first");
                let time = self.source_time * 1000.;
                self.edit(ui, |p| {
                    if action == "split-caption" {
                        subtake_native::caption_editing::split(p, &id, time)?;
                    } else {
                        subtake_native::caption_editing::merge_next(p, &id)?;
                    }
                    Ok(())
                })?;
            }
            "split-clip" => {
                let time = self.source_time * 1000.;
                let duration = self.info.as_ref().context("Open a video first")?.duration;
                self.edit(ui,|p|{
                    if p.regions("clipRegions").is_empty(){let clips=timeline::spans(p,duration).into_iter().map(|s|json!({"id":uuid::Uuid::new_v4().to_string(),"startMs":s.source_start*1000.,"endMs":(s.source_start+s.duration())*1000.,"speed":s.speed,"muted":s.muted})).collect::<Vec<_>>();p.set("clipRegions",json!(clips));}
                    let clip=p.regions("clipRegions").iter().find(|c|{let start=n(c,"startMs",0.);time>start+20.&&time<start+(n(c,"endMs",0.)-start)*n(c,"speed",1.)-20.}).context("Playhead must be inside a clip")?.clone();
                    let start=n(&clip,"startMs",0.);let speed=n(&clip,"speed",1.);let source_end=start+(n(&clip,"endMs",0.)-start)*speed;let id=clip["id"].as_str().unwrap();
                    p.change_region("clipRegions",id,json!({"endMs":start+(time-start)/speed}))?;
                    let mut right=clip.clone();right["id"]=Value::Null;right["startMs"]=json!(time);right["endMs"]=json!(time+(source_end-time)/speed);let new_id=p.add("clipRegions",right)?;
                    if let Some(map)=p.editor.get_mut("sourceAudioTrackSettingsByClip").and_then(Value::as_object_mut){if let Some(settings)=map.get(id).cloned(){map.insert(new_id,settings);}}
                    Ok(())
                })?;
            }
            "delete" => {
                let keys = self.selected_keys();
                ensure!(!keys.is_empty(), "Select a region first");
                self.edit(ui, |p| {
                    for (kind, id) in keys {
                        p.remove_region(&kind, &id)?;
                    }
                    Ok(())
                })?;
                self.selected = None;
                self.extra_selection.clear();
                self.refresh(ui);
            }
            "previous-frame" => self.seek(
                ui,
                self.source_time - 1. / self.info.as_ref().map(|i| i.fps).unwrap_or(30.),
            ),
            "next-frame" => self.seek(
                ui,
                self.source_time + 1. / self.info.as_ref().map(|i| i.fps).unwrap_or(30.),
            ),
            "play" => {
                if self.started.is_some() {
                    self.stop(ui)
                } else {
                    let p = self.project()?.clone();
                    let info = self.info.clone().unwrap();
                    let source = self.source.clone().unwrap();
                    let spans = timeline::spans(&p, info.duration);
                    let mut output = timeline::output_time(&spans, self.source_time);
                    if output >= timeline::duration(&spans) - 0.01 {
                        output = 0.;
                    }
                    self.audio_cancel = Arc::new(AtomicBool::new(false));
                    let cancel = self.audio_cancel.clone();
                    let epoch = self.epoch;
                    let clock = Arc::new(AtomicU64::new(0));
                    self.started = Some((clock.clone(), output));
                    std::thread::spawn(move || {
                        if let Err(e) =
                            export::play_audio(p, source, info, output, cancel.clone(), clock)
                        {
                            post(move |s, ui| {
                                if s.epoch == epoch
                                    && Arc::ptr_eq(&s.audio_cancel, &cancel)
                                    && !cancel.load(Ordering::Relaxed)
                                {
                                    s.stop(ui);
                                    ui.set_status(format!("Audio: {e:#}").into())
                                }
                            })
                        }
                    });
                    ui.set_playing(true);
                    self.playback
                        .start(TimerMode::Repeated, Duration::from_millis(33), || {
                            with_app(|s, ui| {
                                if let (Some((start, at)), Some(info), Some(history)) =
                                    (s.started.clone(), &s.info, &s.history)
                                {
                                    let spans = timeline::spans(&history.project, info.duration);
                                    let output =
                                        at + start.load(Ordering::Relaxed) as f64 / 1_000_000.;
                                    if output >= timeline::duration(&spans) {
                                        s.stop(ui);
                                        return;
                                    }
                                    s.source_time = timeline::source_time(&spans, output);
                                    s.update_time(ui);
                                    s.request();
                                }
                            })
                        });
                }
            }
            "choose-background" | "choose-webcam" => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let value = json!(path.to_string_lossy());
                    self.edit(ui, |p| {
                        if action == "choose-background" {
                            p.set("wallpaper", value)
                        } else {
                            let mut webcam = p.editor.get("webcam").cloned().unwrap_or(json!({}));
                            webcam["sourcePath"] = value;
                            webcam["enabled"] = json!(true);
                            p.set("webcam", webcam);
                        }
                        Ok(())
                    })?;
                }
            }
            "import-captions" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Subtitles", &["srt"])
                    .pick_file()
                {
                    let cues = parse_srt(&std::fs::read_to_string(path)?)?;
                    self.edit(ui, |p| {
                        p.set("autoCaptions", json!(cues));
                        Ok(())
                    })?;
                }
            }
            "export" => {
                let p = self.project()?.clone();
                let source = self.source.clone().unwrap();
                let settings = ExportSettings::for_media(&p, self.info.as_ref().unwrap());
                let Some(path) = rfd::FileDialog::new()
                    .set_file_name(if settings.gif {
                        "SubTake.gif"
                    } else {
                        "SubTake.mp4"
                    })
                    .save_file()
                else {
                    return Ok(());
                };
                self.stop(ui);
                ui.set_busy(true);
                ui.set_progress(0.);
                ui.set_status("Exporting…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    let result = export::export(&p, &source, &settings, &path, &cancel, |value| {
                        post(move |_, ui| ui.set_progress(value))
                    });
                    post(move |s, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(()) => {
                                s.last_export = Some(path.clone());
                                ui.set_status(format!("Exported {}", path.display()).into());
                            }
                            Err(e) => ui.set_status(format!("{e:#}").into()),
                        }
                    });
                });
            }
            "cancel" => {
                self.job_cancel.store(true, Ordering::Relaxed);
                ui.set_status("Cancelling…".into());
            }
            "reveal-export" => platform::reveal(
                self.last_export
                    .as_deref()
                    .context("Export a video first")?,
            )?,
            "devices" => {
                std::thread::spawn(|| {
                    let result = platform::devices();
                    post(move |s, ui| match result {
                        Ok(devices) => {
                            let names = |key: &str| {
                                let mut names = vec![SharedString::from("System default")];
                                if let Some(list) = devices[key].as_array() {
                                    names.extend(list.iter().map(|v| {
                                        SharedString::from(v["name"].as_str().unwrap_or("Device"))
                                    }));
                                }
                                ModelRc::new(VecModel::from(names))
                            };
                            ui.set_camera_names(names("cameras"));
                            ui.set_microphone_names(names("microphones"));
                            s.devices = devices;
                        }
                        Err(e) => ui.set_status(format!("Devices: {e:#}").into()),
                    });
                });
            }
            "sources" | "sources-passive" => {
                self.action(ui, "devices")?;
                ui.set_busy(true);
                ui.set_sources_loading(true);
                ui.set_recording_hint("Finding displays and windows…".into());
                ui.set_status("Finding displays and windows…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                let request_access = action == "sources";
                std::thread::spawn(move || {
                    let result = platform::sources_cancellable(&cancel, request_access);
                    post(move |s, ui| {
                        if !Arc::ptr_eq(&s.job_cancel, &cancel) {
                            return;
                        }
                        s.finish_sources(ui, result, cancel.load(Ordering::Relaxed));
                    });
                });
            }
            "record" => {
                self.stop(ui);
                ui.hide()?;
                platform::set_editor_active(false);
                ui.set_panel("Recording".into());
                self.refresh(ui);
                self.show_launcher(ui)?;
                if !ui.get_busy() {
                    ui.set_status("Choose a source, then press the red Record button.".into());
                }
            }
            "drag-window" => {
                use slint::winit_030::WinitWindowAccessor;
                ui.window().with_winit_window(|window| {
                    let _ = window.drag_window();
                });
            }
            "start-recording" => {
                ensure!(self.recording.is_none(), "A recording is already running");
                ensure!(self.can_replace(ui), "Recording cancelled");
                let mut source = self
                    .sources
                    .get(ui.get_source_index().max(0) as usize)
                    .context("Choose a recording source")?
                    .clone();
                let directory = self.recording_directory()?;
                std::fs::create_dir_all(&directory).context("Create recordings folder")?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                let path = directory.join(format!(
                    "Recording-{stamp}-{}.mp4",
                    &uuid::Uuid::new_v4().to_string()[..8]
                ));
                source["cameraId"] = self.devices["cameras"]
                    .as_array()
                    .and_then(|a| a.get((ui.get_camera_index() - 1) as usize))
                    .map(|v| v["id"].clone())
                    .unwrap_or(Value::Null);
                source["microphoneId"] = self.devices["microphones"]
                    .as_array()
                    .and_then(|a| a.get((ui.get_microphone_index() - 1) as usize))
                    .map(|v| v["id"].clone())
                    .unwrap_or(Value::Null);
                let camera = ui.get_capture_camera();
                let mic = ui.get_capture_mic();
                let system = ui.get_capture_system();
                self.stop(ui);
                self.show_launcher(ui)?;
                ui.hide()?;
                platform::set_editor_active(false);
                self.set_launcher_options_panel(ui, "")?;
                self.capture_started = None;
                self.pause_started = None;
                self.paused_total = Duration::ZERO;
                ui.set_recording_paused(false);
                ui.set_busy(true);
                ui.set_status("Preparing recording…".into());
                let countdown = self.preferences.countdown_seconds;
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    for remaining in (1..=countdown).rev() {
                        post(move |_, ui| {
                            ui.set_status(format!("Recording starts in {remaining}…").into())
                        });
                        for _ in 0..20 {
                            if cancel.load(Ordering::Relaxed) {
                                post(|_, ui| {
                                    ui.set_busy(false);
                                    ui.set_status("Recording cancelled".into());
                                });
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                    let result = Recording::start(&source, path, mic, system, camera, &cancel);
                    post(move |s, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(recording) => {
                                s.recording = Some(recording);
                                ui.set_recording(true);
                                s.capture_started = Some(std::time::Instant::now());
                                s.recording_watch.start(
                                    TimerMode::Repeated,
                                    Duration::from_millis(250),
                                    || {
                                        with_app(|s, ui| {
                                            if let Some(error) =
                                                s.recording.as_mut().and_then(Recording::error)
                                            {
                                                if let Some(recording) = s.recording.take() {
                                                    std::thread::spawn(move || {
                                                        if let Err(e) = recording.stop() {
                                                            post(move |_, ui| {
                                                                ui.set_status(
                                                                    format!("{e:#}").into(),
                                                                )
                                                            });
                                                        }
                                                    });
                                                }
                                                s.recording_watch.stop();
                                                ui.set_recording(false);
                                                ui.set_status(error.into());
                                            }
                                        })
                                    },
                                );
                                ui.set_status(
                                    "Recording · use Stop recording to open the editor".into(),
                                );
                            }
                            Err(e) => ui.set_status(format!("{e:#}").into()),
                        }
                    });
                });
            }
            "pause-recording" => {
                if let Some(mut recording) = self.recording.take() {
                    ui.set_busy(true);
                    ui.set_status("Updating recording…".into());
                    std::thread::spawn(move || {
                        let result = recording.pause();
                        post(move |s, ui| {
                            ui.set_busy(false);
                            ui.set_recording_paused(recording.paused);
                            if recording.paused {
                                s.pause_started.get_or_insert_with(std::time::Instant::now);
                            } else if let Some(start) = s.pause_started.take() {
                                s.paused_total += start.elapsed();
                            }
                            s.recording = Some(recording);
                            match result {
                                Ok(()) => ui.set_status(
                                    if ui.get_recording_paused() {
                                        "Recording paused"
                                    } else {
                                        "Recording resumed"
                                    }
                                    .into(),
                                ),
                                Err(e) => ui.set_status(format!("{e:#}").into()),
                            }
                        });
                    });
                }
            }
            "stop-recording" => {
                if let Some(recording) = self.recording.take() {
                    self.recording_watch.stop();
                    ui.set_recording(false);
                    ui.set_busy(true);
                    ui.set_status("Finalizing recording…".into());
                    std::thread::spawn(move || {
                        let result = recording.stop();
                        post(move |s, ui| {
                            ui.set_busy(false);
                            match result {
                                Ok(path) => {
                                    s.fresh_recording = Some(path.clone());
                                    let r = s.load(ui, path);
                                    report(ui, r);
                                }
                                Err(e) => ui.set_status(format!("{e:#}").into()),
                            }
                        });
                    });
                }
            }
            "auto-zoom" => {
                let source = self.source.as_ref().context("Open a video first")?;
                let duration = self.info.as_ref().unwrap().duration * 1000.;
                let zooms =
                    subtake_native::autozoom::from_source(self.project()?, source, duration)?;
                ensure!(
                    !zooms.is_empty(),
                    "No unused click clusters are available for zoom suggestions"
                );
                self.edit(ui, |p| {
                    for z in zooms {
                        p.add("zoomRegions", z)?;
                    }
                    Ok(())
                })?;
            }
            "transcribe" => {
                self.stop(ui);
                let runtime = match self
                    .preferences
                    .whisper_runtime
                    .clone()
                    .filter(|p| p.is_file())
                    .map(Ok)
                    .unwrap_or_else(|| media::binary("whisper-cli"))
                {
                    Ok(path) => path,
                    Err(_) => {
                        let Some(path) = rfd::FileDialog::new()
                            .set_title("Choose whisper-cli")
                            .pick_file()
                        else {
                            return Ok(());
                        };
                        path
                    }
                };
                let model = if let Some(path) = self
                    .preferences
                    .whisper_model
                    .clone()
                    .filter(|p| p.is_file())
                {
                    path
                } else {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose Whisper model (.bin)")
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    path
                };
                self.preferences.whisper_runtime = Some(runtime.clone());
                self.preferences.whisper_model = Some(model.clone());
                self.preferences.save()?;
                let source = self.source.clone().context("Open a video first")?;
                let epoch = self.epoch;
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                ui.set_busy(true);
                let language = self
                    .project()?
                    .text("nativeCaptionLanguage", "auto")
                    .to_owned();
                ui.set_status("Transcribing locally…".into());
                std::thread::spawn(move || {
                    let result = subtake_native::transcription::transcribe(
                        &source,
                        &runtime,
                        &model,
                        &language,
                        &cancel,
                        |message| {
                            let message = message.to_owned();
                            post(move |_, ui| ui.set_status(message.into()));
                        },
                    );
                    post(move |s, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(cues) if s.epoch == epoch => {
                                let result = s.edit(ui, |p| {
                                    p.set("autoCaptions", json!(cues));
                                    Ok(())
                                });
                                report(ui, result);
                            }
                            Ok(_) => ui.set_status(
                                "Document changed during transcription; captions were not applied."
                                    .into(),
                            ),
                            Err(e) => ui.set_status(format!("{e:#}").into()),
                        }
                    });
                });
            }
            action if action.starts_with("add-") => self.add_region(ui, action)?,
            _ => (),
        }
        Ok(())
    }
    fn add_region(&mut self, ui: &EditorWindow, action: &str) -> Result<()> {
        let duration = self.info.as_ref().context("Open a video first")?.duration * 1000.;
        let start = (self.source_time * 1000.).min((duration - 100.).max(0.));
        let end = (start + 2000.).min(duration);
        let (key, mut region) = match action {
            "add-zoom" => (
                "zoomRegions",
                json!({"depth":3,"focus":{"cx":0.5,"cy":0.5},"mode":"manual"}),
            ),
            "add-trim" => ("trimRegions", json!({})),
            "add-speed" => ("speedRegions", json!({"speed":1.5})),
            "add-caption" => ("autoCaptions", json!({"text":"Your caption"})),
            "add-audio" => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio", &["wav", "mp3", "m4a", "aac", "flac"])
                    .pick_file()
                else {
                    return Ok(());
                };
                ("audioRegions", json!({"audioPath":path,"volume":1.}))
            }
            _ => {
                let kind = action.trim_start_matches("add-");
                let mut r = json!({"type":kind,"content":"Your text","textContent":"Your text","position":{"x":35.,"y":40.},"size":{"width":30.,"height":20.},"style":{"fontSize":64.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"transparent","textAlign":"center","borderRadius":8},"figureData":{"arrowDirection":"right","color":"#2563eb","strokeWidth":5},"zIndex":1,"blurIntensity":20});
                if kind == "image" {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    r["imageContent"] = json!(path);
                }
                ("annotationRegions", r)
            }
        };
        region["startMs"] = json!(start);
        region["endMs"] = json!(end);
        let mut selected = None;
        self.edit(ui, |p| {
            selected = Some(p.add(key, region)?);
            Ok(())
        })?;
        self.extra_selection.clear();
        self.selected = selected.map(|id| (key.into(), id));
        ui.set_panel("Selection".into());
        self.refresh(ui);
        Ok(())
    }
}
fn aspect_ratio(p: &Project, info: &MediaInfo) -> f64 {
    if p.text("aspectRatio", "native") == "native" {
        let crop = p.editor.get("cropRegion").unwrap_or(&Value::Null);
        return (info.width as f64 * n(crop, "width", 1.)
            / (info.height as f64 * n(crop, "height", 1.)))
        .clamp(0.25, 4.);
    }
    p.text("aspectRatio", "16:9")
        .split_once(':')
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .filter(|v| v.is_finite() && *v >= 0.25 && *v <= 4.)
        .unwrap_or(info.width as f64 / info.height as f64)
}

fn get_nested(v: &Value, key: &str) -> Value {
    key.split('.')
        .fold(v, |v, k| v.get(k).unwrap_or(&Value::Null))
        .clone()
}
fn set_nested(v: &mut Value, key: &str, value: Value) {
    if !v.is_object() {
        *v = json!({})
    }
    if let Some((a, b)) = key.split_once('.') {
        if v.get(a).is_none() {
            v[a] = json!({})
        }
        set_nested(&mut v[a], b, value);
    } else {
        v[key] = value;
    }
}

pub fn run(path: Option<PathBuf>) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
        use slint::winit_030::winit::platform::macos::{
            ActivationPolicy, EventLoopBuilderExtMacOS,
        };
        let mut events = slint::winit_030::winit::event_loop::EventLoop::with_user_event();
        events.with_activation_policy(ActivationPolicy::Accessory);
        slint::BackendSelector::new()
            .with_winit_event_loop_builder(events)
            .with_winit_window_attributes_hook(move |attributes| {
                // This hook runs before Slint supplies title/no-frame properties.
                // Never infer window identity here: the recorder must start clear.
                let attributes = attributes.with_transparent(true).with_blur(false);
                if attributes.decorations && attributes.title != "SubTake recording" {
                    attributes
                        .with_titlebar_transparent(true)
                        .with_title_hidden(true)
                        .with_fullsize_content_view(true)
                } else {
                    attributes
                }
            })
            .select()?;
    }
    let ui = EditorWindow::new()?;
    ui.set_mac_titlebar(cfg!(target_os = "macos"));
    ui.on_translate(|text, locale| subtake_native::localization::translate(&text, &locale).into());
    let state = Rc::new(RefCell::new(App::new()));
    STATE.with(|s| *s.borrow_mut() = Some((state.clone(), ui.as_weak())));
    let wallpaper_paths = state.borrow().wallpapers.clone();
    std::thread::spawn(move || {
        let tiles = wallpaper_paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| {
                let relative = path
                    .strip_prefix(media::resources().join("public/wallpapers"))
                    .ok()?;
                let bundled = media::resources().join("assets/wallpaper-thumbnails");
                let thumbnail_root = if bundled.is_dir() {
                    bundled
                } else {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/wallpaper-thumbnails")
                };
                let thumbnail = thumbnail_root.join(relative);
                let img = image::open(if thumbnail.is_file() {
                    &thumbnail
                } else {
                    path
                })
                .ok()?
                .thumbnail(160, 100)
                .to_rgba8();
                Some((
                    index,
                    path.file_stem()?.to_string_lossy().replace(['-', '_'], " "),
                    img.width(),
                    img.height(),
                    img.into_raw(),
                ))
            })
            .collect::<Vec<_>>();
        post(move |_, ui| {
            ui.set_wallpapers(ModelRc::new(VecModel::from(
                tiles
                    .into_iter()
                    .map(|(index, title, w, h, pixels)| Wallpaper {
                        key: format!("wallpaper-{index}").into(),
                        title: title.into(),
                        source: slint::Image::from_rgba8(slint::SharedPixelBuffer::<
                            slint::Rgba8Pixel,
                        >::clone_from_slice(
                            &pixels, w, h
                        )),
                    })
                    .collect::<Vec<_>>(),
            )));
        });
    });
    if let Err(e) = state.borrow_mut().register_hotkeys() {
        ui.set_status(format!("Global shortcuts: {e}").into());
    }
    global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(
        |event: global_hotkey::GlobalHotKeyEvent| {
            if event.state == global_hotkey::HotKeyState::Pressed {
                post(move |s, ui| {
                    let action = s
                        .hotkey_ids
                        .iter()
                        .find(|(id, _)| *id == event.id)
                        .map(|(_, a)| a.clone());
                    if let Some(mut action) = action {
                        if action == "record" && s.recording.is_some() {
                            action = "stop-recording".into();
                        } else if action == "record"
                            && s.launcher.as_ref().is_some_and(|l| l.window().is_visible())
                            && !s.sources.is_empty()
                        {
                            action = "start-recording".into();
                        }
                        let result = s.action(ui, &action);
                        report(ui, result);
                    }
                });
            }
        },
    ));
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_install_document_events(open_document_event);
    }
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
    ui.window().on_winit_window_event(|_, event| {
        if let WindowEvent::DroppedFile(path) = event {
            let path = path.clone();
            post(move |s, ui| {
                if !ui.get_busy() && s.recording.is_none() && s.can_replace(ui) {
                    let result = s.load(ui, path);
                    report(ui, result);
                }
            });
            EventResult::PreventDefault
        } else {
            EventResult::Propagate
        }
    });
    let tray = AppTray::new()?;
    #[cfg(not(target_os = "macos"))]
    tray.show()?;
    tray.on_action(|action| {
        with_app(|s, ui| {
            let result = s.action(ui, &action);
            report(ui, result);
        })
    });
    state.borrow_mut().tray = Some(tray);
    ui.on_action(|a| {
        with_app(|s, ui| {
            let result = s.action(ui, &a);
            report(ui, result);
        })
    });
    ui.on_seek(|time| with_app(|s, ui| s.seek(ui, time as f64)));
    ui.on_panel_change(|panel| {
        with_app(|s, ui| {
            if panel == "Recent" {
                s.recoveries = subtake_native::recovery::list().unwrap_or_default();
                let result = s.reload_library();
                report(ui, result);
            }
            s.refresh(ui);
            s.epoch += 1;
            s.request();
        })
    });
    ui.on_field_change(|key, value| {
        with_app(|s, ui| {
            let result = s.field(ui, &key, &value);
            report(ui, result);
        })
    });
    ui.on_select_region(|kind, id, extend| {
        with_app(|s, ui| {
            let key = (kind.to_string(), id.to_string());
            let mut keys = s.selected_keys();
            if extend {
                if keys.contains(&key) {
                    keys.retain(|k| k != &key);
                } else {
                    keys.push(key.clone());
                }
            } else if !keys.contains(&key) {
                keys = vec![key.clone()];
            }
            s.selected = if keys.contains(&key) {
                Some(key)
            } else {
                keys.last().cloned()
            };
            s.extra_selection = keys;
            ui.set_panel("Selection".into());
            s.refresh(ui);
            s.epoch += 1;
            s.request();
        })
    });
    ui.on_move_region(|kind, id, delta, mode| {
        with_app(|s, ui| {
            if delta.abs() < 0.005 {
                return;
            }
            let duration = s.info.as_ref().map(|i| i.duration * 1000.).unwrap_or(0.);
            let mut snapping = match s.project() {
                Ok(p) => p.clone(),
                Err(_) => return,
            };
            if mode == 0 {
                for (other_kind, other_id) in s.selected_keys() {
                    if other_kind != kind.as_str() || other_id != id.as_str() {
                        let _ = snapping.remove_region(&other_kind, &other_id);
                    }
                }
            }
            let delta = if ui.get_snap() {
                subtake_native::editing::snap_delta(
                    &snapping,
                    &kind,
                    &id,
                    delta as f64 * 1000.,
                    mode,
                    s.source_time * 1000.,
                    duration,
                    ui.get_timeline_visible() as f64 * 5.,
                ) / 1000.
            } else {
                delta as f64
            };
            let keys = if mode == 0 {
                s.selected_keys()
            } else {
                vec![(kind.to_string(), id.to_string())]
            };
            let result = s.edit(ui, |p| {
                subtake_native::editing::move_group(p, &keys, delta * 1000., mode, duration)
            });
            report(ui, result);
        })
    });
    ui.on_canvas_edit(|dx,dy,resize| {
        with_app(|s,ui| {
            let result=(||->Result<()> {
                ensure!(ui.get_edit_visible(),"Select a visible annotation or webcam");
                let scale=(ui.get_edit_scale() as f64).max(0.01);
                let (dx,dy)=(dx as f64,dy as f64);
                if ui.get_panel()=="Crop" {
                    return s.edit(ui,|p| subtake_native::editing::adjust_crop(p,dx,dy,resize));
                }
                if ui.get_panel()=="Webcam" {
                    let aspect=aspect_ratio(s.project()?,s.info.as_ref().unwrap());
                    let w=960.;let h=(w/aspect).round().clamp(100.,1920.);let unit=w/1920.;
                    let x=ui.get_edit_x() as f64;let y=ui.get_edit_y() as f64;
                    let bw=ui.get_edit_width() as f64;let bh=ui.get_edit_height() as f64;
                    s.edit(ui,|p| {
                        let mut wc=p.editor.get("webcam").cloned().unwrap_or(json!({}));
                        if resize {wc["width"]=json!((n(&wc,"width",40.)+dx*w/(w.min(h)*scale)*100.).clamp(5.,100.));wc["height"]=json!((n(&wc,"height",40.)+dy*h/(w.min(h)*scale)*100.).clamp(5.,100.));}
                        else {let margin=n(&wc,"margin",24.)*unit;wc["positionPreset"]=json!("custom");wc["positionX"]=json!(((x+dx-margin/w)/(1.-bw-2.*margin/w).max(0.001)).clamp(0.,1.));wc["positionY"]=json!(((y+dy-margin/h)/(1.-bh-2.*margin/h).max(0.001)).clamp(0.,1.));}
                        p.set("webcam",wc);Ok(())
                    })
                } else {
                    let (kind,id)=s.selected.clone().context("Select an annotation")?;
                    ensure!(kind=="annotationRegions","Select an annotation");
                    s.edit(ui,|p| {let r=p.regions(&kind).iter().find(|r|r["id"]==id).context("Annotation no longer exists")?;
                        let patch=if resize {json!({"size":{"width":(n(&r["size"],"width",30.)+dx/scale*100.).clamp(1.,120.),"height":(n(&r["size"],"height",20.)+dy/scale*100.).clamp(1.,120.)}})}else{json!({"position":{"x":(n(&r["position"],"x",50.)+dx/scale*100.).clamp(-100.,200.),"y":(n(&r["position"],"y",50.)+dy/scale*100.).clamp(-100.,200.)}})};
                        p.change_region(&kind,&id,patch)
                    })
                }
            })();report(ui,result);
        })
    });
    ui.on_preview_click(|x, y| {
        with_app(|s, ui| {
            if ui.get_panel() == "Crop" {
                return;
            }
            if let Some((kind, id)) = s.selected.clone() {
                if kind == "zoomRegions" {
                    let result = (|| -> Result<()> {
                        let p = s.project()?;
                        let info = s.info.as_ref().context("Open a video first")?;
                        let width = 960.;
                        let height = (width / aspect_ratio(p, info)).round().clamp(100., 1920.);
                        let frame = subtake_native::geometry::frame(
                            p,
                            width,
                            height,
                            info.width as f64,
                            info.height as f64,
                        );
                        let mut sidecar = s.source.as_ref().unwrap().as_os_str().to_os_string();
                        sidecar.push(".cursor.json");
                        let telemetry: Value = std::fs::read(PathBuf::from(sidecar))
                            .ok()
                            .and_then(|b| serde_json::from_slice(&b).ok())
                            .unwrap_or(Value::Null);
                        let samples = telemetry
                            .as_array()
                            .or_else(|| telemetry["samples"].as_array())
                            .map(Vec::as_slice)
                            .unwrap_or(&[]);
                        let camera = subtake_native::motion::CameraTrack::default().at(
                            p,
                            samples,
                            s.source_time * 1000.,
                            width,
                            height,
                            &frame,
                        );
                        let cx = (((x as f64 * width - camera.x) / camera.scale - frame.x)
                            / frame.width)
                            .clamp(0., 1.);
                        let cy = (((y as f64 * height - camera.y) / camera.scale - frame.y)
                            / frame.height)
                            .clamp(0., 1.);
                        s.edit(ui, |p| {
                            p.change_region(&kind, &id, json!({"focus":{"cx":cx,"cy":cy}}))
                        })
                    })();
                    report(ui, result);
                }
            }
        })
    });
    ui.on_keyboard(|key, command, shift, alt| {
        let mut handled = false;
        with_app(|s, ui| {
            let configured = subtake_native::shortcuts::action(
                &s.preferences.editor_shortcuts,
                &key,
                command,
                shift,
                alt,
            );
            let action = if let Some(action) = configured.as_deref() {
                Some(action)
            } else if command && !alt {
                match key.to_lowercase().as_str() {
                    "o" => Some("open"),
                    "a" => Some("select-all"),
                    "c" => Some("copy"),
                    "x" => Some("cut"),
                    "v" => Some("paste"),
                    "d" if shift => Some("duplicate"),
                    "b" => Some("split-clip"),
                    "s" => Some(if shift { "save-as" } else { "save" }),
                    "z" => Some(if shift { "redo" } else { "undo" }),
                    _ => None,
                }
            } else if key == " " {
                Some("play")
            } else if key == SharedString::from(slint::platform::Key::Delete)
                || key == SharedString::from(slint::platform::Key::Backspace)
            {
                Some("delete")
            } else {
                None
            };
            if let Some(action) = action {
                handled = true;
                let result = s.action(ui, action);
                report(ui, result);
            }
        });
        handled
    });
    ui.window().on_close_requested(|| {
        let mut close = false;
        with_app(|s, ui| {
            if s.recording.is_some() || ui.get_busy() {
                ui.set_status(
                    "Finish or cancel the current operation before closing SubTake.".into(),
                );
                return;
            }
            close = true;
            s.stop(ui);
            platform::set_editor_active(false);
        });
        if close {
            slint::CloseRequestResponse::HideWindow
        } else {
            slint::CloseRequestResponse::KeepWindowShown
        }
    });
    if let Some(snapshot) = std::env::var_os("SUBTAKE_UI_SNAPSHOT") {
        if let Ok(language) = std::env::var("SUBTAKE_UI_LANGUAGE") {
            ensure!(
                subtake_native::localization::LOCALES.contains(&language.as_str()),
                "Unsupported smoke-test language"
            );
            state.borrow_mut().preferences.language = language;
        }
        let smoke_ui = ui.as_weak();
        Timer::single_shot(Duration::from_secs(3), move || {
            if let Some(ui) = smoke_ui.upgrade() {
                for (y, panel) in [(146.0, "Cursor"), (204.0, "Webcam"), (88.0, "Frame")] {
                    let position = slint::LogicalPosition::new(38., y);
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                            position,
                            button: slint::platform::PointerEventButton::Left,
                        });
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                            position,
                            button: slint::platform::PointerEventButton::Left,
                        });
                    if ui.get_panel() != panel {
                        eprintln!("UI_SMOKE_FAILED: rail click did not open {panel}");
                        std::process::exit(1);
                    }
                }
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerExited);
            }
            with_app(|s, ui| {
                let result = (|| -> Result<()> {
                    ensure!(s.history.is_some(), "UI fixture did not load");
                    if let Some(size) = std::env::var("SUBTAKE_UI_SIZE").ok().and_then(|v| {
                        v.split_once('x').and_then(|(a, b)| {
                            Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
                        })
                    }) {
                        ui.window()
                            .set_size(slint::LogicalSize::new(size.0, size.1));
                    }
                    // Exercise discovery completion, failure, empty results, cancellation and
                    // configuration reopening without recording the user's desktop.
                    let fixtures = vec![
                        json!({"kind":"display","nativeId":1,"name":"Built-in display · 1920×1080"}),
                        json!({"kind":"window","nativeId":2,"name":"SubTake — demo window"}),
                    ];
                    for (result, cancelled) in [
                        (Err(anyhow::anyhow!("Permission denied")), false),
                        (Ok(vec![]), false),
                        (Ok(fixtures.clone()), true),
                        (Ok(fixtures.clone()), false),
                    ] {
                        ui.set_busy(true);
                        ui.set_sources_loading(true);
                        s.finish_sources(ui, result, cancelled);
                        ensure!(
                            !ui.get_busy() && !ui.get_sources_loading(),
                            "Source discovery left recording disabled"
                        );
                    }
                    ui.set_source_index(1);
                    s.finish_sources(ui, Ok(fixtures.clone()), false);
                    ensure!(
                        ui.get_source_index() == 1,
                        "Refresh changed selected recording source"
                    );
                    s.action(ui, "record")?;
                    ensure!(
                        ui.get_panel() == "Recording" && s.recording.is_none() && !ui.get_busy(),
                        "Record must open ready configuration without starting capture"
                    );
                    ensure!(!ui.window().is_visible(), "Record should hide the editor");
                    s.show_editor(ui)?;
                    ui.set_panel("Frame".into());
                    s.refresh(ui);
                    let snapshot_project = s.project()?.clone();
                    let count = s.project()?.regions("zoomRegions").len();
                    s.action(ui, "add-zoom")?;
                    ensure!(
                        s.project()?.regions("zoomRegions").len() == count + 1,
                        "Add zoom callback failed"
                    );
                    s.action(ui, "undo")?;
                    ensure!(
                        s.project()?.regions("zoomRegions").len() == count,
                        "Undo failed"
                    );
                    s.action(ui, "redo")?;
                    ensure!(
                        s.project()?.regions("zoomRegions").len() == count + 1,
                        "Redo failed"
                    );
                    s.action(ui, "copy")?;
                    s.seek(ui, 0.8);
                    s.action(ui, "paste")?;
                    ensure!(
                        s.project()?.regions("zoomRegions").len() == count + 2,
                        "Paste failed"
                    );
                    s.action(ui, "cut")?;
                    ensure!(
                        s.project()?.regions("zoomRegions").len() == count + 1,
                        "Cut failed"
                    );
                    s.action(ui, "split-clip")?;
                    ensure!(
                        s.project()?.regions("clipRegions").len() >= 2,
                        "Split clip failed"
                    );
                    s.action(ui, "undo")?;
                    let caption = s
                        .project()?
                        .regions("autoCaptions")
                        .first()
                        .context("Caption fixture missing")?["id"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    s.extra_selection.clear();
                    s.selected = Some(("autoCaptions".into(), caption.clone()));
                    let captions = s.project()?.regions("autoCaptions").len();
                    s.action(ui, "split-caption")?;
                    ensure!(
                        s.project()?.regions("autoCaptions").len() == captions + 1,
                        "Caption split failed"
                    );
                    s.action(ui, "merge-caption")?;
                    ensure!(
                        s.project()?.regions("autoCaptions").len() == captions,
                        "Caption merge failed"
                    );
                    s.field(ui, "word.0.text", "Edited")?;
                    ensure!(
                        s.project()?.regions("autoCaptions")[0]["text"]
                            .as_str()
                            .unwrap()
                            .starts_with("Edited"),
                        "Word edit failed"
                    );
                    s.action(ui, "undo")?;
                    s.action(ui, "undo")?;
                    s.action(ui, "undo")?;
                    s.action(ui, "select-all")?;
                    let selections = s.selected_keys().len();
                    ensure!(selections > 3, "Group selection failed");
                    s.action(ui, "copy")?;
                    ensure!(s.clipboard.len() == selections, "Group copy failed");
                    s.action(ui, "delete")?;
                    ensure!(s.selected_keys().is_empty(), "Group delete failed");
                    s.action(ui, "undo")?;
                    s.extra_selection.clear();
                    s.selected = None;
                    s.field(ui, "cursorStyle", "figma")?;
                    let style = s
                        .fields("Cursor")
                        .into_iter()
                        .find(|f| f.key == "cursorStyle")
                        .context("Cursor style selector missing")?;
                    ensure!(
                        style.choice == 4 && style.value == "figma",
                        "Cursor style selector lost its value"
                    );
                    s.action(ui, "undo")?;
                    s.field(ui, "padding.all", "32")?;
                    ensure!(
                        s.project()?.editor["padding"]["right"] == 32.,
                        "Linked padding failed"
                    );
                    s.field(ui, "padding.linked", "false")?;
                    s.field(ui, "padding.left", "12")?;
                    ensure!(
                        s.project()?.editor["padding"]["right"] == 32.,
                        "Independent padding changed the opposite side"
                    );
                    s.action(ui, "undo")?;
                    s.action(ui, "undo")?;
                    s.action(ui, "undo")?;
                    let original_crop = s.project()?.editor.get("cropRegion").cloned();
                    s.action(ui, "visual-crop")?;
                    ensure!(
                        ui.get_panel_index() == 11,
                        "Crop inspector selection failed"
                    );
                    s.field(ui, "cropRegion.width", "0.8")?;
                    let info = s.info.as_ref().unwrap();
                    ensure!(
                        (ui.get_preview_aspect() as f64 - info.width as f64 / info.height as f64)
                            .abs()
                            < 0.001,
                        "Crop must show the whole source"
                    );
                    s.action(ui, "finish-crop")?;
                    ensure!(ui.get_panel_index() == 0, "Finish crop failed");
                    s.action(ui, "undo")?;
                    ensure!(
                        s.project()?.editor.get("cropRegion").cloned() == original_crop,
                        "Crop undo failed"
                    );
                    let folder = tempfile::tempdir()?;
                    std::fs::write(folder.path().join("Library Test.recordly"), "{}")?;
                    s.preferences.library_directory = Some(folder.path().to_owned());
                    s.field(ui, "library.query", "Library Test")?;
                    ensure!(s.library.len() == 1, "Folder library search failed");
                    s.preferences.library_directory = None;
                    s.library_query.clear();
                    s.reload_library()?;
                    s.history = Some(History::new(snapshot_project));
                    ui.set_panel("Cursor".into());
                    s.refresh(ui);
                    s.seek(ui, 0.0);
                    // Settings callbacks must persist outside the project and never dirty it.
                    let original_project = s.project()?.clone();
                    for appearance in ["light", "dark", "system"] {
                        s.field(ui, "prefs.appearance", appearance)?;
                        ensure!(
                            ui.get_appearance() == appearance,
                            "Appearance did not reach UI"
                        );
                        ensure!(
                            subtake_native::preferences::Preferences::load()?.appearance
                                == appearance,
                            "Appearance did not persist"
                        );
                    }
                    s.field(ui, "prefs.auto_apply_zooms", "false")?;
                    ensure!(
                        !ui.get_auto_apply_zooms()
                            && !subtake_native::preferences::Preferences::load()?.auto_apply_zooms,
                        "Automatic zoom preference did not persist"
                    );
                    s.field(ui, "prefs.auto_apply_zooms", "true")?;
                    ensure!(
                        s.project()? == &original_project,
                        "Application preferences altered project content"
                    );
                    for (key, value) in [
                        ("cursorStyle", "dot"),
                        ("webcam.positionPreset", "top-left"),
                        ("nativeCaptionLanguage", "fr"),
                        ("autoCaptionSettings.fontFamily", "Georgia"),
                    ] {
                        s.field(ui, key, value)?;
                        s.action(ui, "undo")?;
                        ensure!(
                            s.project()? == &original_project,
                            "Visual choice failed to undo: {key}"
                        );
                    }
                    if let Ok(appearance) = std::env::var("SUBTAKE_UI_APPEARANCE") {
                        s.field(ui, "prefs.appearance", &appearance)?;
                    }
                    if let Ok(panel) = std::env::var("SUBTAKE_UI_PANEL") {
                        if panel == "Crop" {
                            s.action(ui, "visual-crop")?;
                            s.field(ui, "cropRegion.width", "0.8")?;
                            s.field(ui, "cropRegion.height", "0.8")?;
                        } else {
                            ui.set_panel(panel.into());
                            s.refresh(ui);
                            s.epoch += 1;
                            s.request();
                        }
                    }
                    if std::env::var_os("SUBTAKE_UI_EMPTY").is_some() {
                        s.stop(ui);
                        s.epoch += 1;
                        s.source_time = 0.;
                        s.history = None;
                        s.info = None;
                        s.source = None;
                        s.document = None;
                        s.selected = None;
                        s.refresh(ui);
                        ui.set_preview(slint::Image::default());
                        ui.set_status("Choose a source and press Start recording".into());
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    eprintln!("UI_SMOKE_FAILED: {e:#}");
                    std::process::exit(1);
                }
                Timer::single_shot(Duration::from_secs(2), move || {
                    with_app(|s, ui| {
                        let panel = std::env::var("SUBTAKE_UI_PANEL").unwrap_or("Cursor".into());
                        let expected = [
                            "Frame",
                            "Cursor",
                            "Webcam",
                            "Captions",
                            "Selection",
                            "Recording",
                            "Export",
                            "Audio",
                            "Preferences",
                            "Recent",
                            "Wallpapers",
                            "Crop",
                            "Presets",
                            "Shortcuts",
                        ]
                        .iter()
                        .position(|p| *p == panel)
                        .unwrap_or(1) as i32;
                        if ui.get_panel().as_str() != panel || ui.get_panel_index() != expected {
                            eprintln!(
                                "UI_SMOKE_FAILED: inspector selection and displayed panel diverged"
                            );
                            std::process::exit(1);
                        }
                        if panel == "Wallpapers" {
                            use slint::Model;
                            if ui.get_wallpapers().row_count() == 0 {
                                eprintln!("UI_SMOKE_FAILED: wallpaper thumbnails were not loaded");
                                std::process::exit(1);
                            }
                        }
                        s.discard_recovery();
                        s.recovery.flush();
                        match ui.window().take_snapshot() {
                            Ok(buffer) => {
                                if let Err(e) = image::save_buffer(
                                    PathBuf::from(snapshot),
                                    buffer.as_bytes(),
                                    buffer.width(),
                                    buffer.height(),
                                    image::ColorType::Rgba8,
                                ) {
                                    {
                                        eprintln!("UI_SNAPSHOT_FAILED: {e}");
                                        std::process::exit(1)
                                    }
                                } else {
                                    println!("UI_SMOKE_PASSED")
                                }
                            }
                            Err(e) => {
                                eprintln!("UI_SNAPSHOT_FAILED: {e}");
                                std::process::exit(1)
                            }
                        }
                        if std::env::var_os("SUBTAKE_UI_KEEP_OPEN").is_none() {
                            let _ = slint::quit_event_loop();
                        }
                    })
                });
            })
        });
    }
    if path.is_none() && !state.borrow().recoveries.is_empty() {
        ui.set_panel("Recent".into());
        ui.set_status("Unsaved project recovery is available in Recent.".into());
    }
    state.borrow().refresh(&ui);
    if let Some(path) = path {
        let result = state.borrow_mut().load(&ui, path);
        report(&ui, result);
    } else {
        let result = state.borrow_mut().show_launcher(&ui);
        report(&ui, result);
        state.borrow().sync_launcher(&ui);
    }
    #[cfg(target_os = "macos")]
    Timer::single_shot(Duration::from_millis(100), || {
        // AppKit status items must be created on the running main event loop.
        platform::install_status_item(status_menu_action);
    });
    // The development watcher requests a normal quit only after a successful
    // build. Never interrupt capture/export; unsaved projects use the existing
    // save/discard/cancel prompt. This timer is absent in normal app launches.
    // Refresh paused previews after window resizing, monitor scale changes or pinch zoom.
    let preview_size_timer = Timer::default();
    let mut last_preview_size = (0u32, 0u32);
    preview_size_timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
        STATE.with(|slot| {
            let context = slot.borrow().clone();
            if let Some((state, weak)) = context {
                if let Some(ui) = weak.upgrade() {
                    when_idle(&state, |s| {
                        let size = ((ui.get_preview_pixel_width() * ui.window().scale_factor()).ceil() as u32,
                            (ui.get_preview_aspect() * 10000.) as u32);
                        if size != last_preview_size {
                            last_preview_size = size;
                            s.request();
                        }
                    });
                }
            }
        });
    });
    let dev_restart_timer = Timer::default();
    if let Some(request) = std::env::var_os("SUBTAKE_DEV_RESTART_FILE") {
        let request = PathBuf::from(request);
        dev_restart_timer.start(TimerMode::Repeated, Duration::from_millis(300), move || {
            if !request.is_file() {
                return;
            }
            with_app(|s, ui| {
                if s.recording.is_some() || ui.get_busy() {
                    return;
                }
                let result = s.action(ui, "quit");
                report(ui, result);
                // Leave the request present while a save dialog is open.
                let _ = std::fs::remove_file(&request);
            });
        });
    }
    if std::env::var_os("SUBTAKE_LAUNCHER_SMOKE").is_some() {
        Timer::single_shot(Duration::from_secs(3), || launcher_smoke_step(0));
    }
    slint::run_event_loop_until_quit()?;
    state.borrow().recovery.flush();
    Ok(())
}

// Explicit opt-in acceptance harness. Capture requires a caller-selected SubTake
// window; microphone, camera and system audio are always disabled here.
fn launcher_smoke_step(step: u8) {
    with_app(|s, ui| {
        let result = (|| -> Result<()> {
            let mode = std::env::var("SUBTAKE_LAUNCHER_SMOKE")?;
            let output = PathBuf::from(std::env::var("SUBTAKE_LAUNCHER_TEST_DIRECTORY")?);
            std::fs::create_dir_all(&output)?;
            if step == 0 {
                ensure!(
                    !ui.window().is_visible(),
                    "Editor appeared before recording"
                );
                ensure!(
                    s.launcher.as_ref().is_some_and(|l| l.window().is_visible()),
                    "Recorder not visible at launch"
                );
                ensure!(
                    !ui.get_busy(),
                    "Source discovery did not finish: {}",
                    ui.get_status()
                );
                if mode == "autozoom" {
                    let source = PathBuf::from(std::env::var("SUBTAKE_AUTOZOOM_SOURCE")?);
                    s.preferences.auto_apply_zooms = true;
                    s.fresh_recording = Some(source.clone());
                    s.load(ui, source)?;
                    Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(20));
                    return Ok(());
                }
                if mode == "capture" {
                    let id = std::env::var("SUBTAKE_CAPTURE_SMOKE_WINDOW")?.parse::<u64>()?;
                    let index = s
                        .sources
                        .iter()
                        .position(|v| {
                            v["kind"] == "window"
                                && v["nativeId"].as_u64() == Some(id)
                                && v["name"].as_str().is_some_and(|name| {
                                    name.starts_with("SubTake — ")
                                        || name == "SubTake Zoom Fixture — SubTake Capture Fixture"
                                })
                        })
                        .with_context(|| {
                            format!("Capture fixture {id} absent; status: {}", ui.get_status())
                        })?;
                    ui.set_source_index(index as i32);
                } else {
                    s.finish_sources(
                        ui,
                        Ok(vec![
                            json!({"kind":"display","nativeId":1,"name":"Built-in display"}),
                            json!({"kind":"window","nativeId":2,"name":"SubTake — demo window"}),
                        ]),
                        false,
                    );
                }
                s.action(ui, "hide-launcher")?;
                ensure!(
                    !ui.window().is_visible()
                        && !s.launcher.as_ref().unwrap().window().is_visible(),
                    "Hide overlay opened the editor"
                );
                s.action(ui, "show")?;
                ensure!(
                    !ui.window().is_visible() && s.launcher.as_ref().unwrap().window().is_visible(),
                    "Tray Open did not restore the recorder"
                );
                // Exercise the source control with real native pointer events.
                let launcher = s.launcher.as_ref().unwrap();
                launcher.window().set_position(slint::PhysicalPosition::new(210, 160));
                let anchor_position = launcher.window().position();
                let anchor_size = launcher.window().size();
                let anchor_bottom = anchor_position.y + anchor_size.height as i32;
                let anchor_center = anchor_position.x + anchor_size.width as i32 / 2;
                for event in [
                    slint::platform::WindowEvent::PointerPressed {
                        position: slint::LogicalPosition::new(140., 38.),
                        button: slint::platform::PointerEventButton::Left,
                    },
                    slint::platform::WindowEvent::PointerReleased {
                        position: slint::LogicalPosition::new(140., 38.),
                        button: slint::platform::PointerEventButton::Left,
                    },
                ] {
                    launcher.window().dispatch_event(event);
                }
                ensure!(
                    launcher.get_panel() == "sources",
                    "Source control did not open its options"
                );
                s.set_launcher_options_panel(ui, "")?;
                if mode != "capture" {
                    if mode != "idle" {
                        s.set_launcher_options_panel(ui, &mode)?;
                    }
                    Timer::single_shot(Duration::from_millis(600), move || {
                        with_app(|s, _| {
                            let launcher = s.launcher.as_ref().unwrap();
                            let position = launcher.window().position();
                            let size = launcher.window().size();
                            assert!((position.y + size.height as i32 - anchor_bottom).abs() <= 1,
                                "Opening a recorder menu moved the bar vertically");
                            assert!((position.x + size.width as i32 / 2 - anchor_center).abs() <= 1,
                                "Opening a recorder menu moved the bar horizontally");
                            if mode != "idle" {
                                let options = s.launcher_options.as_ref().unwrap();
                                assert!(options.window().is_visible(), "Options window did not open");
                                let option_position = options.window().position();
                                let option_size = options.window().size();
                                assert!(option_position.y + option_size.height as i32 <= position.y - 12,
                                    "Options window overlaps the fixed recorder bar");
                                // A native move notification must carry the independent menu.
                                launcher.window().set_position(slint::PhysicalPosition::new(position.x + 37, position.y + 31));
                                Timer::single_shot(Duration::from_millis(80), move || {
                                    with_app(|s, _| {
                                        let launcher = s.launcher.as_ref().unwrap();
                                        let options = s.launcher_options.as_ref().unwrap();
                                        let moved_menu = options.window().position();
                                        let moved_bar = launcher.window().position();
                                        let _ = (moved_menu, moved_bar);
                                        assert!(platform::launcher_options_are_attached(
                                            options.window(), launcher.window(),
                                        ).unwrap_or(false),
                                            "Options menu detached, moved out of alignment, or overlaps the recorder bar");
                                    });
                                    launcher_smoke_step(10);
                                });
                                return;
                            }
                        });
                        launcher_smoke_step(10);
                    });
                    return Ok(());
                }
                ui.set_capture_mic(false);
                ui.set_capture_camera(false);
                ui.set_capture_system(false);
                s.preferences.recording_directory = Some(output.clone());
                s.preferences.countdown_seconds = 3;
                s.action(ui, "start-recording")?;
                ensure!(
                    !ui.window().is_visible() && ui.get_busy(),
                    "Countdown must keep editor hidden"
                );
                s.action(ui, "cancel")?;
                Timer::single_shot(Duration::from_millis(600), || launcher_smoke_step(1));
            } else if step == 1 {
                ensure!(
                    !ui.get_busy() && !ui.get_recording() && !ui.window().is_visible(),
                    "Countdown cancellation left capture or editor active"
                );
                s.preferences.countdown_seconds = 0;
                s.action(ui, "start-recording")?;
                let seconds = if std::env::var_os("SUBTAKE_CAPTURE_SMOKE_ZOOMS").is_some() {
                    30
                } else {
                    3
                };
                Timer::single_shot(Duration::from_secs(seconds), || launcher_smoke_step(2));
            } else if step == 2 {
                ensure!(
                    ui.get_recording() && !ui.get_busy() && !ui.window().is_visible(),
                    "Recording did not start: {}",
                    ui.get_status()
                );
                s.action(ui, "pause-recording")?;
                Timer::single_shot(Duration::from_millis(700), || launcher_smoke_step(3));
            } else if step == 3 {
                ensure!(
                    ui.get_recording_paused() && !ui.get_busy(),
                    "Pause was not acknowledged"
                );
                s.action(ui, "pause-recording")?;
                Timer::single_shot(Duration::from_millis(700), || launcher_smoke_step(4));
            } else if step == 4 {
                ensure!(
                    !ui.get_recording_paused() && !ui.get_busy(),
                    "Resume was not acknowledged"
                );
                s.action(ui, "stop-recording")?;
                Timer::single_shot(Duration::from_secs(3), || launcher_smoke_step(5));
            } else if step == 5 {
                ensure!(
                    !ui.get_recording() && !ui.get_busy() && ui.window().is_visible(),
                    "Final video did not open the editor: {}",
                    ui.get_status()
                );
                ensure!(
                    !s.launcher.as_ref().unwrap().window().is_visible(),
                    "Recorder remained visible over editor"
                );
                ensure!(
                    ui.get_panel() == "Frame" && s.info.as_ref().is_some_and(|i| i.duration > 1.),
                    "Recorded project was not loaded"
                );
                if std::env::var_os("SUBTAKE_CAPTURE_SMOKE_ZOOMS").is_some() {
                    verify_automatic_zooms(s, ui, &output)?;
                }
                let report = json!({"status":"passed","scope":"menu-bar launch, hide/Open, source control, countdown cancellation, screen-only capture of selected SubTake window, pause/resume/stop, final editor handoff","microphone":false,"camera":false,"system_audio":false,"source":s.source,"media":s.info});
                std::fs::write(
                    output.join("capture-lifecycle.json"),
                    serde_json::to_vec_pretty(&report)?,
                )?;
                let image = ui.window().take_snapshot()?;
                image::save_buffer(
                    output.join("recorded-editor.png"),
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ColorType::Rgba8,
                )?;
                s.discard_recovery();
                s.recovery.flush();
                println!("LAUNCHER_SMOKE_PASSED capture");
                slint::quit_event_loop()?;
            } else if step == 20 {
                verify_automatic_zooms(s, ui, &output)?;
                // Reopening an ordinary video must not silently re-apply zooms.
                let source = s.source.clone().context("Missing source")?;
                s.load(ui, source)?;
                Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(21));
            } else if step == 21 {
                ensure!(
                    s.project()?.regions("zoomRegions").is_empty(),
                    "Ordinary reopen applied fresh-recording zooms"
                );
                s.preferences.auto_apply_zooms = false;
                let source = s.source.clone().context("Missing source")?;
                s.fresh_recording = Some(source.clone());
                s.load(ui, source)?;
                Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(22));
            } else if step == 22 {
                ensure!(
                    s.project()?.regions("zoomRegions").is_empty(),
                    "Disabled preference still applied zooms"
                );
                s.discard_recovery();
                s.recovery.flush();
                println!(
                    "LAUNCHER_SMOKE_PASSED autozoom: fresh applied, ordinary reopen unchanged, preference off respected"
                );
                slint::quit_event_loop()?;
            } else if step == 10 {
                ensure!(!ui.window().is_visible(), "Setup opened an empty editor");
                let image = s.launcher.as_ref().unwrap().window().take_snapshot()?;
                image::save_buffer(
                    output.join(format!("launcher-{mode}.png")),
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ColorType::Rgba8,
                )?;
                println!("LAUNCHER_SMOKE_PASSED {mode}");
                slint::quit_event_loop()?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            if let Some(recording) = s.recording.take() {
                let _ = recording.stop();
            }
            eprintln!("LAUNCHER_SMOKE_FAILED step {step}: {error:#}");
            std::process::exit(1);
        }
    });
}

fn verify_automatic_zooms(s: &mut App, ui: &EditorWindow, output: &std::path::Path) -> Result<()> {
    let info = s.info.as_ref().context("Video did not load")?;
    ensure!(
        ui.window().is_visible(),
        "Fresh recording did not open editor"
    );
    ensure!(
        (info.width as f64 / info.height as f64) < 1.2,
        "Fixture must exercise the former aspect-ratio gate"
    );
    let project = s.project()?;
    let regions = project.regions("zoomRegions");
    ensure!(
        !regions.is_empty(),
        "Fresh near-square recording has zero automatic zooms: {}",
        ui.get_status()
    );
    let source = s.source.as_ref().context("Missing source")?;
    let mut sidecar = source.as_os_str().to_os_string();
    sidecar.push(".cursor.json");
    let data: Value = serde_json::from_slice(&std::fs::read(PathBuf::from(sidecar))?)?;
    let points = data
        .as_array()
        .or_else(|| data["samples"].as_array())
        .context("Missing samples")?;
    let frame =
        subtake_native::geometry::frame(project, 960., 540., info.width as f64, info.height as f64);
    let mut camera = subtake_native::motion::CameraTrack::default();
    let max_scale = (0..(info.duration * 30.) as usize)
        .map(|i| {
            camera
                .at(project, points, i as f64 * 1000. / 30., 960., 540., &frame)
                .scale
        })
        .fold(1.0_f64, f64::max);
    ensure!(max_scale > 1.1, "Generated zooms do not magnify playback");
    project.save(&output.join("automatic-zoom.recordly"))?;
    std::fs::write(
        output.join("automatic-zoom.json"),
        serde_json::to_vec_pretty(&json!({
            "source":source,"media":info,"zoom_regions":regions,"sample_count":points.len(),"max_playback_scale":max_scale,
            "scope":"fresh-recording load and canonical playback camera, without Suggest zooms"
        }))?,
    )?;
    Ok(())
}

fn id_to_null(mut value: Value) -> Value {
    value["id"] = Value::Null;
    value
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_install_document_events(callback: extern "C" fn(*const std::ffi::c_char));
}
#[cfg(target_os = "macos")]
extern "C" fn open_document_event(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    // SAFETY: the Objective-C bridge passes a live, NUL-terminated UTF-8 string;
    // copying it here finishes before the autoreleased URL leaves that callback.
    let path = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .to_string();
    post(move |s, ui| {
        if ui.get_busy() || s.recording.is_some() {
            ui.set_status("Finish the current operation before opening another project.".into());
            return;
        }
        if s.can_replace(ui) {
            let result = s.load(ui, PathBuf::from(path));
            report(ui, result);
        }
    });
}

#[cfg(target_os = "macos")]
extern "C" fn status_menu_action(action: *const std::ffi::c_char) {
    if action.is_null() {
        return;
    }
    let action = unsafe { std::ffi::CStr::from_ptr(action) }
        .to_string_lossy()
        .to_string();
    post(move |s, ui| {
        let result = s.action(ui, &action);
        report(ui, result);
    });
}

fn normalize_crop(crop: &mut Value) {
    let x = n(crop, "x", 0.).clamp(0., 0.99);
    let y = n(crop, "y", 0.).clamp(0., 0.99);
    let width = n(crop, "width", 1.).clamp(0.01, 1. - x);
    let height = n(crop, "height", 1.).clamp(0.01, 1. - y);
    *crop = json!({"x":x,"y":y,"width":width,"height":height});
}

#[cfg(test)]
mod preview_refresh_tests {
    use super::when_idle;
    use std::cell::{Cell, RefCell};

    #[test]
    fn modal_borrow_skips_refresh_then_retries_without_losing_pending_size() {
        let state = RefCell::new(());
        let refreshed = Cell::new(false);
        let modal_action = state.borrow_mut();
        when_idle(&state, |_| refreshed.set(true));
        assert!(!refreshed.get());
        drop(modal_action);
        when_idle(&state, |_| refreshed.set(true));
        assert!(refreshed.get());
    }
}
