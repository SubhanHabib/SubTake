//! `SUBTAKE_WALKTHROUGH=light|dark`: the real app takes itself through a whole
//! take on timers — records the built-in display (the laptop's own panel),
//! opens the recording in the editor, fills every lane, zooms and pans the
//! timeline and the picture, visits every panel and exports — then quits. It
//! is the gallery tour (`src/gallery/tour.rs`) on real capture, real media and
//! a real export, so the flow can be filmed without anyone clicking.
//!
//! Scrolls and pinches are real input, dispatched into the window at a point
//! (`Window::dispatch_input`, `Window::magnify`) without moving the pointer.
//! Clicks and drags are not: they drive the callbacks the controls fire. The
//! microphone and camera stay off, settings and recovery are isolated
//! (`Preferences::directory`), and the recording and export are written to
//! `SUBTAKE_WALKTHROUGH_DIR`.

use super::*;

type Check = fn(&App, &EditorWindow) -> bool;

enum Step {
    /// Wait the milliseconds, then run against the app.
    Do(u64, fn(&mut App, &EditorWindow) -> Result<()>),
    /// Wait the milliseconds, then fire a view callback, as a control would.
    View(u64, fn(&EditorWindow)),
    /// Hold until the check passes, failing after the milliseconds.
    Until(&'static str, Check, u64),
    /// Wait the milliseconds, then run a gesture frame by frame, one each
    /// `FRAME_MS`, given the frame and the count.
    Glide(u64, u32, fn(&EditorWindow, u32, u32)),
}
use Step::{Do, Glide, Until, View};

const POLL_MS: u64 = 250;
const FRAME_MS: u64 = 16;

const STEPS: &[Step] = &[
    Do(0, |app, _| {
        app.preferences.recording_directory = Some(directory()?);
        Ok(())
    }),
    Until(
        "sources",
        |app, ui| !ui.get_busy() && !app.sources.is_empty(),
        20_000,
    ),
    Do(500, |app, ui| {
        let index = app
            .sources
            .iter()
            .position(|s| s["kind"] == "display" && s["nativeId"].as_u64().is_some_and(built_in))
            .or_else(|| app.sources.iter().position(|s| s["kind"] == "display"))
            .context("No display to record")?;
        app.apply_launcher_option(ui, "source", &index.to_string());
        app.apply_launcher_option(ui, "camera", "false");
        app.apply_launcher_option(ui, "microphone", "false");
        Ok(())
    }),
    Do(1500, |app, ui| {
        app.set_launcher_options_panel(ui, "sources")
    }),
    Do(2000, |app, ui| app.set_launcher_options_panel(ui, "audio")),
    Do(1800, |app, ui| app.set_launcher_options_panel(ui, "camera")),
    Do(1800, |app, ui| {
        app.set_launcher_options_panel(ui, "countdown")
    }),
    Do(1800, |app, ui| app.set_launcher_options_panel(ui, "")),
    Do(1000, |app, ui| app.action(ui, "start-recording")),
    Until("capture", |app, _| app.recording.is_some(), 20_000),
    Do(8000, |app, ui| app.action(ui, "pause-recording")),
    Until("pause", |_, ui| !ui.get_busy(), 10_000),
    Do(2000, |app, ui| app.action(ui, "pause-recording")),
    Until("resume", |_, ui| !ui.get_busy(), 10_000),
    Do(8000, |app, ui| app.action(ui, "stop-recording")),
    Until(
        "editor",
        |app, ui| app.history.is_some() && !ui.get_busy() && ui.window().is_visible(),
        60_000,
    ),
    Do(2000, |app, ui| app.action(ui, "play")),
    Do(3000, |app, ui| app.action(ui, "play")),
    // Every lane at once, as the gallery's timeline has it: each addition
    // selects its region, so its Selection inspector shows as it lands.
    Do(1200, |app, ui| {
        add(app, ui, "add-caption", 0.04, Some("A real take"))
    }),
    Do(1100, |app, ui| {
        add(app, ui, "add-caption", 0.36, Some("Every lane in one edit"))
    }),
    Do(1100, |app, ui| {
        add(app, ui, "add-caption", 0.68, Some("Then zoom and pan"))
    }),
    Do(1100, |app, ui| add(app, ui, "add-zoom", 0.1, None)),
    Do(1100, |app, ui| add(app, ui, "add-zoom", 0.55, None)),
    Do(1100, |app, ui| {
        add(app, ui, "add-text", 0.2, Some("SubTake"))
    }),
    Do(1100, |app, ui| add(app, ui, "add-figure", 0.46, None)),
    Do(1100, |app, ui| add(app, ui, "add-blur", 0.74, None)),
    Do(1100, |app, ui| add(app, ui, "split-clip", 0.3, None)),
    Do(1100, |app, ui| add(app, ui, "split-clip", 0.62, None)),
    Do(1100, |app, ui| add(app, ui, "add-speed", 0.8, None)),
    Do(1100, |app, ui| add(app, ui, "add-trim", 0.9, None)),
    Do(1100, add_music),
    // The music lands on a second audio lane, below the console's fold: a
    // plain scroll brings it up, and scrolling back returns to the top.
    Glide(400, 20, |ui, _, _| scroll(ui, CONSOLE, (0., -8.), false)),
    Glide(2000, 20, |ui, _, _| scroll(ui, CONSOLE, (0., 8.), false)),
    // Moves and trims go through the callback a drag ends in.
    View(1500, |ui| {
        if let Some(id) = region_id("zoomRegions", 1) {
            ui.invoke_select_region("zoomRegions".into(), id.clone(), false);
            ui.invoke_move_region("zoomRegions".into(), id, 1.2, 0);
        }
    }),
    View(1500, |ui| {
        if let Some(id) = region_id("zoomRegions", 1) {
            ui.invoke_move_region("zoomRegions".into(), id, 1., 1);
        }
    }),
    View(1500, |ui| {
        if let Some(id) = region_id("annotationRegions", 0) {
            ui.invoke_select_region("annotationRegions".into(), id, false);
        }
    }),
    View(1500, |ui| ui.invoke_action("undo".into())),
    View(1200, |ui| ui.invoke_action("redo".into())),
    View(1200, |ui| ui.invoke_action("deselect".into())),
    // The timeline: ⌘-scroll zooms in about the pointer, a sideways scroll
    // pans along, and Fit brings the whole take back.
    Glide(1000, 24, |ui, _, _| scroll(ui, CONSOLE, (0., -7.), true)),
    Glide(700, 50, |ui, _, _| scroll(ui, CONSOLE, (-18., 0.), false)),
    Glide(500, 50, |ui, _, _| scroll(ui, CONSOLE, (18., 0.), false)),
    Glide(500, 16, |ui, _, _| scroll(ui, CONSOLE, (0., 5.), true)),
    View(900, |ui| {
        ui.set_timeline_zoom(1.);
        ui.set_timeline_offset(0.);
    }),
    // The picture: a pinch zooms in about the fingers, scrolling pans the
    // zoomed picture around, and a pinch back out fits it again.
    Glide(1200, 36, |ui, frame, frames| {
        pinch(ui, PICTURE, 3., frame, frames)
    }),
    Glide(500, 30, |ui, _, _| scroll(ui, PICTURE, (14., 0.), false)),
    Glide(200, 30, |ui, _, _| scroll(ui, PICTURE, (0., 10.), false)),
    Glide(200, 60, |ui, _, _| scroll(ui, PICTURE, (-14., 0.), false)),
    Glide(200, 30, |ui, _, _| scroll(ui, PICTURE, (0., -10.), false)),
    Glide(200, 30, |ui, _, _| scroll(ui, PICTURE, (14., 0.), false)),
    Glide(600, 30, |ui, frame, frames| {
        pinch(ui, PICTURE, 1. / 3., frame, frames)
    }),
    View(800, |ui| ui.invoke_reset_preview()),
    // The whole edit plays back: zooms, clips, speed, trim, overlays.
    Do(1200, |app, ui| {
        app.seek(ui, 0.);
        app.action(ui, "play")
    }),
    Do(15000, |app, ui| app.action(ui, "play")),
    View(1000, |ui| ui.invoke_panel_change("Cursor".into())),
    View(1800, |ui| ui.invoke_panel_change("Webcam".into())),
    View(1800, |ui| ui.invoke_panel_change("Captions".into())),
    View(1800, |ui| ui.invoke_panel_change("Audio".into())),
    View(1800, |ui| ui.invoke_panel_change("Frame".into())),
    View(1800, |ui| ui.invoke_panel_change("Wallpapers".into())),
    View(1800, |ui| ui.invoke_action("visual-crop".into())),
    View(1800, |ui| ui.invoke_action("finish-crop".into())),
    View(1500, |ui| ui.invoke_panel_change("Preferences".into())),
    View(1800, |ui| ui.invoke_panel_change("Shortcuts".into())),
    View(1800, |ui| ui.invoke_panel_change("Recent".into())),
    View(1800, |ui| ui.invoke_panel_change("Frame".into())),
    View(1200, |ui| ui.set_dialog("presets".into())),
    View(3000, |ui| ui.set_dialog(String::new())),
    View(1000, |ui| ui.invoke_panel_change("Export".into())),
    Do(2000, |app, ui| {
        let theme = std::env::var("SUBTAKE_WALKTHROUGH")?;
        let path = directory()?.join(format!("walkthrough-{theme}.mp4"));
        let _ = std::fs::remove_file(&path);
        app.export_path = Some(path);
        app.refresh(ui);
        app.action(ui, "export")
    }),
    Until(
        "export",
        |_, ui| matches!(ui.get_export_state().as_str(), "done" | "failed"),
        300_000,
    ),
    Do(4000, |app, ui| {
        ensure!(
            ui.get_export_state() == "done",
            "Export failed: {}",
            ui.get_export_detail()
        );
        let path = app.last_export.clone().context("No export written")?;
        let info = media::probe(&path)?;
        println!(
            "WALKTHROUGH_PASSED: {} ({}×{}, {:.1}s)",
            path.display(),
            info.width,
            info.height,
            info.duration
        );
        app.discard_recovery();
        app.recovery.flush();
        let _ = ui_runtime::quit_event_loop();
        Ok(())
    }),
];

/// Where the gestures land, as fractions of the editor window: the middle of
/// the picture, and the timeline console's lower lanes, whose scroll handler
/// covers the whole console.
const PICTURE: (f32, f32) = (0.4, 0.25);
const CONSOLE: (f32, f32) = (0.5, 0.86);

/// Seeks to the fraction of the take, runs the action there and, for a
/// caption or text, gives what it added its own words.
fn add(app: &mut App, ui: &EditorWindow, action: &str, at: f64, text: Option<&str>) -> Result<()> {
    let duration = app.info.as_ref().context("No recording open")?.duration;
    app.seek(ui, duration * at);
    app.action(ui, action)?;
    if let Some(text) = text {
        let (key, id) = app.selected.clone().context("Nothing was added")?;
        let patch = if key == "autoCaptions" {
            json!({"text": text})
        } else {
            json!({"content": text, "textContent": text})
        };
        app.edit(ui, |p| p.change_region(&key, &id, patch))?;
    }
    Ok(())
}

/// A music bed under the middle of the take. "Add audio" asks for a file, so
/// the walkthrough writes one and adds it as that action would.
fn add_music(app: &mut App, ui: &EditorWindow) -> Result<()> {
    let duration = app.info.as_ref().context("No recording open")?.duration;
    let (start, length) = (duration * 0.15, duration * 0.6);
    let path = directory()?.join("walkthrough-music.wav");
    write_music(&path, length)?;
    let mut id = String::new();
    app.edit(ui, |p| {
        id = p.add(
            "audioRegions",
            json!({"audioPath": path, "volume": 0.6, "startMs": start * 1000., "endMs": (start + length) * 1000.}),
        )?;
        Ok(())
    })?;
    app.selected = Some(("audioRegions".into(), id));
    app.show_selection(ui);
    app.refresh(ui);
    Ok(())
}

/// A soft chord pulsing on the beat, so its lane draws a waveform.
fn write_music(path: &Path, seconds: f64) -> Result<()> {
    const RATE: u32 = 44_100;
    let samples = (seconds * f64::from(RATE)) as u32;
    let bytes = samples * 2;
    let mut wav = Vec::with_capacity(44 + bytes as usize);
    // A 16-bit mono PCM header.
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&[1, 0, 1, 0]);
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&[2, 0, 16, 0]);
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    for i in 0..samples {
        let t = f64::from(i) / f64::from(RATE);
        let beat = (t * 2.).fract();
        let envelope = (-beat * 5.).exp() * 0.25;
        let chord: f64 = [220., 277.18, 329.63]
            .iter()
            .map(|f| (t * f * std::f64::consts::TAU).sin())
            .sum();
        let sample = (chord / 3. * envelope * f64::from(i16::MAX)) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, wav)?;
    Ok(())
}

/// The id of the kind's region at the index, read without holding the app
/// while a callback that takes it runs.
fn region_id(key: &str, index: usize) -> Option<String> {
    let mut id = None;
    with_app(|app, _| {
        id = app
            .project()
            .ok()
            .and_then(|p| p.regions(key).get(index)?["id"].as_str().map(String::from));
    });
    id
}

/// Where a fraction of the editor window is, in its own points.
fn at(ui: &EditorWindow, (x, y): (f32, f32)) -> gpui::Point<gpui::Pixels> {
    let window = ui.window();
    let size = window.size();
    let scale = window.scale_factor().max(1.);
    gpui::point(
        gpui::px(size.width as f32 / scale * x),
        gpui::px(size.height as f32 / scale * y),
    )
}

/// One scroll-wheel step at the place, ⌘ held or not.
fn scroll(ui: &EditorWindow, place: (f32, f32), (dx, dy): (f32, f32), command: bool) {
    ui.window()
        .dispatch_input(gpui::PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
            position: at(ui, place),
            delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(dx), gpui::px(dy))),
            modifiers: gpui::Modifiers {
                platform: command,
                ..Default::default()
            },
            touch_phase: gpui::TouchPhase::Moved,
        }));
}

/// One frame of a pinch that scales by `factor` over its frames.
fn pinch(ui: &EditorWindow, place: (f32, f32), factor: f32, frame: u32, frames: u32) {
    let point = at(ui, place);
    let (x, y) = (f32::from(point.x), f32::from(point.y));
    let steps = frames.saturating_sub(2).max(1) as f32;
    let (delta, phase) = match frame {
        0 => (0., 0),
        f if f + 1 == frames => (0., 2),
        _ => (factor.powf(1. / steps) - 1., 1),
    };
    ui.window().magnify(x, y, delta, phase);
}

/// Whether the display is the laptop's own panel. Sources are named "Display
/// N", so the name cannot tell.
#[cfg(target_os = "macos")]
fn built_in(id: u64) -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayIsBuiltin(display: u32) -> u32;
    }
    u32::try_from(id).is_ok_and(|id| unsafe { CGDisplayIsBuiltin(id) } != 0)
}

#[cfg(not(target_os = "macos"))]
fn built_in(_: u64) -> bool {
    false
}

fn directory() -> Result<PathBuf> {
    let directory = PathBuf::from(std::env::var("SUBTAKE_WALKTHROUGH_DIR")?);
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub(super) fn start() {
    run(0);
}

fn run(index: usize) {
    let Some(step) = STEPS.get(index) else {
        return;
    };
    match step {
        Do(wait, f) => {
            let f = *f;
            Timer::single_shot(Duration::from_millis(*wait), move || {
                let mut result = Ok(());
                with_app(|app, ui| result = f(app, ui));
                finish(index, result);
            });
        }
        View(wait, f) => {
            let f = *f;
            Timer::single_shot(Duration::from_millis(*wait), move || {
                // Not under `with_app`: the callback takes the app itself.
                if let Some(ui) = editor() {
                    f(&ui);
                }
                run(index + 1);
            });
        }
        Until(what, check, timeout) => wait(index, what, *check, *timeout),
        Glide(wait, frames, f) => {
            let (f, frames) = (*f, *frames);
            Timer::single_shot(Duration::from_millis(*wait), move || {
                glide(index, f, 0, frames)
            });
        }
    }
}

fn glide(index: usize, f: fn(&EditorWindow, u32, u32), frame: u32, frames: u32) {
    if frame == frames {
        return run(index + 1);
    }
    if let Some(ui) = editor() {
        f(&ui, frame, frames);
    }
    Timer::single_shot(Duration::from_millis(FRAME_MS), move || {
        glide(index, f, frame + 1, frames)
    });
}

fn wait(index: usize, what: &'static str, check: Check, left: u64) {
    let mut passed = false;
    let mut status = String::new();
    with_app(|app, ui| {
        passed = check(app, ui);
        status = ui.get_status().to_string();
    });
    if passed {
        run(index + 1);
    } else if left == 0 {
        fail(format!("timed out waiting for {what}; status: {status}"));
    } else {
        Timer::single_shot(Duration::from_millis(POLL_MS), move || {
            wait(index, what, check, left.saturating_sub(POLL_MS))
        });
    }
}

fn finish(index: usize, result: Result<()>) {
    match result {
        Ok(()) => run(index + 1),
        Err(e) => fail(format!("step {index}: {e:#}")),
    }
}

fn fail(message: String) {
    eprintln!("WALKTHROUGH_FAILED: {message}");
    std::process::exit(1);
}

fn editor() -> Option<EditorWindow> {
    STATE.with(|slot| slot.borrow().as_ref().and_then(|(_, weak)| weak.upgrade()))
}
