//! Presentation metadata for the native inspector. Stored project keys stay unchanged.
use crate::Field;
use subtake_native::ui_runtime as ui_runtime;
use ui_runtime::{ModelRc, SharedString, VecModel};

pub fn present(mut fields: Vec<Field>, panel: &str, language: &str) -> Vec<Field> {
    if panel == "Frame" {
        fields.retain(|f| {
            !is_preset(&f.key)
                && !matches!(
                    f.key.as_str(),
                    "visual-crop"
                        | "wallpapers"
                        | "wallpaper"
                        | "choose-background"
                        | "aspectRatio"
                )
                && !f.key.starts_with("cropRegion.")
        });
    } else if panel == "Presets" {
        fields.retain(|f| is_preset(&f.key));
    } else if panel == "Wallpapers" {
        fields.retain(|f| f.key == "backgroundBlur");
    }
    fields.retain(|f| !f.key.starts_with("motion-"));
    if panel == "Preferences" {
        fields.retain(|f| f.key == "prefs.language" || f.key == "prefs.countdown_seconds");
    } else if panel == "Shortcuts" {
        fields.retain(|f| {
            f.key.starts_with("shortcut.")
                || f.key == "prefs.record_shortcut"
                || f.key == "prefs.pause_shortcut"
        });
    }
    for f in &mut fields {
        let key = f.key.as_str();
        f.label = match key {
            "cursorStyle" => "Style",
            "cursorClickEffect" => "Click effect",
            "webcam.positionPreset" => "Position",
            "autoCaptionSettings.animationStyle" => "Animation",
            "export.quality" => "Quality",
            "aspectRatio" => "Aspect ratio",
            "prefs.language" => "Language",
            "nativeCaptionLanguage" => "Speech language",
            "borderRadius" => "Radius",
            "showCursor" => "Show cursor",
            "zoomSmoothness" => "Smoothness",
            "connectZooms" => "Connect zooms",
            "cursorSmoothing" => "Smooth movement",
            "cursorSway" => "Sway",
            "webcam.width" => "Width",
            "webcam.height" => "Height",
            "webcam.positionX" => "Horizontal position",
            "webcam.positionY" => "Vertical position",
            "padding.top" => "Top",
            "padding.bottom" => "Bottom",
            "padding.left" => "Left",
            "padding.right" => "Right",
            "defaultSourceAudioTrackSettings.system.volume" => "System audio",
            "defaultSourceAudioTrackSettings.mic.volume" => "Microphone",
            "defaultSourceAudioTrackSettings.mixed.volume" => "Embedded audio",
            _ => f.label.as_str(),
        }
        .into();
        let choices: &[(&str, &str)] = match key {
            "cursorStyle" => &[
                ("tahoe", "macOS Tahoe"),
                ("macos", "macOS"),
                ("windows11", "Windows 11"),
                ("dot", "Dot"),
                ("figma", "Figma"),
            ],
            "cursorClickEffect" => &[
                ("none", "None"),
                ("ripple", "Ripple"),
                ("spotlight", "Spotlight"),
                ("echo", "Echo"),
            ],
            "aspectRatio" => &[
                ("native", "Native"),
                ("16:9", "16:9"),
                ("9:16", "9:16"),
                ("1:1", "1:1"),
                ("4:3", "4:3"),
                ("3:2", "3:2"),
            ],
            "webcam.positionPreset" => &[
                ("custom", "Custom"),
                ("bottom-right", "Bottom right"),
                ("bottom-left", "Bottom left"),
                ("top-right", "Top right"),
                ("top-left", "Top left"),
                ("top-center", "Top center"),
                ("center-left", "Center left"),
                ("center", "Center"),
                ("center-right", "Center right"),
                ("bottom-center", "Bottom center"),
            ],
            "autoCaptionSettings.animationStyle" => &[
                ("none", "None"),
                ("fade", "Fade"),
                ("rise", "Rise"),
                ("pop", "Pop"),
            ],
            "nativeCaptionLanguage" => &[
                ("auto", "Auto detect"),
                ("en", "English"),
                ("es", "Spanish"),
                ("fr", "French"),
                ("de", "German"),
                ("it", "Italian"),
                ("pt", "Portuguese"),
                ("nl", "Dutch"),
                ("ja", "Japanese"),
                ("ko", "Korean"),
                ("zh", "Chinese"),
            ],
            "autoCaptionSettings.fontFamily" => &[
                ("Helvetica", "Helvetica"),
                ("Arial", "Arial"),
                ("Georgia", "Georgia"),
                ("Verdana", "Verdana"),
                ("Times New Roman", "Times New Roman"),
                ("Courier New", "Courier New"),
            ],
            "prefs.countdown_seconds" => &[
                ("0", "No delay"),
                ("3", "3 seconds"),
                ("5", "5 seconds"),
                ("10", "10 seconds"),
            ],
            "export.quality" => &[("low", "Low"), ("medium", "Medium"), ("high", "High")],
            "prefs.language" => &[
                ("en", "English"),
                ("es", "Español"),
                ("fr", "Français"),
                ("de", "Deutsch"),
                ("it", "Italiano"),
                ("nl", "Nederlands"),
                ("ko", "한국어"),
                ("pt-BR", "Português (Brasil)"),
                ("zh-CN", "简体中文"),
                ("zh-TW", "繁體中文"),
            ],
            _ => &[],
        };
        if !choices.is_empty() {
            let mut options = choices
                .iter()
                .map(|(value, label)| {
                    (
                        value.to_string(),
                        subtake_native::localization::translate(label, language),
                    )
                })
                .collect::<Vec<_>>();
            // Preserve unfamiliar imported values instead of displaying a false selection.
            if !options.iter().any(|(v, _)| v == f.value.as_str()) {
                options.push((f.value.to_string(), f.value.to_string()));
            }
            f.choice = options
                .iter()
                .position(|(v, _)| v == f.value.as_str())
                .unwrap_or(0) as i32;
            f.choices = ModelRc::new(VecModel::from(
                options
                    .iter()
                    .map(|(_, label)| SharedString::from(label.as_str()))
                    .collect::<Vec<_>>(),
            ));
            f.values = ModelRc::new(VecModel::from(
                options
                    .iter()
                    .map(|(value, _)| SharedString::from(value.as_str()))
                    .collect::<Vec<_>>(),
            ));
            f.kind = 4;
        }
    }
    fields.sort_by_key(|f| {
        (
            group(panel, &f.key).0,
            if f.key == "webcam.positionPreset" {
                0
            } else {
                1
            },
        )
    });
    let mut result = Vec::new();
    let mut previous = "";
    for field in fields {
        let (_, section) = group(panel, &field.key);
        if section != previous && !section.is_empty() {
            result.push(Field {
                kind: 5,
                label: section.into(),
                ..Default::default()
            });
            previous = section;
        }
        result.push(field);
    }
    result
}
fn is_preset(key: &str) -> bool {
    key.contains("preset")
}
fn group<'a>(panel: &str, key: &str) -> (i32, &'a str) {
    match panel {
        "Frame" => {
            if key.starts_with("padding.") {
                (1, "Padding")
            } else if key == "shadowIntensity" || key == "borderRadius" {
                (0, "Frame")
            } else if key == "backgroundBlur" {
                (3, "Background")
            } else {
                (2, "Animation")
            }
        }
        "Cursor" => {
            if key.starts_with("motion-") {
                (3, "Motion presets")
            } else if key.starts_with("cursorClick") {
                (2, "Click effect")
            } else if matches!(
                key,
                "showCursor" | "cursorStyle" | "cursorSize" | "loopCursor"
            ) {
                (0, "Cursor")
            } else {
                (1, "Movement")
            }
        }
        "Webcam" => {
            if key == "webcam.positionPreset" {
                (1, "Position")
            } else if key.contains("cropRegion") {
                (4, "Crop")
            } else if key.contains("position")
                || key == "webcam.margin"
                || key == "webcam.timeOffsetMs"
            {
                (3, "Fine positioning")
            } else if matches!(
                key,
                "webcam.width"
                    | "webcam.height"
                    | "webcam.roundness"
                    | "webcam.shadow"
                    | "webcam.reactToZoom"
            ) {
                (2, "Appearance")
            } else {
                (0, "Webcam")
            }
        }
        "Captions" => {
            if key.starts_with("autoCaptionSettings.") {
                (1, "Caption style")
            } else {
                (0, "Transcription")
            }
        }
        "Export" => {
            if key.starts_with("export-preset") {
                (0, "Resolution")
            } else if key.contains("Caption") {
                (2, "Subtitles")
            } else if key == "reveal-export" {
                (3, "Last export")
            } else {
                (1, "Export settings")
            }
        }
        "Preferences" => {
            if key.starts_with("shortcut.") {
                (2, "Keyboard shortcuts")
            } else if key == "prefs.language" {
                (0, "General")
            } else {
                (1, "Recording")
            }
        }
        "Presets" => (0, "Appearance presets"),
        "Audio" => (0, "Source audio"),
        _ => (0, ""),
    }
}
