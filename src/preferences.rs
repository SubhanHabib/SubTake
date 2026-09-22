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
}

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
        }
    }
}

impl Preferences {
    pub fn directory() -> Result<PathBuf> {
        // UI smoke runs have isolated settings/recovery and never change the user's language or recents.
        if std::env::var_os("SUBTAKE_UI_SNAPSHOT").is_some()
            || std::env::var_os("SUBTAKE_LAUNCHER_SMOKE").is_some()
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

    pub fn opened(&mut self, path: PathBuf) {
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(16);
    }
}
