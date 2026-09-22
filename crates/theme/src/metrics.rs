//! Sizes, radii, type scale, fonts and layout columns.

use super::Theme;

impl Theme {
    // ---- window material -------------------------------------------------

    /// Whether translucent window chrome can be composited.
    ///
    /// SubTake renders on `zeronsh/zui`, which carries the two fixes this
    /// needs and stock gpui 0.2.2 lacks: `f596cde` (destination alpha on
    /// transparent windows — Porter-Duff OVER, not additive) and `8a8954c`
    /// (macOS blurred view on `UnderWindowBackground`, because macOS 26
    /// stopped vending `CABackdropLayer` for the `Selection` material).
    ///
    /// Every surface token above assumes this is on: they are tints, not
    /// paints. With it off the whole interface would wash out.
    pub const WINDOW_GLASS_SUPPORTED: bool = true;

    // ---- metrics ---------------------------------------------------------

    pub const DISABLED_OPACITY: f32 = 0.4;
    /// How far a control dims while held. A press is the one state that
    /// must not fade: the feedback has to land with the finger, so this is
    /// applied as an immediate style rather than through the tween store.
    pub const PRESSED_OPACITY: f32 = 0.7;

    /// The whole type scale. Nothing in the interface may set a size that is
    /// not one of these four — `FONT_BODY` is the root default that every
    /// element inherits, so most elements set nothing at all.
    pub const FONT_SMALL: f32 = 10.0;
    pub const FONT_CONTROL: f32 = 12.0;
    pub const FONT_BODY: f32 = 12.0;
    pub const FONT_HEADING: f32 = 14.0;
    /// Empty-state headlines — the one step above the interface scale.
    pub const FONT_DISPLAY: f32 = 22.0;

    pub const RADIUS_SMALL: f32 = 8.0;
    pub const RADIUS_CONTROL: f32 = 16.0;
    pub const RADIUS_CARD: f32 = 16.0;
    pub const RADIUS_PANEL: f32 = 20.0;
    pub const RADIUS_OVERLAY: f32 = 24.0;

    pub const BORDER_WIDTH: f32 = 1.0;
    pub const FOCUS_WIDTH: f32 = 2.0;
    pub const SLIDER_FOCUS_WIDTH: f32 = 1.0;

    /// One comfortable control height everywhere — the "unified control
    /// geometry" the product settled on.
    pub const CONTROL_HEIGHT: f32 = 40.0;
    /// The quiet strips that frame a composer: the context chip above its
    /// input and the affordance row beneath it. Both sit below the control
    /// height so the input stays the only full-weight element on the card.
    pub const CHIP_HEIGHT: f32 = 24.0;
    pub const FOOTER_HEIGHT: f32 = 28.0;
    /// The whole icon scale, matching the type scale: `ICON_SIZE` is the
    /// default a control's glyph takes, `SMALL` is for marks inside a control
    /// (a dropdown caret, a resize grip) and `LARGE` for the transport and
    /// the brand mark. Bespoke sizes belong to artwork, not to icons.
    pub const ICON_SIZE_SMALL: f32 = 12.0;
    pub const ICON_SIZE: f32 = 16.0;
    pub const ICON_SIZE_LARGE: f32 = 20.0;
    pub const GAP_SMALL: f32 = 4.0;
    pub const GAP: f32 = 8.0;
    pub const GAP_LARGE: f32 = 12.0;
    /// Centres a 16px glyph in a 40px control.
    pub const CONTROL_PADDING: f32 = (Self::CONTROL_HEIGHT - Self::ICON_SIZE) / 2.0;
    pub const INPUT_PADDING: f32 = Self::CONTROL_PADDING;

    /// The filled slider's fill is inset by a hair so the plate's radius still
    /// reads at the edges.
    pub const SLIDER_FILL_INSET: f32 = 1.0;
    pub const SLIDER_FILL_RADIUS: f32 = Self::RADIUS_CONTROL - Self::SLIDER_FILL_INSET;

    /// Value input inside a scrub field.
    pub const SCRUB_VALUE_WIDTH: f32 = 70.0;
    /// Rail button footprint: the 40px action plus the marker that shows
    /// which panel it has open.
    pub const RAIL_BUTTON_HEIGHT: f32 = Self::CONTROL_HEIGHT;
    /// Small marks: the unsaved dot, a progress rule, a colour sample and a
    /// picker thumbnail. Named here so no view invents its own.
    /// A transient menu's list box: how tall it grows before it scrolls, and
    /// the width it will not shrink below when its trigger is narrower than
    /// its rows. Shared by the dropdown and the command palette so one is
    /// never a different shape from the other.
    pub const MENU_MAX_HEIGHT: f32 = 280.0;
    pub const MENU_MIN_WIDTH: f32 = 180.0;

    pub const DOT_SIZE: f32 = 6.0;
    pub const PROGRESS_HEIGHT: f32 = 4.0;
    pub const SWATCH_SIZE: f32 = 26.0;
    pub const TILE_WIDTH: f32 = 68.0;
    pub const TILE_HEIGHT: f32 = 48.0;
    /// Timeline playhead column.
    pub const SCRUBBER_WIDTH: f32 = 26.0;
    pub const SCRUBBER_GRIP_HEIGHT: f32 = 62.0;
}

// ---------------------------------------------------------------------------
// typography
// ---------------------------------------------------------------------------

/// Interface family, as the product's own tokens name it. The bundled Geist
/// faces stay registered so a theme can opt into them.
pub const FONT_SANS: &str = "Helvetica Neue";
pub const FONT_MONO: &str = "Geist Mono";

/// The eight bundled Geist faces, in the order gpui should register them.
pub const GEIST_FACES: [&str; 8] = [
    "Geist.ttf",
    "Geist-Italic.ttf",
    "Geist-Medium.ttf",
    "Geist-MediumItalic.ttf",
    "Geist-SemiBold.ttf",
    "Geist-SemiBoldItalic.ttf",
    "Geist-Bold.ttf",
    "Geist-BoldItalic.ttf",
];

pub const GEIST_MONO_FACES: [&str; 4] = [
    "GeistMono.ttf",
    "GeistMono-Medium.ttf",
    "GeistMono-SemiBold.ttf",
    "GeistMono-Bold.ttf",
];

// ---------------------------------------------------------------------------
// layout metrics
// ---------------------------------------------------------------------------

/// Left rail column.
pub const RAIL_WIDTH: f32 = 64.0;
/// Inspector column.
pub const PANEL_WIDTH: f32 = 300.0;
/// Timeline track row height.
pub const TRACK_HEIGHT: f32 = 44.0;
