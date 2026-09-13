//! Editable bindings are shared across desktop platforms; Primary means Cmd/Ctrl.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;
pub const ACTIONS: [(&str, &str, &str); 6] = [
    ("add-zoom", "Add zoom", "Z"),
    ("split-clip", "Split clip", "C"),
    ("add-text", "Add annotation", "A"),
    ("add-marker", "Add marker", "F"),
    ("delete", "Delete selection", "Primary+D"),
    ("play", "Play / pause", "Space"),
];
#[derive(Debug, PartialEq, Eq)]
pub struct Binding {
    key: String,
    primary: bool,
    shift: bool,
    alt: bool,
}
impl Binding {
    pub fn parse(text: &str) -> Result<Self> {
        let mut result = Self {
            key: String::new(),
            primary: false,
            shift: false,
            alt: false,
        };
        let parts = text.split('+').map(str::trim).collect::<Vec<_>>();
        for (i, part) in parts.iter().enumerate() {
            let p = part.to_lowercase();
            if i == parts.len() - 1 {
                result.key = if p == " " { "space".into() } else { p };
            } else {
                match p.as_str() {
                    "primary" | "cmd" | "command" | "ctrl" | "control" | "super" => {
                        result.primary = true
                    }
                    "shift" => result.shift = true,
                    "alt" | "option" => result.alt = true,
                    _ => anyhow::bail!("Unknown shortcut modifier: {part}"),
                }
            }
        }
        ensure!(!result.key.is_empty(), "Shortcut needs a key");
        Ok(result)
    }
    pub fn matches(&self, key: &str, primary: bool, shift: bool, alt: bool) -> bool {
        self.key
            == if key == " " {
                "space".to_owned()
            } else {
                key.to_lowercase()
            }
            && self.primary == primary
            && self.shift == shift
            && self.alt == alt
    }
}
pub fn defaults() -> BTreeMap<String, String> {
    ACTIONS
        .into_iter()
        .map(|(a, _, b)| (a.into(), b.into()))
        .collect()
}
pub fn validate(action: &str, text: &str, bindings: &BTreeMap<String, String>) -> Result<()> {
    let binding = Binding::parse(text)?;
    for fixed in [
        "Primary+O",
        "Primary+A",
        "Primary+S",
        "Primary+Shift+S",
        "Primary+Z",
        "Primary+Shift+Z",
        "Primary+C",
        "Primary+X",
        "Primary+V",
        "Primary+Shift+D",
    ] {
        ensure!(
            binding != Binding::parse(fixed)?,
            "Shortcut is reserved for a document/edit command"
        );
    }
    for (other, text) in bindings {
        if other != action {
            ensure!(
                binding != Binding::parse(text)?,
                "Shortcut is already assigned to {other}"
            );
        }
    }
    Ok(())
}
pub fn action(
    bindings: &BTreeMap<String, String>,
    key: &str,
    primary: bool,
    shift: bool,
    alt: bool,
) -> Option<String> {
    ACTIONS.iter().find_map(|(action, _, default)| {
        Binding::parse(bindings.get(*action).map(String::as_str).unwrap_or(default))
            .ok()
            .filter(|b| b.matches(key, primary, shift, alt))
            .map(|_| action.to_string())
    })
}
