//! The Presets dialog's saved presets: making, naming, parting and sharing
//! them, and which one new videos start from. Every command addresses a
//! preset by its place in `App::presets`, as `apply-preset-{index}` does.

use super::*;
use subtake_native::presets;

impl App {
    /// Run `action` if it is one of the saved-preset commands, reporting
    /// whether it was.
    pub(super) fn preset_action(&mut self, ui: &EditorWindow, action: &str) -> Result<bool> {
        if action == "new-preset" {
            let parts: Vec<&str> = presets::GROUPS.iter().map(|(g, _, _)| *g).collect();
            let path = presets::create(&presets::directory()?, self.project()?, &parts)?;
            self.presets_changed(ui, Some(&path));
            return Ok(true);
        }
        if action == "import-preset" {
            if let Some(from) = rfd::FileDialog::new()
                .set_title("Import preset")
                .add_filter("SubTake preset", &["json"])
                .pick_file()
            {
                let path = presets::import(&presets::directory()?, &from)?;
                self.presets_changed(ui, Some(&path));
            }
            return Ok(true);
        }
        if let Some(index) = action.strip_prefix("duplicate-preset-") {
            let path = presets::duplicate(&presets::directory()?, self.preset(index)?)?;
            self.presets_changed(ui, Some(&path));
            return Ok(true);
        }
        if let Some(index) = action.strip_prefix("update-preset-") {
            let path = self.preset(index)?.clone();
            presets::update(&path, self.project()?)?;
            ui.set_status(format!(
                "{} now holds this project's settings",
                presets::name(&path)
            ));
            self.presets_changed(ui, Some(&path));
            return Ok(true);
        }
        if let Some(index) = action.strip_prefix("share-preset-") {
            let path = self.preset(index)?.clone();
            if let Some(to) = rfd::FileDialog::new()
                .set_title("Export preset")
                .set_file_name(format!("{}.json", presets::name(&path)))
                .add_filter("SubTake preset", &["json"])
                .save_file()
            {
                std::fs::copy(&path, &to)?;
                ui.set_status(format!("Preset exported to {}", to.display()));
            }
            return Ok(true);
        }
        if let Some(index) = action.strip_prefix("default-preset-") {
            let name = presets::name(self.preset(index)?);
            let chosen = self.preferences.default_preset.as_ref() == Some(&name);
            self.preferences.default_preset = (!chosen).then_some(name);
            self.preferences.save()?;
            self.refresh(ui);
            return Ok(true);
        }
        // `preset-part-{index}-{part}`, turning that part over.
        if let Some(rest) = action.strip_prefix("preset-part-") {
            let (index, part) = rest.split_once('-').context("Preset part missing")?;
            let path = self.preset(index)?.clone();
            let on = presets::groups(&presets::load(&path)?)
                .iter()
                .any(|g| g == part);
            presets::set_group(&path, part, !on)?;
            self.presets_changed(ui, Some(&path));
            return Ok(true);
        }
        Ok(false)
    }

    /// Rename the preset at `index`, following it as the default.
    pub(super) fn rename_preset(&mut self, ui: &EditorWindow, index: &str, to: &str) -> Result<()> {
        let path = self.preset(index)?.clone();
        let old = presets::name(&path);
        let path = presets::rename(&presets::directory()?, &path, to)?;
        if self.preferences.default_preset.as_ref() == Some(&old) {
            self.preferences.default_preset = Some(presets::name(&path));
            self.preferences.save()?;
        }
        self.presets_changed(ui, Some(&path));
        Ok(())
    }

    fn preset(&self, index: &str) -> Result<&PathBuf> {
        self.presets
            .get(index.parse::<usize>()?)
            .context("Preset no longer exists")
    }

    /// Read the list again, keeping `select` selected in the dialog.
    pub(super) fn presets_changed(&mut self, ui: &EditorWindow, select: Option<&Path>) {
        self.presets = presets::list();
        if let Some(path) = select {
            ui.set_selected_preset(presets::name(path));
        }
        self.refresh(ui);
    }
}
