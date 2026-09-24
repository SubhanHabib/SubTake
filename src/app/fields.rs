//! Inspector fields: building each panel's rows from the project and
//! applying an edited value back.

use super::*;

impl App {
    pub(super) fn fields(&self, panel: &str) -> Vec<Field> {
        let mut raw = self.raw_fields(if panel == "Wallpapers" {
            "Frame"
        } else {
            panel
        });
        if panel == "Frame" {
            let padding = self
                .history
                .as_ref()
                .and_then(|h| h.project.editor.get("padding"))
                .cloned()
                .unwrap_or(json!(20));
            let linked = padding
                .get("linked")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            for field in &mut raw {
                if let Some(side) = field.key.strip_prefix("padding.") {
                    field.value = padding
                        .as_f64()
                        .unwrap_or_else(|| n(&padding, side, 20.))
                        .to_string();
                }
            }
            if linked {
                raw.retain(|f| !f.key.starts_with("padding.") || f.key == "padding.top");
                if let Some(f) = raw.iter_mut().find(|f| f.key == "padding.top") {
                    f.key = "padding.all".into();
                    f.label = "Padding".into();
                }
            }
            raw.push(Field {
                key: "padding.linked".into(),
                label: "Link all sides".into(),
                kind: 2,
                value: linked.to_string(),
                ..Default::default()
            });
        }
        crate::inspector::present(raw, panel, &self.preferences.language)
    }

    /// The folders and model file the app keeps, each a button whose
    /// action picks another and whose value is where it is now, or empty.
    fn folder_fields(&self) -> Vec<Field> {
        let path =
            |p: Option<&std::path::Path>| p.map(|p| p.display().to_string()).unwrap_or_default();
        [
            (
                "choose-library",
                "Project folder",
                path(self.preferences.library_directory.as_deref()),
            ),
            (
                "recording-folder",
                "Recordings folder",
                path(self.recording_directory().ok().as_deref()),
            ),
            (
                "choose-model",
                "Transcription model",
                path(self.preferences.whisper_model.as_deref()),
            ),
        ]
        .into_iter()
        .map(|(key, label, value)| Field {
            key: key.into(),
            label: label.into(),
            value,
            kind: 3,
            ..Default::default()
        })
        .collect()
    }

    pub(super) fn raw_fields(&self, panel: &str) -> Vec<Field> {
        let p = self.history.as_ref().map(|h| &h.project);
        let settings = p
            .map(|p| {
                self.info
                    .as_ref()
                    .map(|i| ExportSettings::for_media(p, i))
                    .unwrap_or_else(|| ExportSettings::from_project(p))
            })
            .unwrap_or_default();
        let mut fields = vec![];
        if panel == "Settings" {
            return [
                (
                    "prefs.language",
                    "Language (en, es, fr, de, it, nl, ko, pt-BR, zh-CN, zh-TW)",
                    json!(self.preferences.language),
                ),
                (
                    "prefs.record_shortcut",
                    "Record / stop global shortcut",
                    json!(self.preferences.record_shortcut),
                ),
                (
                    "prefs.pause_shortcut",
                    "Pause / resume global shortcut",
                    json!(self.preferences.pause_shortcut),
                ),
                (
                    "prefs.countdown_seconds",
                    "Recording countdown (seconds)",
                    json!(self.preferences.countdown_seconds),
                ),
            ]
            .into_iter()
            .map(|(key, label, value)| Field {
                key: key.into(),
                label: label.into(),
                value: value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string()),
                kind: 0,
                minimum: 0.,
                maximum: 0.,
                ..Default::default()
            })
            .chain(subtake_native::shortcuts::ACTIONS.into_iter().map(
                |(action, label, default)| {
                    Field {
                        key: format!("shortcut.{action}"),
                        label: label.into(),
                        value: self
                            .preferences
                            .editor_shortcuts
                            .get(action)
                            .map(String::as_str)
                            .unwrap_or(default)
                            .into(),
                        kind: 0,
                        minimum: 0.,
                        maximum: 0.,
                        ..Default::default()
                    }
                },
            ))
            .chain(self.folder_fields())
            .collect();
        }
        if panel == "Wallpapers" {
            return self
                .wallpapers
                .iter()
                .enumerate()
                .map(|(i, path)| Field {
                    key: format!("wallpaper-{i}"),
                    label: "Built-in wallpaper".into(),
                    value: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                })
                .collect();
        }
        if panel == "Recent" {
            return vec![
                Field {
                    key: "choose-library".into(),
                    label: "Project folder".into(),
                    value: self
                        .preferences
                        .library_directory
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Choose folder…".into()),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
                Field {
                    key: "library.query".into(),
                    label: "Search projects and recordings".into(),
                    value: self.library_query.as_str().into(),
                    kind: 0,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
                Field {
                    key: "refresh-library".into(),
                    label: "".into(),
                    value: "Refresh".into(),
                    kind: 3,
                    minimum: 0.,
                    maximum: 0.,
                    ..Default::default()
                },
            ]
            .into_iter()
            .chain(
                self.recoveries
                    .iter()
                    .enumerate()
                    .map(|(i, path)| Field {
                        key: format!("recovery-{i}"),
                        label: "Unsaved project recovery".into(),
                        value: Project::load(path)
                            .ok()
                            .map(|p| {
                                format!(
                                    "Recover {}",
                                    Path::new(&p.video_path)
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                )
                            })
                            .unwrap_or_else(|| "Recovery file".into()),
                        kind: 3,
                        minimum: 0.,
                        maximum: 0.,
                        ..Default::default()
                    })
                    .chain(self.library.iter().enumerate().map(|(i, path)| {
                        Field {
                            key: format!("library-open-{i}"),
                            label: path.parent().unwrap_or(Path::new("")).display().to_string(),
                            value: path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            kind: 3,
                            minimum: 0.,
                            maximum: 0.,
                            ..Default::default()
                        }
                    })),
            )
            .collect();
        }
        let mut add = |key: &str, label: &str, kind: i32, min: f32, max: f32, default: Value| {
            let value = if key.starts_with("export.") {
                match key {
                    "export.width" => json!(settings.width),
                    "export.height" => json!(settings.height),
                    "export.fps" => json!(settings.fps),
                    "export.gif" => json!(settings.gif),
                    "export.loop" => json!(settings.gif_loop),
                    "export.hardware" => json!(settings.hardware),
                    _ => json!(settings.quality),
                }
            } else if let Some(rest) = key.strip_prefix("region.") {
                self.selected
                    .as_ref()
                    .and_then(|(kind, id)| p?.regions(kind).iter().find(|r| r["id"] == *id))
                    .map(|r| get_nested(r, rest))
                    .filter(|v| !v.is_null())
                    .unwrap_or(default)
            } else {
                p.and_then(|p| {
                    let (a, b) = key.split_once('.').unwrap_or((key, ""));
                    p.editor.get(a).map(|v| {
                        if b.is_empty() {
                            v.clone()
                        } else {
                            get_nested(v, b)
                        }
                    })
                })
                .filter(|v| !v.is_null())
                .unwrap_or(default)
            };
            let value = if let Some(s) = value.as_str() {
                s.into()
            } else {
                value.to_string()
            };
            fields.push(Field {
                key: key.into(),
                label: label.into(),
                value,
                kind,
                minimum: min,
                maximum: max,
                ..Default::default()
            });
        };
        match panel {
            "Crop" => {
                add("finish-crop", "Crop editing", 3, 0., 0., json!("Done"));
                for key in ["x", "y", "width", "height"] {
                    add(
                        &format!("cropRegion.{key}"),
                        key,
                        1,
                        0.,
                        1.,
                        json!(if key == "width" || key == "height" {
                            1.
                        } else {
                            0.
                        }),
                    );
                }
            }
            "Frame" => {
                add(
                    "visual-crop",
                    "Crop",
                    3,
                    0.,
                    0.,
                    json!("Edit crop visually"),
                );
                add(
                    "save-preset",
                    "Appearance presets",
                    3,
                    0.,
                    0.,
                    json!("Save current appearance…"),
                );
                add("load-preset", "", 3, 0., 0., json!("Load preset file…"));
                for (index, path) in self.presets.iter().enumerate() {
                    add(
                        &format!("apply-preset-{index}"),
                        "Saved preset",
                        3,
                        0.,
                        0.,
                        json!(path.file_stem().unwrap_or_default().to_string_lossy()),
                    );
                    add(
                        &format!("remove-preset-{index}"),
                        "",
                        3,
                        0.,
                        0.,
                        json!("Delete preset"),
                    );
                }
                add(
                    "wallpapers",
                    "Built-in backgrounds",
                    3,
                    0.,
                    0.,
                    json!("Browse wallpapers…"),
                );
                add("zoomSmoothness", "Camera smoothness", 1, 0., 1., json!(0.5));
                add(
                    "connectZooms",
                    "Connect nearby zooms",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add(
                    "zoomClassicMode",
                    "Classic zoom motion",
                    2,
                    0.,
                    0.,
                    json!(false),
                );
                add(
                    "wallpaper",
                    "Background color, gradient or media path",
                    0,
                    0.,
                    0.,
                    json!(subtake_native::project::DEFAULT_WALLPAPER),
                );
                add(
                    "choose-background",
                    "Background image or video",
                    3,
                    0.,
                    0.,
                    json!("Choose media…"),
                );
                add(
                    "aspectRatio",
                    "Aspect ratio (16:9, 9:16, 1:1)",
                    0,
                    0.,
                    0.,
                    json!("16:9"),
                );
                for side in ["top", "bottom", "left", "right"] {
                    add(
                        &format!("padding.{side}"),
                        &format!("Padding · {side}"),
                        1,
                        0.,
                        250.,
                        json!(56),
                    );
                }
                add("borderRadius", "Rounded corners", 1, 0., 100., json!(4));
                add("shadowIntensity", "Shadow", 1, 0., 1., json!(0.5));
                add("backgroundBlur", "Background blur", 1, 0., 100., json!(0));
                for (key, default) in [("x", 0.), ("y", 0.), ("width", 1.), ("height", 1.)] {
                    add(
                        &format!("cropRegion.{key}"),
                        &format!("Crop · {key}"),
                        1,
                        0.,
                        1.,
                        json!(default),
                    );
                }
            }
            "Cursor" => {
                add(
                    "motion-focused",
                    "Motion presets",
                    3,
                    0.,
                    0.,
                    json!("Focused"),
                );
                add("motion-smooth", "", 3, 0., 0., json!("Smooth"));
                add(
                    "zoomMotionBlur",
                    "Camera motion blur",
                    1,
                    0.,
                    2.,
                    json!(0.35),
                );
                add("showCursor", "Rendered cursor", 2, 0., 0., json!(true));
                add(
                    "cursorStyle",
                    "Style (tahoe, macos, windows11, dot, figma)",
                    0,
                    0.,
                    0.,
                    json!("tahoe"),
                );
                add("cursorSize", "Size", 1, 0.5, 8., json!(3));
                add("cursorSmoothing", "Smoothing", 1, 0., 2., json!(0.67));
                add("cursorSway", "Sway", 1, 0., 2., json!(0.4));
                add("cursorMotionBlur", "Motion blur", 1, 0., 2., json!(0.6));
                add(
                    "cursorClickEffectScale",
                    "Click effect size",
                    1,
                    0.25,
                    4.,
                    json!(1),
                );
                add(
                    "cursorClickEffectOpacity",
                    "Click effect opacity",
                    1,
                    0.,
                    1.,
                    json!(1),
                );
                add(
                    "cursorClickEffectDurationMs",
                    "Click effect duration (ms)",
                    1,
                    60.,
                    1200.,
                    json!(600),
                );
                add("loopCursor", "Loop cursor", 2, 0., 0., json!(false));
                add("cursorClickBounce", "Click bounce", 1, 0., 4., json!(2));
                add(
                    "cursorClickBounceDuration",
                    "Bounce duration (ms)",
                    1,
                    50.,
                    1000.,
                    json!(350),
                );
                add(
                    "cursorClickEffect",
                    "Click effect (none, ripple, spotlight, echo)",
                    0,
                    0.,
                    0.,
                    json!("ripple"),
                );
                add(
                    "cursorClickEffectColor",
                    "Click effect color",
                    0,
                    0.,
                    0.,
                    json!("#2563eb"),
                );
            }
            "Webcam" => {
                add(
                    "choose-webcam",
                    "Webcam footage",
                    3,
                    0.,
                    0.,
                    json!("Choose video…"),
                );
                add("webcam.enabled", "Show webcam", 2, 0., 0., json!(false));
                add("webcam.mirror", "Mirror", 2, 0., 0., json!(true));
                for key in ["width", "height"] {
                    add(&format!("webcam.{key}"), key, 1, 5., 100., json!(40));
                }
                for key in ["positionX", "positionY"] {
                    add(&format!("webcam.{key}"), key, 1, 0., 1., json!(1));
                }
                add(
                    "webcam.positionPreset",
                    "Position preset (custom, bottom-right…)",
                    0,
                    0.,
                    0.,
                    json!("custom"),
                );
                add(
                    "webcam.reactToZoom",
                    "React to zoom",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add("webcam.shadow", "Shadow", 1, 0., 1., json!(0.3));
                for key in ["x", "y", "width", "height"] {
                    add(
                        &format!("webcam.cropRegion.{key}"),
                        &format!("Crop · {key}"),
                        1,
                        0.,
                        1.,
                        json!(if key == "width" || key == "height" {
                            1.
                        } else {
                            0.
                        }),
                    );
                }
                add("webcam.roundness", "Roundness", 1, 0., 100., json!(100));
                add("webcam.margin", "Margin", 1, 0., 150., json!(24));
                add(
                    "webcam.timeOffsetMs",
                    "Time offset (ms)",
                    0,
                    0.,
                    0.,
                    json!(0),
                );
            }
            "Captions" => {
                add(
                    "nativeCaptionLanguage",
                    "Speech language (auto, en, fr, …)",
                    0,
                    0.,
                    0.,
                    json!("auto"),
                );
                add(
                    "download-model",
                    "Local caption model",
                    3,
                    0.,
                    0.,
                    json!("Download Whisper Small"),
                );
                add(
                    "choose-model",
                    "Use an existing model",
                    3,
                    0.,
                    0.,
                    json!("Choose model…"),
                );
                add(
                    "import-font",
                    "Custom font",
                    3,
                    0.,
                    0.,
                    json!("Import font…"),
                );
                add(
                    "autoCaptionSettings.fontFamily",
                    "Font family",
                    0,
                    0.,
                    0.,
                    json!("Helvetica"),
                );
                add(
                    "autoCaptionSettings.animationStyle",
                    "Animation (none, fade, rise, pop)",
                    0,
                    0.,
                    0.,
                    json!("fade"),
                );
                add(
                    "autoCaptionSettings.enabled",
                    "Show captions",
                    2,
                    0.,
                    0.,
                    json!(true),
                );
                add(
                    "autoCaptionSettings.fontSize",
                    "Font size",
                    1,
                    12.,
                    120.,
                    json!(30),
                );
                add(
                    "autoCaptionSettings.bottomOffset",
                    "Bottom offset (%)",
                    1,
                    0.,
                    45.,
                    json!(3),
                );
                add(
                    "autoCaptionSettings.maxWidth",
                    "Maximum width (%)",
                    1,
                    20.,
                    100.,
                    json!(62),
                );
                add(
                    "autoCaptionSettings.maxRows",
                    "Maximum rows",
                    1,
                    1.,
                    6.,
                    json!(2),
                );
                add(
                    "autoCaptionSettings.textColor",
                    "Text color",
                    0,
                    0.,
                    0.,
                    json!("#ffffff"),
                );
                add(
                    "autoCaptionSettings.backgroundOpacity",
                    "Box opacity",
                    1,
                    0.,
                    1.,
                    json!(0.9),
                );
            }
            "Selection" => {
                if let Some((kind, _)) = &self.selected {
                    add("region.startMs", "Start (ms)", 0, 0., 0., json!(0));
                    add("region.endMs", "End (ms)", 0, 0., 0., json!(1000));
                    match kind.as_str() {
                        "zoomRegions" => {
                            add(
                                "region.mode",
                                "Focus tracking (manual or auto)",
                                0,
                                0.,
                                0.,
                                json!("manual"),
                            );
                            add("region.depth", "Zoom depth", 1, 1., 6., json!(3));
                            add("region.focus.cx", "Focus X", 1, 0., 1., json!(0.5));
                            add("region.focus.cy", "Focus Y", 1, 0., 1., json!(0.5));
                        }
                        "speedRegions" | "clipRegions" => {
                            if kind == "clipRegions" {
                                add("region.muted", "Mute clip", 2, 0., 0., json!(false));
                            }
                            add("region.speed", "Playback speed", 1, 0.25, 4., json!(1.5));
                        }
                        "autoCaptions" => {
                            add("region.text", "Caption", 0, 0., 0., json!(""));
                            add(
                                "split-caption",
                                "Phrase editing",
                                3,
                                0.,
                                0.,
                                json!("Split near playhead"),
                            );
                            add(
                                "merge-caption",
                                "",
                                3,
                                0.,
                                0.,
                                json!("Merge with following caption"),
                            );
                            if let Some(cue) = p.and_then(|p| {
                                p.regions("autoCaptions").iter().find(|c| {
                                    self.selected.as_ref().is_some_and(|s| c["id"] == s.1)
                                })
                            }) {
                                for (index, word) in
                                    subtake_native::caption_editing::normalized_words(cue)
                                        .iter()
                                        .enumerate()
                                {
                                    for (key, label) in [
                                        ("text", "Text"),
                                        ("startMs", "Start (ms)"),
                                        ("endMs", "End (ms)"),
                                    ] {
                                        add(
                                            &format!("word.{index}.{key}"),
                                            &format!("Word {} · {label}", index + 1),
                                            0,
                                            0.,
                                            0.,
                                            word[key].clone(),
                                        );
                                    }
                                }
                            }
                        }
                        "audioRegions" => {
                            add("region.volume", "Volume", 1, 0., 3., json!(1));
                            add(
                                "region.normalize",
                                "Normalize audio",
                                2,
                                0.,
                                0.,
                                json!(false),
                            );
                        }
                        "annotationRegions" => {
                            let annotation = self
                                .selected
                                .as_ref()
                                .and_then(|(kind, id)| {
                                    p?.regions(kind).iter().find(|r| r["id"] == *id)
                                })
                                .cloned()
                                .unwrap_or_default();
                            for row in subtake_native::annotations::rows(&annotation) {
                                let (min, max) = row.range;
                                add(&row.key, row.label, row.kind, min, max, row.value);
                            }
                        }
                        _ => (),
                    }
                }
            }
            "Audio" => {
                for track in ["system", "mic", "mixed"] {
                    add(
                        &format!("defaultSourceAudioTrackSettings.{track}.volume"),
                        &format!("{track} volume"),
                        1,
                        0.,
                        3.,
                        json!(1),
                    );
                    add(
                        &format!("defaultSourceAudioTrackSettings.{track}.normalize"),
                        &format!("{track} normalization"),
                        2,
                        0.,
                        0.,
                        json!(false),
                    );
                }
            }
            _ => (),
        }
        if panel == "Export" {
            fields.extend(self.export_fields(&settings));
        }
        fields
    }

    /// The Export panel's rows. The panel is drawn to the handoff rather
    /// than stacked from these, so each key is one thing it looks up.
    fn export_fields(&self, settings: &ExportSettings) -> Vec<Field> {
        let choice = |key: &str, options: &[(&str, &str)], value: String| {
            let mut options: Vec<(String, String)> = options
                .iter()
                .map(|(v, l)| (v.to_string(), l.to_string()))
                .collect();
            if !options.iter().any(|(v, _)| *v == value) {
                options.push((value.clone(), value.clone()));
            }
            Field {
                key: key.into(),
                choice: options.iter().position(|(v, _)| *v == value).unwrap_or(0) as i32,
                choices: ModelRc::new(VecModel::from(
                    options
                        .iter()
                        .map(|(_, l)| SharedString::from(l.as_str()))
                        .collect::<Vec<_>>(),
                )),
                values: ModelRc::new(VecModel::from(
                    options
                        .iter()
                        .map(|(v, _)| SharedString::from(v.as_str()))
                        .collect::<Vec<_>>(),
                )),
                value: value.into(),
                kind: 4,
                ..Default::default()
            }
        };
        let plain = |key: &str, kind: i32, value: String| Field {
            key: key.into(),
            value: value.into(),
            kind,
            ..Default::default()
        };
        let source_height = self.info.as_ref().map_or(0, |i| i.height);
        let resolution = match settings.height {
            720 => "720".to_owned(),
            1080 => "1080".to_owned(),
            h if h == source_height => "source".to_owned(),
            h => format!("{} × {h}", settings.width),
        };
        let format = if self.export_frame {
            "frame"
        } else if settings.gif {
            "gif"
        } else {
            "video"
        };
        let seconds = self.info.as_ref().map_or(0., |i| i.duration);
        let bytes = if self.export_frame {
            export::estimate_frame_bytes(settings)
        } else {
            export::estimate_bytes(settings, seconds)
        };
        vec![
            plain("export.format", 0, format.into()),
            choice(
                "export.resolution",
                &[("720", "720p"), ("1080", "1080p"), ("source", "Source")],
                resolution,
            ),
            choice(
                "export.fps",
                &[("24", "24 fps"), ("30", "30 fps"), ("60", "60 fps")],
                settings.fps.to_string(),
            ),
            choice(
                "export.quality",
                &[("low", "Low"), ("medium", "Medium"), ("high", "High")],
                settings.quality.clone(),
            ),
            plain(
                "export.destination",
                3,
                display_path(&self.export_destination(settings)),
            ),
            plain("export.estimate", 3, format!("≈ {}", file_size(bytes))),
            plain("export.hardware", 2, settings.hardware.to_string()),
            plain("export.loop", 2, settings.gif_loop.to_string()),
            plain(
                "nativeCaptionSidecars",
                2,
                self.history
                    .as_ref()
                    .is_some_and(|h| h.project.flag("nativeCaptionSidecars", false))
                    .to_string(),
            ),
        ]
    }

    /// Where Export writes: the file the user chose with Change…, or the
    /// Movies folder under the recording's own name, with the extension the
    /// format takes.
    pub(super) fn export_destination(&self, settings: &ExportSettings) -> PathBuf {
        let extension = if self.export_frame {
            "png"
        } else if settings.gif {
            "gif"
        } else {
            "mp4"
        };
        if let Some(path) = &self.export_path {
            return path.with_extension(extension);
        }
        let stem = self
            .history
            .as_ref()
            .and_then(|h| {
                Path::new(&h.project.video_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "SubTake".into());
        directories::UserDirs::new()
            .and_then(|d| d.video_dir().map(Path::to_path_buf))
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("{stem}.{extension}"))
    }

    pub(super) fn field(&mut self, ui: &EditorWindow, key: &str, value: &str) -> Result<()> {
        if key == "padding.all" || key == "padding.linked" {
            return self.edit(ui, |p| {
                let old = p.editor.get("padding").cloned().unwrap_or(json!(20));
                let amount = if key == "padding.all" {
                    value.parse::<f64>()?
                } else {
                    old.as_f64().unwrap_or_else(|| n(&old, "top", 20.))
                };
                ensure!(
                    amount.is_finite() && (0.0..=250.0).contains(&amount),
                    "Padding must be between 0 and 250"
                );
                let linked = key == "padding.all" || value == "true";
                let mut padding = if old.is_object() {
                    old
                } else {
                    json!({"top":amount,"bottom":amount,"left":amount,"right":amount})
                };
                if linked {
                    for side in ["top", "bottom", "left", "right"] {
                        padding[side] = json!(amount);
                    }
                }
                padding["linked"] = json!(linked);
                p.set("padding", padding);
                Ok(())
            });
        }

        if key == "library.query" {
            self.library_query = value.into();
            self.reload_library()?;
            self.refresh(ui);
            return Ok(());
        }
        if let Some(rest) = key.strip_prefix("word.") {
            let (index, field) = rest.split_once('.').context("Invalid word edit")?;
            let index: usize = index.parse()?;
            let (kind, id) = self.selected.clone().context("Select a caption")?;
            ensure!(kind == "autoCaptions", "Select a caption");
            return self.edit(ui, |p| {
                subtake_native::caption_editing::edit_word(p, &id, index, field, value)
            });
        }
        // `preset.{index}.name`, the Presets dialog renaming a saved one.
        if let Some(index) = key
            .strip_prefix("preset.")
            .and_then(|k| k.strip_suffix(".name"))
        {
            return self.rename_preset(ui, index, value);
        }
        if let Some(action) = key.strip_prefix("shortcut.") {
            subtake_native::shortcuts::validate(action, value, &self.preferences.editor_shortcuts)?;
            self.preferences
                .editor_shortcuts
                .insert(action.into(), value.into());
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key == "prefs.language" {
            ensure!(
                subtake_native::localization::LOCALES.contains(&value),
                "Unsupported interface language"
            );
            self.preferences.language = value.into();
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key == "prefs.appearance" || key == "prefs.auto_apply_zooms" {
            if key == "prefs.appearance" {
                ensure!(
                    ["light", "dark", "system"].contains(&value),
                    "Unknown appearance"
                );
                self.preferences.appearance = value.into();
            } else {
                self.preferences.auto_apply_zooms = value.parse()?;
            }
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        if key.starts_with("prefs.") {
            let previous = self.preferences.clone();
            match key {
                "prefs.record_shortcut" => self.preferences.record_shortcut = value.into(),
                "prefs.pause_shortcut" => self.preferences.pause_shortcut = value.into(),
                "prefs.countdown_seconds" => {
                    self.preferences.countdown_seconds = value.parse::<u32>()?.min(10)
                }
                _ => (),
            }
            if let Err(e) = self.register_hotkeys() {
                self.preferences = previous;
                let _ = self.register_hotkeys();
                return Err(e);
            }
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(());
        }
        let text_field = matches!(
            key,
            "wallpaper"
                | "aspectRatio"
                | "cursorStyle"
                | "cursorClickEffect"
                | "cursorClickEffectColor"
                | "nativeCaptionLanguage"
                | "webcam.positionPreset"
                | "autoCaptionSettings.fontFamily"
                | "autoCaptionSettings.animationStyle"
                | "autoCaptionSettings.textColor"
                | "region.text"
                | "region.textContent"
                | "region.style.fontFamily"
                | "region.style.color"
                | "region.style.backgroundColor"
                | "region.style.fontWeight"
                | "region.style.fontStyle"
                | "region.style.textAlign"
                | "region.figureData.arrowDirection"
                | "region.figureData.color"
                | "region.blurMode"
                | "region.mode"
        );
        let v = if text_field {
            json!(value)
        } else {
            serde_json::from_str::<Value>(value).unwrap_or(json!(value))
        };
        if key == "export.format" {
            self.export_frame = value == "frame";
            if value == "frame" {
                self.refresh(ui);
                return Ok(());
            }
        }
        if key == "export.resolution" {
            return self.action(ui, &format!("export-preset-{value}"));
        }
        if key.starts_with("export.") {
            let mut settings = ExportSettings::for_media(
                self.project()?,
                self.info.as_ref().context("Open a video first")?,
            );
            match key {
                "export.width" => settings.width = value.parse()?,
                "export.height" => settings.height = value.parse()?,
                "export.fps" => settings.fps = value.parse()?,
                "export.gif" => settings.gif = value == "true",
                "export.format" => settings.gif = value == "gif",
                "export.loop" => settings.gif_loop = value == "true",
                "export.hardware" => settings.hardware = value == "true",
                "export.quality" => settings.quality = value.into(),
                _ => (),
            }
            return self.edit(ui, |p| {
                settings.store(p);
                Ok(())
            });
        }
        if let Some(key) = key.strip_prefix("region.") {
            let (kind, id) = self.selected.clone().context("Select a timeline region")?;
            self.edit(ui, |p| {
                let mut r = p
                    .regions(&kind)
                    .iter()
                    .find(|r| r["id"] == id)
                    .context("Region not found")?
                    .clone();
                set_nested(&mut r, key, v);
                p.change_region(&kind, &id, r)
            })
        } else {
            self.edit(ui, |p| {
                let (base, rest) = key.split_once('.').unwrap_or((key, ""));
                if rest.is_empty() {
                    p.set(base, v)
                } else {
                    let mut object = p.editor.get(base).cloned().unwrap_or(json!({}));
                    set_nested(&mut object, rest, v);
                    if base == "cropRegion" {
                        normalize_crop(&mut object);
                    }
                    if base == "webcam" {
                        if rest == "positionX" || rest == "positionY" {
                            object["positionPreset"] = json!("custom");
                        }
                        if let Some(crop) = object.get_mut("cropRegion") {
                            normalize_crop(crop);
                        }
                    }
                    p.set(base, object)
                }
                Ok(())
            })
        }
    }
}

pub(super) fn get_nested(value: &Value, key: &str) -> Value {
    key.split('.')
        .fold(value, |value, k| value.get(k).unwrap_or(&Value::Null))
        .clone()
}

pub(super) fn set_nested(root: &mut Value, key: &str, value: Value) {
    if !root.is_object() {
        *root = json!({})
    }
    if let Some((a, b)) = key.split_once('.') {
        if root.get(a).is_none() {
            root[a] = json!({})
        }
        set_nested(&mut root[a], b, value);
    } else {
        root[key] = value;
    }
}

pub(super) fn normalize_crop(crop: &mut Value) {
    let x = n(crop, "x", 0.).clamp(0., 0.99);
    let y = n(crop, "y", 0.).clamp(0., 0.99);
    let width = n(crop, "width", 1.).clamp(0.01, 1. - x);
    let height = n(crop, "height", 1.).clamp(0.01, 1. - y);
    *crop = json!({"x":x,"y":y,"width":width,"height":height});
}

/// A path as the Export panel shows it: the home folder as `~`.
pub(super) fn display_path(path: &Path) -> String {
    directories::BaseDirs::new()
        .and_then(|d| path.strip_prefix(d.home_dir()).ok().map(Path::to_path_buf))
        .map(|rest| format!("~/{}", rest.display()))
        .unwrap_or_else(|| path.display().to_string())
}

/// A byte count in the one unit that keeps it short.
pub(super) fn file_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", (b / 1e3).max(1.))
    }
}
