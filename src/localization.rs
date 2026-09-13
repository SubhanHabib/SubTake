//! Embedded translations; native-only labels fall back to English.
use serde_json::Value;
use std::sync::OnceLock;
pub const LOCALES: &[&str] = &[
    "en", "es", "fr", "de", "it", "nl", "ko", "pt-BR", "zh-CN", "zh-TW",
];
pub fn translate(text: &str, locale: &str) -> String {
    if locale == "en" || !LOCALES.contains(&locale) {
        return text.into();
    }
    if let Some(rest) = text.strip_prefix("+ ") {
        return format!("+ {}", translate(rest, locale));
    }
    static CATALOG: OnceLock<Value> = OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/localization.json"))
            .expect("embedded localization catalog")
    });
    let key = text
        .trim()
        .trim_end_matches("...")
        .trim_end_matches('…')
        .to_lowercase();
    catalog[locale][&key]
        .as_str()
        .map(|value| {
            if (text.ends_with('…') || text.ends_with("...")) && !value.ends_with(['…', '.']) {
                format!("{value}…")
            } else {
                value.into()
            }
        })
        .unwrap_or_else(|| text.into())
}
