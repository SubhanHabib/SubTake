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
//! `SUBTAKE_GALLERY_SCREEN=empty` or `=presets` starts on either,
//! `=presets-saved` on the dialog's Saved tab; `=settings` opens the
//! Settings dialog, `=settings-Shortcuts` on one section; `=export`,
//! `=export-gif` or `=export-frame` opens the Export panel, whose button runs
//! a fake eight-second export in the titlebar pill; `=export-progress`,
//! `=export-done` or `=export-failed` holds the pill in one state;
//! `=selection` opens Selection on a zoom region, `=selection-empty` with
//! nothing selected (clicking any region opens it too), `=selection-a1` to
//! `-a4` on an annotation, `=stage-hover` with an annotation's outline under
//! the pointer, `=add-empty` the Add panel with no video; `=cursor` and
//! `=cursor-hidden` open Cursor with the cursor shown and hidden, `=camera`
//! and `=camera-off` Camera with the overlay on and off; `=card-<panel>`
//! opens that recorder card (`=card-sources-busy` with its controls off);
//! `=panel-<name>` opens any other panel by its
//! model name (`=panel-Preferences`); `=inspector-open` slides the folded
//! inspector in, with `SUBTAKE_GALLERY_WIDTH=1100` (any width under 1280)
//! folding it, and `SUBTAKE_GALLERY_HEIGHT` sets the height the same way
//! (for a panel too long for 880); `=rec-counting`, `=rec-recording`, `=rec-paused` or
//! `=rec-stopping` shows the bar mid-capture (counting also covers the screen);
//! `=status-cycle` swaps the title pill's status chip to a running job and back on a timer;
//! `=card-cycle` opens, swaps and closes the recorder cards on a timer, for their fades;
//! `=tour` walks through a whole take on timers and quits (`gallery/tour.rs`).
//! `SUBTAKE_GALLERY_LANES=100` and `SUBTAKE_GALLERY_INSPECTOR=460` start the
//! lane region and the inspector at a height and width their edges could be
//! dragged to (`SUBTAKE_HOVER_PIN=resize` shows both grips).
//! `SUBTAKE_GALLERY_INPUTS=off` turns the recorder's microphone, system
//! audio and camera off, so its bar shows them struck through.
//! `SUBTAKE_GALLERY_OPEN=aspect` opens the inspector dropdown with that id
//! (the aspect pod's menu), as a click on its trigger would; `=menu-Add`
//! (or `-File`, `-Edit`, `-Help`) opens that command palette.
//! A second window, "SubTake components" (`gallery/catalogue.rs`), lays out
//! every primitive in every state on one scrolling page with its own Light /
//! Dark switch; `SUBTAKE_GALLERY_COMPONENTS=off` leaves it closed and
//! `=frost` (or any section's name) opens it scrolled to that section.
//! `SUBTAKE_HOVER_PIN=switch,look` holds every control whose tween key
//! contains one of those words hovered, since the gallery's unfocused windows
//! never receive the pointer's hover.
use crate::{
    CaptureSource, EditorWindow, Field, Recent, RecordingCountdown, RecordingLauncher,
    RecordingOptions, Region, Wallpaper,
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

mod catalogue;
mod fixtures;
mod images;
mod tour;

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
    countdown: RecordingCountdown,
    playhead: f32,
    playing: bool,
    appearance: &'static str,
    /// Field values keyed by field key, so panels rebuild with the edits kept.
    values: Vec<(String, String)>,
    regions: Vec<Region>,
    /// The panel a click on a region replaced with Selection.
    before_selection: Option<String>,
    playback: Timer,
    /// Handle for timers, which run from the pump loop and never re-enter a callback.
    me: Weak<RefCell<Gallery>>,
}

/// Show the status chip as a running transcription, or put it back to
/// "Gallery mode", and schedule the opposite.
fn cycle_status(editor: EditorWindow, on: bool) {
    editor.set_busy(on);
    editor.set_progress(if on { 0.4 } else { 0. });
    editor.set_status(if on {
        "Transcribing… 40%".into()
    } else {
        "Gallery mode".into()
    });
    Timer::single_shot(Duration::from_millis(1500), move || {
        cycle_status(editor, !on)
    });
}

/// The recorder cards in turn, one step each `CARD_CYCLE_MS`: opened, swapped
/// twice, closed, opened again and closed — every way a card fades.
fn cycle_cards(gallery: Weak<RefCell<Gallery>>, step: usize) {
    const STEPS: [&str; 6] = ["sources", "more", "audio", "", "countdown", ""];
    const CARD_CYCLE_MS: u64 = 900;
    let Some(g) = gallery.upgrade() else {
        return;
    };
    {
        let g = g.borrow();
        let panel = STEPS[step % STEPS.len()];
        g.launcher.set_panel(panel.into());
        g.options.set_panel(panel.into());
        g.position_options();
    }
    Timer::single_shot(Duration::from_millis(CARD_CYCLE_MS), move || {
        cycle_cards(gallery, step + 1)
    });
}

pub fn run() -> Result<()> {
    let editor = EditorWindow::new()?;
    let launcher = RecordingLauncher::new()?;
    let options = RecordingOptions::new()?;
    let countdown = RecordingCountdown::new()?;
    let gallery = Rc::new(RefCell::new(Gallery {
        editor: editor.clone(),
        launcher: launcher.clone(),
        options: options.clone(),
        countdown,
        playhead: 37.5,
        playing: false,
        // `SUBTAKE_GALLERY=light` starts in light mode; anything else is dark.
        appearance: match std::env::var("SUBTAKE_GALLERY").as_deref() {
            Ok("light") => "light",
            _ => "dark",
        },
        values: vec![],
        regions: fixture_regions(),
        before_selection: None,
        playback: Timer::default(),
        me: Weak::new(),
    }));
    gallery.borrow_mut().me = Rc::downgrade(&gallery);

    seed_editor(&editor);
    seed_recorder(&launcher, &options);
    // The title pill's status chip says where the window's data comes from.
    editor.set_status("Gallery mode".into());
    // `SUBTAKE_GALLERY_INPUTS=off` starts the recorder with the microphone,
    // system audio and camera off, on whichever bar the screen shows.
    if std::env::var("SUBTAKE_GALLERY_INPUTS").as_deref() == Ok("off") {
        let g = gallery.borrow();
        for key in ["camera", "microphone", "system_audio"] {
            g.option(key, "false");
        }
    }
    match std::env::var("SUBTAKE_GALLERY_SCREEN").as_deref() {
        Ok("empty") => editor.set_has_video(false),
        // The Add panel with no video open, its tiles dimmed.
        Ok("add-empty") => {
            editor.set_has_video(false);
            editor.set_panel("Add".into());
        }
        Ok("presets") => editor.set_dialog("presets".into()),
        // The Presets dialog on its Saved tab, the second preset open.
        Ok("presets-saved") => {
            editor.set_selected_preset("Launch keynote".into());
            editor.set_dialog("presets".into());
        }
        // The Settings dialog, `=settings-Shortcuts` on that section.
        Ok(screen) if screen.starts_with("settings") => {
            if let Some(section) = screen.strip_prefix("settings-") {
                editor.set_settings_section(section.into());
            }
            editor.set_dialog("settings".into());
        }
        Ok("inspector-open") => editor.set_inspector_open(true),
        // The Selection panel over a zoom region, or with nothing selected.
        Ok("selection") => {
            let mut g = gallery.borrow_mut();
            if let Some(r) = g.regions.iter_mut().find(|r| r.id == "z2") {
                r.selected = true;
            }
            editor.set_selected_id("z2".into());
            editor.set_panel("Selection".into());
            g.before_selection = Some("Frame".into());
        }
        Ok("selection-empty") => editor.set_panel("Selection".into()),
        // The pointer over an annotation on the stage. The gallery's picture
        // draws none, so the outline stands where one would be.
        Ok("stage-hover") => {
            editor.set_stage_annotations(vec![[0.36, 0.42, 0.28, 0.14]]);
            editor.set_hovered_annotation(0);
        }
        // Selection over an annotation: `=selection-a1` (text), `-a2` (arrow),
        // `-a3` (step) or `-a4` (blur).
        Ok(screen) if screen.starts_with("selection-a") => {
            let id = &screen["selection-".len()..];
            let mut g = gallery.borrow_mut();
            for r in g.regions.iter_mut() {
                r.selected = r.id == id;
            }
            editor.set_selected_id(id.into());
            editor.set_panel("Selection".into());
            g.before_selection = Some("Frame".into());
        }
        // The timeline zoomed to a third of the take, 30 seconds in, so its
        // lanes run on past both ends of the track column.
        Ok("timeline-zoomed") => {
            editor.set_timeline_zoom(3.);
            editor.set_timeline_offset(30.);
        }
        // A project with only zooms and its sound: every other lane is
        // gone. An imported voice-over under the recording's sound takes a
        // second audio lane.
        Ok("lanes-sparse") => {
            let mut g = gallery.borrow_mut();
            g.regions
                .retain(|r| matches!(r.kind.as_str(), "zoomRegions" | Region::TAKE_AUDIO));
            g.regions.push(fixture_region(
                "audioRegions",
                "au1",
                "Voice-over",
                20.,
                64.,
                5,
                0xa468e9,
            ));
            let mut labels: Vec<String> = editor.get_track_labels().iter().collect();
            labels.push("Audio".into());
            editor.set_track_labels(ModelRc::new(VecModel::from(labels)));
        }
        // The Cursor panel, and with the cursor hidden.
        Ok("cursor") => editor.set_panel("Cursor".into()),
        // The Camera panel, and with the overlay off.
        Ok("camera") => editor.set_panel("Webcam".into()),
        Ok("camera-off") => {
            gallery.borrow_mut().set_value("webcam.enabled", "false");
            editor.set_panel("Webcam".into());
        }
        // The stage magnified, `zoom-111` for 111%: the Fit pill's figures.
        Ok(screen) if screen.starts_with("zoom-") => {
            if let Ok(percent) = screen[5..].parse::<f32>() {
                editor.set_preview_zoom(percent / 100.);
            }
        }
        // The title pill's status chip as a running transcription, coming
        // and going every second and a half.
        Ok("status-cycle") => cycle_status(editor.clone(), true),
        // Any other panel by its name, `panel-Frame` through `panel-Recent`.
        Ok(screen) if screen.starts_with("panel-") => editor.set_panel(screen[6..].into()),
        // A style the project names that has no tile.
        Ok("cursor-unknown") => {
            gallery.borrow_mut().set_value("cursorStyle", "retro");
            editor.set_panel("Cursor".into());
        }
        Ok("cursor-hidden") => {
            gallery.borrow_mut().set_value("showCursor", "false");
            editor.set_panel("Cursor".into());
        }
        // The titlebar pill: `export-progress` (held at 62%), `export-done`
        // or `export-failed`.
        Ok("export-progress") => {
            editor.set_export_name("gallery.mp4".into());
            editor.set_export_progress(0.62);
            editor.set_export_detail("62% · 40s left".into());
            editor.set_export_state("exporting".into());
        }
        Ok("export-done") => {
            editor.set_export_name("gallery.mp4".into());
            editor.set_export_state("done".into());
        }
        Ok("export-failed") => {
            editor.set_export_name("gallery.mp4".into());
            editor.set_export_detail("Not enough disk space: 2.1 GB needed, 0.4 GB free".into());
            editor.set_export_state("failed".into());
        }
        // The Export panel: `export`, or `export-gif` / `export-frame` for
        // the other two formats.
        Ok(screen) if screen.starts_with("export") => {
            let mut g = gallery.borrow_mut();
            if let Some(format) = screen.strip_prefix("export-") {
                g.set_value("export.format", format);
            }
            editor.set_panel("Export".into());
        }
        Ok("card-cycle") => {
            let g = gallery.clone();
            Timer::single_shot(Duration::from_millis(400), move || {
                show_recorder(&g.borrow().launcher, &g.borrow().options);
                cycle_cards(Rc::downgrade(&g), 0);
            });
        }
        // A recorder card open over the bar: `card-sources`, `card-audio`,
        // `card-camera`, `card-countdown` or `card-more`. A `-busy` suffix
        // (`card-sources-busy`) holds the card mid-change, its controls off.
        Ok(screen) if screen.starts_with("card-") => {
            let panel = screen.trim_start_matches("card-");
            let busy = panel.ends_with("-busy");
            let panel = panel.trim_end_matches("-busy").to_owned();
            let g = gallery.clone();
            Timer::single_shot(Duration::from_millis(400), move || {
                let g = g.borrow();
                show_recorder(&g.launcher, &g.options);
                g.options.set_busy(busy);
                g.launcher.set_panel(panel.clone().into());
                g.options.set_panel(panel.into());
                g.position_options();
            });
        }
        // The bar mid-capture: `rec-counting`, `rec-recording`, `rec-paused`
        // or `rec-stopping`.
        Ok(screen) if screen.starts_with("rec-") => {
            let state = screen.trim_start_matches("rec-").to_owned();
            let g = gallery.clone();
            Timer::single_shot(Duration::from_millis(400), move || {
                let g = g.borrow();
                show_recorder(&g.launcher, &g.options);
                let l = &g.launcher;
                match state.as_str() {
                    "counting" => {
                        l.set_busy(true);
                        g.count(3);
                    }
                    "stopping" => {
                        l.set_busy(true);
                        l.set_stopping(true);
                        l.set_elapsed("00:42".into());
                    }
                    _ => {
                        start_capture(l, &g.playback, 42);
                        l.set_paused(state == "paused");
                    }
                }
            });
        }
        Ok("tour") => tour::start(gallery.clone()),
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
        if let ("region.startMs" | "region.endMs", Ok(ms)) = (key.as_str(), value.parse::<f32>())
            && let Some(r) = g.regions.iter_mut().find(|r| r.selected)
        {
            if key == "region.startMs" {
                r.start = ms / 1000.;
            } else {
                r.end = ms / 1000.;
            }
            g.push_timeline();
        }
        if key == "prefs.appearance" {
            g.appearance = match value.as_str() {
                "light" => "light",
                _ => "dark",
            };
            g.apply_appearance();
        }
        if key == "prefs.language" {
            g.editor.set_language(value.clone());
        }
        if key == "prefs.auto_apply_zooms" {
            g.editor.set_auto_apply_zooms(value == "true");
        }
        g.rename_preset(&key, &value);
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
        let panel = g.editor.get_panel();
        if panel != "Selection" {
            g.before_selection = Some(panel.to_string());
        }
        g.editor.set_panel("Selection".into());
        g.push_timeline();
        g.push_fields();
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
    // Under 1280 the inspector folds away to its toggle.
    let width = std::env::var("SUBTAKE_GALLERY_WIDTH")
        .ok()
        .and_then(|w| w.parse::<f32>().ok())
        .unwrap_or(1360.);
    let height = std::env::var("SUBTAKE_GALLERY_HEIGHT")
        .ok()
        .and_then(|h| h.parse::<f32>().ok())
        .unwrap_or(880.);
    editor
        .window()
        .set_size(ui_runtime::LogicalSize::new(width, height));
    editor.show()?;
    show_recorder(&launcher, &options);
    // The component catalogue opens beside them once the app is running:
    // it is a plain gpui view, opened from a timer outside the app borrow.
    Timer::single_shot(Duration::from_millis(200), catalogue::open);

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
        if self.preset_action(action) {
            return;
        }
        match action {
            "open-settings" => self.editor.set_dialog("settings".into()),
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
                // The count first, the way the app runs it, then capture.
                self.options.set_panel("".into());
                self.launcher.set_panel("".into());
                let _ = self.options.hide();
                let count = self.launcher.get_countdown();
                if count > 0 {
                    self.launcher.set_busy(true);
                    self.count(count);
                    let me = self.me.clone();
                    self.playback
                        .start(TimerMode::Repeated, Duration::from_secs(1), move || {
                            if let Some(g) = me.upgrade() {
                                let g = g.borrow();
                                let left = g.launcher.get_counting() - 1;
                                g.count(left.max(0));
                                if left <= 0 {
                                    g.launcher.set_busy(false);
                                    start_capture(&g.launcher, &g.playback, 0);
                                }
                            }
                        });
                } else {
                    start_capture(&self.launcher, &self.playback, 0);
                }
            }
            "cancel" => {
                self.playback.stop();
                self.count(0);
                self.launcher.set_busy(false);
            }
            "discard-recording" => {
                self.playback.stop();
                self.launcher.set_recording(false);
                self.launcher.set_paused(false);
                self.launcher.set_elapsed("".into());
            }
            "stop" | "stop-recording" | "finish" => {
                // A writing-out pause long enough to see, then the editor.
                self.playback.stop();
                self.launcher.set_recording(false);
                self.launcher.set_paused(false);
                self.launcher.set_busy(true);
                self.launcher.set_stopping(true);
                let me = self.me.clone();
                Timer::single_shot(Duration::from_millis(2400), move || {
                    if let Some(g) = me.upgrade() {
                        let g = g.borrow();
                        g.launcher.set_stopping(false);
                        g.launcher.set_busy(false);
                        g.launcher.set_elapsed("".into());
                    }
                });
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
                // A fake eight-second run so the pill can be watched from
                // start to finish; Cancel stops it.
                let editor = self.editor.clone();
                editor.set_export_name("gallery.mp4".into());
                editor.set_export_progress(0.);
                editor.set_export_detail("0%".into());
                editor.set_export_state("exporting".into());
                let started = std::time::Instant::now();
                Timer::single_shot(Duration::from_millis(16), move || {
                    tick_export(editor, started)
                });
            }
            "cancel-export" => self.editor.set_export_state(String::new()),
            "delete" | "deselect" => {
                if action == "delete" {
                    self.regions.retain(|r| !r.selected);
                }
                for r in &mut self.regions {
                    r.selected = false;
                }
                self.editor.set_selected_id(String::new());
                if self.editor.get_panel() == "Selection"
                    && let Some(panel) = self.before_selection.take()
                {
                    self.editor.set_panel(panel.into());
                }
                self.push_timeline();
                self.push_fields();
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

    /// The count on the bar and over the screen, as the app runs both.
    fn count(&self, left: i32) {
        self.launcher.set_counting(left);
        self.countdown.set_counting(left);
        if left > 0 && !self.countdown.window().is_visible() {
            let _ = self.countdown.show();
            // Over the bar's screen, once the bar has been placed on it.
            Timer::single_shot(Duration::from_millis(60), || {
                platform::set_countdown_display(0)
            });
        } else if left <= 0 {
            let _ = self.countdown.hide();
        }
    }

    fn apply_appearance(&self) {
        for window in [
            &self.editor as &dyn Appearance,
            &self.launcher,
            &self.options,
            &self.countdown,
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
        self.editor
            .set_settings_fields(ModelRc::new(VecModel::from(settings_fields(self))));
    }

    /// The saved presets' names and parts, as the dialog reads them.
    fn saved_presets(&self) -> (Vec<String>, Vec<String>) {
        (
            self.editor
                .get_saved_presets()
                .iter()
                .map(Into::into)
                .collect(),
            self.editor
                .get_saved_preset_parts()
                .iter()
                .map(Into::into)
                .collect(),
        )
    }

    fn set_saved_presets(&self, names: Vec<String>, parts: Vec<String>) {
        self.editor
            .set_saved_presets(ModelRc::new(VecModel::from(names)));
        self.editor
            .set_saved_preset_parts(ModelRc::new(VecModel::from(parts)));
    }

    /// The saved-preset commands, acted out on the fixture list: nothing
    /// is written, and import and export only say what they would do.
    fn preset_action(&mut self, action: &str) -> bool {
        let (mut names, mut parts) = self.saved_presets();
        let index = |prefix: &str| {
            action
                .strip_prefix(prefix)
                .and_then(|i| i.parse::<usize>().ok())
                .filter(|i| *i < names.len())
        };
        let unused = |names: &[String], base: &str| {
            std::iter::once(base.to_owned())
                .chain((2..).map(|n| format!("{base} {n}")))
                .find(|n| !names.contains(n))
                .unwrap_or_default()
        };
        if action == "new-preset" {
            let name = unused(&names, "Preset");
            names.push(name.clone());
            parts.push("look,motion,cursor,camera,captions,export".into());
            self.set_saved_presets(names, parts);
            self.editor.set_selected_preset(name);
        } else if let Some(i) = index("duplicate-preset-") {
            let name = unused(&names, &format!("{} copy", names[i]));
            let p = parts[i].clone();
            names.push(name.clone());
            parts.push(p);
            self.set_saved_presets(names, parts);
            self.editor.set_selected_preset(name);
        } else if let Some(i) = index("remove-preset-") {
            if self.editor.get_default_preset() == names[i] {
                self.editor.set_default_preset(String::new());
            }
            names.remove(i);
            parts.remove(i);
            self.set_saved_presets(names, parts);
        } else if let Some(i) = index("default-preset-") {
            let chosen = self.editor.get_default_preset() == names[i];
            self.editor.set_default_preset(if chosen {
                String::new()
            } else {
                names[i].clone()
            });
        } else if let Some(rest) = action.strip_prefix("preset-part-") {
            let Some((i, part)) = rest.split_once('-') else {
                return true;
            };
            let Some(i) = i.parse::<usize>().ok().filter(|i| *i < names.len()) else {
                return true;
            };
            let mut on: Vec<&str> = parts[i].split(',').filter(|p| !p.is_empty()).collect();
            if on.contains(&part) {
                if on.len() > 1 {
                    on.retain(|p| *p != part);
                }
            } else {
                on.push(part);
            }
            let order = |p: &&str| {
                subtake_native::presets::GROUPS
                    .iter()
                    .position(|(g, _, _)| g == p)
            };
            on.sort_by_key(order);
            parts[i] = on.join(",");
            self.set_saved_presets(names, parts);
        } else if action == "import-preset"
            || action.starts_with("share-preset-")
            || action.starts_with("update-preset-")
            || action.starts_with("apply-preset-")
        {
            self.editor.set_status(format!("Gallery: {action}"));
        } else {
            return false;
        }
        true
    }

    /// `preset.{index}.name`, renaming a fixture preset.
    fn rename_preset(&mut self, key: &str, to: &str) {
        let Some(i) = key
            .strip_prefix("preset.")
            .and_then(|k| k.strip_suffix(".name"))
            .and_then(|i| i.parse::<usize>().ok())
        else {
            return;
        };
        let (mut names, parts) = self.saved_presets();
        let to = to.trim();
        if i >= names.len() || to.is_empty() || names.iter().any(|n| n == to) {
            return;
        }
        if self.editor.get_default_preset() == names[i] {
            self.editor.set_default_preset(to.into());
        }
        names[i] = to.into();
        self.set_saved_presets(names, parts);
        self.editor.set_selected_preset(to.into());
    }

    fn position_options(&self) {
        // A closing card fades out and hides its own window.
        if self.launcher.get_panel().is_empty() {
            return;
        }
        self.options.show().ok();
        self.options.window().set_blur(false);
        self.options.window().set_transparent(true);
        let _ = platform::position_launcher_options(self.options.window(), self.launcher.window());
        // As in the app: the open card holds focus so it redraws at full rate.
        self.options.window().make_key();
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

impl Appearance for RecordingCountdown {
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
    if editor.get_export_state() != "exporting" {
        return;
    }
    let progress = (started.elapsed().as_secs_f32() / 8.).min(1.);
    editor.set_export_progress(progress);
    let left = (8. - started.elapsed().as_secs_f32()).max(0.).ceil();
    editor.set_export_detail(format!("{:.0}% · {left:.0}s left", progress * 100.));
    if progress < 1. {
        Timer::single_shot(Duration::from_millis(33), move || {
            tick_export(editor, started)
        });
    } else {
        editor.set_export_state("done".into());
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

/// The transport's clock in hundredths, as the playhead chip writes it, so
/// the two read the same time.
fn time_label(time: f32) -> String {
    let centis = (time * 100.).round() as u32;
    format!(
        "{:02}:{:02}.{:02} / {:02}:{:02}",
        centis / 6000,
        (centis / 100) % 60,
        centis % 100,
        (DURATION as u32) / 60,
        (DURATION as u32) % 60
    )
}

/// Capture running in the gallery: the bar's clock ticks from `from`
/// seconds so a screenshot shows a plausible recording.
fn start_capture(launcher: &RecordingLauncher, clock: &Timer, from: u64) {
    launcher.set_recording(true);
    launcher.set_paused(false);
    launcher.set_status("Recording".into());
    launcher.set_elapsed(format!("{:02}:{:02}", from / 60, from % 60));
    let launcher = launcher.clone();
    let started = std::time::Instant::now();
    clock.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        if launcher.get_paused() {
            return;
        }
        let s = from + started.elapsed().as_secs();
        launcher.set_elapsed(format!("{:02}:{:02}", s / 60, s % 60));
    });
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
