//! The three families, and the two that are not the default.
//!
//! Geist is the interface face and the root default, so almost nothing asks
//! for it by name. The other two are asked for here rather than at call
//! sites, because both had been bundled, registered and verified present
//! while nothing in the app ever named them — every timecode and every
//! headline rendered in Geist while the font registry reported three
//! families available.
//!
//! Geist Mono is for anything numeric that changes under the user's hand:
//! timecodes, ruler ticks, durations, hex values, a slider's value, a
//! shortcut hint. Digits in a proportional face reflow as they count, so a
//! timecode ticking from 9 to 10 shifts every glyph beside it.
//!
//! Space Grotesk is for titles only — never inside a control.

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, FONT_TITLE, Theme};

/// A number that changes: a timecode, a duration, a measured value, a hex
/// string. Takes the size and colour of whatever holds it unless told
/// otherwise, so it drops into a row without restating the row's styling.
pub fn mono(text: impl Into<SharedString>) -> Div {
    div().font_family(FONT_MONO).child(text.into())
}

/// A piece of metadata at the small size — a ruler tick, a region's
/// timecode, the meta line under a preset's name.
pub fn mono_small(text: impl Into<SharedString>, theme: Theme) -> Div {
    mono(text)
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
}

/// A title. Space Grotesk at medium, at whatever size the surface asks for:
/// 40 on the empty state, 22 on a panel or dialog, 17 on an inspector
/// heading. It is a display face, so it never appears inside a control.
pub fn title(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .font_family(FONT_TITLE)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(size))
        .child(text.into())
}

/// A panel's or dialog's own name.
pub fn panel_title(text: impl Into<SharedString>) -> Div {
    title(text, Theme::FONT_PANEL)
}

/// An inspector's heading — the step below a panel title.
pub fn heading(text: impl Into<SharedString>) -> Div {
    title(text, Theme::FONT_HEADING)
}
