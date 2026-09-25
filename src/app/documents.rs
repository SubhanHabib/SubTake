//! Project documents: the library, load, save, replace and crash recovery.

use super::*;

impl App {
    /// The files opened lately and those in the project folder; with no
    /// project folder chosen, the recordings folder stands in, so every take
    /// is in the library and All projects counts past the recent list.
    pub(super) fn reload_library(&mut self) -> Result<()> {
        let folder = self.preferences.library_directory.clone().or_else(|| {
            self.recording_directory()
                .ok()
                .filter(|directory| directory.is_dir())
        });
        self.library = subtake_native::library::entries(
            folder.as_deref(),
            &self.preferences.recent_projects,
            &self.library_query,
        )?;
        self.request_stills(self.library.iter().take(3).cloned().collect());
        Ok(())
    }

    /// `reload_library` for the Projects view, which shows every entry and so
    /// asks for every entry's still.
    pub(super) fn browse_library(&mut self) -> Result<()> {
        self.reload_library()?;
        self.request_stills(self.library.clone());
        Ok(())
    }

    /// The library as cards, newest first: the empty state's Recent row
    /// draws the first three and the Projects view all of them.
    ///
    /// A card reads the take's running time and how old it is, as the
    /// handoff's "1:24 · 2m ago". The running time comes with the still from
    /// `request_stills`; until then, or when the file cannot be read, the
    /// card says what kind of file it is instead, and draws its own
    /// placeholder for the picture.
    pub(super) fn recents(&self) -> Vec<Recent> {
        self.library
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let kind = match path.extension().and_then(|e| e.to_str()) {
                    Some("recordly" | "openscreen") => "Project",
                    _ => "Video",
                };
                let age = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .map(relative_age);
                let still = self.stills.get(path).cloned().flatten();
                let length = match &still {
                    Some((_, duration)) => running_time(*duration),
                    None => kind.to_owned(),
                };
                Recent {
                    key: format!("library-open-{index}"),
                    title: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    meta: [Some(length), age]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · "),
                    thumbnail: still.map(|(image, _)| image).unwrap_or_default(),
                }
            })
            .collect()
    }

    /// The More card's Recent row: the library's first three, each opening
    /// in the editor and bringing it forward (`recent-open-`).
    pub(super) fn recorder_recents(&self) -> Vec<Recent> {
        self.recents()
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(index, recent)| Recent {
                key: format!("recent-open-{index}"),
                ..recent
            })
            .collect()
    }

    /// Makes the still of each of `paths` not yet asked for, one worker for
    /// them all, and redraws the Recent cards as each arrives.
    pub(super) fn request_stills(&mut self, paths: Vec<PathBuf>) {
        let paths: Vec<_> = paths
            .into_iter()
            .filter(|path| !self.stills.contains_key(path))
            .collect();
        if paths.is_empty() {
            return;
        }
        for path in &paths {
            self.stills.insert(path.clone(), None);
        }
        std::thread::spawn(move || {
            for path in paths {
                let card = media::library_still(&path).and_then(|still| {
                    ui_runtime::Image::load_from_path(&still.still)
                        .map(|image| (image, still.duration))
                });
                let card = match card {
                    Ok(card) => card,
                    Err(error) => {
                        eprintln!("Library still for {}: {error:#}", path.display());
                        continue;
                    }
                };
                post(move |app, ui| {
                    app.stills.insert(path, Some(card));
                    if app.history.is_none() || ui.get_panel() == "Recent" {
                        ui.set_recents(ModelRc::new(VecModel::from(app.recents())));
                    }
                    if let Some(options) = &app.launcher_options {
                        options.set_recents(ModelRc::new(VecModel::from(app.recorder_recents())));
                    }
                });
            }
        });
    }

    pub(super) fn schedule_recovery(&self) {
        self.recovery_timer
            .start(TimerMode::SingleShot, Duration::from_secs(2), || {
                with_app(|app, _| {
                    if let Some(h) = &app.history {
                        if h.dirty() {
                            app.recovery.save(h.project.clone(), app.document.clone());
                        } else {
                            app.recovery.clear(app.recovery_origin.take());
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
        // Opened from the Projects view, the take comes up in the editor.
        if ui.get_panel() == "Recent" {
            ui.set_panel("Frame".into());
        }
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
            post(move |app, ui| {
                if epoch != app.epoch {
                    return;
                }
                match result {
                    Ok(info) => {
                        let mut project = project;
                        project.video_path = source.to_string_lossy().into();
                        // A new video starts from the default preset, before
                        // its own camera file is found, so that stays on.
                        let mut preset_warning = None;
                        if let (false, Some(name)) = (is_project, &app.preferences.default_preset) {
                            let applied = subtake_native::presets::directory()
                                .and_then(|d| {
                                    subtake_native::presets::load(&d.join(format!("{name}.json")))
                                })
                                .and_then(|data| {
                                    subtake_native::presets::apply(&mut project, &data)
                                });
                            if let Err(error) = applied {
                                preset_warning = Some(format!("Default preset {name}: {error:#}"));
                            }
                        }
                        if !is_project {
                            let webcam = source.with_extension("webcam.mp4");
                            if webcam.is_file() {
                                let mut settings =
                                    project.editor.get("webcam").cloned().unwrap_or(json!({}));
                                settings["enabled"] = json!(true);
                                settings["sourcePath"] = json!(webcam);
                                // Where the Camera card put it, over the
                                // default preset: the card is this
                                // recording's own choice.
                                recorder::recorder_webcam(
                                    &mut settings,
                                    &app.preferences,
                                    (info.width, info.height),
                                );
                                project.set("webcam", settings);
                            }
                        }
                        // Cursor positions are normalized; square and portrait captures
                        // use the same suggestion and camera pipeline as landscape ones.
                        let mut zoom_warning = preset_warning;
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
                            post(move |app, ui| {
                                if app.source.as_ref() != Some(&artwork_source) {
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
                                        if let Some(wave) = wave
                                            && let Ok(image) =
                                                ui_runtime::Image::load_from_path(&wave)
                                        {
                                            ui.set_waveform(image);
                                        }
                                    }
                                    Err(e) => eprintln!("Timeline artwork: {e:#}"),
                                }
                            });
                        });
                        app.source_revision = app.source_revision.wrapping_add(1);
                        app.history = Some(History::new(project));
                        if recovery_origin.is_some() {
                            app.history.as_mut().unwrap().mark_unsaved();
                        }
                        app.recovery_origin = recovery_origin;
                        app.schedule_recovery();
                        app.preferences
                            .opened(document.clone().unwrap_or_else(|| source.clone()));
                        if let Err(e) = app.preferences.save() {
                            eprintln!("Recent projects: {e}");
                        }
                        app.document = document;
                        app.proxy_cancel.store(true, Ordering::Relaxed);
                        app.proxy = None;
                        if info.width.max(info.height) > media::PROXY_EDGE {
                            let cancel = Arc::new(AtomicBool::new(false));
                            app.proxy_cancel = cancel.clone();
                            let (take, take_info) = (source.clone(), info.clone());
                            std::thread::spawn(move || {
                                let made = media::proxy(&take, &take_info, &cancel);
                                post(move |app, _| match made {
                                    Ok(proxy) if app.source.as_ref() == Some(&take) => {
                                        app.proxy = proxy
                                    }
                                    Ok(_) => {}
                                    // The preview goes on decoding the take itself.
                                    Err(e) if !cancel.load(Ordering::Relaxed) => {
                                        eprintln!("Preview proxy: {e:#}")
                                    }
                                    Err(_) => {}
                                });
                            });
                        }
                        app.source = Some(source);
                        app.info = Some(info);
                        app.source_time = 0.;
                        ui.invoke_reset_preview();
                        ui.set_timeline_zoom(1.);
                        ui.set_timeline_offset(0.);
                        app.selected = None;
                        app.extra_selection.clear();
                        if auto_zoom {
                            ui.set_panel("Frame".into());
                        }
                        app.refresh(ui);
                        app.request();
                        ui.set_status(zoom_warning.unwrap_or_else(|| "Ready".into()));
                        if let Err(error) = app.show_editor(ui) {
                            ui.set_status(format!("Open editor: {error:#}"));
                        }
                    }
                    Err(e) => ui.set_status(format!("Open failed: {e:#}")),
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

/// How long ago a file was touched, short enough to share a Recent tile's
/// mono line with its running time: "2m ago", "3h ago", "4d ago".
fn relative_age(modified: std::time::SystemTime) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    let seconds = modified.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    match seconds {
        s if s < MINUTE => "just now".into(),
        s if s < HOUR => format!("{}m ago", s / MINUTE),
        s if s < DAY => format!("{}h ago", s / HOUR),
        s if s < WEEK => format!("{}d ago", s / DAY),
        s if s < 2 * MONTH => format!("{}w ago", s / WEEK),
        s => format!("{}mo ago", s / MONTH),
    }
}

/// A take's length as a clock reads it: "1:24", or "1:02:03" past an hour.
fn running_time(seconds: f64) -> String {
    let total = seconds.round().max(0.) as u64;
    let (hours, minutes, seconds) = (total / 3_600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests;
