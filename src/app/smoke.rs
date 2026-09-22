//! Opt-in acceptance harnesses and the unit tests for the refresh guard.

use super::*;

// Explicit opt-in acceptance harness. Capture requires a caller-selected SubTake
// window; microphone, camera and system audio are always disabled here.
pub(super) fn launcher_smoke_step(step: u8) {
    with_app(|app, ui| {
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
                    app.launcher
                        .as_ref()
                        .is_some_and(|l| l.window().is_visible()),
                    "Recorder not visible at launch"
                );
                ensure!(
                    !ui.get_busy(),
                    "Source discovery did not finish: {}",
                    ui.get_status()
                );
                if mode == "autozoom" {
                    let source = PathBuf::from(std::env::var("SUBTAKE_AUTOZOOM_SOURCE")?);
                    app.preferences.auto_apply_zooms = true;
                    app.fresh_recording = Some(source.clone());
                    app.load(ui, source)?;
                    Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(20));
                    return Ok(());
                }
                if mode == "capture" {
                    let id = std::env::var("SUBTAKE_CAPTURE_SMOKE_WINDOW")?.parse::<u64>()?;
                    let index = app
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
                    app.finish_sources(
                        ui,
                        Ok(vec![
                            json!({"kind":"display","nativeId":1,"name":"Built-in display"}),
                            json!({"kind":"window","nativeId":2,"name":"SubTake — demo window"}),
                        ]),
                        false,
                    );
                }
                app.action(ui, "hide-launcher")?;
                ensure!(
                    !ui.window().is_visible()
                        && !app.launcher.as_ref().unwrap().window().is_visible(),
                    "Hide overlay opened the editor"
                );
                app.action(ui, "show")?;
                ensure!(
                    !ui.window().is_visible()
                        && app.launcher.as_ref().unwrap().window().is_visible(),
                    "Tray Open did not restore the recorder"
                );
                // Exercise options state and native window placement. This does
                // not inject pointer events or test source-control hit testing.
                let launcher = app.launcher.as_ref().unwrap();
                launcher
                    .window()
                    .set_position(ui_runtime::PhysicalPosition::new(210, 500));
                let anchor_position = launcher.window().position();
                let anchor_size = launcher.window().size();
                // Native positions are logical points; sizes are physical pixels.
                // Compare edges in points, using each window's current scale.
                let anchor_scale = launcher.window().scale_factor() as f64;
                let anchor_bottom =
                    anchor_position.y as f64 + anchor_size.height as f64 / anchor_scale;
                let anchor_center =
                    anchor_position.x as f64 + anchor_size.width as f64 / anchor_scale / 2.;
                app.set_launcher_options_panel(ui, "sources")?;
                ensure!(
                    app.launcher.as_ref().unwrap().get_panel() == "sources"
                        && app.launcher_options.as_ref().unwrap().window().is_visible(),
                    "Source options controller did not open its native window"
                );
                app.set_launcher_options_panel(ui, "")?;
                if mode != "capture" {
                    if mode != "idle" {
                        app.set_launcher_options_panel(ui, &mode)?;
                    }
                    Timer::single_shot(Duration::from_millis(600), move || {
                        with_app(|app, _| {
                            let launcher = app.launcher.as_ref().unwrap();
                            let position = launcher.window().position();
                            let size = launcher.window().size();
                            let scale = launcher.window().scale_factor() as f64;
                            assert!(
                                (position.y as f64 + size.height as f64 / scale - anchor_bottom)
                                    .abs()
                                    <= 1.,
                                "Opening a recorder menu moved the bar vertically"
                            );
                            assert!(
                                (position.x as f64 + size.width as f64 / scale / 2.
                                    - anchor_center)
                                    .abs()
                                    <= 1.,
                                "Opening a recorder menu moved the bar horizontally"
                            );
                            if mode != "idle" {
                                let options = app.launcher_options.as_ref().unwrap();
                                assert!(
                                    options.window().is_visible(),
                                    "Options window did not open"
                                );
                                let option_position = options.window().position();
                                let option_size = options.window().size();
                                let option_scale = options.window().scale_factor() as f64;
                                assert!(
                                    option_position.y as f64
                                        + option_size.height as f64 / option_scale
                                        <= position.y as f64 - 12.,
                                    "Options window overlaps the fixed recorder bar: menu={option_position:?} size={option_size:?} scale={option_scale}, bar={position:?} size={size:?} scale={scale}"
                                );
                                // A native move notification must carry the independent menu.
                                launcher
                                    .window()
                                    .set_position(ui_runtime::PhysicalPosition::new(
                                        position.x + 37,
                                        position.y + 31,
                                    ));
                                Timer::single_shot(Duration::from_millis(80), move || {
                                    with_app(|app, _| {
                                        let launcher = app.launcher.as_ref().unwrap();
                                        let options = app.launcher_options.as_ref().unwrap();
                                        let moved_menu = options.window().position();
                                        let moved_bar = launcher.window().position();
                                        let _ = (moved_menu, moved_bar);
                                        assert!(
                                            platform::launcher_options_are_attached(
                                                options.window(),
                                                launcher.window(),
                                            )
                                            .unwrap_or(false),
                                            "Options menu detached, moved out of alignment, or overlaps the recorder bar"
                                        );
                                    });
                                    launcher_smoke_step(10);
                                });
                            }
                        });
                        launcher_smoke_step(10);
                    });
                    return Ok(());
                }
                ui.set_capture_mic(false);
                ui.set_capture_camera(false);
                ui.set_capture_system(false);
                app.preferences.recording_directory = Some(output.clone());
                app.preferences.countdown_seconds = 3;
                app.action(ui, "start-recording")?;
                ensure!(
                    !ui.window().is_visible() && ui.get_busy(),
                    "Countdown must keep editor hidden"
                );
                app.action(ui, "cancel")?;
                Timer::single_shot(Duration::from_millis(600), || launcher_smoke_step(1));
            } else if step == 1 {
                ensure!(
                    !ui.get_busy() && !ui.get_recording() && !ui.window().is_visible(),
                    "Countdown cancellation left capture or editor active"
                );
                app.preferences.countdown_seconds = 0;
                app.action(ui, "start-recording")?;
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
                app.action(ui, "pause-recording")?;
                Timer::single_shot(Duration::from_millis(700), || launcher_smoke_step(3));
            } else if step == 3 {
                ensure!(
                    ui.get_recording_paused() && !ui.get_busy(),
                    "Pause was not acknowledged"
                );
                app.action(ui, "pause-recording")?;
                Timer::single_shot(Duration::from_millis(700), || launcher_smoke_step(4));
            } else if step == 4 {
                ensure!(
                    !ui.get_recording_paused() && !ui.get_busy(),
                    "Resume was not acknowledged"
                );
                app.action(ui, "stop-recording")?;
                Timer::single_shot(Duration::from_secs(3), || launcher_smoke_step(5));
            } else if step == 5 {
                ensure!(
                    !ui.get_recording() && !ui.get_busy() && ui.window().is_visible(),
                    "Final video did not open the editor: {}",
                    ui.get_status()
                );
                ensure!(
                    !app.launcher.as_ref().unwrap().window().is_visible(),
                    "Recorder remained visible over editor"
                );
                ensure!(
                    ui.get_panel() == "Frame" && app.info.as_ref().is_some_and(|i| i.duration > 1.),
                    "Recorded project was not loaded"
                );
                if std::env::var_os("SUBTAKE_CAPTURE_SMOKE_ZOOMS").is_some() {
                    verify_automatic_zooms(app, ui, &output)?;
                }
                let report = json!({"status":"passed","scope":"menu-bar launch, hide/Open, source control, countdown cancellation, screen-only capture of selected SubTake window, pause/resume/stop, final editor handoff","microphone":false,"camera":false,"system_audio":false,"source":app.source,"media":app.info});
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
                app.discard_recovery();
                app.recovery.flush();
                println!("LAUNCHER_SMOKE_PASSED capture");
                ui_runtime::quit_event_loop()?;
            } else if step == 20 {
                verify_automatic_zooms(app, ui, &output)?;
                // Reopening an ordinary video must not silently re-apply zooms.
                let source = app.source.clone().context("Missing source")?;
                app.load(ui, source)?;
                Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(21));
            } else if step == 21 {
                ensure!(
                    app.project()?.regions("zoomRegions").is_empty(),
                    "Ordinary reopen applied fresh-recording zooms"
                );
                app.preferences.auto_apply_zooms = false;
                let source = app.source.clone().context("Missing source")?;
                app.fresh_recording = Some(source.clone());
                app.load(ui, source)?;
                Timer::single_shot(Duration::from_secs(4), || launcher_smoke_step(22));
            } else if step == 22 {
                ensure!(
                    app.project()?.regions("zoomRegions").is_empty(),
                    "Disabled preference still applied zooms"
                );
                app.discard_recovery();
                app.recovery.flush();
                println!(
                    "LAUNCHER_SMOKE_PASSED autozoom: fresh applied, ordinary reopen unchanged, preference off respected"
                );
                ui_runtime::quit_event_loop()?;
            } else if step == 10 {
                ensure!(!ui.window().is_visible(), "Setup opened an empty editor");
                let image = app.launcher.as_ref().unwrap().window().take_snapshot()?;
                image::save_buffer(
                    output.join(format!("launcher-{mode}.png")),
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ColorType::Rgba8,
                )?;
                println!("LAUNCHER_SMOKE_PASSED {mode}");
                ui_runtime::quit_event_loop()?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            if let Some(recording) = app.recording.take() {
                let _ = recording.stop();
            }
            eprintln!("LAUNCHER_SMOKE_FAILED step {step}: {error:#}");
            std::process::exit(1);
        }
    });
}

pub(super) fn verify_automatic_zooms(
    s: &mut App,
    ui: &EditorWindow,
    output: &std::path::Path,
) -> Result<()> {
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
