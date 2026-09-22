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
            _ => {}
        }
    }

    pub(super) fn sync_launcher_options(&self, ui: &EditorWindow) {
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
        options.set_directory(
            self.recording_directory()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
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
        if panel.is_empty() {
            options.hide()?;
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
        platform::set_editor_active(true);
        if !self.editor_shown {
            ui.window()
                .set_size(ui_runtime::LogicalSize::new(1360., 880.));
            self.editor_shown = true;
        }
        ui.show()?;
        // Editor glass is the window's own `WindowBackgroundAppearance::Blurred`
        // now that SubTake renders on the zui fork: gpui installs the
        // `UnderWindowBackground` view itself. Adding SubTake's helper on top
        // would stack a second material and double-darken the chrome.
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
        Ok(())
    }
}
