//! The recorder: launcher and options windows, capture sources, hotkeys and
//! the tray item.

use super::*;

// A cold accessory launch may precede creation of the AppKit host.
// Retry native panel configuration briefly instead of showing a generic window
// first or emitting a spurious handle error.
pub(super) fn position_launcher_when_ready(attempt: u8) {
    Timer::single_shot(Duration::from_millis(20), move || {
        with_app(|app, _| {
            if let Some(launcher) = &app.launcher
                && platform::position_launcher(launcher.window()).is_err()
                && attempt < 9
            {
                position_launcher_when_ready(attempt + 1);
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
        let (Some(launcher), Some(options)) = (&self.launcher, &self.launcher_options) else {
            return;
        };
        options.set_panel(launcher.get_panel());
        options.set_appearance(self.preferences.appearance.as_str().into());
        options.set_source_names(ui.get_source_names());
        options.set_capture_sources(ui.get_capture_sources());
        options.set_sources_loading(ui.get_sources_loading());
        options.set_status(ui.get_status());
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

    pub(super) fn sync_launcher(&self, ui: &EditorWindow) {
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
            if self.counting > 0 && !shown {
                let _ = overlay.show();
            } else if self.counting == 0 && shown {
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
                    ui.set_capture_sources(ModelRc::new(VecModel::from(
                        sources.iter().map(capture_source).collect::<Vec<_>>(),
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
        ui.set_recording_hint(message.clone());
        ui.set_status(message);
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

/// The microphone meter's level, from its capture thread. A level that
/// arrives after the meter has stopped is dropped.
extern "C" fn mic_level(level: f32) {
    post(move |app, ui| {
        if let Some(options) = &app.launcher_options
            && options.get_panel() == "audio"
            && ui.get_capture_mic()
        {
            options.set_mic_level(level);
        }
    });
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
    }
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

#[cfg(test)]
mod tests;
