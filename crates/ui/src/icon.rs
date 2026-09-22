//! Themed SVG glyphs at the design system's icon sizes.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

/// A themed glyph at the shared 16px icon size.
pub fn icon(name: &str, color: Hsla) -> Svg {
    icon_sized(name, Theme::ICON_SIZE, color)
}

/// A glyph at a caller-chosen size (the record dot and play triangle run larger).
pub fn icon_sized(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(format!("assets/icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(color)
}
