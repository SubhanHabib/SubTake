//! The fake project, regions, devices and inspector fields every gallery
//! window is seeded with.

use super::*;

pub(super) fn seed_editor(ui: &EditorWindow) {
    ui.set_mac_titlebar(cfg!(target_os = "macos"));
    ui.set_document_title("Gallery · Onboarding walkthrough".into());
    ui.set_has_project(true);
    ui.set_has_video(true);
    // Nothing on the picture is selected, so it carries no selection box;
    // one with no bounds drew as a stray square at the picture's corner.
    ui.set_edit_visible(false);
    ui.set_dirty(true);
    ui.set_can_undo(true);
    ui.set_can_redo(false);
    ui.set_duration(DURATION);
    ui.set_timeline_zoom(1.);
    ui.set_timeline_offset(0.);
    ui.set_panel("Frame".into());
    ui.set_panel_index(0);
    ui.set_language("en".into());
    ui.set_status("Gallery mode — nothing here touches a project".into());
    ui.set_preview_aspect(16. / 9.);
    ui.set_preview_pixel_width(PREVIEW_W as f32);
    ui.set_preview_zoom(1.);
    ui.set_snap(true);
    ui.set_auto_apply_zooms(true);
    ui.set_connect_zooms(false);
    ui.set_motion_choice("smooth".into());
    ui.set_look_choice("studio".into());
    ui.set_background_value("wallpaper-1".into());
    ui.set_aspect_index(0);
    ui.set_preview(gradient(
        PREVIEW_W,
        PREVIEW_H,
        [0x1f, 0x3b, 0x73],
        [0xd9, 0x6c, 0x9d],
        Style::Preview,
    ));
    ui.set_thumbnails(gradient(
        1600,
        48,
        [0x1f, 0x3b, 0x73],
        [0xd9, 0x6c, 0x9d],
        Style::Strip,
    ));
    ui.set_frosted_thumbnails(gradient(
        1600,
        48,
        [0x4a, 0x5c, 0x86],
        [0xc7, 0x9a, 0xb4],
        Style::Strip,
    ));
    ui.set_waveform(gradient(
        1600,
        40,
        [0xa4, 0x68, 0xe9],
        [0xa4, 0x68, 0xe9],
        Style::Waveform,
    ));
    ui.set_track_labels(ModelRc::new(VecModel::from(vec![
        "Zoom".to_string(),
        "Clip".into(),
        "Annotation".into(),
        "Audio".into(),
        "Caption".into(),
    ])));
    ui.set_audio_row(3);
    ui.set_wallpapers(ModelRc::new(VecModel::from(
        [
            ("Dusk", [0x1f, 0x3b, 0x73], [0xd9, 0x6c, 0x9d]),
            ("Meadow", [0x1a, 0x6b, 0x4a], [0xd8, 0xe3, 0x6b]),
            ("Ember", [0x6b, 0x1a, 0x1a], [0xf5, 0xbb, 0x6b]),
            ("Slate", [0x2b, 0x2f, 0x3a], [0x9a, 0xa4, 0xb8]),
            ("Lagoon", [0x0e, 0x4d, 0x64], [0x7f, 0xd3, 0xe0]),
            ("Blossom", [0x8a, 0x2f, 0x6a], [0xff, 0xc6, 0xd9]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (title, a, b))| Wallpaper {
            key: format!("wallpaper-{}", i + 1),
            title: title.into(),
            source: gradient(160, 90, a, b, Style::Preview),
        })
        .collect::<Vec<_>>(),
    )));
    // The empty state's cards, as the handoff draws them (⌘⇧E shows it).
    ui.set_recents(ModelRc::new(VecModel::from(
        [
            (
                "Onboarding walkthrough",
                "1:42 · yesterday",
                [0xe8, 0x9a, 0x5c],
                [0x3b, 0x2a, 0x6b],
            ),
            (
                "Bug repro · timeline",
                "0:38 · Tuesday",
                [0x1a, 0x6b, 0x4a],
                [0x9a, 0xc8, 0xe0],
            ),
            (
                "Release notes demo",
                "3:05 · last week",
                [0x8a, 0x2f, 0x6a],
                [0x7f, 0xd3, 0xe0],
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (title, meta, a, b))| Recent {
            key: format!("library-open-{i}"),
            title: title.into(),
            meta: meta.into(),
            thumbnail: gradient(480, 192, a, b, Style::Preview),
        })
        .collect::<Vec<_>>(),
    )));
    ui.set_saved_presets(ModelRc::new(VecModel::from(vec!["Client demo".to_owned()])));
    ui.set_source_names(ModelRc::new(VecModel::from(source_names())));
    ui.set_camera_names(ModelRc::new(VecModel::from(camera_names())));
    ui.set_microphone_names(ModelRc::new(VecModel::from(microphone_names())));
    ui.set_source_index(1);
    ui.set_camera_index(0);
    ui.set_microphone_index(0);
    ui.set_capture_camera(true);
    ui.set_capture_mic(true);
    ui.set_capture_system(false);
}

pub(super) fn seed_recorder(launcher: &RecordingLauncher, options: &RecordingOptions) {
    // Both recorder windows mirror the same capture state, as `sync_launcher`
    // keeps them in the product.
    for (set_names, set_index, set_flag) in [
        (
            Box::new(|s, c, m| {
                launcher.set_source_names(s);
                launcher.set_camera_names(c);
                launcher.set_microphone_names(m);
            }) as Box<dyn Fn(ModelRc<String>, ModelRc<String>, ModelRc<String>)>,
            Box::new(|i, j, k| {
                launcher.set_source_index(i);
                launcher.set_camera_index(j);
                launcher.set_microphone_index(k);
            }) as Box<dyn Fn(i32, i32, i32)>,
            Box::new(|c, m, s| {
                launcher.set_camera(c);
                launcher.set_microphone(m);
                launcher.set_system_audio(s);
            }) as Box<dyn Fn(bool, bool, bool)>,
        ),
        (
            Box::new(|s, c, m| {
                options.set_source_names(s);
                options.set_camera_names(c);
                options.set_microphone_names(m);
            }),
            Box::new(|i, j, k| {
                options.set_source_index(i);
                options.set_camera_index(j);
                options.set_microphone_index(k);
            }),
            Box::new(|c, m, s| {
                options.set_camera(c);
                options.set_microphone(m);
                options.set_system_audio(s);
            }),
        ),
    ] {
        set_names(
            ModelRc::new(VecModel::from(source_names())),
            ModelRc::new(VecModel::from(camera_names())),
            ModelRc::new(VecModel::from(microphone_names())),
        );
        set_index(1, 0, 0);
        set_flag(true, true, false);
    }
    // The Source card's pictures and the camera card's picture: stand-ins
    // for the stills the platform layer does not take yet.
    options.set_capture_sources(ModelRc::new(VecModel::from(
        source_names()
            .into_iter()
            .zip([
                ([0xe8, 0xa0, 0x5c], [0x3a, 0x47, 0x63]),
                ([0x2c, 0x5e, 0x3f], [0xa8, 0xc8, 0xd8]),
                ([0x6a, 0x4f, 0xc8], [0xe0, 0x9a, 0xc8]),
                ([0x10, 0x14, 0x24], [0x3a, 0x4a, 0x6a]),
                ([0x8a, 0xa0, 0x4a], [0xe0, 0xd0, 0x8a]),
            ])
            .map(|(full, (a, b))| {
                let (name, detail) = match full.split_once(" · ") {
                    Some((name, detail)) => (name.to_owned(), detail.replace('×', " × ")),
                    None => (full.clone(), String::new()),
                };
                CaptureSource {
                    kind: if detail.is_empty() {
                        "window"
                    } else {
                        "display"
                    }
                    .into(),
                    name,
                    detail,
                    thumbnail: gradient(240, 132, a, b, Style::Preview),
                }
            })
            .collect::<Vec<_>>(),
    )));
    options.set_camera_preview(gradient(
        640,
        264,
        [0xd8, 0x8a, 0x5a],
        [0x2a, 0x2f, 0x4a],
        Style::Preview,
    ));
    for w in [launcher as &dyn Recorder, options] {
        w.seed();
    }
    launcher.set_status("Ready to record".into());
    launcher.set_recording_hint("⌘⇧R starts a recording".into());
    launcher.set_recording(false);
    launcher.set_paused(false);
    launcher.set_cancellable(false);
    launcher.set_sources_loading(false);
    launcher.set_elapsed("".into());
}

/// The shared subset of recorder state both windows carry.
pub(super) trait Recorder {
    fn seed(&self);
}

impl Recorder for RecordingLauncher {
    fn seed(&self) {
        self.set_has_project(true);
        self.set_countdown(3);
        self.set_directory("~/Movies/SubTake".into());
        self.set_busy(false);
        self.set_panel("".into());
    }
}

impl Recorder for RecordingOptions {
    fn seed(&self) {
        self.set_has_project(true);
        self.set_countdown(3);
        self.set_directory("~/Movies/SubTake".into());
        self.set_busy(false);
        self.set_panel("".into());
    }
}

pub(super) fn source_names() -> Vec<String> {
    [
        "Built-in Display · 3456×2234",
        "Studio Display · 5120×2880",
        "Safari — Release notes",
        "Terminal",
        "Figma — SubTake redesign",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub(super) fn camera_names() -> Vec<String> {
    [
        "FaceTime HD Camera",
        "Studio Display Camera",
        "iPhone Continuity Camera",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub(super) fn microphone_names() -> Vec<String> {
    [
        "MacBook Pro Microphone",
        "Studio Display Microphone",
        "Shure MV7",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub(super) fn fixture_regions() -> Vec<Region> {
    pub(super) fn region(
        kind: &str,
        id: &str,
        label: &str,
        start: f32,
        end: f32,
        row: i32,
        tint: u32,
    ) -> Region {
        Region {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            start,
            end,
            row,
            tint: Color::from_rgb_u8((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
            selected: false,
        }
    }
    vec![
        region("zoomRegions", "z1", "Zoom", 4., 12.5, 0, 0x397afa),
        region("zoomRegions", "z2", "Zoom", 41., 52., 0, 0x397afa),
        region("zoomRegions", "z3", "Zoom", 98., 111., 0, 0x397afa),
        region("nativeMarkers", "m1", "◆", 60., 60.4, 0, 0xf5bb6b),
        region("clipRegions", "c1", "Intro", 0., 33., 1, 0x357c65),
        region("speedRegions", "s1", "2× Speed", 33., 41., 1, 0xdc922d),
        region("trimRegions", "t1", "Trim", 71., 76., 1, 0xee5261),
        region(
            "clipRegions",
            "c2",
            "Walkthrough",
            76.,
            DURATION,
            1,
            0x357c65,
        ),
        region(
            "annotationRegions",
            "a1",
            "Click Save",
            14.,
            22.,
            2,
            0xcbb44f,
        ),
        region("annotationRegions", "a2", "Arrow", 86., 94., 2, 0xcbb44f),
        region(
            "audioRegions",
            "au1",
            "Voice-over",
            0.,
            DURATION,
            3,
            0xa468e9,
        ),
        region(
            "autoCaptions",
            "cap1",
            "Welcome to SubTake",
            1.,
            5.5,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap2",
            "Let's set up your workspace",
            6.,
            11.,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap3",
            "Choose a wallpaper",
            12.,
            17.,
            4,
            0x6396dc,
        ),
        region(
            "autoCaptions",
            "cap4",
            "And export in one click",
            120.,
            126.,
            4,
            0x6396dc,
        ),
    ]
}

/// Field kinds as `ui::inspector` renders them: 0 text/number, 1 slider,
/// 2 toggle, 3 action row, 4 dropdown, 5 section label.
pub(super) fn fixture_fields(fixture: &Gallery, panel: &str) -> Vec<Field> {
    let v = |key: &str, default: &str| fixture.value(key, default).to_owned();
    let section = |label: &str| Field {
        key: format!("section.{label}"),
        label: label.into(),
        kind: 5,
        ..Default::default()
    };
    let slider = |key: &str, label: &str, default: &str, min: f32, max: f32| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, default),
        kind: 1,
        minimum: min,
        maximum: max,
        ..Default::default()
    };
    let text = |key: &str, label: &str, default: &str| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, default),
        kind: 0,
        ..Default::default()
    };
    let toggle = |key: &str, label: &str, default: bool| Field {
        key: key.into(),
        label: label.into(),
        value: v(key, &default.to_string()),
        kind: 2,
        ..Default::default()
    };
    // An action row shows its value, which the app sets to the button's
    // words.
    let action = |key: &str, label: &str| Field {
        key: key.into(),
        label: label.into(),
        value: label.into(),
        kind: 3,
        ..Default::default()
    };
    let dropdown = |key: &str, label: &str, choices: &[(&str, &str)], default: &str| {
        let value = v(key, default);
        let choice = choices
            .iter()
            .position(|(val, _)| *val == value)
            .unwrap_or(0) as i32;
        Field {
            key: key.into(),
            label: label.into(),
            value,
            kind: 4,
            choices: ModelRc::new(VecModel::from(
                choices
                    .iter()
                    .map(|(_, c)| c.to_string())
                    .collect::<Vec<_>>(),
            )),
            values: ModelRc::new(VecModel::from(
                choices
                    .iter()
                    .map(|(val, _)| val.to_string())
                    .collect::<Vec<_>>(),
            )),
            choice,
            ..Default::default()
        }
    };

    match panel {
        "Frame" => vec![
            section("Layout"),
            slider("padding.all", "Padding", "64", 0., 320.),
            toggle("padding.linked", "Link all sides", true),
            slider("borderRadius", "Radius", "18", 0., 64.),
            slider("shadow", "Shadow", "0.4", 0., 1.),
            section("Cursor"),
            toggle("showCursor", "Show cursor", true),
            dropdown(
                "cursorStyle",
                "Style",
                &[("arrow", "Arrow"), ("hand", "Hand"), ("dot", "Dot")],
                "arrow",
            ),
            dropdown(
                "cursorClickEffect",
                "Click effect",
                &[("none", "None"), ("ripple", "Ripple"), ("pulse", "Pulse")],
                "ripple",
            ),
            slider("cursorSize", "Size", "1.4", 0.5, 3.),
            toggle("cursorSmoothing", "Smooth movement", true),
            section("Zoom"),
            slider("zoomSmoothness", "Smoothness", "60", 0., 100.),
            toggle("connectZooms", "Connect zooms", false),
            section("Camera"),
            dropdown(
                "webcam.positionPreset",
                "Position",
                &[
                    ("bottom-right", "Bottom right"),
                    ("bottom-left", "Bottom left"),
                    ("top-right", "Top right"),
                    ("custom", "Custom"),
                ],
                "bottom-right",
            ),
            slider("webcam.width", "Width", "24", 8., 60.),
            dropdown(
                "webcam.shape",
                "Shape",
                &[
                    ("circle", "Circle"),
                    ("rounded", "Rounded"),
                    ("square", "Square"),
                ],
                "circle",
            ),
        ],
        // The keys `App::raw_fields("Cursor")` hands over.
        "Cursor" => vec![
            toggle("showCursor", "Rendered cursor", true),
            dropdown(
                "cursorStyle",
                "Style",
                &[
                    ("tahoe", "macOS Tahoe"),
                    ("macos", "macOS"),
                    ("windows11", "Windows 11"),
                    ("dot", "Dot"),
                    ("figma", "Figma"),
                ],
                "tahoe",
            ),
            slider("cursorSize", "Size", "3", 0.5, 8.),
            dropdown(
                "cursorClickEffect",
                "Click effect",
                &[
                    ("none", "None"),
                    ("ripple", "Ripple"),
                    ("spotlight", "Spotlight"),
                    ("echo", "Echo"),
                ],
                "ripple",
            ),
            slider("cursorSmoothing", "Smooth movement", "0.67", 0., 2.),
            slider("cursorSway", "Sway", "0.4", 0., 2.),
            slider("cursorMotionBlur", "Motion blur", "0.6", 0., 2.),
            slider("cursorClickBounce", "Click bounce", "2", 0., 4.),
            toggle("loopCursor", "Loop cursor", false),
            slider("zoomMotionBlur", "Camera motion blur", "0.35", 0., 2.),
        ],
        // The keys `App::raw_fields("Webcam")` hands over.
        "Webcam" => vec![
            action("choose-webcam", "Choose video…"),
            toggle("webcam.enabled", "Show webcam", true),
            toggle("webcam.mirror", "Mirror", false),
            slider("webcam.width", "Width", "24", 5., 100.),
            slider("webcam.height", "Height", "24", 5., 100.),
            slider("webcam.positionX", "Horizontal position", "1", 0., 1.),
            slider("webcam.positionY", "Vertical position", "1", 0., 1.),
            text("webcam.positionPreset", "", "bottom-right"),
            toggle("webcam.reactToZoom", "React to zoom", true),
            slider("webcam.shadow", "Shadow", "0.3", 0., 1.),
            slider("webcam.roundness", "Roundness", "100", 0., 100.),
            slider("webcam.margin", "Margin", "24", 0., 150.),
        ],
        "Wallpapers" => vec![
            section("Background"),
            slider("backgroundBlur", "Blur", "12", 0., 80.),
            text("background.color", "Solid colour", "#1F3B73"),
            action("choose-background", "Choose image…"),
        ],
        // The keys `App::export_fields` hands over, with fixture values.
        "Export" => {
            let format = v("export.format", "video");
            let (w, h, fps) = (
                match v("export.resolution", "1080").as_str() {
                    "720" => 1280.,
                    "source" => 2880.,
                    _ => 1920.,
                },
                match v("export.resolution", "1080").as_str() {
                    "720" => 720.,
                    "source" => 1800.,
                    _ => 1080.,
                },
                v("export.fps", "60").parse::<f64>().unwrap_or(60.),
            );
            let bpp = match v("export.quality", "high").as_str() {
                "low" => 0.02,
                "medium" => 0.04,
                _ => 0.08,
            };
            let bytes = match format.as_str() {
                "frame" => w * h * 0.8,
                "gif" => w * h * fps * DURATION as f64 / 8.,
                _ => w * h * fps * DURATION as f64 * bpp / 8.,
            };
            let extension = match format.as_str() {
                "frame" => "png",
                "gif" => "gif",
                _ => "mp4",
            };
            let plain = |key: &str, kind: i32, value: String| Field {
                key: key.into(),
                value: value.into(),
                kind,
                ..Default::default()
            };
            vec![
                plain("export.format", 0, format),
                dropdown(
                    "export.resolution",
                    "",
                    &[("720", "720p"), ("1080", "1080p"), ("source", "Source")],
                    "1080",
                ),
                dropdown(
                    "export.fps",
                    "",
                    &[("24", "24 fps"), ("30", "30 fps"), ("60", "60 fps")],
                    "60",
                ),
                dropdown(
                    "export.quality",
                    "",
                    &[("low", "Low"), ("medium", "Medium"), ("high", "High")],
                    "high",
                ),
                plain(
                    "export.destination",
                    3,
                    format!("~/Movies/gallery.{extension}"),
                ),
                plain(
                    "export.estimate",
                    3,
                    if bytes >= 1e9 {
                        format!("≈ {:.1} GB", bytes / 1e9)
                    } else {
                        format!("≈ {:.0} MB", bytes / 1e6)
                    },
                ),
                toggle("export.hardware", "", false),
                toggle("export.loop", "", true),
                toggle("nativeCaptionSidecars", "", false),
            ]
        }
        "Selection" => {
            let Some(r) = fixture.regions.iter().find(|r| r.selected) else {
                return vec![];
            };
            // The timing is the region's own, so a drag on the timeline and
            // an edit here stay one value.
            let time = |key: &str, seconds: f32| Field {
                key: key.into(),
                value: format!("{:.0}", seconds * 1000.).into(),
                ..Default::default()
            };
            let mut fields = vec![time("region.startMs", r.start), time("region.endMs", r.end)];
            match r.kind.as_str() {
                "zoomRegions" => fields.extend([
                    slider("region.depth", "Zoom depth", "3", 1., 6.),
                    text("region.mode", "", "auto"),
                    slider("region.focus.cx", "Focus X", "0.5", 0., 1.),
                    slider("region.focus.cy", "Focus Y", "0.5", 0., 1.),
                ]),
                "clipRegions" => fields.extend([
                    toggle("region.muted", "Mute clip", false),
                    slider("region.speed", "Playback speed", "1", 0.25, 4.),
                ]),
                "speedRegions" => {
                    fields.push(slider("region.speed", "Playback speed", "2", 0.25, 4.))
                }
                "audioRegions" => fields.extend([
                    slider("region.volume", "Volume", "1", 0., 3.),
                    toggle("region.normalize", "Normalize audio", false),
                ]),
                "autoCaptions" | "annotationRegions" => {
                    fields.push(text("region.text", "Text", &r.label))
                }
                _ => {}
            }
            fields
        }
        "Recording" => vec![
            section("Sources"),
            dropdown(
                "recording.source",
                "Screen",
                &[
                    ("0", "Built-in Retina Display"),
                    ("1", "Studio Display"),
                    ("2", "Safari — SubTake docs"),
                ],
                "1",
            ),
            toggle("recording.camera", "Camera", true),
            dropdown(
                "recording.camera_device",
                "Camera device",
                &[("0", "FaceTime HD Camera"), ("1", "Studio Display Camera")],
                "0",
            ),
            toggle("recording.microphone", "Microphone", true),
            toggle("recording.system", "System audio", false),
            section("Behaviour"),
            slider("prefs.countdown_seconds", "Countdown", "3", 0., 10.),
            text("recording.directory", "Save to", "~/Movies/SubTake"),
            action("show-launcher", "Open recorder"),
        ],
        "Preferences" => vec![
            section("Appearance"),
            dropdown(
                "prefs.appearance",
                "Appearance",
                &[("dark", "Dark"), ("light", "Light")],
                fixture.appearance,
            ),
            dropdown(
                "prefs.language",
                "Language",
                &[
                    ("en", "English"),
                    ("de", "Deutsch"),
                    ("fr", "Français"),
                    ("ja", "日本語"),
                ],
                "en",
            ),
            section("Editing"),
            toggle("prefs.auto_apply_zooms", "Auto-apply zooms", true),
            toggle("prefs.snap", "Snap to regions", true),
            slider("prefs.countdown_seconds", "Countdown", "3", 0., 10.),
            section("Storage"),
            text(
                "prefs.library_directory",
                "Library folder",
                "~/Movies/SubTake",
            ),
            action("prefs.reveal", "Reveal in Finder"),
        ],
        "Shortcuts" => vec![
            section("Recording"),
            text("prefs.record_shortcut", "Start / stop", "⌘⇧R"),
            text("prefs.pause_shortcut", "Pause", "⌘⇧P"),
            section("Editor"),
            text("shortcut.play", "Play / pause", "Space"),
            text("shortcut.split", "Split clip", "S"),
            text("shortcut.zoom-in", "Zoom in", "⌘="),
            text("shortcut.zoom-out", "Zoom out", "⌘-"),
            text("shortcut.appearance", "Toggle light / dark", "⌘⇧D"),
        ],
        "Recent" => vec![
            section("Recent projects"),
            action("recent.0", "Onboarding walkthrough — today"),
            action("recent.1", "Release notes 2.4 — yesterday"),
            action("recent.2", "Bug repro for Adclear — 3 days ago"),
            action("recent.3", "Keyboard shortcuts tour — last week"),
            section("Library"),
            text("library.query", "Search", ""),
        ],
        _ => vec![
            section(panel),
            action("noop", "This panel has no gallery fixtures yet"),
        ],
    }
}
