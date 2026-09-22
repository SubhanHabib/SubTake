//! Every named command the UI can fire, and the regions they add.

use super::*;

impl App {
    pub(super) fn action(&mut self, ui: &EditorWindow, action: &str) -> Result<()> {
        ensure!(
            !ui.get_busy()
                || matches!(
                    action,
                    "cancel" | "show" | "drag-window" | "drag-launcher" | "hide-launcher"
                ),
            "Wait for the current operation or cancel it first"
        );
        if let Some(index) = action.strip_prefix("apply-preset-") {
            let path = self
                .presets
                .get(index.parse::<usize>()?)
                .context("Preset no longer exists")?;
            let data = subtake_native::presets::load(path)?;
            return self.edit(ui, |p| subtake_native::presets::apply(p, &data));
        }
        if let Some(index) = action.strip_prefix("library-open-") {
            let path = self
                .library
                .get(index.parse::<usize>()?)
                .context("Library item no longer exists")?
                .clone();
            if self.can_replace(ui) {
                self.load(ui, path)?;
            }
            return Ok(());
        }
        if let Some(index) = action.strip_prefix("remove-preset-") {
            let path = self
                .presets
                .get(index.parse::<usize>()?)
                .context("Preset no longer exists")?;
            let retained = subtake_native::presets::remove(path)?;
            self.presets = subtake_native::presets::list();
            self.refresh(ui);
            ui.set_status(format!(
                "Preset removed. Recoverable copy: {}",
                retained.display()
            ));
            return Ok(());
        }
        match action {
            "visual-crop" | "finish-crop" => {
                self.stop(ui);
                ui.set_panel(
                    if action == "visual-crop" {
                        "Crop"
                    } else {
                        "Frame"
                    }
                    .into(),
                );
                self.refresh(ui);
                self.epoch += 1;
                self.request();
            }
            "choose-library" => {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose project and recording folder")
                    .pick_folder()
                {
                    self.preferences.library_directory = Some(folder);
                    self.preferences.save()?;
                    self.reload_library()?;
                    self.refresh(ui);
                }
            }
            "refresh-library" => {
                self.reload_library()?;
                self.refresh(ui);
            }

            "next-annotation" | "previous-annotation" => {
                let mut items = self
                    .project()?
                    .regions("annotationRegions")
                    .iter()
                    .filter(|r| {
                        n(r, "startMs", 0.) <= self.source_time * 1000.
                            && n(r, "endMs", 0.) > self.source_time * 1000.
                    })
                    .collect::<Vec<_>>();
                items.sort_by(|a, b| n(a, "zIndex", 0.).total_cmp(&n(b, "zIndex", 0.)));
                ensure!(!items.is_empty(), "No annotations at the playhead");
                let current = items
                    .iter()
                    .position(|r| self.selected.as_ref().is_some_and(|s| r["id"] == s.1));
                let index = match current {
                    Some(i) if action == "previous-annotation" => {
                        (i + items.len() - 1) % items.len()
                    }
                    Some(i) => (i + 1) % items.len(),
                    None => 0,
                };
                let id = items[index]["id"]
                    .as_str()
                    .context("Annotation id missing")?
                    .to_owned();
                self.extra_selection.clear();
                self.selected = Some(("annotationRegions".into(), id));
                ui.set_panel("Selection".into());
                self.refresh(ui);
                self.epoch += 1;
                self.request();
            }
            "look-studio" | "look-minimal" | "look-bold" => {
                self.edit(ui, |p| {
                    subtake_native::presets::appearance(p, action.trim_start_matches("look-"))
                })?;
            }
            "motion-focused" | "motion-smooth" => {
                self.edit(ui, |p| {
                    subtake_native::presets::motion(p, action == "motion-smooth");
                    Ok(())
                })?;
            }
            "save-preset" => {
                let dir = subtake_native::presets::directory()?;
                std::fs::create_dir_all(&dir)?;
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Save appearance preset")
                    .set_directory(dir)
                    .set_file_name("My preset.json")
                    .add_filter("SubTake preset", &["json"])
                    .save_file()
                {
                    subtake_native::presets::save(&path, self.project()?)?;
                    self.presets = subtake_native::presets::list();
                    self.refresh(ui);
                }
            }
            "load-preset" => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Load appearance preset")
                    .add_filter("SubTake preset", &["json"])
                    .pick_file()
                {
                    let data = subtake_native::presets::load(&path)?;
                    self.edit(ui, |p| subtake_native::presets::apply(p, &data))?;
                }
            }

            "add-marker" => {
                let time = self.source_time * 1000.;
                let mut id = None;
                self.edit(ui, |p| {
                    id = Some(p.add("nativeMarkers", json!({"startMs":time,"endMs":time+1.}))?);
                    Ok(())
                })?;
                self.extra_selection.clear();
                self.selected = id.map(|id| ("nativeMarkers".into(), id));
                self.refresh(ui);
            }
            "previous-marker" | "next-marker" => {
                let time = self.source_time * 1000.;
                let mut markers = self
                    .project()?
                    .regions("nativeMarkers")
                    .iter()
                    .map(|m| n(m, "startMs", 0.))
                    .collect::<Vec<_>>();
                markers.sort_by(f64::total_cmp);
                let next = if action == "previous-marker" {
                    markers.into_iter().rev().find(|&t| t < time - 1.)
                } else {
                    markers.into_iter().find(|&t| t > time + 1.)
                };
                if let Some(time) = next {
                    self.seek(ui, time / 1000.);
                }
            }
            "feedback" => {
                platform::open_feedback()?;
            }
            "wallpapers" => {
                self.wallpapers = media::wallpapers();
                ui.set_panel("Wallpapers".into());
                self.refresh(ui);
            }
            action if action.starts_with("wallpaper-") => {
                let index = action[10..].parse::<usize>()?;
                let path = self
                    .wallpapers
                    .get(index)
                    .context("Wallpaper is missing")?
                    .clone();
                self.edit(ui, |p| {
                    p.set(
                        "wallpaper",
                        json!(
                            path.strip_prefix(media::resources().join("public"))
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| path.to_string_lossy().to_string())
                        ),
                    );
                    Ok(())
                })?;
            }
            "shortcut-reference" => {
                let editable = subtake_native::shortcuts::ACTIONS
                    .iter()
                    .map(|(action, label, default)| {
                        format!(
                            "{}: {}",
                            label,
                            self.preferences
                                .editor_shortcuts
                                .get(*action)
                                .map(String::as_str)
                                .unwrap_or(default)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                rfd::MessageDialog::new().set_title("SubTake keyboard shortcuts").set_description(format!("Primary means Cmd on Mac and Ctrl on Windows.\n\nOpen: Primary+O\nSave: Primary+S\nSave as: Primary+Shift+S\nUndo / redo: Primary+Z / Shift+Z\nCopy / cut / paste: Primary+C / X / V\nDuplicate: Primary+Shift+D\nSplit clip: Primary+B\nDelete selected region: Delete\n\n{editable}\n\nGlobal record/stop: {}\nGlobal pause/resume: {}",self.preferences.record_shortcut,self.preferences.pause_shortcut)).set_buttons(rfd::MessageButtons::Ok).show();
            }
            action if action.starts_with("export-preset-") => {
                let info = self.info.as_ref().context("Open a video first")?;
                let mut settings = ExportSettings::for_media(self.project()?, info);
                let max = match &action[14..] {
                    "720" => 720.,
                    "1080" => 1080.,
                    _ => info.height as f64,
                };
                let aspect = aspect_ratio(self.project()?, info);
                let h = max.min(info.height as f64);
                settings.height = ((h / 2.).floor().max(1.) * 2.) as u32;
                settings.width = ((h * aspect / 2.).floor().max(1.) * 2.) as u32;
                self.edit(ui, |p| {
                    settings.store(p);
                    Ok(())
                })?;
            }
            action if action.starts_with("recovery-") => {
                let index = action[9..].parse::<usize>()?;
                let path = self
                    .recoveries
                    .get(index)
                    .context("Recovery no longer exists")?
                    .clone();
                if self.can_replace(ui) {
                    self.load(ui, path)?;
                }
            }
            action if action.starts_with("recent-") => {
                let index = action[7..].parse::<usize>()?;
                let path = self
                    .preferences
                    .recent_projects
                    .get(index)
                    .cloned()
                    .context("Recent project is missing")?;
                if self.can_replace(ui) {
                    self.load(ui, path)?;
                }
            }
            "download-model" => {
                ui.set_busy(true);
                ui.set_progress(0.);
                ui.set_status("Downloading Whisper Small for local captions…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    let result = subtake_native::models::download(&cancel, |progress| {
                        post(move |_, ui| ui.set_progress(progress))
                    });
                    post(move |app, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(path) => {
                                app.preferences.whisper_model = Some(path);
                                let result = app.preferences.save();
                                report(ui, result);
                                ui.set_status(
                                    "Whisper Small is ready. Transcription runs locally.".into(),
                                );
                            }
                            Err(e) => ui.set_status(format!("{e:#}")),
                        }
                    });
                });
            }
            "choose-model" => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose Whisper model")
                    .add_filter("Whisper model", &["bin"])
                    .pick_file()
                {
                    self.preferences.whisper_model = Some(path);
                    self.preferences.save()?;
                    ui.set_status("Caption model selected".into());
                }
            }
            "import-font" => {
                use base64::Engine;
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fonts", &["ttf", "otf", "ttc"])
                    .pick_file()
                else {
                    return Ok(());
                };
                let bytes = std::fs::read(path)?;
                ensure!(bytes.len() <= 16 * 1024 * 1024, "Font exceeds 16 MiB");
                let face = skia_safe::FontMgr::new()
                    .new_from_data(&bytes, None)
                    .context("Invalid font file")?;
                let family = face.family_name();
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                let captions = ui.get_panel() == "Captions";
                self.edit(ui, |p| {
                    let mut fonts = p
                        .editor
                        .get("nativeFonts")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    fonts.retain(|f| f["family"] != family);
                    fonts.push(json!({"family":family,"data":data}));
                    p.set("nativeFonts", json!(fonts));
                    if captions {
                        let mut settings = p
                            .editor
                            .get("autoCaptionSettings")
                            .cloned()
                            .unwrap_or(json!({}));
                        settings["fontFamily"] = json!(family);
                        p.set("autoCaptionSettings", settings);
                    }
                    Ok(())
                })?;
                ui.set_status(format!(
                    "Imported {family}; use this family in text annotations or captions."
                ));
            }
            "show" => self.show_launcher(ui)?,
            "show-editor" => self.show_editor(ui)?,
            "storyboard-spike" => {
                let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("spikes/storyboard/storyboard.py");
                ensure!(
                    script.is_file(),
                    "Storyboard spike requires the local checkout"
                );
                let weak = ui.as_weak();
                std::thread::spawn(move || {
                    let result = (|| -> anyhow::Result<String> {
                        let output = std::process::Command::new("python3")
                            .arg(script)
                            .args(["launch", "--no-open"])
                            .output()?;
                        ensure!(
                            output.status.success(),
                            "{}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                        Ok(result["url"]
                            .as_str()
                            .context("Missing workspace URL")?
                            .to_owned())
                    })();
                    let _ = ui_runtime::invoke_from_event_loop(move || {
                        let result = result.and_then(|url| platform::open_agent_workspace(&url));
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status(match result {
                                Ok(()) => "Agent video workspace opened".into(),
                                Err(error) => format!("Agent workspace: {error}"),
                            });
                        }
                    });
                });
                ui.set_status("Opening the agent video workspace…".into());
            }
            "projects" => {
                ui.set_panel("Recent".into());
                self.recoveries = subtake_native::recovery::list().unwrap_or_default();
                self.reload_library()?;
                self.refresh(ui);
                self.show_editor(ui)?;
            }
            "hide-launcher" => {
                if let Some(launcher) = &self.launcher {
                    launcher.hide()?;
                }
                if let Some(options) = &self.launcher_options {
                    options.hide()?;
                }
            }
            "drag-launcher" => {
                if let Some(launcher) = &self.launcher {
                    launcher.window().drag_window()?;
                }
            }
            "recording-folder" => {
                if let Some(directory) = rfd::FileDialog::new()
                    .set_directory(self.recording_directory()?)
                    .pick_folder()
                {
                    self.preferences.recording_directory = Some(directory);
                    self.preferences.save()?;
                }
            }
            "quit" => {
                ensure!(self.recording.is_none(), "Stop recording before quitting");
                if self.can_replace(ui) {
                    self.stop(ui);
                    self.job_cancel.store(true, Ordering::Relaxed);
                    ui_runtime::quit_event_loop()?;
                }
            }
            "open" => {
                if self.can_replace(ui)
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "Videos and projects",
                            &[
                                "mp4",
                                "mov",
                                "mkv",
                                "webm",
                                "avi",
                                "recordly",
                                "openscreen",
                                "json",
                            ],
                        )
                        .pick_file()
                {
                    self.load(ui, path)?;
                }
            }
            "save" => self.save(ui, false)?,
            "save-as" => self.save(ui, true)?,
            "undo" | "redo" => {
                self.stop(ui);
                if let Some(h) = &mut self.history {
                    if action == "undo" { h.undo() } else { h.redo() }
                }
                self.epoch += 1;
                self.schedule_recovery();
                self.refresh(ui);
                self.request();
            }
            "select-all" => {
                self.extra_selection.clear();
                for kind in [
                    "zoomRegions",
                    "trimRegions",
                    "speedRegions",
                    "clipRegions",
                    "annotationRegions",
                    "audioRegions",
                    "autoCaptions",
                    "nativeMarkers",
                ] {
                    let ids = self
                        .project()?
                        .regions(kind)
                        .iter()
                        .filter_map(|r| r["id"].as_str().map(|id| (kind.to_owned(), id.to_owned())))
                        .collect::<Vec<_>>();
                    self.extra_selection.extend(ids);
                }
                self.selected = self.extra_selection.last().cloned();
                self.refresh(ui);
            }
            "copy" | "cut" => {
                let keys = self.selected_keys();
                ensure!(!keys.is_empty(), "Select a region first");
                self.clipboard = keys
                    .into_iter()
                    .map(|(kind, id)| {
                        let p = self.project().unwrap();
                        let region = p
                            .regions(&kind)
                            .iter()
                            .find(|r| r["id"] == id)
                            .unwrap()
                            .clone();
                        let audio = (kind == "clipRegions")
                            .then(|| {
                                p.editor
                                    .get("sourceAudioTrackSettingsByClip")
                                    .and_then(|m| m.get(&id))
                                    .cloned()
                            })
                            .flatten();
                        (kind, id_to_null(region), audio)
                    })
                    .collect();
                if action == "cut" {
                    self.action(ui, "delete")?;
                }
            }
            "paste" | "duplicate" => {
                if action == "duplicate" {
                    self.action(ui, "copy")?;
                }
                ensure!(!self.clipboard.is_empty(), "Copy a region first");
                let copied = self.clipboard.clone();
                let earliest = copied
                    .iter()
                    .map(|(_, r, _)| n(r, "startMs", 0.))
                    .fold(f64::INFINITY, f64::min);
                let last = copied
                    .iter()
                    .map(|(kind, r, _)| subtake_native::editing::source_end(kind, r))
                    .fold(0., f64::max);
                let duration = self.info.as_ref().unwrap().duration * 1000.;
                ensure!(
                    last - earliest <= duration,
                    "Copied regions exceed the source duration"
                );
                let delta = (self.source_time * 1000.)
                    .min(duration - (last - earliest))
                    .max(0.)
                    - earliest;
                let mut selection = vec![];
                self.edit(ui, |p| {
                    for (kind, mut region, audio) in copied {
                        let a = n(&region, "startMs", 0.);
                        let b = n(&region, "endMs", 0.);
                        region["startMs"] = json!(a + delta);
                        region["endMs"] = json!(b + delta);
                        if kind == "autoCaptions" {
                            subtake_native::project::retime_caption_words(&mut region, a, b);
                        }
                        let id = p.add(&kind, region)?;
                        if let Some(audio) = audio {
                            let map = p
                                .editor
                                .entry("sourceAudioTrackSettingsByClip")
                                .or_insert_with(|| json!({}));
                            map[&id] = audio;
                        }
                        selection.push((kind, id));
                    }
                    Ok(())
                })?;
                self.selected = selection.last().cloned();
                self.extra_selection = selection;
                self.refresh(ui);
            }
            "split-caption" | "merge-caption" => {
                let (kind, id) = self.selected.clone().context("Select a caption first")?;
                ensure!(kind == "autoCaptions", "Select a caption first");
                let time = self.source_time * 1000.;
                self.edit(ui, |p| {
                    if action == "split-caption" {
                        subtake_native::caption_editing::split(p, &id, time)?;
                    } else {
                        subtake_native::caption_editing::merge_next(p, &id)?;
                    }
                    Ok(())
                })?;
            }
            "split-clip" => {
                let time = self.source_time * 1000.;
                let duration = self.info.as_ref().context("Open a video first")?.duration;
                self.edit(ui,|p|{
                    if p.regions("clipRegions").is_empty(){let clips=timeline::spans(p,duration).into_iter().map(|s|json!({"id":uuid::Uuid::new_v4().to_string(),"startMs":s.source_start*1000.,"endMs":(s.source_start+s.duration())*1000.,"speed":s.speed,"muted":s.muted})).collect::<Vec<_>>();p.set("clipRegions",json!(clips));}
                    let clip=p.regions("clipRegions").iter().find(|c|{let start=n(c,"startMs",0.);time>start+20.&&time<start+(n(c,"endMs",0.)-start)*n(c,"speed",1.)-20.}).context("Playhead must be inside a clip")?.clone();
                    let start=n(&clip,"startMs",0.);let speed=n(&clip,"speed",1.);let source_end=start+(n(&clip,"endMs",0.)-start)*speed;let id=clip["id"].as_str().unwrap();
                    p.change_region("clipRegions",id,json!({"endMs":start+(time-start)/speed}))?;
                    let mut right=clip.clone();right["id"]=Value::Null;right["startMs"]=json!(time);right["endMs"]=json!(time+(source_end-time)/speed);let new_id=p.add("clipRegions",right)?;
                    if let Some(map)=p.editor.get_mut("sourceAudioTrackSettingsByClip").and_then(Value::as_object_mut)&& let Some(settings)=map.get(id).cloned(){map.insert(new_id,settings);}
                    Ok(())
                })?;
            }
            "delete" => {
                let keys = self.selected_keys();
                ensure!(!keys.is_empty(), "Select a region first");
                self.edit(ui, |p| {
                    for (kind, id) in keys {
                        p.remove_region(&kind, &id)?;
                    }
                    Ok(())
                })?;
                self.selected = None;
                self.extra_selection.clear();
                self.refresh(ui);
            }
            "previous-frame" => self.seek(
                ui,
                self.source_time - 1. / self.info.as_ref().map(|i| i.fps).unwrap_or(30.),
            ),
            "next-frame" => self.seek(
                ui,
                self.source_time + 1. / self.info.as_ref().map(|i| i.fps).unwrap_or(30.),
            ),
            "play" => {
                if self.started.is_some() {
                    self.stop(ui)
                } else {
                    let p = self.project()?.clone();
                    let info = self.info.clone().unwrap();
                    let source = self.source.clone().unwrap();
                    let spans = timeline::spans(&p, info.duration);
                    let mut output = timeline::output_time(&spans, self.source_time);
                    if output >= timeline::duration(&spans) - 0.01 {
                        output = 0.;
                    }
                    self.audio_cancel = Arc::new(AtomicBool::new(false));
                    let cancel = self.audio_cancel.clone();
                    let epoch = self.epoch;
                    let clock = Arc::new(AtomicU64::new(0));
                    self.started = Some((clock.clone(), output));
                    std::thread::spawn(move || {
                        if let Err(e) =
                            export::play_audio(p, source, info, output, cancel.clone(), clock)
                        {
                            post(move |app, ui| {
                                if app.epoch == epoch
                                    && Arc::ptr_eq(&app.audio_cancel, &cancel)
                                    && !cancel.load(Ordering::Relaxed)
                                {
                                    app.stop(ui);
                                    ui.set_status(format!("Audio: {e:#}"))
                                }
                            })
                        }
                    });
                    ui.set_playing(true);
                    self.playback
                        .start(TimerMode::Repeated, Duration::from_millis(33), || {
                            with_app(|app, ui| {
                                if let (Some((start, at)), Some(info), Some(history)) =
                                    (app.started.clone(), &app.info, &app.history)
                                {
                                    let spans = timeline::spans(&history.project, info.duration);
                                    let output =
                                        at + start.load(Ordering::Relaxed) as f64 / 1_000_000.;
                                    if output >= timeline::duration(&spans) {
                                        app.stop(ui);
                                        return;
                                    }
                                    app.source_time = timeline::source_time(&spans, output);
                                    app.update_time(ui);
                                    app.request();
                                }
                            })
                        });
                }
            }
            "choose-background" | "choose-webcam" => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let value = json!(path.to_string_lossy());
                    self.edit(ui, |p| {
                        if action == "choose-background" {
                            p.set("wallpaper", value)
                        } else {
                            let mut webcam = p.editor.get("webcam").cloned().unwrap_or(json!({}));
                            webcam["sourcePath"] = value;
                            webcam["enabled"] = json!(true);
                            p.set("webcam", webcam);
                        }
                        Ok(())
                    })?;
                }
            }
            "import-captions" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Subtitles", &["srt"])
                    .pick_file()
                {
                    let cues = parse_srt(&std::fs::read_to_string(path)?)?;
                    self.edit(ui, |p| {
                        p.set("autoCaptions", json!(cues));
                        Ok(())
                    })?;
                }
            }
            "export" => {
                let p = self.project()?.clone();
                let source = self.source.clone().unwrap();
                let settings = ExportSettings::for_media(&p, self.info.as_ref().unwrap());
                let Some(path) = rfd::FileDialog::new()
                    .set_file_name(if settings.gif {
                        "SubTake.gif"
                    } else {
                        "SubTake.mp4"
                    })
                    .save_file()
                else {
                    return Ok(());
                };
                self.stop(ui);
                ui.set_busy(true);
                ui.set_progress(0.);
                ui.set_status("Exporting…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    let result = export::export(&p, &source, &settings, &path, &cancel, |value| {
                        post(move |_, ui| ui.set_progress(value))
                    });
                    post(move |app, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(()) => {
                                app.last_export = Some(path.clone());
                                ui.set_status(format!("Exported {}", path.display()));
                            }
                            Err(e) => ui.set_status(format!("{e:#}")),
                        }
                    });
                });
            }
            "cancel" => {
                self.job_cancel.store(true, Ordering::Relaxed);
                ui.set_status("Cancelling…".into());
            }
            "reveal-export" => platform::reveal(
                self.last_export
                    .as_deref()
                    .context("Export a video first")?,
            )?,
            "devices" => {
                std::thread::spawn(|| {
                    let result = platform::devices();
                    post(move |app, ui| match result {
                        Ok(devices) => {
                            let names = |key: &str| {
                                let mut names = vec![SharedString::from("System default")];
                                if let Some(list) = devices[key].as_array() {
                                    names.extend(list.iter().map(|v| {
                                        SharedString::from(v["name"].as_str().unwrap_or("Device"))
                                    }));
                                }
                                ModelRc::new(VecModel::from(names))
                            };
                            ui.set_camera_names(names("cameras"));
                            ui.set_microphone_names(names("microphones"));
                            app.devices = devices;
                        }
                        Err(e) => ui.set_status(format!("Devices: {e:#}")),
                    });
                });
            }
            "sources" | "sources-passive" => {
                self.action(ui, "devices")?;
                ui.set_busy(true);
                ui.set_sources_loading(true);
                ui.set_recording_hint("Finding displays and windows…".into());
                ui.set_status("Finding displays and windows…".into());
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                let request_access = action == "sources";
                std::thread::spawn(move || {
                    let result = platform::sources_cancellable(&cancel, request_access);
                    post(move |app, ui| {
                        if !Arc::ptr_eq(&app.job_cancel, &cancel) {
                            return;
                        }
                        app.finish_sources(ui, result, cancel.load(Ordering::Relaxed));
                    });
                });
            }
            "record" => {
                self.stop(ui);
                ui.hide()?;
                platform::set_editor_active(false);
                ui.set_panel("Recording".into());
                self.refresh(ui);
                self.show_launcher(ui)?;
                if !ui.get_busy() {
                    ui.set_status("Choose a source, then press the red Record button.".into());
                }
            }
            "drag-window" => {
                ui.window().drag_window()?;
            }
            "start-recording" => {
                ensure!(self.recording.is_none(), "A recording is already running");
                ensure!(self.can_replace(ui), "Recording cancelled");
                let mut source = self
                    .sources
                    .get(ui.get_source_index().max(0) as usize)
                    .context("Choose a recording source")?
                    .clone();
                let directory = self.recording_directory()?;
                std::fs::create_dir_all(&directory).context("Create recordings folder")?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                let path = directory.join(format!(
                    "Recording-{stamp}-{}.mp4",
                    &uuid::Uuid::new_v4().to_string()[..8]
                ));
                source["cameraId"] = self.devices["cameras"]
                    .as_array()
                    .and_then(|a| a.get((ui.get_camera_index() - 1) as usize))
                    .map(|v| v["id"].clone())
                    .unwrap_or(Value::Null);
                source["microphoneId"] = self.devices["microphones"]
                    .as_array()
                    .and_then(|a| a.get((ui.get_microphone_index() - 1) as usize))
                    .map(|v| v["id"].clone())
                    .unwrap_or(Value::Null);
                let camera = ui.get_capture_camera();
                let mic = ui.get_capture_mic();
                let system = ui.get_capture_system();
                self.stop(ui);
                self.show_launcher(ui)?;
                ui.hide()?;
                platform::set_editor_active(false);
                self.set_launcher_options_panel(ui, "")?;
                self.capture_started = None;
                self.pause_started = None;
                self.paused_total = Duration::ZERO;
                ui.set_recording_paused(false);
                ui.set_busy(true);
                ui.set_status("Preparing recording…".into());
                let countdown = self.preferences.countdown_seconds;
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                std::thread::spawn(move || {
                    for remaining in (1..=countdown).rev() {
                        post(move |_, ui| {
                            ui.set_status(format!("Recording starts in {remaining}…"))
                        });
                        for _ in 0..20 {
                            if cancel.load(Ordering::Relaxed) {
                                post(|_, ui| {
                                    ui.set_busy(false);
                                    ui.set_status("Recording cancelled".into());
                                });
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                    let result = Recording::start(&source, path, mic, system, camera, &cancel);
                    post(move |app, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(recording) => {
                                app.recording = Some(recording);
                                ui.set_recording(true);
                                app.capture_started = Some(std::time::Instant::now());
                                app.recording_watch.start(
                                    TimerMode::Repeated,
                                    Duration::from_millis(250),
                                    || {
                                        with_app(|app, ui| {
                                            if let Some(error) =
                                                app.recording.as_mut().and_then(Recording::error)
                                            {
                                                if let Some(recording) = app.recording.take() {
                                                    std::thread::spawn(move || {
                                                        if let Err(e) = recording.stop() {
                                                            post(move |_, ui| {
                                                                ui.set_status(format!("{e:#}"))
                                                            });
                                                        }
                                                    });
                                                }
                                                app.recording_watch.stop();
                                                ui.set_recording(false);
                                                ui.set_status(error);
                                            }
                                        })
                                    },
                                );
                                ui.set_status(
                                    "Recording · use Stop recording to open the editor".into(),
                                );
                            }
                            Err(e) => ui.set_status(format!("{e:#}")),
                        }
                    });
                });
            }
            "pause-recording" => {
                if let Some(mut recording) = self.recording.take() {
                    ui.set_busy(true);
                    ui.set_status("Updating recording…".into());
                    std::thread::spawn(move || {
                        let result = recording.pause();
                        post(move |app, ui| {
                            ui.set_busy(false);
                            ui.set_recording_paused(recording.paused);
                            if recording.paused {
                                app.pause_started
                                    .get_or_insert_with(std::time::Instant::now);
                            } else if let Some(start) = app.pause_started.take() {
                                app.paused_total += start.elapsed();
                            }
                            app.recording = Some(recording);
                            match result {
                                Ok(()) => ui.set_status(
                                    if ui.get_recording_paused() {
                                        "Recording paused"
                                    } else {
                                        "Recording resumed"
                                    }
                                    .into(),
                                ),
                                Err(e) => ui.set_status(format!("{e:#}")),
                            }
                        });
                    });
                }
            }
            "stop-recording" => {
                if let Some(recording) = self.recording.take() {
                    self.recording_watch.stop();
                    ui.set_recording(false);
                    ui.set_busy(true);
                    ui.set_status("Finalizing recording…".into());
                    std::thread::spawn(move || {
                        let result = recording.stop();
                        post(move |app, ui| {
                            ui.set_busy(false);
                            match result {
                                Ok(path) => {
                                    app.fresh_recording = Some(path.clone());
                                    let r = app.load(ui, path);
                                    report(ui, r);
                                }
                                Err(e) => ui.set_status(format!("{e:#}")),
                            }
                        });
                    });
                }
            }
            "auto-zoom" => {
                let source = self.source.as_ref().context("Open a video first")?;
                let duration = self.info.as_ref().unwrap().duration * 1000.;
                let zooms =
                    subtake_native::autozoom::from_source(self.project()?, source, duration)?;
                ensure!(
                    !zooms.is_empty(),
                    "No unused click clusters are available for zoom suggestions"
                );
                self.edit(ui, |p| {
                    for z in zooms {
                        p.add("zoomRegions", z)?;
                    }
                    Ok(())
                })?;
            }
            "transcribe" => {
                self.stop(ui);
                let runtime = match self
                    .preferences
                    .whisper_runtime
                    .clone()
                    .filter(|p| p.is_file())
                    .map(Ok)
                    .unwrap_or_else(|| media::binary("whisper-cli"))
                {
                    Ok(path) => path,
                    Err(_) => {
                        let Some(path) = rfd::FileDialog::new()
                            .set_title("Choose whisper-cli")
                            .pick_file()
                        else {
                            return Ok(());
                        };
                        path
                    }
                };
                let model = if let Some(path) = self
                    .preferences
                    .whisper_model
                    .clone()
                    .filter(|p| p.is_file())
                {
                    path
                } else {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose Whisper model (.bin)")
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    path
                };
                self.preferences.whisper_runtime = Some(runtime.clone());
                self.preferences.whisper_model = Some(model.clone());
                self.preferences.save()?;
                let source = self.source.clone().context("Open a video first")?;
                let epoch = self.epoch;
                self.job_cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.job_cancel.clone();
                ui.set_busy(true);
                let language = self
                    .project()?
                    .text("nativeCaptionLanguage", "auto")
                    .to_owned();
                ui.set_status("Transcribing locally…".into());
                std::thread::spawn(move || {
                    let result = subtake_native::transcription::transcribe(
                        &source,
                        &runtime,
                        &model,
                        &language,
                        &cancel,
                        |message| {
                            let message = message.to_owned();
                            post(move |_, ui| ui.set_status(message));
                        },
                    );
                    post(move |app, ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(cues) if app.epoch == epoch => {
                                let result = app.edit(ui, |p| {
                                    p.set("autoCaptions", json!(cues));
                                    Ok(())
                                });
                                report(ui, result);
                            }
                            Ok(_) => ui.set_status(
                                "Document changed during transcription; captions were not applied."
                                    .into(),
                            ),
                            Err(e) => ui.set_status(format!("{e:#}")),
                        }
                    });
                });
            }
            action if action.starts_with("add-") => self.add_region(ui, action)?,
            _ => (),
        }
        Ok(())
    }

    pub(super) fn add_region(&mut self, ui: &EditorWindow, action: &str) -> Result<()> {
        let duration = self.info.as_ref().context("Open a video first")?.duration * 1000.;
        let start = (self.source_time * 1000.).min((duration - 100.).max(0.));
        let end = (start + 2000.).min(duration);
        let (key, mut region) = match action {
            "add-zoom" => (
                "zoomRegions",
                json!({"depth":3,"focus":{"cx":0.5,"cy":0.5},"mode":"manual"}),
            ),
            "add-trim" => ("trimRegions", json!({})),
            "add-speed" => ("speedRegions", json!({"speed":1.5})),
            "add-caption" => ("autoCaptions", json!({"text":"Your caption"})),
            "add-audio" => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio", &["wav", "mp3", "m4a", "aac", "flac"])
                    .pick_file()
                else {
                    return Ok(());
                };
                ("audioRegions", json!({"audioPath":path,"volume":1.}))
            }
            _ => {
                let kind = action.trim_start_matches("add-");
                let mut r = json!({"type":kind,"content":"Your text","textContent":"Your text","position":{"x":35.,"y":40.},"size":{"width":30.,"height":20.},"style":{"fontSize":64.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"transparent","textAlign":"center","borderRadius":8},"figureData":{"arrowDirection":"right","color":"#2563eb","strokeWidth":5},"zIndex":1,"blurIntensity":20});
                if kind == "image" {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    r["imageContent"] = json!(path);
                }
                ("annotationRegions", r)
            }
        };
        region["startMs"] = json!(start);
        region["endMs"] = json!(end);
        let mut selected = None;
        self.edit(ui, |p| {
            selected = Some(p.add(key, region)?);
            Ok(())
        })?;
        self.extra_selection.clear();
        self.selected = selected.map(|id| (key.into(), id));
        ui.set_panel("Selection".into());
        self.refresh(ui);
        Ok(())
    }
}
