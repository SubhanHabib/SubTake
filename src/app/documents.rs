//! Project documents: the library, load, save, replace and crash recovery.

use super::*;

impl App {
    pub(super) fn reload_library(&mut self) -> Result<()> {
        self.library = subtake_native::library::entries(
            self.preferences.library_directory.as_deref(),
            &self.preferences.recent_projects,
            &self.library_query,
        )?;
        Ok(())
    }

    pub(super) fn schedule_recovery(&self) {
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

    pub(super) fn discard_recovery(&mut self) {
        self.recovery_timer.stop();
        self.recovery.clear(self.recovery_origin.take());
    }

    pub(super) fn load(&mut self, ui: &EditorWindow, path: PathBuf) -> Result<()> {
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
                        ui.set_thumbnails(ui_runtime::Image::default());
                        ui.set_frosted_thumbnails(ui_runtime::Image::default());
                        ui.set_waveform(ui_runtime::Image::default());
                        std::thread::spawn(move || {
                            let result = media::timeline_artwork(&artwork_source, &artwork_info);
                            // Blur the real filmstrip once on the artwork worker, never capture the desktop.
                            let frosted = result
                                .as_ref()
                                .ok()
                                .and_then(|(path, _)| image::open(path).ok())
                                .map(|image| image.blur(10.).to_rgba8());
                            post(move |s, ui| {
                                if s.source.as_ref() != Some(&artwork_source) {
                                    return;
                                }
                                match result {
                                    Ok((thumbs, wave)) => {
                                        if let Ok(image) =
                                            ui_runtime::Image::load_from_path(&thumbs)
                                        {
                                            ui.set_thumbnails(image);
                                            if let Some(ref pixels) = frosted {
                                                ui.set_frosted_thumbnails(
                                                    ui_runtime::Image::from_rgba8(
                                                        ui_runtime::SharedPixelBuffer::<
                                                            ui_runtime::Rgba8Pixel,
                                                        >::clone_from_slice(
                                                            pixels.as_raw(),
                                                            pixels.width(),
                                                            pixels.height(),
                                                        ),
                                                    ),
                                                );
                                            }
                                        }
                                        if let Some(wave) = wave {
                                            if let Ok(image) =
                                                ui_runtime::Image::load_from_path(&wave)
                                            {
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

    pub(super) fn save(&mut self, ui: &EditorWindow, save_as: bool) -> Result<()> {
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

    pub(super) fn can_replace(&mut self, ui: &EditorWindow) -> bool {
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
}

pub(super) fn id_to_null(mut value: Value) -> Value {
    value["id"] = Value::Null;
    value
}
