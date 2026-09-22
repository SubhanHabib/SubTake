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
//!
//! ⌘⇧E swaps the editor for the empty state ("Nothing open yet") and back;
//! the titlebar's Presets button, or ⌘⇧P, opens the Presets dialog.
//! `SUBTAKE_GALLERY_SCREEN=empty` or `=presets` starts on either; `=card-<panel>`
//! opens that recorder card.
use crate::{
    CaptureSource, EditorWindow, Field, Recent, RecordingLauncher, RecordingOptions, Region,
    Wallpaper,
};
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

mod fixtures;
mod images;

use fixtures::*;
use images::*;

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
    match std::env::var("SUBTAKE_GALLERY_SCREEN").as_deref() {
        Ok("empty") => editor.set_has_video(false),
        Ok("presets") => editor.set_dialog("presets".into()),
        // A recorder card open over the bar: `card-sources`, `card-audio`,
        // `card-camera`, `card-countdown` or `card-more`.
        Ok(screen) if screen.starts_with("card-") => {
            let panel = screen.trim_start_matches("card-").to_owned();
            let g = gallery.clone();
            Timer::single_shot(Duration::from_millis(400), move || {
                let g = g.borrow();
                show_recorder(&g.launcher, &g.options);
                g.launcher.set_panel(panel.clone().into());
                g.options.set_panel(panel.into());
                g.position_options();
            });
        }
        _ => {}
    }
    tick_meter(options.clone(), 0);
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
            g.editor.set_language(value);
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
            ("d" | "D", true, true) => g.action("toggle-appearance"),
            ("e" | "E", true, true) => g.action("toggle-empty"),
            ("p" | "P", true, true) => g.action("toggle-presets"),
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
        g.editor.set_selected_id(id);
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

    // The dev supervisor asks for a restart by touching this file. The gallery
    // holds no project, so there is nothing to save: quit as soon as it appears.
    let dev_restart_timer = Timer::default();
    if let Some(request) = std::env::var_os("SUBTAKE_DEV_RESTART_FILE") {
        let request = std::path::PathBuf::from(request);
        dev_restart_timer.start(TimerMode::Repeated, Duration::from_millis(300), move || {
            if !request.is_file() {
                return;
            }
            let _ = std::fs::remove_file(&request);
            let _ = ui_runtime::quit_event_loop();
        });
    }

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
            "toggle-empty" => {
                let has_video = !self.editor.get_has_video();
                self.editor.set_has_video(has_video);
                self.editor.set_panel("Frame".into());
                self.push_fields();
            }
            "toggle-presets" => {
                let open = self.editor.get_dialog() == "presets";
                self.editor
                    .set_dialog(if open { "" } else { "presets" }.into());
            }
            // Opening anything from the empty state lands back on the
            // fixture project.
            "open" => self.action("show-project"),
            key if key.starts_with("library-open-") => self.action("show-project"),
            "show-project" => {
                self.editor.set_has_video(true);
                self.push_fields();
            }
            key if key.starts_with("look-") => {
                self.editor.set_look_choice(key["look-".len()..].into())
            }
            key if key.starts_with("motion-") => {
                self.editor.set_motion_choice(key["motion-".len()..].into())
            }
            "record" | "start-recording" => {
                self.launcher.set_recording(true);
                self.launcher.set_paused(false);
                self.launcher.set_status("Recording".into());
                let launcher = self.launcher.clone();
                let started = std::time::Instant::now();
                self.playback
                    .start(TimerMode::Repeated, Duration::from_millis(250), move || {
                        let s = started.elapsed().as_secs();
                        launcher.set_elapsed(format!("{:02}:{:02}", s / 60, s % 60));
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
            "sources" => {
                // A fake refresh, long enough to see the Refreshing state.
                self.options.set_sources_loading(true);
                let options = self.options.clone();
                Timer::single_shot(Duration::from_millis(1600), move || {
                    options.set_sources_loading(false)
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
                    .set_status(format!("Gallery: “{other}” has no effect here"));
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
            "system_audio" | "system" | "system-audio" => {
                self.launcher.set_system_audio(on);
                self.options.set_system_audio(on);
            }
            "source" | "source_index" => {
                self.launcher.set_source_index(index);
                self.options.set_source_index(index);
            }
            "camera_index" | "camera-device" => {
                self.launcher.set_camera_index(index);
                self.options.set_camera_index(index);
            }
            "microphone_index" | "microphone-device" => {
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
                .set_status(format!("Gallery: option {key} = {value}")),
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
        self.editor.set_time_label(time_label(self.playhead));
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

/// A fake microphone for the Audio card's meter: a level wandering around
/// −18 dB, with an occasional peak hard enough to clip.
fn tick_meter(options: RecordingOptions, step: u32) {
    if options.get_panel() == "audio" {
        let wobble = ((step as f32 * 0.9).sin() + (step as f32 * 0.37).sin()) * 5.;
        let level = if step % 70 == 69 { 0. } else { -18. + wobble };
        options.set_mic_level(if options.get_microphone() {
            level
        } else {
            f32::NEG_INFINITY
        });
    }
    Timer::single_shot(Duration::from_millis(90), move || {
        tick_meter(options, step + 1)
    });
}

fn follow_playhead(editor: &EditorWindow, time: f32) {
    let visible = editor.get_timeline_visible();
    let offset = editor.get_timeline_offset();
    if time < offset || time > offset + visible {
        editor.set_timeline_offset((time - visible * 0.1).clamp(0., (DURATION - visible).max(0.)));
    }
}

fn time_label(time: f32) -> String {
    let frames = ((time - time.floor()) * 30.) as u32;
    format!(
        "{:02}:{:02}.{:02} / {:02}:{:02}",
        (time as u32) / 60,
        (time as u32) % 60,
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
