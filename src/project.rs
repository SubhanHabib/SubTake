//! The document is lossless: fields the native editor does not understand survive save.
//! Platform APIs and UI types must never enter this module.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub version: u32,
    pub video_path: String,
    #[serde(default)]
    pub editor: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Apricot: a warm peach-to-rose gradient, the look a new recording opens
/// in. Its shadow is tinted from the wallpaper and a hairline keeps a light
/// recording's edge on it.
///
/// Not wired: `shadowColor` and `frameEdgeColor` have no inspector control.
/// A new project and the built-in looks set them; the Scene panel cannot.
pub const DEFAULT_WALLPAPER: &str = "linear-gradient(135deg, #f7dcc2, #eda88f, #d27b86)";

/// The editor field listing the timeline lanes turned off, by label.
pub const LANES_OFF: &str = "lanesOff";

impl Project {
    pub fn new(video: &Path) -> Self {
        let editor = json!({
            "wallpaper":DEFAULT_WALLPAPER, "padding":{"top":56,"bottom":56,"left":56,"right":56,"linked":true},
            "borderRadius":4,"shadowIntensity":0.5,"shadowColor":"#5c2224","frameEdgeColor":"#4614141a",
            "backgroundBlur":0,
            "cropRegion":{"x":0,"y":0,"width":1,"height":1},
            "zoomRegions":[],"trimRegions":[],"clipRegions":[],"speedRegions":[],
            "annotationRegions":[],"audioRegions":[],"autoCaptions":[],
            "autoCaptionSettings":{"enabled":true,"fontSize":30,"bottomOffset":3,"maxWidth":62,"maxRows":2,"animationStyle":"fade","boxRadius":17.5,"textColor":"#ffffff","backgroundOpacity":0.9},
            "showCursor":true,"cursorSize":3,"cursorSmoothing":0.67,"cursorClickBounce":2,
            "cursorClickBounceDuration":350,"cursorClickEffect":"ripple","cursorClickEffectColor":"#2563eb",
            "cursorStyle":"tahoe","cursorSway":0.4,"cursorMotionBlur":0.6,
            "zoomInDurationMs":1522.575,"zoomOutDurationMs":1015.05,
            "webcam":{"enabled":false,"mirror":true,"sourcePath":null,"width":40,"height":40,"roundness":100,"margin":24,"positionX":1,"positionY":1},
            "aspectRatio":"16:9","exportFormat":"mp4","exportQuality":"high","mp4FrameRate":30,"gifFrameRate":15,"gifLoop":true
        }).as_object().unwrap().clone();
        Self {
            version: 2,
            video_path: video.to_string_lossy().into(),
            editor,
            extra: json!({"projectId":uuid::Uuid::new_v4().to_string()})
                .as_object()
                .unwrap()
                .clone(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path).context("Read project metadata")?;
        ensure!(
            metadata.len() <= 64 * 1024 * 1024,
            "Project exceeds the 64 MiB document limit"
        );
        let p: Self = serde_json::from_slice(&fs::read(path)?).context("Invalid project JSON")?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=2).contains(&self.version),
            "Unsupported project version {}",
            self.version
        );
        ensure!(
            !self.video_path.trim().is_empty(),
            "Project has no source video"
        );
        for key in [
            "zoomRegions",
            "trimRegions",
            "clipRegions",
            "speedRegions",
            "annotationRegions",
            "audioRegions",
            "autoCaptions",
            "nativeMarkers",
        ] {
            if let Some(value) = self.editor.get(key) {
                let regions = value
                    .as_array()
                    .with_context(|| format!("{key} must be an array"))?;
                let mut ids = std::collections::HashSet::new();
                for region in regions {
                    let start = region["startMs"]
                        .as_f64()
                        .context("Region has no numeric start")?;
                    let end = region["endMs"]
                        .as_f64()
                        .context("Region has no numeric end")?;
                    ensure!(
                        start >= 0.0 && end > start && end.is_finite(),
                        "Invalid time range in {key}"
                    );
                    let id = region["id"].as_str().context("Region has no ID")?;
                    ensure!(ids.insert(id), "Duplicate region ID {id}");
                    if let Some(speed) = region.get("speed") {
                        ensure!(
                            speed.as_f64().is_some_and(|s| s > 0.0 && s <= 100.0),
                            "Invalid playback speed"
                        );
                    }
                }
            }
        }
        Ok(())
    }
    /// Resolve external media relative to the project, keeping bundled assets portable.
    pub fn resolve_assets(&mut self, document: &Path) {
        let base = document.parent().unwrap_or(Path::new("."));
        fn resolve(value: &mut Value, base: &Path) {
            let Some(name) = value.as_str() else { return };
            if name.is_empty()
                || name.starts_with(['#'])
                || name.contains("://")
                || name.starts_with("data:")
                || name.contains("gradient(")
                || name.trim_start_matches('/').starts_with("wallpapers/")
            {
                return;
            }
            let path = local_path(name);
            if path.is_relative() {
                *value = json!(base.join(path));
            }
        }
        if let Some(bg) = self.editor.get_mut("wallpaper") {
            resolve(bg, base);
        }
        if let Some(path) = self
            .editor
            .get_mut("webcam")
            .and_then(|v| v.get_mut("sourcePath"))
        {
            resolve(path, base);
        }
        for (key, field) in [
            ("annotationRegions", "imageContent"),
            ("audioRegions", "audioPath"),
        ] {
            if let Some(regions) = self.editor.get_mut(key).and_then(Value::as_array_mut) {
                for region in regions {
                    if let Some(path) = region.get_mut(field) {
                        resolve(path, base);
                    }
                }
            }
        }
    }

    pub fn source_path(&self, document_path: Option<&Path>) -> PathBuf {
        let path = local_path(&self.video_path);
        if path.is_absolute() {
            path
        } else {
            document_path
                .and_then(Path::parent)
                .unwrap_or(Path::new("."))
                .join(path)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temp, self)?;
        temp.write_all(b"\n")?;
        ensure!(
            temp.as_file().metadata()?.len() <= 64 * 1024 * 1024,
            "Project exceeds the 64 MiB document limit"
        );
        temp.as_file().sync_all()?;
        if path.exists() {
            let mut backup = tempfile::NamedTempFile::new_in(parent)?;
            std::io::copy(&mut fs::File::open(path)?, &mut backup)?;
            backup.as_file().sync_all()?;
            let mut name = path.as_os_str().to_os_string();
            name.push(".bak");
            backup.persist(PathBuf::from(name)).map_err(|e| e.error)?;
        }
        temp.persist(path)
            .map_err(|e| e.error)
            .context("Replace project atomically")?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    pub fn number(&self, key: &str, default: f64) -> f64 {
        self.editor
            .get(key)
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite())
            .unwrap_or(default)
    }

    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.editor
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    pub fn text<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.editor
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(default)
    }

    pub fn regions(&self, key: &str) -> &[Value] {
        self.editor
            .get(key)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.editor.insert(key.into(), value);
    }

    /// Whether the timeline lane with this label (`Zoom`, `Clip`,
    /// `Annotation`, `Caption` or `Audio`) is turned off from its header:
    /// the audio lane muted, any other hidden. Saved in `lanesOff`.
    pub fn lane_off(&self, label: &str) -> bool {
        self.regions(LANES_OFF)
            .iter()
            .any(|lane| lane.as_str() == Some(label))
    }

    /// Turns a lane off, or on again.
    pub fn toggle_lane(&mut self, label: &str) {
        let mut lanes: Vec<Value> = self.regions(LANES_OFF).to_vec();
        if self.lane_off(label) {
            lanes.retain(|lane| lane.as_str() != Some(label));
        } else {
            lanes.push(json!(label));
        }
        if lanes.is_empty() {
            self.editor.remove(LANES_OFF);
        } else {
            self.set(LANES_OFF, Value::Array(lanes));
        }
    }

    /// The project as the picture plays it: a hidden zoom, annotation or
    /// caption lane has nothing on it. A hidden clip lane and a muted audio
    /// lane are left to the renderer and the mix, which ask `lane_off`.
    pub fn played(&self) -> std::borrow::Cow<'_, Project> {
        let hidden: Vec<&str> = [
            ("Zoom", "zoomRegions"),
            ("Annotation", "annotationRegions"),
            ("Caption", "autoCaptions"),
        ]
        .into_iter()
        .filter(|(label, _)| self.lane_off(label))
        .map(|(_, key)| key)
        .collect();
        if hidden.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut played = self.clone();
        for key in hidden {
            played.set(key, json!([]));
        }
        std::borrow::Cow::Owned(played)
    }

    pub fn add(&mut self, key: &str, mut region: Value) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        region["id"] = json!(id);
        self.editor
            .entry(key.to_owned())
            .or_insert(json!([]))
            .as_array_mut()
            .context("Expected region array")?
            .push(region);
        Ok(id)
    }

    pub fn change_region(&mut self, key: &str, id: &str, patch: Value) -> Result<()> {
        let region = self
            .editor
            .get_mut(key)
            .and_then(Value::as_array_mut)
            .and_then(|a| a.iter_mut().find(|r| r["id"] == id))
            .context("Region no longer exists")?;
        let original = region.clone();
        for (k, v) in patch
            .as_object()
            .context("Region patch must be an object")?
        {
            if k != "id" {
                region[k] = v.clone();
            }
        }
        if key == "autoCaptions" && region["words"] == original["words"] {
            if region["text"] != original["text"] {
                region.as_object_mut().unwrap().remove("words");
            } else {
                retime_caption_words(
                    region,
                    crate::timeline::n(&original, "startMs", 0.),
                    crate::timeline::n(&original, "endMs", 0.),
                );
            }
        }
        Ok(())
    }

    pub fn remove_region(&mut self, key: &str, id: &str) -> Result<()> {
        let regions = self
            .editor
            .get_mut(key)
            .and_then(Value::as_array_mut)
            .context("Expected region array")?;
        let len = regions.len();
        regions.retain(|r| r["id"] != id);
        ensure!(regions.len() < len, "Region no longer exists");
        Ok(())
    }
}

pub struct History {
    pub project: Project,
    past: Vec<Project>,
    future: Vec<Project>,
    saved: Option<Project>,
}

impl History {
    pub fn new(project: Project) -> Self {
        Self {
            saved: Some(project.clone()),
            project,
            past: vec![],
            future: vec![],
        }
    }

    pub fn edit(&mut self, change: impl FnOnce(&mut Project) -> Result<()>) -> Result<()> {
        let mut next = self.project.clone();
        change(&mut next)?;
        next.validate()?;
        if next != self.project {
            self.past.push(std::mem::replace(&mut self.project, next));
            if self.past.len() > 100 {
                self.past.remove(0);
            }
            self.future.clear();
        }
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.past.pop() {
            self.future.push(std::mem::replace(&mut self.project, p));
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.future.pop() {
            self.past.push(std::mem::replace(&mut self.project, p));
        }
    }

    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.project)
    }

    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.project.clone());
    }
}

pub fn parse_color(text: &str) -> [u8; 4] {
    let text = text.trim().trim_start_matches('#');
    if text.len() == 8
        && let Ok(n) = u32::from_str_radix(text, 16)
    {
        return [(n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8];
    }
    if text.len() == 6
        && let Ok(n) = u32::from_str_radix(text, 16)
    {
        return [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255];
    }
    if text.len() == 3
        && let Ok(n) = u16::from_str_radix(text, 16)
    {
        return [
            ((n >> 8) & 15) as u8 * 17,
            ((n >> 4) & 15) as u8 * 17,
            (n & 15) as u8 * 17,
            255,
        ];
    }
    if text == "transparent" {
        return [0, 0, 0, 0];
    }
    [23, 28, 53, 255]
}

pub fn parse_srt(text: &str) -> Result<Vec<Value>> {
    fn time(text: &str) -> Result<f64> {
        let numbers: Vec<f64> = text
            .trim()
            .replace(',', ".")
            .split(':')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        if numbers.len() != 3 {
            bail!("Invalid subtitle time")
        };
        Ok((numbers[0] * 3600. + numbers[1] * 60. + numbers[2]) * 1000.)
    }
    let mut cues = vec![];
    for block in text.replace('\r', "").split("\n\n") {
        let mut lines = block.lines();
        let Some(first) = lines.next() else { continue };
        let timing = if first.contains("-->") {
            first
        } else {
            lines.next().unwrap_or("")
        };
        if let Some((a, b)) = timing.split_once("-->") {
            let start = time(a)?;
            let end = time(b)?;
            ensure!(end > start, "Invalid subtitle range");
            cues.push(json!({"id":uuid::Uuid::new_v4().to_string(),"startMs":start,"endMs":end,"text":lines.collect::<Vec<_>>().join("\n")}));
        }
    }
    Ok(cues)
}

pub fn local_path(value: &str) -> PathBuf {
    if value.starts_with("file:")
        && let Ok(url) = url::Url::parse(value)
        && let Ok(path) = url.to_file_path()
    {
        return path;
    }
    PathBuf::from(value)
}

/// Keep recognized word timings attached when a caption is moved or resized.
pub fn retime_caption_words(cue: &mut Value, old_start: f64, old_end: f64) {
    use crate::timeline::n;
    let start = n(cue, "startMs", 0.).round();
    let mut end = n(cue, "endMs", 0.).round();
    if start == old_start && end == old_end {
        return;
    }
    if let Some(words) = cue.get_mut("words").and_then(Value::as_array_mut) {
        end = end.max(start + words.len() as f64);
        let factor = (end - start) / (old_end - old_start).max(1.);
        let mut cursor = start;
        for (i, word) in words.iter_mut().enumerate() {
            let a = (start + (n(word, "startMs", old_start) - old_start) * factor)
                .round()
                .max(cursor)
                .min(end - 1.);
            let b = (start + (n(word, "endMs", old_end) - old_start) * factor)
                .round()
                .max(a + 1.)
                .min(end);
            word["startMs"] = json!(a);
            word["endMs"] = json!(b);
            cursor = b;
            if i > 0 {
                word["leadingSpace"] = json!(true);
            } else {
                word.as_object_mut().unwrap().remove("leadingSpace");
            }
        }
    }
    cue["startMs"] = json!(start);
    cue["endMs"] = json!(end);
}
