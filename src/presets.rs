//! Portable appearance presets never replace source media, regions or document identity.
use crate::project::Project;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const KEYS: &[&str] = &[
    "wallpaper",
    "shadowIntensity",
    "backgroundBlur",
    "zoomMotionBlur",
    "zoomMotionBlurTuning",
    "zoomTemporalMotionBlur",
    "zoomMotionBlurSampleCount",
    "zoomMotionBlurShutterFraction",
    "connectZooms",
    "zoomInDurationMs",
    "zoomInOverlapMs",
    "zoomOutDurationMs",
    "connectedZoomGapMs",
    "connectedZoomDurationMs",
    "zoomInEasing",
    "zoomOutEasing",
    "connectedZoomEasing",
    "showCursor",
    "loopCursor",
    "cursorStyle",
    "cursorSize",
    "cursorSmoothing",
    "cursorSpringStiffnessMultiplier",
    "cursorSpringDampingMultiplier",
    "cursorSpringMassMultiplier",
    "cameraSpringStiffnessMultiplier",
    "cameraSpringDampingMultiplier",
    "cameraSpringMassMultiplier",
    "cursorMotionBlur",
    "cursorClickEffect",
    "cursorClickEffectColor",
    "cursorClickEffectScale",
    "cursorClickEffectOpacity",
    "cursorClickEffectDurationMs",
    "cursorClickBounce",
    "cursorClickBounceDuration",
    "cursorSway",
    "borderRadius",
    "padding",
    "webcam",
    "aspectRatio",
    "exportEncodingMode",
    "exportBackendPreference",
    "exportPipelineModel",
    "exportQuality",
    "mp4FrameRate",
    "exportFormat",
    "gifFrameRate",
    "gifLoop",
    "gifSizePreset",
    "cropRegion",
    "autoCaptionSettings",
    "borderRadiusUnit",
    "nativeFonts",
    "nativeExportWidth",
    "nativeExportHeight",
    "nativeExportFps",
    "nativeExportQuality",
    "nativeExportHardware",
    "nativeExportGif",
    "nativeExportLoop",
    "nativeCaptionSidecars",
];
pub fn snapshot(project: &Project) -> Value {
    let mut data = serde_json::Map::new();
    for key in KEYS {
        if let Some(v) = project.editor.get(*key) {
            data.insert((*key).into(), v.clone());
        }
    }
    if let Some(webcam) = data.get_mut("webcam").and_then(Value::as_object_mut) {
        webcam.remove("sourcePath");
    }
    Value::Object(data)
}

pub fn apply(project: &mut Project, data: &Value) -> Result<()> {
    let data = data.get("snapshot").unwrap_or(data);
    ensure!(data.is_object(), "Preset must contain settings");
    let source = project
        .editor
        .get("webcam")
        .and_then(|v| v.get("sourcePath"))
        .cloned();
    for key in KEYS {
        if let Some(v) = data.get(*key) {
            let mut v = v.clone();
            if *key == "webcam" {
                ensure!(v.is_object(), "Invalid webcam preset");
                v.as_object_mut().unwrap().remove("sourcePath");
                if let Some(source) = &source {
                    v["sourcePath"] = source.clone();
                }
            }
            project.set(key, v);
        }
    }
    Ok(())
}

pub fn load(path: &Path) -> Result<Value> {
    ensure!(
        path.metadata()?.len() <= 16 * 1024 * 1024,
        "Preset exceeds 16 MiB"
    );
    let data: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        data.get("snapshot").unwrap_or(&data).is_object(),
        "Invalid preset"
    );
    Ok(data)
}

pub fn save(path: &Path, project: &Project) -> Result<()> {
    let dir = path.parent().context("Preset needs a parent folder")?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(
        &mut file,
        &json!({"version":1,"snapshot":snapshot(project)}),
    )?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn directory() -> Result<PathBuf> {
    Ok(crate::preferences::Preferences::directory()?.join("presets"))
}

pub fn list() -> Vec<PathBuf> {
    let mut entries = directory()
        .ok()
        .and_then(|p| std::fs::read_dir(p).ok())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json") && p.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    entries.truncate(200);
    entries
}

/// A motion preset's zoom-in and zoom-out durations, in milliseconds — the
/// two figures the Presets dialog shows for it.
pub fn motion_durations(smooth: bool) -> (f64, f64) {
    if smooth {
        (1522.575, 1015.05)
    } else {
        (200., 200.)
    }
}

pub fn motion(project: &mut Project, smooth: bool) {
    let (zoom_in, zoom_out) = motion_durations(smooth);
    for (key, value) in [
        ("zoomSmoothness", 0.5),
        ("zoomInDurationMs", zoom_in),
        ("zoomOutDurationMs", zoom_out),
        ("cursorSize", 2.5),
        ("cursorSmoothing", 0.67),
        ("cursorSpringMassMultiplier", 1.29),
        ("cursorClickBounce", 2.),
        ("cursorClickBounceDuration", 350.),
        (
            "cursorSpringStiffnessMultiplier",
            if smooth { 0.92 } else { 1.35 },
        ),
        (
            "cursorSpringDampingMultiplier",
            if smooth { 1.36 } else { 0.79 },
        ),
    ] {
        project.set(key, json!(value));
    }
}

/// Compare every setting written by a motion preset so edits display as Custom.
pub fn motion_choice(project: &Project) -> &'static str {
    for (name, smooth) in [("focused", false), ("smooth", true)] {
        let mut reference = Project::new(Path::new("preset-preview.mp4"));
        reference.editor.clear();
        motion(&mut reference, smooth);
        if reference
            .editor
            .iter()
            .all(|(k, v)| project.editor.get(k) == Some(v))
        {
            return name;
        }
    }
    ""
}

pub fn appearance_choice(project: &Project) -> &'static str {
    for Look { name, .. } in LOOKS {
        let mut reference = Project::new(Path::new("preset-preview.mp4"));
        reference.editor.clear();
        appearance(&mut reference, name).expect("built-in appearance");
        if reference
            .editor
            .iter()
            .all(|(k, v)| project.editor.get(k) == Some(v))
        {
            return name;
        }
    }
    ""
}

/// A built-in appearance: what it writes, and so what the Presets dialog
/// previews and prints for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub name: &'static str,
    pub title: &'static str,
    pub wallpaper: &'static str,
    pub padding: u32,
    pub radius: u32,
    pub shadow: f64,
}

pub const LOOKS: [Look; 3] = [
    Look {
        name: "studio",
        title: "Studio",
        wallpaper: "#171c35",
        padding: 24,
        radius: 16,
        shadow: 0.35,
    },
    Look {
        name: "minimal",
        title: "Minimal",
        wallpaper: "#ededed",
        padding: 12,
        radius: 6,
        shadow: 0.1,
    },
    Look {
        name: "bold",
        title: "Bold",
        wallpaper: "#54324a",
        padding: 40,
        radius: 28,
        shadow: 0.5,
    },
];

pub fn appearance(project: &mut Project, name: &str) -> Result<()> {
    let Some(&Look {
        wallpaper,
        padding,
        radius,
        shadow,
        ..
    }) = LOOKS.iter().find(|look| look.name == name)
    else {
        anyhow::bail!("Unknown appearance preset")
    };
    for (key, value) in [
        ("wallpaper", json!(wallpaper)),
        ("padding", json!(padding)),
        ("borderRadius", json!(radius)),
        ("shadowIntensity", json!(shadow)),
    ] {
        project.set(key, value);
    }
    Ok(())
}

/// Remove from the list while retaining a recoverable copy in the local preset trash.
pub fn remove(path: &Path) -> Result<PathBuf> {
    remove_from(&directory()?, path)
}

fn remove_from(directory: &Path, path: &Path) -> Result<PathBuf> {
    ensure!(
        path.parent()
            .context("Preset parent missing")?
            .canonicalize()?
            == directory.canonicalize()?,
        "Preset is outside the managed preset directory"
    );
    let trash = directory.join(".trash");
    std::fs::create_dir_all(&trash)?;
    let target = trash.join(format!(
        "{}-{}",
        uuid::Uuid::new_v4(),
        path.file_name()
            .context("Preset filename missing")?
            .to_string_lossy()
    ));
    std::fs::rename(path, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests;
