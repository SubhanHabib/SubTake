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
    "shadowColor",
    "frameEdgeColor",
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
    let on = groups(data);
    let data = data.get("snapshot").unwrap_or(data);
    ensure!(data.is_object(), "Preset must contain settings");
    let source = project
        .editor
        .get("webcam")
        .and_then(|v| v.get("sourcePath"))
        .cloned();
    for key in KEYS {
        if !on.iter().any(|group| group_of(key) == Some(group.as_str())) {
            continue;
        }
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
    write(path, &json!({"version":1,"snapshot":snapshot(project)}))
}

/// Replace `path` with `data` in one step, so a crash leaves the old file.
fn write(path: &Path, data: &Value) -> Result<()> {
    let dir = path.parent().context("Preset needs a parent folder")?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut file, data)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// The parts a saved preset can carry, by key, name and the project
/// settings each writes. Every key in `KEYS` is in exactly one.
///
/// A preset keeps every setting it was saved with and applies only the
/// parts it has on, so turning a part off and on again loses nothing.
pub const GROUPS: [(&str, &str, &[&str]); 6] = [
    (
        "look",
        "Look",
        &[
            "wallpaper",
            "shadowIntensity",
            "shadowColor",
            "frameEdgeColor",
            "backgroundBlur",
            "borderRadius",
            "borderRadiusUnit",
            "padding",
            "aspectRatio",
            "cropRegion",
        ],
    ),
    (
        "motion",
        "Motion",
        &[
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
            "cameraSpringStiffnessMultiplier",
            "cameraSpringDampingMultiplier",
            "cameraSpringMassMultiplier",
        ],
    ),
    (
        "cursor",
        "Cursor",
        &[
            "showCursor",
            "loopCursor",
            "cursorStyle",
            "cursorSize",
            "cursorSmoothing",
            "cursorSpringStiffnessMultiplier",
            "cursorSpringDampingMultiplier",
            "cursorSpringMassMultiplier",
            "cursorMotionBlur",
            "cursorClickEffect",
            "cursorClickEffectColor",
            "cursorClickEffectScale",
            "cursorClickEffectOpacity",
            "cursorClickEffectDurationMs",
            "cursorClickBounce",
            "cursorClickBounceDuration",
            "cursorSway",
        ],
    ),
    ("camera", "Camera", &["webcam"]),
    (
        "captions",
        "Captions",
        &[
            "autoCaptionSettings",
            "nativeFonts",
            "nativeCaptionSidecars",
        ],
    ),
    (
        "export",
        "Export",
        &[
            "exportEncodingMode",
            "exportBackendPreference",
            "exportPipelineModel",
            "exportQuality",
            "mp4FrameRate",
            "exportFormat",
            "gifFrameRate",
            "gifLoop",
            "gifSizePreset",
            "nativeExportWidth",
            "nativeExportHeight",
            "nativeExportFps",
            "nativeExportQuality",
            "nativeExportHardware",
            "nativeExportGif",
            "nativeExportLoop",
        ],
    ),
];

fn group_of(key: &str) -> Option<&'static str> {
    GROUPS
        .iter()
        .find(|(_, _, keys)| keys.contains(&key))
        .map(|(group, _, _)| *group)
}

/// The parts a preset file has on. A file saved before there were parts
/// carries all of them, as it always did.
pub fn groups(data: &Value) -> Vec<String> {
    match data.get("groups").and_then(Value::as_array) {
        Some(on) => on
            .iter()
            .filter_map(Value::as_str)
            .filter(|g| GROUPS.iter().any(|(group, _, _)| group == g))
            .map(str::to_owned)
            .collect(),
        None => GROUPS.iter().map(|(group, _, _)| (*group).into()).collect(),
    }
}

/// A preset's name is its file's name.
pub fn name(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A free file in `directory` for a preset called `base`, counting up
/// from `base 2` when that name is taken.
fn unused(directory: &Path, base: &str) -> PathBuf {
    let path = directory.join(format!("{base}.json"));
    if !path.exists() {
        return path;
    }
    (2..)
        .map(|n| directory.join(format!("{base} {n}.json")))
        .find(|p| !p.exists())
        .expect("a free name")
}

fn managed(directory: &Path, path: &Path) -> Result<()> {
    ensure!(
        path.parent()
            .context("Preset parent missing")?
            .canonicalize()?
            == directory.canonicalize()?,
        "Preset is outside the managed preset directory"
    );
    Ok(())
}

/// Save the project's settings as a new preset in `directory` carrying
/// `on`, named "Preset", "Preset 2" and so on.
pub fn create(directory: &Path, project: &Project, on: &[&str]) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    let path = unused(directory, "Preset");
    write(
        &path,
        &json!({"version":1,"groups":on,"snapshot":snapshot(project)}),
    )?;
    Ok(path)
}

/// Rename a preset, returning where it now is. A name another preset has
/// is refused rather than overwritten.
pub fn rename(directory: &Path, path: &Path, to: &str) -> Result<PathBuf> {
    managed(directory, path)?;
    let to = to.trim();
    ensure!(!to.is_empty(), "A preset needs a name");
    ensure!(
        !to.starts_with('.') && !to.contains(['/', '\\', ':']),
        "A preset name can't contain / \\ or : or start with a dot"
    );
    let target = directory.join(format!("{to}.json"));
    // On a case-insensitive disk a new casing of the same name exists
    // already, and is the same file.
    let same = target
        .canonicalize()
        .is_ok_and(|t| path.canonicalize().is_ok_and(|p| p == t));
    ensure!(
        same || !target.exists(),
        "A preset called {to} already exists"
    );
    std::fs::rename(path, &target)?;
    Ok(target)
}

/// Copy a preset beside itself as "<name> copy".
pub fn duplicate(directory: &Path, path: &Path) -> Result<PathBuf> {
    managed(directory, path)?;
    let target = unused(directory, &format!("{} copy", name(path)));
    std::fs::copy(path, &target)?;
    Ok(target)
}

/// Replace a preset's settings with the project's, keeping its parts.
pub fn update(path: &Path, project: &Project) -> Result<()> {
    let mut data = load(path)?;
    let on = groups(&data);
    data = json!({"version":1,"groups":on,"snapshot":snapshot(project)});
    write(path, &data)
}

/// Turn one of a preset's parts on or off. The last part stays on, since
/// a preset with none would apply nothing.
pub fn set_group(path: &Path, group: &str, on: bool) -> Result<()> {
    ensure!(
        GROUPS.iter().any(|(g, _, _)| *g == group),
        "Unknown preset part"
    );
    let mut data = load(path)?;
    let mut groups = groups(&data);
    groups.retain(|g| g != group);
    if on {
        groups.push(group.into());
    }
    ensure!(!groups.is_empty(), "A preset needs at least one part");
    // Kept in the table's order, so the file reads the same however the
    // parts were toggled.
    groups.sort_by_key(|g| GROUPS.iter().position(|(k, _, _)| k == g));
    let snapshot = data.get("snapshot").cloned().unwrap_or(data.clone());
    data = json!({"version":1,"groups":groups,"snapshot":snapshot});
    write(path, &data)
}

/// Copy a preset file into `directory` under its own name, or the next
/// free one.
pub fn import(directory: &Path, from: &Path) -> Result<PathBuf> {
    load(from)?;
    std::fs::create_dir_all(directory)?;
    let target = unused(directory, &name(from));
    std::fs::copy(from, &target)?;
    Ok(target)
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
    /// The shadow's tint and the frame's hairline. A look sets both, so
    /// one applied over a tinted project does not keep its tint.
    pub shadow_color: &'static str,
    pub edge: &'static str,
}

pub const LOOKS: [Look; 3] = [
    Look {
        name: "studio",
        title: "Studio",
        wallpaper: "#171c35",
        padding: 24,
        radius: 16,
        shadow: 0.35,
        shadow_color: "#000000",
        edge: "transparent",
    },
    Look {
        name: "minimal",
        title: "Minimal",
        wallpaper: "#ededed",
        padding: 12,
        radius: 6,
        shadow: 0.1,
        shadow_color: "#000000",
        edge: "transparent",
    },
    Look {
        name: "bold",
        title: "Bold",
        wallpaper: "#54324a",
        padding: 40,
        radius: 28,
        shadow: 0.5,
        shadow_color: "#000000",
        edge: "transparent",
    },
];

pub fn appearance(project: &mut Project, name: &str) -> Result<()> {
    let Some(&Look {
        wallpaper,
        padding,
        radius,
        shadow,
        shadow_color,
        edge,
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
        ("shadowColor", json!(shadow_color)),
        ("frameEdgeColor", json!(edge)),
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
    managed(directory, path)?;
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
