//! User preferences are separate from portable project documents.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: String,
    pub appearance: String,
    pub auto_apply_zooms: bool,
    pub library_directory: Option<PathBuf>,
    pub recording_directory: Option<PathBuf>,
    pub whisper_model: Option<PathBuf>,
    pub whisper_runtime: Option<PathBuf>,
    pub record_shortcut: String,
    pub pause_shortcut: String,
    pub countdown_seconds: u32,
    pub recent_projects: Vec<PathBuf>,
    pub editor_shortcuts: std::collections::BTreeMap<String, String>,
    /// The editor's lane stack height and inspector width, where they were
    /// last dragged to. `None` until the first drag.
    pub lane_height: Option<f32>,
    pub inspector_width: Option<f32>,
    /// The saved preset, by name, a newly opened video starts from.
    pub default_preset: Option<String>,
    /// What the recorder's cards set beyond the source and devices, by the
    /// key a card sends it under (`RECORDER_DEFAULTS`). Only what has been
    /// changed is kept.
    pub recorder: std::collections::BTreeMap<String, String>,
}

/// Each recorder setting and what it is until changed.
pub const RECORDER_DEFAULTS: &[(&str, &str)] = &[
    ("hide-desktop-icons", "false"),
    ("show-recorder", "false"),
    ("area-aspect", "free"),
    // The area last drawn: its display's id, then its left, top, width and
    // height in points from that display's top-left corner. Empty before
    // one is drawn.
    ("area", ""),
    ("count-on-screen", "true"),
    ("tick-sound", "false"),
    // A corner, or `custom` once the camera has been dragged on the stage
    // in the editor, to `camera-position`: the project's positionX and
    // positionY, 0–1 across the room the overlay can move in.
    ("camera-corner", "bottom-right"),
    ("camera-position", ""),
    ("camera-shape", "circle"),
    ("camera-size", "m"),
    ("resolution", "native"),
    ("frame-rate", "60"),
    ("input-level", "100"),
    ("hide-bar", "false"),
];

impl Default for Preferences {
    fn default() -> Self {
        let modifier = if cfg!(target_os = "macos") {
            "Super"
        } else {
            "Control"
        };
        Self {
            language: "en".into(),
            appearance: "system".into(),
            auto_apply_zooms: true,
            library_directory: None,
            recording_directory: None,
            whisper_model: None,
            whisper_runtime: None,
            record_shortcut: format!("{modifier}+Shift+R"),
            pause_shortcut: format!("{modifier}+Shift+P"),
            countdown_seconds: 3,
            recent_projects: vec![],
            editor_shortcuts: crate::shortcuts::defaults(),
            lane_height: None,
            inspector_width: None,
            default_preset: None,
            recorder: Default::default(),
        }
    }
}

impl Preferences {
    pub fn directory() -> Result<PathBuf> {
        // UI smoke runs have isolated settings/recovery and never change the user's language or recents.
        if std::env::var_os("SUBTAKE_UI_SNAPSHOT").is_some()
            || std::env::var_os("SUBTAKE_LAUNCHER_SMOKE").is_some()
            || std::env::var_os("SUBTAKE_WALKTHROUGH").is_some()
        {
            return Ok(std::env::temp_dir().join(format!("SubTake-ui-test-{}", std::process::id())));
        }
        Ok(
            directories::ProjectDirs::from("com", "SubTake", "SubTake Native")
                .context("Find application data directory")?
                .config_dir()
                .to_path_buf(),
        )
    }

    pub fn load() -> Result<Self> {
        let path = Self::directory()?.join("preferences.json");
        let mut preferences: Self = if path.exists() {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Self::default()
        };
        if preferences
            .whisper_model
            .as_ref()
            .is_none_or(|p| !p.is_file())
        {
            let model = Self::directory()?.join("whisper/ggml-small.bin");
            if model.is_file() {
                preferences.whisper_model = Some(model);
            }
        }
        Ok(preferences)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::directory()?;
        std::fs::create_dir_all(&dir)?;
        let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
        serde_json::to_writer_pretty(&mut temp, self)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(dir.join("preferences.json"))
            .map_err(|e| e.error)?;
        Ok(())
    }

    /// A recorder setting, or its default; empty for a key there is none of.
    pub fn recorder_setting(&self, key: &str) -> &str {
        self.recorder
            .get(key)
            .map(String::as_str)
            .unwrap_or_else(|| {
                RECORDER_DEFAULTS
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map_or("", |(_, v)| v)
            })
    }

    /// Sets a recorder setting, if it is one. Returns whether it was.
    pub fn set_recorder_setting(&mut self, key: &str, value: &str) -> bool {
        if !RECORDER_DEFAULTS.iter().any(|(k, _)| *k == key) {
            return false;
        }
        self.recorder.insert(key.to_owned(), value.to_owned());
        true
    }

    /// Every recorder setting, defaults filled in.
    pub fn recorder_settings(&self) -> std::collections::BTreeMap<String, String> {
        RECORDER_DEFAULTS
            .iter()
            .map(|(k, _)| ((*k).to_owned(), self.recorder_setting(k).to_owned()))
            .collect()
    }

    pub fn opened(&mut self, path: PathBuf) {
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(16);
    }
}
