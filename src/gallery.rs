//! UI-only gallery: every SubTake surface, driven by fixture data instead of
//! a project, a media engine or the recorder. Launched with `--gallery` or
//! `SUBTAKE_GALLERY=1` (`python3 scripts/dev.py --gallery`; `=light` starts light) so the
//! presentation can be iterated on without opening a file or signing in.
//!
//! The same `EditorWindow` / `RecordingLauncher` / `RecordingOptions`
//! property surface and callbacks that `App` drives are used here, so the
//! views render exactly as they do in the product; only the data is fake.
//! Callbacks echo their input back into the properties so every control
//! looks live: toggles flip, dropdowns pick, sliders move, the scrubber
//! seeks, and ⌘⇧D (or the appearance dropdown) swaps light and dark on all
//! three windows at once.
use crate::{EditorWindow, Field, RecordingLauncher, RecordingOptions, Region, Wallpaper};
use anyhow::Result;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};
use subtake_native::platform;
use subtake_native::ui_runtime::{
    self, Color, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};

const DURATION: f32 = 148.;
const PREVIEW_W: u32 = 960;
const PREVIEW_H: u32 = 540;

/// Everything the echo callbacks mutate. One shared cell keeps the closures
/// small and lets the appearance toggle reach all three windows.
struct Gallery {
    editor: EditorWindow,
    launcher: RecordingLauncher,
    options: RecordingOptions,
    playhead: f32,
    playing: bool,
    appearance: &'static str,
    /// Field values keyed by field key, so panels rebuild with the edits kept.
    values: Vec<(String, String)>,
    regions: Vec<Region>,
    playback: Timer,
    /// Handle for timers, which run from the pump loop and never re-enter a callback.
    me: Weak<RefCell<Gallery>>,
}

pub fn run() -> Result<()> {
    let editor = EditorWindow::new()?;
    let launcher = RecordingLauncher::new()?;
    let options = RecordingOptions::new()?;
    let gallery = Rc::new(RefCell::new(Gallery {
        editor: editor.clone(),
        launcher: launcher.clone(),
        options: options.clone(),
        playhead: 37.5,
        playing: false,
        // `SUBTAKE_GALLERY=light` starts in light mode; anything else is dark.
        appearance: match std::env::var("SUBTAKE_GALLERY").as_deref() {
            Ok("light") => "light",
            _ => "dark",
        },
        values: vec![],
        regions: fixture_regions(),
        playback: Timer::default(),
        me: Weak::new(),
    }));
    gallery.borrow_mut().me = Rc::downgrade(&gallery);

    seed_editor(&editor);
    seed_recorder(&launcher, &options);
    {
        let g = gallery.borrow();
        g.apply_appearance();
        g.push_timeline();
        g.push_fields();
        g.push_time();
    }

    // -- Editor callbacks -------------------------------------------------
    let g = gallery.clone();
    editor.on_seek(move |time| {
        let mut g = g.borrow_mut();
        g.playhead = time.clamp(0., DURATION);
        g.push_time();
    });
    let g = gallery.clone();
    editor.on_panel_change(move |panel| {
        let g = g.borrow();
        g.editor.set_panel(panel);
        g.push_fields();
    });
    let g = gallery.clone();
    editor.on_field_change(move |key, value| {
        let mut g = g.borrow_mut();
        g.set_value(&key, &value);
        if key == "prefs.appearance" {
            g.appearance = match value.as_str() {
                "light" => "light",
                _ => "dark",
            };
            g.apply_appearance();
        }
        if key == "prefs.language" {
            g.editor.set_language(value.into());
        }
        g.push_fields();
    });
    let g = gallery.clone();
    editor.on_action(move |action| {
        let mut g = g.borrow_mut();
        g.action(&action);
    });
    let g = gallery.clone();
    editor.on_keyboard(move |key, command, shift, _alt| {
        let mut g = g.borrow_mut();
        match (key.as_str(), command, shift) {
            ("d", true, true) => g.action("toggle-appearance"),
            (" ", false, false) | ("space", false, false) => g.action("play-pause"),
            ("arrowleft", false, _) => g.nudge(-1.),
            ("arrowright", false, _) => g.nudge(1.),
            ("home", false, false) => g.nudge(-DURATION),
            ("end", false, false) => g.nudge(DURATION),
            _ => return false,
        }
        true
    });
    let g = gallery.clone();
    editor.on_select_region(move |kind, id, additive| {
        let mut g = g.borrow_mut();
        for r in &mut g.regions {
            let hit = r.kind == kind && r.id == id;
            r.selected = if additive { r.selected ^ hit } else { hit };
        }
        g.editor.set_selected_id(id.into());
        g.push_timeline();
    });
    let g = gallery.clone();
    editor.on_move_region(move |kind, id, delta, mode| {
        let mut g = g.borrow_mut();
        if let Some(r) = g.regions.iter_mut().find(|r| r.kind == kind && r.id == id) {
            // mode 0 moves the whole region; 1 and 2 drag its start or end.
            match mode {
                1 => r.start = (r.start + delta).clamp(0., r.end - 0.2),
                2 => r.end = (r.end + delta).clamp(r.start + 0.2, DURATION),
                _ => {
                    let len = r.end - r.start;
                    r.start = (r.start + delta).clamp(0., DURATION - len);
                    r.end = r.start + len;
                }
            }
        }
        g.push_timeline();
    });
    editor.on_canvas_edit(|_, _, _| {});
    editor.on_preview_click(|_, _| {});
    editor.on_translate(|key, _| key);

    // -- Recorder callbacks -----------------------------------------------
    let g = gallery.clone();
    launcher.on_action(move |action| g.borrow_mut().action(&action));
    let g = gallery.clone();
    launcher.on_panel_change(move |panel| {
        let g = g.borrow();
        g.launcher.set_panel(panel.clone());
        g.options.set_panel(panel);
        g.position_options();
    });
    let g = gallery.clone();
    options.on_action(move |action| g.borrow_mut().action(&action));
    let g = gallery.clone();
    options.on_panel_change(move |_| {
        let g = g.borrow();
        g.launcher.set_panel("".into());
        g.options.set_panel("".into());
        g.position_options();
    });
    let g = gallery.clone();
    options.on_option(move |key, value| g.borrow().option(&key, &value));
    launcher
        .window()
        .on_close_requested(|| ui_runtime::CloseRequestResponse::HideWindow);
    options
        .window()
        .on_close_requested(|| ui_runtime::CloseRequestResponse::HideWindow);

    // -- Show ---------------------------------------------------------------
    editor
        .window()
        .set_size(ui_runtime::LogicalSize::new(1360., 880.));
    editor.show()?;
    show_recorder(&launcher, &options);

    ui_runtime::run_event_loop_until_quit()
}

impl Gallery {
    fn action(&mut self, action: &str) {
        match action {
            "toggle-appearance" => {
                self.appearance = if self.appearance == "dark" {
                    "light"
                } else {
                    "dark"
                };
                self.set_value("prefs.appearance", self.appearance);
                self.apply_appearance();
                self.push_fields();
            }
            "play" | "pause" | "play-pause" | "toggle-play" => self.toggle_play(),
            "record" | "start-recording" => {
                self.launcher.set_recording(true);
                self.launcher.set_paused(false);
                self.launcher.set_status("Recording".into());
                let launcher = self.launcher.clone();
                let started = std::time::Instant::now();
                self.playback
                    .start(TimerMode::Repeated, Duration::from_millis(250), move || {
                        let s = started.elapsed().as_secs();
                        launcher.set_elapsed(format!("{:02}:{:02}", s / 60, s % 60).into());
                    });
            }
            "stop" | "stop-recording" | "finish" => {
                self.playback.stop();
                self.launcher.set_recording(false);
                self.launcher.set_paused(false);
                self.launcher.set_elapsed("".into());
                self.launcher.set_status("Saved to Movies/SubTake".into());
            }
            "pause-recording" | "resume-recording" => {
                let paused = !self.launcher.get_paused();
                self.launcher.set_paused(paused);
            }
            "show-launcher" | "record-new" | "new-recording" => {
                show_recorder(&self.launcher, &self.options);
            }
            "show-editor" | "open-editor" => {
                let _ = self.launcher.hide();
                let _ = self.options.hide();
                self.editor.show().ok();
                self.editor.window().focus_window();
            }
            "undo" => self.editor.set_can_redo(true),
            "redo" => self.editor.set_can_redo(false),
            "zoom-in" => self.zoom(1.25),
            "zoom-out" => self.zoom(0.8),
            "zoom-fit" => {
                self.editor.set_timeline_zoom(1.);
                self.editor.set_timeline_offset(0.);
            }
            "export" => {
                // A fake progress run so the export affordances can be seen.
                let editor = self.editor.clone();
                editor.set_busy(true);
                editor.set_status("Exporting…".into());
                let started = std::time::Instant::now();
                Timer::single_shot(Duration::from_millis(16), move || {
                    tick_export(editor, started)
                });
            }
            "toggle-camera" => self.option("camera", &(!self.launcher.get_camera()).to_string()),
            "toggle-microphone" => {
                self.option("microphone", &(!self.launcher.get_microphone()).to_string())
            }
            "toggle-system-audio" => self.option(
                "system_audio",
                &(!self.launcher.get_system_audio()).to_string(),
            ),
            other => {
                self.editor
                    .set_status(format!("Gallery: “{other}” has no effect here").into());
            }
        }
    }

    /// Recorder options echo straight back into both recorder windows, the
    /// way `App::apply_launcher_option` mirrors them through `sync_launcher`.
    fn option(&self, key: &str, value: &str) {
        let on = value == "true";
        let index = value.parse::<i32>().unwrap_or(0);
        match key {
            "camera" => {
                self.launcher.set_camera(on);
                self.options.set_camera(on);
            }
            "microphone" => {
                self.launcher.set_microphone(on);
                self.options.set_microphone(on);
            }
            "system_audio" | "system" => {
                self.launcher.set_system_audio(on);
                self.options.set_system_audio(on);
            }
            "source" | "source_index" => {
                self.launcher.set_source_index(index);
                self.options.set_source_index(index);
            }
            "camera_index" => {
                self.launcher.set_camera_index(index);
                self.options.set_camera_index(index);
            }
            "microphone_index" => {
                self.launcher.set_microphone_index(index);
                self.options.set_microphone_index(index);
            }
            "countdown" => {
                self.launcher.set_countdown(index);
                self.options.set_countdown(index);
            }
            "directory" => {
                self.launcher.set_directory(value.into());
                self.options.set_directory(value.into());
            }
            _ => self
                .launcher
                .set_status(format!("Gallery: option {key} = {value}").into()),
        }
    }

    fn set_value(&mut self, key: &str, value: &str) {
        if let Some(slot) = self.values.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value.to_owned();
        } else {
            self.values.push((key.to_owned(), value.to_owned()));
        }
    }

    fn value<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or(default)
    }

    fn apply_appearance(&self) {
        for window in [
            &self.editor as &dyn Appearance,
            &self.launcher,
            &self.options,
        ] {
            window.theme(self.appearance);
        }
    }

    fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.editor.set_playing(self.playing);
        if !self.playing {
            self.playback.stop();
            return;
        }
        // 33 ms is the product's playback tick. The timer fires from the pump
        // loop, never from inside a callback, so the borrow here is safe.
        let me = self.me.clone();
        self.playback
            .start(TimerMode::Repeated, Duration::from_millis(33), move || {
                let Some(g) = me.upgrade() else { return };
                let mut g = g.borrow_mut();
                g.playhead = if g.playhead + 0.033 >= DURATION {
                    0.
                } else {
                    g.playhead + 0.033
                };
                g.push_time();
                follow_playhead(&g.editor, g.playhead);
            });
    }

    fn nudge(&mut self, seconds: f32) {
        self.playhead = (self.playhead + seconds).clamp(0., DURATION);
        self.push_time();
    }

    fn zoom(&self, factor: f32) {
        let zoom = (self.editor.get_timeline_zoom() * factor).clamp(1., 40.);
        self.editor.set_timeline_zoom(zoom);
        follow_playhead(&self.editor, self.editor.get_playhead());
    }

    fn push_time(&self) {
        self.editor.set_playhead(self.playhead);
        self.editor.set_time_label(time_label(self.playhead).into());
    }

    fn push_timeline(&self) {
        self.editor
            .set_regions(ModelRc::new(VecModel::from(self.regions.clone())));
    }

    fn push_fields(&self) {
        let panel = self.editor.get_panel();
        let fields = fixture_fields(self, &panel);
        self.editor.set_fields(ModelRc::new(VecModel::from(fields)));
    }

    fn position_options(&self) {
        if self.launcher.get_panel().is_empty() {
            let _ = self.options.hide();
            return;
        }
        self.options.show().ok();
        self.options.window().set_blur(false);
        self.options.window().set_transparent(true);
        let _ = platform::position_launcher_options(self.options.window(), self.launcher.window());
    }
}

/// The three surfaces share one appearance setter but are distinct types.
trait Appearance {
    fn theme(&self, value: &str);
}
impl Appearance for EditorWindow {
    fn theme(&self, value: &str) {
        self.set_appearance(value.into());
    }
}
impl Appearance for RecordingLauncher {
    fn theme(&self, value: &str) {
        self.set_appearance(value.into());
    }
}
impl Appearance for RecordingOptions {
    fn theme(&self, value: &str) {
        self.set_appearance(value.into());
    }
}

fn tick_export(editor: EditorWindow, started: std::time::Instant) {
    let progress = (started.elapsed().as_secs_f32() / 4.).min(1.);
    editor.set_progress(progress);
    if progress < 1. {
        Timer::single_shot(Duration::from_millis(33), move || {
            tick_export(editor, started)
        });
    } else {
        editor.set_busy(false);
        editor.set_progress(0.);
        editor.set_status("Exported gallery.mp4".into());
    }
}

fn follow_playhead(editor: &EditorWindow, t: f32) {
    let visible = editor.get_timeline_visible();
    let offset = editor.get_timeline_offset();
    if t < offset || t > offset + visible {
        editor.set_timeline_offset((t - visible * 0.1).clamp(0., (DURATION - visible).max(0.)));
    }
}

fn time_label(t: f32) -> String {
    let frames = ((t - t.floor()) * 30.) as u32;
    format!(
        "{:02}:{:02}.{:02} / {:02}:{:02}",
        (t as u32) / 60,
        (t as u32) % 60,
        frames,
        (DURATION as u32) / 60,
        (DURATION as u32) % 60
    )
}

fn show_recorder(launcher: &RecordingLauncher, options: &RecordingOptions) {
    if launcher.show().is_err() {
        return;
    }
    platform::activate_launcher();
    launcher.window().set_blur(false);
    launcher.window().set_transparent(true);
    launcher.window().focus_window();
    let _ = platform::configure_recording_hud(launcher.window(), true);
    let _ = platform::configure_recording_hud(options.window(), true);
    position_launcher(launcher.clone(), 0);
}

fn position_launcher(launcher: RecordingLauncher, attempt: u8) {
    Timer::single_shot(Duration::from_millis(20), move || {
        if platform::position_launcher(launcher.window()).is_err() && attempt < 9 {
            position_launcher(launcher, attempt + 1);
        }
    });
}

// ---------------------------------------------------------------------------
// Fixtures

fn seed_editor(ui: &EditorWindow) {
    ui.set_mac_titlebar(cfg!(target_os = "macos"));
    ui.set_document_title("Gallery · Onboarding walkthrough".into());
    ui.set_has_project(true);
    ui.set_has_video(true);
    ui.set_edit_visible(true);
    ui.set_dirty(true);
    ui.set_can_undo(true);
    ui.set_can_redo(false);
    ui.set_duration(DURATION);
    ui.set_timeline_zoom(1.);
    ui.set_timeline_offset(0.);
    ui.set_panel("Frame".into());
    ui.set_panel_index(0);
    ui.set_language("en".into());
    ui.set_status("Gallery mode — nothing here touches a project".into());
    ui.set_preview_aspect(16. / 9.);
    ui.set_preview_pixel_width(PREVIEW_W as f32);
    ui.set_preview_zoom(1.);
    ui.set_snap(true);
    ui.set_auto_apply_zooms(true);
    ui.set_connect_zooms(false);
    ui.set_motion_choice("Smooth".into());
    ui.set_look_choice("Studio".into());
    ui.set_background_value("wallpaper-1".into());
    ui.set_aspect_index(0);
    ui.set_preview(gradient(
        PREVIEW_W,
        PREVIEW_H,
        [0x1f, 0x3b, 0x73],
        [0xd9, 0x6c, 0x9d],
        Style::Preview,
    ));
    ui.set_thumbnails(gradient(
        1600,
        48,
        [0x1f, 0x3b, 0x73],
        [0xd9, 0x6c, 0x9d],
        Style::Strip,
    ));
    ui.set_frosted_thumbnails(gradient(
        1600,
        48,
        [0x4a, 0x5c, 0x86],
        [0xc7, 0x9a, 0xb4],
        Style::Strip,
    ));
    ui.set_waveform(gradient(
        1600,
        40,
        [0xa4, 0x68, 0xe9],
        [0xa4, 0x68, 0xe9],
        Style::Waveform,
    ));
    ui.set_track_labels(ModelRc::new(VecModel::from(vec![
        "Zoom".to_string(),
        "Clip".into(),
        "Annotation".into(),
        "Audio".into(),
        "Caption".into(),
    ])));
    ui.set_audio_row(3);
    ui.set_wallpapers(ModelRc::new(VecModel::from(
        [
            ("Dusk", [0x1f, 0x3b, 0x73], [0xd9, 0x6c, 0x9d]),
            ("Meadow", [0x1a, 0x6b, 0x4a], [0xd8, 0xe3, 0x6b]),
            ("Ember", [0x6b, 0x1a, 0x1a], [0xf5, 0xbb, 0x6b]),
            ("Slate", [0x2b, 0x2f, 0x3a], [0x9a, 0xa4, 0xb8]),
            ("Lagoon", [0x0e, 0x4d, 0x64], [0x7f, 0xd3, 0xe0]),
            ("Blossom", [0x8a, 0x2f, 0x6a], [0xff, 0xc6, 0xd9]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (title, a, b))| Wallpaper {
            key: format!("wallpaper-{}", i + 1),
            title: title.into(),
            source: gradient(160, 90, a, b, Style::Preview),
        })
        .collect::<Vec<_>>(),
    )));
    ui.set_source_names(ModelRc::new(VecModel::from(source_names())));
    ui.set_camera_names(ModelRc::new(VecModel::from(camera_names())));
    ui.set_microphone_names(ModelRc::new(VecModel::from(microphone_names())));
    ui.set_source_index(1);
    ui.set_camera_index(0);
    ui.set_microphone_index(0);
    ui.set_capture_camera(true);
    ui.set_capture_mic(true);
    ui.set_capture_system(false);
}

fn seed_recorder(launcher: &RecordingLauncher, options: &RecordingOptions) {
    // Both recorder windows mirror the same capture state, as `sync_launcher`
    // keeps them in the product.
    for (set_names, set_index, set_flag) in [
        (
            Box::new(|s, c, m| {
                launcher.set_source_names(s);
                launcher.set_camera_names(c);
                launcher.set_microphone_names(m);
            }) as Box<dyn Fn(ModelRc<String>, ModelRc<String>, ModelRc<String>)>,
            Box::new(|i, j, k| {
                launcher.set_source_index(i);
                launcher.set_camera_index(j);
                launcher.set_microphone_index(k);
            }) as Box<dyn Fn(i32, i32, i32)>,
            Box::new(|c, m, s| {
                launcher.set_camera(c);
                launcher.set_microphone(m);
                launcher.set_system_audio(s);
            }) as Box<dyn Fn(bool, bool, bool)>,
        ),
        (
            Box::new(|s, c, m| {
                options.set_source_names(s);
                options.set_camera_names(c);
                options.set_microphone_names(m);
            }),
            Box::new(|i, j, k| {
                options.set_source_index(i);
                options.set_camera_index(j);
                options.set_microphone_index(k);
            }),
            Box::new(|c, m, s| {
                options.set_camera(c);
                options.set_microphone(m);
                options.set_system_audio(s);
            }),
        ),
    ] {
        set_names(
            ModelRc::new(VecModel::from(source_names())),
            ModelRc::new(VecModel::from(camera_names())),
            ModelRc::new(VecModel::from(microphone_names())),
        );
        set_index(1, 0, 0);
        set_flag(true, true, false);
    }
    for w in [launcher as &dyn Recorder, options] {
        w.seed();
    }
    launcher.set_status("Ready to record".into());
    launcher.set_recording_hint("⌘⇧R starts a recording".into());
    launcher.set_recording(false);
    launcher.set_paused(false);
    launcher.set_cancellable(false);
    launcher.set_sources_loading(false);
    launcher.set_elapsed("".into());
}

/// The shared subset of recorder state both windows carry.
trait Recorder {
    fn seed(&self);
}
impl Recorder for RecordingLauncher {
    fn seed(&self) {
        self.set_has_project(true);
        self.set_countdown(3);
        self.set_directory("~/Movies/SubTake".into());
        self.set_busy(false);
        self.set_panel("".into());
    }
}
impl Recorder for RecordingOptions {
    fn seed(&self) {
        self.set_has_project(true);
        self.set_countdown(3);
        self.set_directory("~/Movies/SubTake".into());
        self.set_busy(false);
        self.set_panel("".into());
    }
}

fn source_names() -> Vec<String> {
    [
        "Built-in Retina Display",
        "Studio Display",
        "Safari — SubTake docs",
        "Xcode",
        "Figma",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
fn camera_names() -> Vec<String> {
    [
        "FaceTime HD Camera",
        "Studio Display Camera",
        "iPhone Continuity Camera",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
fn microphone_names() -> Vec<String> {
    [
        "MacBook Pro Microphone",
        "Studio Display Microphone",
        "Shure MV7",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn fixture_regions() -> Vec<Region> {
    fn region(
        kind: &str,
        id: &str,
        label: &str,
        start: f32,
        end: f32,
        row: i32,
        tint: u32,
    ) -> Region {
        Region {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            start,
            end,
            row,
            tint: Color::from_rgb_u8((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
            selected: false,
        }
    }
    vec![
        region("zoomRegions", "z1", "Zoom", 4., 12.5, 0, 0x397afa),
        region("zoomRegions", "z2", "Zoom", 41., 52., 0, 0x397afa),
        region("zoomRegions", "z3", "Zoom", 98., 111., 0, 0x397afa),
        region("nativeMarkers", "m1", "◆", 60., 60.4, 0, 0xf5bb6b),
        region("clipRegions", "c1", "Intro", 0., 33., 1, 0x357c65),
        region("speedRegions", "s1", "2× Speed", 33., 41., 1, 0xdc922d),
        region("trimRegions", "t1", "Trim", 71., 76., 1, 0xee5261),
        region(
            "clipRegions",
            "c2",
            "Walkthrough",
            76.,
            DURATION,
            1,
            0x357c65,
        ),
        region(
            "annotationRegions",
            "a1",
            "Click Save",
            14.,
            22.,
            2,
            0xcbb44f,
        ),
        region("annotationRegions", "a2", "Arrow", 86., 94., 2, 0xcbb44f),
        region(
            "audioRegions",
            "au1",
            "Voice-over",
            0.,
            DURATION,
            3,
            0xa468e9,
        ),
        region(
            "autoCaptions",
            "cap1",
            "Welcome to SubTake",
            1.,
            5.5,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap2",
            "Let's set up your workspace",
            6.,
            11.,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap3",
            "Choose a wallpaper",
            12.,
            17.,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap4",
            "And export in one click",
            120.,
            126.,
            4,
            0x6396dc,
        ),
    ]
}

/// Field kinds as `gpui_views` renders them: 0 text/number, 1 slider,
/// 2 toggle, 3 action row, 4 dropdown, 5 section label.
fn fixture_fields(g: &Gallery, panel: &str) -> Vec<Field> {
    let v = |key: &str, default: &str| g.value(key, default).to_owned();
    let section = |label: &str| Field {
        key: format!("section.{label}"),
        label: label.into(),
        kind: 5,
        ..Default::default()
    };
    let slider = |key: &str, label: &str, default: &str, min: f32, max: f32| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, default),
        kind: 1,
        minimum: min,
        maximum: max,
        ..Default::default()
    };
    let text = |key: &str, label: &str, default: &str| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, default),
        kind: 0,
        ..Default::default()
    };
    let toggle = |key: &str, label: &str, default: bool| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, &default.to_string()),
        kind: 2,
        ..Default::default()
    };
    let action = |key: &str, label: &str| Field {
        key: key.into(),
        label: label.into(),
        kind: 3,
        ..Default::default()
    };
    let dropdown = |key: &str, label: &str, choices: &[(&str, &str)], default: &str| {
        let value = v(key, default);
        let choice = choices
            .iter()
            .position(|(val, _)| *val == value)
            .unwrap_or(0) as i32;
        Field {
            key: key.into(),
            label: label.into(),
            value,
            kind: 4,
            choices: ModelRc::new(VecModel::from(
                choices
                    .iter()
                    .map(|(_, c)| c.to_string())
                    .collect::<Vec<_>>(),
            )),
            values: ModelRc::new(VecModel::from(
                choices
                    .iter()
                    .map(|(val, _)| val.to_string())
                    .collect::<Vec<_>>(),
            )),
            choice,
            ..Default::default()
        }
    };

    match panel {
        "Frame" => vec![
            section("Layout"),
            slider("padding.all", "Padding", "64", 0., 320.),
            toggle("padding.linked", "Link all sides", true),
            slider("borderRadius", "Radius", "18", 0., 64.),
            slider("shadow", "Shadow", "0.4", 0., 1.),
            section("Cursor"),
            toggle("showCursor", "Show cursor", true),
            dropdown(
                "cursorStyle",
                "Style",
                &[("arrow", "Arrow"), ("hand", "Hand"), ("dot", "Dot")],
                "arrow",
            ),
            dropdown(
                "cursorClickEffect",
                "Click effect",
                &[("none", "None"), ("ripple", "Ripple"), ("pulse", "Pulse")],
                "ripple",
            ),
            slider("cursorSize", "Size", "1.4", 0.5, 3.),
            toggle("cursorSmoothing", "Smooth movement", true),
            section("Zoom"),
            slider("zoomSmoothness", "Smoothness", "60", 0., 100.),
            toggle("connectZooms", "Connect zooms", false),
            section("Camera"),
            dropdown(
                "webcam.positionPreset",
                "Position",
                &[
                    ("bottom-right", "Bottom right"),
                    ("bottom-left", "Bottom left"),
                    ("top-right", "Top right"),
                    ("custom", "Custom"),
                ],
                "bottom-right",
            ),
            slider("webcam.width", "Width", "24", 8., 60.),
            dropdown(
                "webcam.shape",
                "Shape",
                &[
                    ("circle", "Circle"),
                    ("rounded", "Rounded"),
                    ("square", "Square"),
                ],
                "circle",
            ),
        ],
        "Wallpapers" => vec![
            section("Background"),
            slider("backgroundBlur", "Blur", "12", 0., 80.),
            text("background.color", "Solid colour", "#1F3B73"),
            action("choose-background", "Choose image…"),
        ],
        "Presets" => vec![
            section("Motion"),
            dropdown(
                "preset.motion",
                "Motion",
                &[
                    ("gentle", "Gentle"),
                    ("smooth", "Smooth"),
                    ("snappy", "Snappy"),
                ],
                "smooth",
            ),
            section("Look"),
            dropdown(
                "preset.look",
                "Look",
                &[
                    ("studio", "Studio"),
                    ("minimal", "Minimal"),
                    ("vivid", "Vivid"),
                ],
                "studio",
            ),
            action("preset.save", "Save as preset…"),
            action("preset.reset", "Reset to defaults"),
        ],
        "Selection" => {
            let selected = g.regions.iter().find(|r| r.selected);
            match selected {
                Some(r) => vec![
                    section(&format!(
                        "{} · {}",
                        r.label,
                        r.kind.trim_end_matches("Regions")
                    )),
                    text("region.start", "Start", &format!("{:.2}", r.start)),
                    text("region.end", "End", &format!("{:.2}", r.end)),
                    slider("region.zoom", "Zoom level", "2.0", 1., 4.),
                    dropdown(
                        "region.easing",
                        "Easing",
                        &[("ease", "Ease"), ("linear", "Linear"), ("spring", "Spring")],
                        "ease",
                    ),
                    action("region.delete", "Delete region"),
                ],
                None => vec![
                    section("Nothing selected"),
                    action("select-hint", "Click a region in the timeline"),
                ],
            }
        }
        "Recording" => vec![
            section("Sources"),
            dropdown(
                "recording.source",
                "Screen",
                &[
                    ("0", "Built-in Retina Display"),
                    ("1", "Studio Display"),
                    ("2", "Safari — SubTake docs"),
                ],
                "1",
            ),
            toggle("recording.camera", "Camera", true),
            dropdown(
                "recording.camera_device",
                "Camera device",
                &[("0", "FaceTime HD Camera"), ("1", "Studio Display Camera")],
                "0",
            ),
            toggle("recording.microphone", "Microphone", true),
            toggle("recording.system", "System audio", false),
            section("Behaviour"),
            slider("prefs.countdown_seconds", "Countdown", "3", 0., 10.),
            text("recording.directory", "Save to", "~/Movies/SubTake"),
            action("show-launcher", "Open recorder"),
        ],
        "Preferences" => vec![
            section("Appearance"),
            dropdown(
                "prefs.appearance",
                "Appearance",
                &[("dark", "Dark"), ("light", "Light")],
                g.appearance,
            ),
            dropdown(
                "prefs.language",
                "Language",
                &[
                    ("en", "English"),
                    ("de", "Deutsch"),
                    ("fr", "Français"),
                    ("ja", "日本語"),
                ],
                "en",
            ),
            section("Editing"),
            toggle("prefs.auto_apply_zooms", "Auto-apply zooms", true),
            toggle("prefs.snap", "Snap to regions", true),
            slider("prefs.countdown_seconds", "Countdown", "3", 0., 10.),
            section("Storage"),
            text(
                "prefs.library_directory",
                "Library folder",
                "~/Movies/SubTake",
            ),
            action("prefs.reveal", "Reveal in Finder"),
        ],
        "Shortcuts" => vec![
            section("Recording"),
            text("prefs.record_shortcut", "Start / stop", "⌘⇧R"),
            text("prefs.pause_shortcut", "Pause", "⌘⇧P"),
            section("Editor"),
            text("shortcut.play", "Play / pause", "Space"),
            text("shortcut.split", "Split clip", "S"),
            text("shortcut.zoom-in", "Zoom in", "⌘="),
            text("shortcut.zoom-out", "Zoom out", "⌘-"),
            text("shortcut.appearance", "Toggle light / dark", "⌘⇧D"),
        ],
        "Recent" => vec![
            section("Recent projects"),
            action("recent.0", "Onboarding walkthrough — today"),
            action("recent.1", "Release notes 2.4 — yesterday"),
            action("recent.2", "Bug repro for Adclear — 3 days ago"),
            action("recent.3", "Keyboard shortcuts tour — last week"),
            section("Library"),
            text("library.query", "Search", ""),
        ],
        _ => vec![
            section(panel),
            action("noop", "This panel has no gallery fixtures yet"),
        ],
    }
}

// ---------------------------------------------------------------------------
// Generated imagery

#[derive(Clone, Copy)]
enum Style {
    /// Diagonal two-tone gradient with a lighter "window" card in the middle.
    Preview,
    /// Horizontal gradient chopped into frame-like cells.
    Strip,
    /// A symmetrical pseudo-random waveform on transparent.
    Waveform,
}

fn gradient(w: u32, h: u32, a: [u8; 3], b: [u8; 3], style: Style) -> Image {
    let mut bytes = vec![0u8; (w * h * 4) as usize];
    let mut seed = 0x9e37_79b9u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 1000) as f32 / 1000.
    };
    let columns: Vec<f32> = (0..w).map(|_| noise()).collect();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let (rgb, alpha) = match style {
                Style::Preview => {
                    let t = (fx * 0.7 + fy * 0.3).clamp(0., 1.);
                    let mut c = lerp(a, b, t);
                    // A pale inset card reads as the recorded window.
                    if (0.12..0.88).contains(&fx) && (0.14..0.86).contains(&fy) {
                        c = lerp(c, [0xf4, 0xf4, 0xf7], 0.85);
                        if fy < 0.2 {
                            c = lerp(c, [0xd8, 0xd8, 0xde], 0.5);
                        }
                    }
                    (c, 255)
                }
                Style::Strip => {
                    let cell = (fx * 24.).floor() / 24.;
                    let mut c = lerp(a, b, cell);
                    if (x % (w / 24).max(1)) < 2 {
                        c = [0x10, 0x10, 0x14];
                    }
                    (c, 255)
                }
                Style::Waveform => {
                    let amp =
                        0.15 + 0.8 * (columns[x as usize] * (0.5 + 0.5 * (fx * 12.).sin().abs()));
                    let inside = (fy - 0.5).abs() * 2. < amp;
                    (a, if inside { 255 } else { 0 })
                }
            };
            bytes[i..i + 3].copy_from_slice(&rgb);
            bytes[i + 3] = alpha;
        }
    }
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &bytes, w, h,
    ))
}

fn lerp(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t)
            .round()
            .clamp(0., 255.) as u8;
    }
    out
}
