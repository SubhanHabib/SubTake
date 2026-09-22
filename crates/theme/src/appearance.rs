//! Light or dark, and how a saved preference resolves against the window.

use gpui::WindowAppearance;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

impl Appearance {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }

    /// Resolve the persisted preference (`"dark"` / `"light"` / anything else
    /// meaning "system") against the window's current appearance.
    pub fn resolve(preference: &str, appearance: WindowAppearance) -> Self {
        match preference {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => match appearance {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
                _ => Self::Light,
            },
        }
    }
}
