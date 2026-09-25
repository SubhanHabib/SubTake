//! The recorder: launcher and options windows, capture sources, hotkeys and
//! the tray item.

use super::*;

// A cold accessory launch may precede creation of the AppKit host.
// Retry native panel configuration briefly instead of showing a generic window
// first or emitting a spurious handle error.
pub(super) fn position_launcher_when_ready(attempt: u8) {
    Timer::single_shot(Duration::from_millis(20), move || {
        with_app(|app, _| {
            let Some(launcher) = &app.launcher else {
                return;
            };
            if platform::position_launcher(launcher.window()).is_err() {
                if attempt < 9 {
                    position_launcher_when_ready(attempt + 1);
                }
            } else {
                watch_bar_hover(launcher);
            }
        });
    });
}

impl App {
    #[cfg(target_os = "macos")]
    pub(super) fn ensure_tray_visible(&self) -> Result<()> {
        // macOS uses the retained AppKit status item installed after the event
        // loop starts. Keeping the GPUI tray hidden avoids a second menu-bar
        // item while preserving the cross-platform tray object.
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub(super) fn ensure_tray_visible(&self) -> Result<()> {
        if let Some(tray) = &self.tray {
            tray.show()?;
        }
        Ok(())
    }

    pub(super) fn apply_launcher_option(&mut self, ui: &EditorWindow, key: &str, value: &str) {
        if ui.get_busy() || ui.get_recording() {
            return;
        }
        match key {
            "source" => {
                if let Ok(index) = value.parse::<i32>()
                    && index >= 0
                    && (index as usize) < self.sources.len()
                {
                    ui.set_source_index(index);
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
            key => {
                if self.preferences.set_recorder_setting(key, value) {
                    report(ui, self.preferences.save());
                }
            }
        }
    }

    pub(super) fn sync_launcher_options(&self, ui: &EditorWindow) {
        self.sync_mic_meter(ui);
        self.sync_camera_preview(ui);
        let (Some(launcher), Some(options)) = (&self.launcher, &self.launcher_options) else {
            return;
        };
        options.set_panel(launcher.get_panel());
        options.set_appearance(self.preferences.appearance.as_str().into());
        // Asked again on every sync, so coming back from System Settings
        // shows what was allowed there. Screen access that has just come on
        // lists the sources it kept from the Source card.
        let screen = platform::has_access(platform::Access::Screen);
        if screen && !options.get_screen_access() && !ui.get_busy() {
            post(|app, ui| report(ui, app.action(ui, "sources-passive")));
        }
        for surface in [&**launcher, &**options] {
            surface.set_screen_access(screen);
            surface.set_microphone_access(platform::has_access(platform::Access::Microphone));
            surface.set_camera_access(platform::has_access(platform::Access::Camera));
        }
        options.set_source_names(ui.get_source_names());
        options.set_capture_sources(ui.get_capture_sources());
        options.set_sources_loading(ui.get_sources_loading());
        options.set_status(ui.get_status());
        options.set_source_index(ui.get_source_index());
        options.set_camera_names(ui.get_camera_names());
        options.set_camera_index(ui.get_camera_index());
        options.set_camera_notice(ui.get_camera_notice());
        options.set_microphone_notice(ui.get_microphone_notice());
        options.set_microphone_names(ui.get_microphone_names());
        options.set_microphone_kinds(ui.get_microphone_kinds());
        options.set_microphone_index(ui.get_microphone_index());
        options.set_camera(ui.get_capture_camera());
        options.set_microphone(ui.get_capture_mic());
        options.set_system_audio(ui.get_capture_system());
        options.set_busy(ui.get_busy());
        options.set_has_project(self.history.is_some());
        options.set_recents(ModelRc::new(VecModel::from(self.recorder_recents())));
        options.set_project_count(self.library.len() as i32);
        options.set_record_shortcut(self.preferences.record_shortcut.clone());
        options.set_countdown(self.preferences.countdown_seconds as i32);
        options.set_recorder_settings(self.preferences.recorder_settings());
        options.set_directory(
            self.recording_directory()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
    }

    /// Meters the chosen microphone while the Audio card is open with it on,
    /// and stops once the card closes, the microphone goes off or a
    /// recording starts, which opens the microphone itself.
    pub(super) fn sync_mic_meter(&self, ui: &EditorWindow) {
        // Hiding the bar leaves its panel set, so a hidden bar counts as
        // closed.
        let open = self.launcher.as_ref().is_some_and(|launcher| {
            launcher.get_panel() == "audio" && launcher.window().is_visible()
        });
        let device =
            (open && ui.get_capture_mic() && !ui.get_busy() && !ui.get_recording()).then(|| {
                self.devices["microphones"]
                    .as_array()
                    .and_then(|list| list.get((ui.get_microphone_index() - 1) as usize))
                    .and_then(|microphone| microphone["id"].as_str())
                    .unwrap_or_default()
            });
        platform::meter_microphone(device, mic_level);
        if device.is_none()
            && let Some(options) = &self.launcher_options
        {
            options.set_mic_level(f32::NEG_INFINITY);
        }
    }

    /// Streams the chosen camera into the Camera card while it is open with
    /// the camera on, and stops once the card closes, the camera goes off or
    /// a recording starts. The first time, the card opening is what asks
    /// the system for the camera; its answer syncs the card again. A camera
    /// starting clears the picture to its spinner; one stopping leaves its
    /// last frame, for the card to fade out on.
    pub(super) fn sync_camera_preview(&self, ui: &EditorWindow) {
        let open = self.launcher.as_ref().is_some_and(|launcher| {
            launcher.get_panel() == "camera" && launcher.window().is_visible()
        });
        let cameras = self.devices["cameras"].as_array();
        let wanted = open
            && ui.get_capture_camera()
            && !ui.get_busy()
            && !ui.get_recording()
            && cameras.is_some_and(|list| !list.is_empty())
            && platform::has_access(platform::Access::Camera);
        let device = wanted.then(|| {
            cameras
                .and_then(|list| list.get((ui.get_camera_index() - 1) as usize))
                .and_then(|camera| camera["id"].as_str())
                .unwrap_or_default()
        });
        let preview = platform::preview_camera(device, camera_frame);
        if preview == platform::CameraPreview::Off && device.is_some() {
            platform::request_camera_access(camera_answer);
        }
        if preview != platform::CameraPreview::Streaming
            && device.is_some()
            && let Some(options) = &self.launcher_options
        {
            options.set_camera_preview(Default::default());
        }
    }

    /// Takes a fresh list of microphones and cameras. The one chosen is kept
    /// by its id, wherever it now sits in the list; one that has been taken
    /// away gives way to the system default, and for `DEVICE_NOTICE` its
    /// card's header says so.
    pub(super) fn take_devices(&mut self, ui: &EditorWindow, devices: Value) {
        let lists = [
            (
                "microphones",
                "defaultMicrophone",
                ui.get_microphone_index(),
            ),
            ("cameras", "defaultCamera", ui.get_camera_index()),
        ];
        for (key, default_key, index) in lists {
            let list = devices[key].as_array().cloned().unwrap_or_default();
            // Index 0 is the system default, which is no device of its own.
            let chosen = usize::try_from(index - 1)
                .ok()
                .and_then(|i| self.devices[key].as_array()?.get(i).cloned());
            let found = chosen
                .as_ref()
                .and_then(|chosen| list.iter().position(|device| device["id"] == chosen["id"]));
            let index = found.map_or(0, |i| i as i32 + 1);
            let notice = match (&chosen, found) {
                (Some(gone), None) => {
                    let default = list
                        .iter()
                        .find(|device| device["id"] == devices[default_key])
                        .and_then(|device| device["name"].as_str())
                        .unwrap_or("the system default");
                    Some(format!(
                        "{} disconnected · using {default}",
                        gone["name"].as_str().unwrap_or("Device")
                    ))
                }
                _ => None,
            };
            let mut names = vec![SharedString::from("System default")];
            names.extend(
                list.iter()
                    .map(|v| SharedString::from(v["name"].as_str().unwrap_or("Device"))),
            );
            let names = ModelRc::new(VecModel::from(names));
            if key == "microphones" {
                ui.set_microphone_names(names);
                ui.set_microphone_index(index);
                // Beside each name, how it connects; the system default
                // first, which is no device of its own.
                let mut kinds = vec![SharedString::default()];
                kinds.extend(
                    list.iter()
                        .map(|v| SharedString::from(v["transport"].as_str().unwrap_or(""))),
                );
                ui.set_microphone_kinds(ModelRc::new(VecModel::from(kinds)));
            } else {
                ui.set_camera_names(names);
                ui.set_camera_index(index);
            }
            if let Some(notice) = notice {
                let camera = key == "cameras";
                if camera {
                    ui.set_camera_notice(notice.clone());
                } else {
                    ui.set_microphone_notice(notice.clone());
                }
                Timer::single_shot(DEVICE_NOTICE, move || {
                    with_app(|_, ui| {
                        // A later notice keeps its own time.
                        if camera && ui.get_camera_notice() == notice {
                            ui.set_camera_notice(String::new());
                        } else if !camera && ui.get_microphone_notice() == notice {
                            ui.set_microphone_notice(String::new());
                        }
                    })
                });
            }
        }
        self.devices = devices;
    }

    pub(super) fn sync_launcher(&self, ui: &EditorWindow) {
        let Some(launcher) = &self.launcher else {
            return;
        };
        launcher.set_source_names(ui.get_source_names());
        launcher.set_capture_sources(ui.get_capture_sources());
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
        launcher.set_cancellable(
            ui.get_busy() && (ui.get_sources_loading() || self.capture_started.is_none()),
        );
        launcher.set_countdown(self.preferences.countdown_seconds as i32);
        launcher.set_recorder_settings(self.preferences.recorder_settings());
        launcher.set_counting(self.counting as i32);
        launcher.set_stopping(self.stopping);
        if let Some(overlay) = &self.countdown_overlay {
            overlay.set_appearance(self.preferences.appearance.as_str().into());
            overlay.set_counting(self.counting as i32);
            let shown = overlay.window().is_visible();
            // "Count on screen" off leaves the count to the bar's Record
            // button alone.
            let wanted =
                self.counting > 0 && self.preferences.recorder_setting("count-on-screen") == "true";
            if wanted && !shown {
                let _ = overlay.show();
            } else if !wanted && shown {
                let _ = overlay.hide();
            }
        }
        launcher.set_directory(
            self.recording_directory()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
        launcher.set_status(ui.get_status());
        let seconds = self
            .capture_started
            .map(|start| {
                self.pause_started
                    .unwrap_or_else(std::time::Instant::now)
                    .duration_since(start)
                    .saturating_sub(self.paused_total)
                    .as_secs()
            })
            .unwrap_or(0);
        launcher.set_elapsed(format!("{:02}:{:02}", seconds / 60, seconds % 60));
        self.sync_launcher_options(ui);
    }

    pub(super) fn set_launcher_options_panel(
        &mut self,
        ui: &EditorWindow,
        panel: &str,
    ) -> Result<()> {
        let Some(launcher) = &self.launcher else {
            return Ok(());
        };
        launcher.set_panel(panel.into());
        self.sync_launcher(ui);
        let Some(options) = &self.launcher_options else {
            return Ok(());
        };
        // A closing card fades out and hides its own window once it has
        // gone (`RootView::card_fade`).
        if panel.is_empty() {
            return Ok(());
        }
        options.show()?;
        options.window().set_blur(false);
        options.window().set_transparent(true);
        // Existing windows can move immediately. First-show placement belongs
        // to the runtime's post-creation callback, not a timing-dependent retry.
        if options.window().native_view().is_some() {
            platform::position_launcher_options(options.window(), launcher.window())?;
        }
        // GPUI draws a window without focus at half speed or less, so the
        // card takes focus from the bar while it is open and redraws at the
        // display's rate; hiding hands it back. A new window takes focus as
        // it opens.
        options.window().make_key();
        Ok(())
    }

    pub(super) fn recording_directory(&self) -> Result<PathBuf> {
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

    pub(super) fn show_launcher(&mut self, ui: &EditorWindow) -> Result<()> {
        self.ensure_tray_visible()?;
        let first_show = self.launcher.is_none();
        if self.launcher.is_none() {
            let launcher = RecordingLauncher::new()?;
            let options = RecordingOptions::new()?;
            launcher.on_action(|action| {
                post(move |app, ui| report(ui, app.action(ui, &action)));
            });
            launcher.on_panel_change(|panel| {
                let panel = panel.to_string();
                post(move |app, ui| report(ui, app.set_launcher_options_panel(ui, &panel)));
            });
            options.on_action(|action| {
                post(move |app, ui| report(ui, app.action(ui, &action)));
            });
            options.on_option(|key, value| {
                let key = key.to_string();
                let value = value.to_string();
                post(move |app, ui| app.apply_launcher_option(ui, &key, &value));
            });
            options.on_panel_change(|_| {
                post(move |app, ui| report(ui, app.set_launcher_options_panel(ui, "")));
            });
            launcher
                .window()
                .on_close_requested(|| ui_runtime::CloseRequestResponse::HideWindow);
            options
                .window()
                .on_close_requested(|| ui_runtime::CloseRequestResponse::HideWindow);
            let overlay = RecordingCountdown::new()?;
            overlay
                .window()
                .on_close_requested(|| ui_runtime::CloseRequestResponse::HideWindow);
            self.launcher = Some(launcher);
            self.launcher_options = Some(options);
            self.countdown_overlay = Some(overlay);
        }
        self.sync_launcher(ui);
        if !ui.window().is_visible() {
            platform::set_editor_active(false);
        }
        let launcher = self.launcher.as_ref().unwrap();
        launcher.show()?;
        platform::activate_launcher();
        // The recorder uses masked card-level frosting. Window-wide blur
        // would blur the whole transparent envelope when a menu resizes it.
        launcher.window().set_blur(false);
        launcher.window().set_transparent(true);
        launcher.window().focus_window();
        // A launch-time GPUI window has no native handle until the event loop starts.
        // Positioning must not prevent source discovery or opening the recorder.
        // Configure immediately when possible; the bounded retry handles a
        // cold launch before GPUI has exposed the backing NSView.
        let _ = platform::configure_recording_hud(launcher.window(), true);
        watch_bar_hover(launcher);
        if first_show {
            position_launcher_when_ready(0);
        }
        if self.sources.is_empty() && !ui.get_busy() && !ui.get_recording() {
            self.action(ui, "sources-passive")?;
        }
        Ok(())
    }

    pub(super) fn show_editor(&mut self, ui: &EditorWindow) -> Result<()> {
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
        self.sync_mic_meter(ui);
        self.sync_camera_preview(ui);
        platform::set_editor_active(true);
        if !self.editor_shown {
            ui.window()
                .set_size(ui_runtime::LogicalSize::new(1360., 880.));
            self.editor_shown = true;
        }
        ui.show()?;
        // Editor glass is installed by the editor's own render
        // (`platform::set_window_glass`). A second material here would stack
        // under it and double-darken the chrome.
        ui.window().set_minimized(false);
        ui.window().focus_window();
        Ok(())
    }

    pub(super) fn finish_sources(
        &mut self,
        ui: &EditorWindow,
        result: Result<Vec<Value>>,
        cancelled: bool,
    ) {
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
                    self.set_sources(ui, sources, selected.as_ref());
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
        ui.set_recording_hint(message.clone());
        ui.set_status(message);
    }

    /// Lists `sources` on the Source card, the saved area after them, and
    /// keeps `selected` chosen if it is still there, else the first.
    pub(super) fn set_sources(
        &mut self,
        ui: &EditorWindow,
        mut sources: Vec<Value>,
        selected: Option<&Value>,
    ) {
        sources.retain(|source| source["kind"] != "area");
        if let Some(area) = area_source(&sources, self.preferences.recorder_setting("area")) {
            sources.push(area);
        }
        let index = sources
            .iter()
            .position(|source| {
                selected.is_some_and(|old| {
                    old["nativeId"] == source["nativeId"] && old["kind"] == source["kind"]
                })
            })
            .unwrap_or(0);
        ui.set_source_names(ModelRc::new(VecModel::from(
            sources
                .iter()
                .map(|source| SharedString::from(source["name"].as_str().unwrap_or("Source")))
                .collect::<Vec<_>>(),
        )));
        ui.set_capture_sources(ModelRc::new(VecModel::from(
            sources.iter().map(capture_source).collect::<Vec<_>>(),
        )));
        ui.set_source_index(index as i32);
        self.sources = sources;
    }

    /// The area overlay closing: an area drawn is kept and chosen, and
    /// either way the bar comes back with the Source card open, as it was
    /// when Draw area on screen was pressed.
    pub(super) fn finish_area(&mut self, ui: &EditorWindow, setting: Option<String>) {
        self.drawing_area = false;
        if let Some(setting) = setting {
            if self.preferences.set_recorder_setting("area", &setting) {
                report(ui, self.preferences.save());
            }
            self.set_sources(ui, self.sources.clone(), None);
            if let Some(index) = self
                .sources
                .iter()
                .position(|source| source["kind"] == "area")
            {
                ui.set_source_index(index as i32);
            }
        }
        report(ui, self.show_launcher(ui));
        report(ui, self.set_launcher_options_panel(ui, "sources"));
    }

    pub(super) fn register_hotkeys(&mut self) -> Result<()> {
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
        self.escape_hotkey = None;
        Ok(())
    }

    /// Take Esc from every app while the countdown runs, and give it back
    /// when the count ends. It is only a convenience — Cancel on the bar
    /// still works — so a refused registration is not an error.
    pub(super) fn hold_escape(&mut self, hold: bool) {
        use global_hotkey::hotkey::{Code, HotKey};
        let Some(manager) = &self.hotkeys else {
            return;
        };
        if hold && self.escape_hotkey.is_none() {
            let key = HotKey::new(None, Code::Escape);
            if manager.register(key).is_ok() {
                self.hotkey_ids.push((key.id(), "cancel".into()));
                self.escape_hotkey = Some(key);
            }
        } else if !hold && let Some(key) = self.escape_hotkey.take() {
            let _ = manager.unregister(key);
            self.hotkey_ids.retain(|(id, _)| *id != key.id());
        }
    }

    /// Put the on-screen count over the display this source records: the
    /// display itself, or the one a window's centre is on.
    pub(super) fn place_countdown(&self, source: &Value) {
        let number = |v: &Value, key: &str| v[key].as_f64().unwrap_or(0.);
        let display = if source["kind"] == "display" {
            source["nativeId"].as_u64()
        } else {
            let (x, y) = (
                number(source, "x") + number(source, "width") / 2.,
                number(source, "y") + number(source, "height") / 2.,
            );
            self.sources
                .iter()
                .filter(|s| s["kind"] == "display")
                .find(|d| {
                    (number(d, "x")..number(d, "x") + number(d, "width")).contains(&x)
                        && (number(d, "y")..number(d, "y") + number(d, "height")).contains(&y)
                })
                .and_then(|d| d["nativeId"].as_u64())
        };
        let display = display.unwrap_or(0) as u32;
        // AppKit reports the move synchronously; make it outside this borrow.
        ui_runtime::Timer::single_shot(Duration::ZERO, move || {
            platform::set_countdown_display(display)
        });
    }
}

/// How long a card's header says its device was taken away.
const DEVICE_NOTICE: Duration = Duration::from_secs(4);

/// A microphone or camera plugged in or taken away: the lists again.
pub(super) extern "C" fn devices_changed() {
    post(|app, ui| report(ui, app.action(ui, "devices")));
}

/// The microphone meter's level, from its capture thread, as it records:
/// after the Input level's gain. A level that arrives after the meter has
/// stopped is dropped.
extern "C" fn mic_level(level: f32) {
    post(move |app, ui| {
        if let Some(options) = &app.launcher_options
            && options.get_panel() == "audio"
            && ui.get_capture_mic()
        {
            let gain = input_gain(app.preferences.recorder_setting("input-level"));
            options.set_mic_level(level + 20. * gain.log10());
        }
    });
}

/// Follows the pointer over the bar, for "Hide bar while recording" to
/// bring the whole bar back while it is there.
fn watch_bar_hover(launcher: &RecordingLauncher) {
    platform::watch_launcher_hover(
        launcher.window(),
        bar_hover,
        subtake_theme::BAR_HIDE_LINGER_MS,
    );
}

extern "C" fn bar_hover(over: bool) {
    post(move |app, _| {
        if let Some(launcher) = &app.launcher {
            launcher.set_bar_hovered(over);
        }
    });
}

/// A frame is on its way to the card; until it lands, those after it are
/// dropped rather than queued behind it.
static CAMERA_FRAME_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// One frame of the camera's picture, from its capture thread. A frame that
/// arrives after the picture has stopped is dropped.
extern "C" fn camera_frame(rows: *const u8, width: i32, height: i32, stride: i32) {
    use std::sync::atomic::Ordering;
    let (Ok(width), Ok(height), Ok(stride)) = (
        u32::try_from(width),
        u32::try_from(height),
        usize::try_from(stride),
    ) else {
        return;
    };
    if rows.is_null() || CAMERA_FRAME_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let row = width as usize * 4;
    // SAFETY: the platform hands over `height` rows of `stride` bytes, valid
    // for this call.
    let all = unsafe { std::slice::from_raw_parts(rows, stride * height as usize) };
    let bytes: Vec<u8> = all
        .chunks(stride)
        .flat_map(|line| &line[..row])
        .copied()
        .collect();
    post(move |app, ui| {
        CAMERA_FRAME_PENDING.store(false, Ordering::Release);
        if let Some(options) = &app.launcher_options
            && options.get_panel() == "camera"
            && ui.get_capture_camera()
        {
            options.set_camera_preview(ui_runtime::Image::from_bgra8(bytes, width, height));
        }
    });
}

/// The system's answer on the camera: the card syncs again, streaming, or
/// showing that access is off.
extern "C" fn camera_answer(_granted: bool) {
    post(|app, ui| app.sync_launcher_options(ui));
}

/// The Microphone card's Test moving on: listening, playing, over.
pub(super) extern "C" fn mic_test(phase: i32) {
    post(move |app, _| {
        if let Some(options) = &app.launcher_options {
            options.set_mic_test(phase);
        }
    });
}

/// The area overlay closing, with the area drawn or, at a width of 0, none.
pub(super) extern "C" fn area_drawn(display: u32, left: f64, top: f64, width: f64, height: f64) {
    let setting =
        (width > 0.).then(|| format!("{display} {left:.0} {top:.0} {width:.0} {height:.0}"));
    post(move |app, ui| app.finish_area(ui, setting));
}

/// The Source card's Aspect as the width over the height the overlay holds
/// an area to; `None` for Free.
pub(super) fn area_aspect(setting: &str) -> Option<f32> {
    let (width, height) = setting.split_once(':')?;
    let (width, height) = (width.parse::<f32>().ok()?, height.parse::<f32>().ok()?);
    (width > 0. && height > 0.).then(|| width / height)
}

/// A palette as the area overlay takes it, for its chips and buttons to be
/// the cards' own.
pub(super) fn area_colours(theme: &subtake_theme::Theme) -> platform::AreaColours {
    [
        theme.accent,
        theme.accent_hover,
        theme.on_accent,
        theme.sunk,
        // Controls lift to `sunk2` under the pointer.
        theme.sunk2,
        theme.text,
        theme.muted,
        theme.frost,
        theme.line,
    ]
    .map(|colour| {
        let colour = colour.to_rgb();
        [colour.r, colour.g, colour.b, colour.a].map(f64::from)
    })
}

/// The Microphone card's Input level, a percentage, as the amplitude the
/// recording is scaled by: its square, so the slider's travel follows the
/// ear rather than the waveform, 100% leaving the microphone as it comes.
///
/// The handoff sets the OS input gain where the OS allows; that is the
/// device's own volume, for every app and after SubTake quits, so the
/// level is a gain at capture everywhere.
pub(super) fn input_gain(level: &str) -> f32 {
    (level.parse::<f32>().unwrap_or(100.).clamp(0., 100.) / 100.).powi(2)
}

/// The area saved from the Source card as a source to record: the display
/// it was drawn on, cropped to it. `setting` is the display's id, then the
/// area's left, top, width and height in points from the display's
/// top-left corner; `None` when nothing is saved, or its display is not
/// among `sources`.
///
/// Its `x`, `y`, `width` and `height` are the area's own on the desktop,
/// as a window's are, so the countdown and the cursor's telemetry follow
/// it; `areaX` and the rest are what the capture helper crops to.
pub(super) fn area_source(sources: &[Value], setting: &str) -> Option<Value> {
    let mut fields = setting.split_whitespace();
    let display = fields.next()?;
    let [left, top, width, height]: [f64; 4] = fields
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()?
        .try_into()
        .ok()?;
    let source = sources.iter().find(|source| {
        source["kind"] == "display"
            && match &source["nativeId"] {
                Value::String(id) => id == display,
                id => id.to_string() == display,
            }
    })?;
    let number = |key: &str| source[key].as_f64().unwrap_or_default();
    let (bounds_width, bounds_height) = (number("width"), number("height"));
    let left = left.clamp(0., bounds_width);
    let top = top.clamp(0., bounds_height);
    let width = width.min(bounds_width - left).round();
    let height = height.min(bounds_height - top).round();
    if width < 2. || height < 2. {
        return None;
    }
    let name = source["name"].as_str().unwrap_or("Display");
    let display_name = name.split_once(" · ").map_or(name, |(name, _)| name);
    Some(json!({
        "kind": "area",
        "nativeId": source["nativeId"],
        "name": format!("Area · {width:.0} × {height:.0}"),
        "display": display_name,
        "areaX": left,
        "areaY": top,
        "areaWidth": width,
        "areaHeight": height,
        "x": number("x") + left,
        "y": number("y") + top,
        "width": width,
        "height": height,
        "displayWidth": bounds_width,
        "displayHeight": bounds_height,
        "thumbnail": source["thumbnail"],
    }))
}

/// What the Source card draws for one platform source. The platform names a
/// display "<name> · <width> × <height>"; the card sets the resolution on its
/// own line, so it is split back out here.
fn capture_source(source: &Value) -> CaptureSource {
    let full = source["name"].as_str().unwrap_or("Source");
    let (name, detail) = match full.split_once(" · ") {
        Some((name, detail)) => (name, detail.to_owned()),
        None => (full, String::new()),
    };
    CaptureSource {
        kind: source["kind"].as_str().unwrap_or("window").into(),
        name: name.into(),
        detail,
        thumbnail: source_still(source),
        area: capture_area(source),
    }
}

/// Where an area source sits on its display, for the Source card's Area
/// tab to draw it over the display's still.
fn capture_area(source: &Value) -> Option<CaptureArea> {
    if source["kind"] != "area" {
        return None;
    }
    let number = |key: &str| source[key].as_f64().unwrap_or_default() as f32;
    let (width, height) = (number("displayWidth"), number("displayHeight"));
    if width <= 0. || height <= 0. {
        return None;
    }
    Some(CaptureArea {
        display: source["display"].as_str().unwrap_or("Display").into(),
        aspect: width / height,
        share: [
            number("areaX") / width,
            number("areaY") / height,
            number("areaWidth") / width,
            number("areaHeight") / height,
        ],
    })
}

/// The platform's still of a source, sent as base64 JPEG, or the placeholder
/// for a source it could not take one of in time.
fn source_still(source: &Value) -> ui_runtime::Image {
    use base64::Engine;
    let still = source["thumbnail"]
        .as_str()
        .and_then(|encoded| {
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .ok()
        })
        .and_then(|bytes| image::load_from_memory(&bytes).ok());
    let Some(still) = still else {
        return Default::default();
    };
    let still = still.into_rgba8();
    ui_runtime::Image::from_rgba8(ui_runtime::SharedPixelBuffer::clone_from_slice(
        still.as_raw(),
        still.width(),
        still.height(),
    ))
}

/// The Camera card's corner, shape and size as the project's `webcam`
/// settings for footage `width` × `height`.
///
/// Size is a share of the output's width (S, M and L at 16, 22 and 30%);
/// the model keeps the overlay's side as a share of the frame's shorter
/// side, so it is scaled across. Shape is `roundness`, as the editor's
/// Camera panel sets it (`src/ui/camera.rs`), and a corner is the
/// `positionX`/`positionY` a custom position would give.
pub(super) fn recorder_webcam(
    settings: &mut Value,
    preferences: &subtake_native::preferences::Preferences,
    (width, height): (u32, u32),
) {
    let (x, y) = match preferences.recorder_setting("camera-corner") {
        "top-left" => (0., 0.),
        "top-right" => (1., 0.),
        "bottom-left" => (0., 1.),
        _ => (1., 1.),
    };
    let roundness = match preferences.recorder_setting("camera-shape") {
        "square" => 0.,
        "rounded" => 25.,
        _ => 100.,
    };
    let share = match preferences.recorder_setting("camera-size") {
        "s" => 16.,
        "l" => 30.,
        _ => 22.,
    };
    let side = f64::from(width) / f64::from(width.min(height).max(1));
    let side = (share * side * 10.).round() / 10.;
    settings["positionX"] = json!(x);
    settings["positionY"] = json!(y);
    settings["roundness"] = json!(roundness);
    settings["width"] = json!(side);
    settings["height"] = json!(side);
}

#[cfg(test)]
mod tests;
