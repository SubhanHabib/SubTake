//! Surface planes and the headings drawn on them.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{hairline, row};

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
        Surface::Overlay => Theme::RADIUS_BAR,
        Surface::Popup => Theme::RADIUS_MENU,
        Surface::Card => Theme::RADIUS_ROW,
        Surface::Panel => Theme::RADIUS_PANEL,
    };
    let background = match variant {
        Surface::Popup => theme.card,
        Surface::Card => theme.sunk,
        Surface::Overlay => theme.overlay,
        Surface::Panel => theme.glass,
    };
    let mut el = div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP_BLOCK))
        .rounded(px(radius))
        .bg(background);
    // Every edge in the redesign is an inset shadow, never a border: a border
    // would add to what the panel measures, so a hairline appearing or
    // changing width would move everything inside it. A card is nested inside
    // something that already has an edge, so it gets none of its own.
    //
    // An Overlay gets the hairline and nothing else. It is the one surface
    // that IS its window — the recorder's borderless windows are sized to the
    // plate and the plate is `size_full()` — so a drop shadow has nowhere to
    // fall: it is clipped to the window frame and paints as a grey rectangle
    // in the plate's corners. Its shadow has to come from the window server
    // or not at all.
    let mut shadows = Vec::new();
    if variant != Surface::Card {
        shadows.push(hairline(theme.line, Theme::HAIRLINE_WIDTH));
        if variant != Surface::Overlay {
            shadows.extend(theme.panel_shadow());
        }
    }
    if !shadows.is_empty() {
        el = el.shadow(shadows);
    }
    el
}

/// The default plane: an inspector / timeline panel.
pub fn panel(theme: Theme) -> Div {
    panel_variant(theme, Surface::Panel).p(px(Theme::PANEL_PADDING))
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

/// A section's caption with a rule running out from it. The caption is the
/// same caps label a panel heading uses — the redesign draws "FRAME",
/// "BACKGROUND" and "MOTION" in one style, not two.
pub fn section_label(text: impl Into<SharedString>, theme: Theme) -> Div {
    row().child(caps_label(text, theme)).child(divider(theme))
}
