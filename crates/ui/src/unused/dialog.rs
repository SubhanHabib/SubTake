//! A modal dialog and the scrim behind it: "radius 22, 16px padding, actions
//! right-aligned at 40 tall. Scrim is `rgba(10,10,14,.4)` light / `.55` dark,
//! no blur."

use gpui::{prelude::*, *};
use subtake_theme::{Appearance, Theme};

use crate::{Surface, panel_variant, row};

const DIALOG_PADDING: f32 = 16.0;
const DIALOG_ACTION_HEIGHT: f32 = 40.0;
const DIALOG_ACTION_GAP: f32 = 8.0;

/// The wash behind a modal.
///
/// No blur, and that is the point rather than an omission: every other float
/// in the app blurs what is behind it, so the one surface that does not is
/// the one that has stopped the app rather than covered part of it. The tone
/// is a fixed near-black in both appearances — it darkens the window, and a
/// light scrim on a light window would not.
pub fn scrim(theme: Theme) -> Div {
    let alpha = match theme.appearance {
        Appearance::Light => 0.40,
        Appearance::Dark => 0.55,
    };
    div()
        .absolute()
        .inset_0()
        .bg(hsla(250. / 360., 0.17, 0.05, alpha))
        .occlude()
}

/// The dialog plate. `Surface::Popup` gives it the card fill, the hairline
/// and the float shadow; only the radius and padding are its own.
pub fn dialog(theme: Theme) -> Div {
    panel_variant(theme, Surface::Popup)
        .rounded(px(Theme::radius_row()))
        .p(px(DIALOG_PADDING))
        .occlude()
}

/// A dialog's footer: its buttons, right-aligned, at the large control
/// height. Right-aligned because the last thing read before a decision should
/// be nearest the pointer that makes it.
pub fn dialog_actions() -> Div {
    row()
        .flex_none()
        .justify_end()
        .h(px(DIALOG_ACTION_HEIGHT))
        .gap(px(DIALOG_ACTION_GAP))
}
