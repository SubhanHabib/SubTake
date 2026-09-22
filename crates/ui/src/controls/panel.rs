//! Surface planes and the headings drawn on them.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::row;

/// Surface planes, matching the original `Panel` variants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Inspector / timeline plane.
    Panel,
    /// Inline card on a panel.
    Card,
    /// Menu or popover.
    Popup,
    /// Floating overlay: the plate a borderless recorder window draws, which
    /// has no material behind it and so carries its own near-opaque tone.
    Overlay,
}

pub fn panel_variant(theme: Theme, variant: Surface) -> Div {
    let radius = match variant {
        Surface::Overlay => Theme::RADIUS_OVERLAY,
        Surface::Card | Surface::Popup => Theme::RADIUS_CARD,
        Surface::Panel => Theme::RADIUS_PANEL,
    };
    let background = match variant {
        Surface::Popup => theme.card,
        Surface::Card => theme.sunk,
        Surface::Overlay => theme.overlay,
        Surface::Panel => theme.glass,
    };
    let el = div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP))
        .rounded(px(radius))
        .bg(background);
    if variant == Surface::Overlay {
        // el.shadow_lg()
        el
    } else {
        el.border_1().border_color(theme.line)
    }
}

/// The default plane: an inspector / timeline panel.
pub fn panel(theme: Theme) -> Div {
    panel_variant(theme, Surface::Panel).p(px(Theme::GAP_LARGE))
}

/// A hairline rule.
pub fn divider(theme: Theme) -> Div {
    div().h(px(Theme::BORDER_WIDTH)).flex_1().bg(theme.line)
}

/// A small muted section caption ("Frame", "Padding", "Animation").
/// The heading weight above `section_label`: a panel's own name and its major
/// sections, set in caps at the small size. gpui at the pinned revision has no
/// letter-spacing, so the caps and the weight carry it on their own.
pub fn caps_label(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .flex_none()
        .text_size(px(Theme::FONT_SMALL))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted)
        .child(SharedString::from(text.into().to_uppercase()))
}

/// A panel's heading strip: its name in caps, then whatever else the panel
/// puts on that line — a reset link, a master switch.
pub fn panel_header(theme: Theme, title: impl Into<SharedString>) -> Div {
    row()
        .h(px(Theme::CONTROL_HEIGHT))
        .flex_none()
        .gap(px(Theme::GAP))
        .child(caps_label(title, theme))
}

pub fn section_label(text: impl Into<SharedString>, theme: Theme) -> Div {
    row()
        .child(
            div()
                .flex_none()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(theme.muted)
                .child(text.into()),
        )
        .child(divider(theme))
}
