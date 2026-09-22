//! Surface planes and the headings drawn on them.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{frost, hairline, row};

/// Surface planes.
///
/// The split that matters is `Panel` against `Content`. The handoff gives one
/// rule for which fill a float takes — "`--glass` for controls, `--card` for
/// content" — and a single `Panel` variant could not express it: the console
/// is a strip of buttons and belongs on glass, the inspector is a column of
/// text and belongs on card. Both were glass, so the inspector's labels sat
/// on the same translucent plane as the desktop behind them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// A float that holds controls: the console, the player bar.
    Panel,
    /// A float that holds text: the inspector, a dialog.
    Content,
    /// A float that holds nothing but icons: the tool pod, the aspect pod.
    Pod,
    /// Inline card on a panel.
    Card,
    /// Menu or popover.
    Popup,
    /// Floating overlay: the plate a borderless recorder window draws. It is
    /// the `glass` tint over a native material masked to exactly this plate,
    /// so it must be drawn at `radius()` and fill its window.
    Overlay,
}

impl Surface {
    /// The radius this plane is drawn at. Public because a backdrop blur has
    /// to be given the same rounding as the plate it sits under, or it paints
    /// square corners behind round ones.
    pub fn radius(self) -> f32 {
        match self {
            Self::Overlay => Theme::RADIUS_BAR,
            Self::Pod => Theme::RADIUS_POD,
            Self::Popup => Theme::RADIUS_MENU,
            Self::Card => Theme::RADIUS_ROW,
            Self::Panel | Self::Content => Theme::RADIUS_PANEL,
        }
    }

    /// How much of what is behind this plane it takes out. A float's blur is
    /// chosen by what it is, not by how big it is.
    pub fn blur(self) -> f32 {
        match self {
            Self::Pod => frost::POD_BLUR,
            Self::Overlay => frost::BAR_BLUR,
            Self::Popup => frost::MENU_BLUR,
            Self::Card | Self::Panel | Self::Content => frost::PANEL_BLUR,
        }
    }
}

pub fn panel_variant(theme: Theme, variant: Surface) -> Div {
    let radius = variant.radius();
    let background = match variant {
        Surface::Popup | Surface::Content => theme.card,
        Surface::Card => theme.sunk,
        Surface::Panel | Surface::Pod | Surface::Overlay => theme.glass,
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

/// The default plane: a console-style panel holding controls.
pub fn panel(theme: Theme) -> Div {
    panel_variant(theme, Surface::Panel).p(px(Theme::PANEL_PADDING))
}

/// A panel holding text rather than controls — the inspector, a dialog.
pub fn content_panel(theme: Theme) -> Div {
    panel_variant(theme, Surface::Content).p(px(Theme::PANEL_PADDING))
}

/// A pod: a small float carrying nothing but icons, over the stage. Tight
/// padding and a wider radius than a panel, so a 40px control inside it very
/// nearly fills it and the plate reads as a holder rather than a container.
pub fn pod(theme: Theme) -> Div {
    panel_variant(theme, Surface::Pod)
        .flex_row()
        .items_center()
        .p(px(Theme::POD_PADDING))
        .gap(px(Theme::GAP_SMALL))
        .occlude()
}

/// A hairline rule, one row of a stack.
///
/// It takes no part in the distribution of its parent's spare height: a
/// `flex_1` here reads as "grow" on a column's main axis, which turns the
/// hairline into a filled block and starves whatever scrolls above it. A rule
/// that has to run out along a row asks for the growth itself.
pub fn divider(theme: Theme) -> Div {
    div().h(px(Theme::BORDER_WIDTH)).flex_none().bg(theme.line)
}

/// A small muted section caption ("Frame", "Padding", "Animation"), set in
/// caps at the small size. gpui at the pinned revision has no letter-spacing,
/// so the caps and the weight carry it on their own.
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
