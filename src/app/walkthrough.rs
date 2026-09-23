//! `SUBTAKE_WALKTHROUGH=light|dark`: the real app takes itself through a whole
//! take on timers — records the built-in display, opens the recording in the
//! editor, visits every panel and exports — then quits. It is the gallery tour
//! (`src/gallery/tour.rs`) on real capture, real media and a real export, so
//! the flow can be filmed without anyone clicking.
//!
//! It drives the same callbacks the controls fire; pointer hit testing is not
//! exercised. The microphone and camera stay off, settings and recovery are
//! isolated (`Preferences::directory`), and the recording and export are
//! written to `SUBTAKE_WALKTHROUGH_DIR`.

use super::*;

type Check = fn(&App, &EditorWindow) -> bool;

enum Step {
    /// Wait the milliseconds, then run against the app.
    Do(u64, fn(&mut App, &EditorWindow) -> Result<()>),
    /// Wait the milliseconds, then fire a view callback, as a control would.
    View(u64, fn(&EditorWindow)),
    /// Hold until the check passes, failing after the milliseconds.
    Until(&'static str, Check, u64),
}
use Step::{Do, Until, View};

const POLL_MS: u64 = 250;

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
            .position(|s| {
                s["kind"] == "display" && s["name"].as_str().is_some_and(|n| n.contains("Built-in"))
            })
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
    Do(5000, |app, ui| app.action(ui, "pause-recording")),
    Until("pause", |_, ui| !ui.get_busy(), 10_000),
    Do(2000, |app, ui| app.action(ui, "pause-recording")),
    Until("resume", |_, ui| !ui.get_busy(), 10_000),
    Do(4000, |app, ui| app.action(ui, "stop-recording")),
    Until(
        "editor",
        |app, ui| app.history.is_some() && !ui.get_busy() && ui.window().is_visible(),
        60_000,
    ),
    Do(2000, |app, ui| app.action(ui, "play")),
    Do(3500, |app, ui| app.action(ui, "play")),
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
    // A region: Add at the playhead selects it, and Selection takes the
    // inspector's place until it is deselected.
    Do(1200, |app, ui| {
        let duration = app.info.as_ref().context("No recording open")?.duration;
        app.seek(ui, duration * 0.4);
        Ok(())
    }),
    View(800, |ui| ui.invoke_action("add-zoom".into())),
    View(2500, |ui| ui.invoke_action("deselect".into())),
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
    }
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
