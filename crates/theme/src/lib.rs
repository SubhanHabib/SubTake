//! SubTake presentation tokens.
//!
//! These are SubTake's own design tokens, carried over verbatim from the
//! pre-GPUI `ui/theme.slint` so the native editor keeps the frosted look the
//! product was designed around: translucent surfaces with their alpha baked
//! into the token, an achromatic accent, 40px controls on a 16px radius, and 16px
//! icons.
//!
//! Alpha is part of the colour here, not applied at the call site. The window
//! sits on a real vibrancy material (see [`Theme::WINDOW_GLASS_SUPPORTED`]),
//! so every surface tone is chosen to sit *on* the blurred desktop rather
//! than to hide it.

use gpui::{Hsla, Rgba, WindowAppearance, rgba};

// ---------------------------------------------------------------------------
// appearance
// ---------------------------------------------------------------------------

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

/// `#rrggbbaa`, the notation the original tokens were authored in.
fn c(value: u32) -> Hsla {
    let rgba: Rgba = rgba(value);
    rgba.into()
}

// ---------------------------------------------------------------------------
// theme
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub appearance: Appearance,

    // ---- surfaces (alpha baked in — these ride the vibrancy material) ----
    /// Window background.
    pub bg: Hsla,
    /// Title bar strip.
    pub header: Hsla,
    /// Inspector / timeline panel.
    pub panel: Hsla,
    /// Raised control plate — button rest, slider track, cards.
    pub surface: Hsla,
    /// Floating overlay plate. The recorder's borderless windows carry no
    /// vibrancy material — macOS gives a borderless window no corner mask, so
    /// a blurred view fills the frame square behind a rounded plate — so this
    /// tone is near-opaque rather than a tint over glass.
    pub overlay: Hsla,
    /// Menu / popover plate. A translucent tint: every menu is painted over
    /// its own backdrop blur (`frosted`), so the frost shows through while
    /// the rows stay legible against a softened, not raw, background.
    pub popup: Hsla,
    pub overlay_shadow: Hsla,

    // ---- lines and text ----
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,

    // ---- accent ----
    //
    // Deliberately achromatic. Every control is white/grey/black at some
    // opacity, either filled or outlined, and the colour in the interface
    // comes from the desktop showing through the frost. `accent` is the
    // *filled* emphasis plate — near-black on light, near-white on dark —
    // with `on_accent` the glyph laid on it.
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_text: Hsla,
    pub selection: Hsla,
    pub on_accent: Hsla,

    // ---- state ----
    pub hover: Hsla,
    /// The recess a text field sits in. A wash, like `hover` and `selection`,
    /// rather than a plate: every surface tone is translucent over the window
    /// glass, so an opaque fill here punches a solid hole through the material
    /// instead of sitting on it.
    pub input: Hsla,
    pub unchecked: Hsla,
    pub toggle_thumb: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,

    // ---- editor surfaces ----
    pub canvas_dot: Hsla,
    pub track: Hsla,
    pub playhead: Hsla,
}

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

    // ---- construction ----------------------------------------------------

    pub fn new(preference: &str, appearance: WindowAppearance) -> Self {
        match Appearance::resolve(preference, appearance) {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }

    pub fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            bg: c(0xfafaf966),
            header: c(0xffffff55),
            panel: c(0xffffff48),
            surface: c(0xe7e7e766),
            overlay: c(0xfcfcfcf7),
            popup: c(0xfafafacc),
            overlay_shadow: c(0x00000018),
            border: c(0x2222221c),
            text: c(0x292a2dff),
            muted: c(0x77797eff),
            accent: c(0x242424ff),
            accent_hover: c(0x3b3b3bff),
            accent_text: c(0x242424ff),
            selection: c(0x0000001a),
            on_accent: c(0xffffffff),
            hover: c(0x0000000c),
            input: c(0x00000012),
            unchecked: c(0x00000030),
            toggle_thumb: c(0xffffffff),
            danger: c(0xd51e43ff),
            warning: c(0xe8b251ff),
            canvas_dot: c(0x292a2d13),
            track: c(0xececeb66),
            playhead: c(0x737570ff),
        }
    }

    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            bg: c(0x11111388),
            header: c(0x15151766),
            panel: c(0x16161855),
            surface: c(0x20202466),
            overlay: c(0x1c1d21f7),
            popup: c(0x202126cc),
            overlay_shadow: c(0x00000055),
            border: c(0xffffff22),
            text: c(0xf5f5f6ff),
            muted: c(0xa3a3adff),
            accent: c(0xf0f0f0ff),
            accent_hover: c(0xffffffff),
            accent_text: c(0xf0f0f0ff),
            selection: c(0xffffff1f),
            on_accent: c(0x171717ff),
            hover: c(0xffffff0e),
            input: c(0x00000038),
            unchecked: c(0xffffff22),
            toggle_thumb: c(0xffffffff),
            danger: c(0xf43f5eff),
            warning: c(0xe8b251ff),
            canvas_dot: c(0xffffff10),
            track: c(0xffffff09),
            playhead: c(0xd6d6d5ff),
        }
    }

    // ---- derived ---------------------------------------------------------

    /// The fill painted over a filled slider's plate. Deliberately a white
    /// wash rather than the accent: a scrub field reads as a level, not as a
    /// coloured control.
    pub fn slider_fill(&self) -> Hsla {
        match self.appearance {
            Appearance::Dark => c(0xffffff30),
            Appearance::Light => c(0xffffff80),
        }
    }

    /// The hairline marking the fill edge in a scrub field.
    pub fn slider_marker(&self) -> Hsla {
        self.muted.opacity(0.6)
    }

    pub fn slider_focus(&self) -> Hsla {
        c(0x00000080)
    }

    /// How the platform should composite the window behind our paint.
    pub fn window_background_appearance(&self) -> gpui::WindowBackgroundAppearance {
        if cfg!(target_os = "linux") {
            gpui::WindowBackgroundAppearance::Transparent
        } else if Self::WINDOW_GLASS_SUPPORTED {
            gpui::WindowBackgroundAppearance::Blurred
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_keep_the_alpha_they_were_authored_with() {
        // The frosted look lives in these alphas; an opaque token would hide
        // the vibrancy material the whole palette is designed to sit on.
        let light = Theme::light();
        assert!(light.bg.a < 1.0, "window background must stay translucent");
        assert!(light.panel.a < 1.0, "panels must stay translucent");
        assert!(
            light.surface.a < 1.0,
            "control plates must stay translucent"
        );
        assert_eq!(light.text.a, 1.0, "text stays fully opaque");

        let dark = Theme::dark();
        assert!(dark.bg.a < 1.0);
        assert!(dark.panel.a < 1.0);
    }

    #[test]
    fn the_type_and_icon_scales_are_closed() {
        // Every size the interface may use, and nothing between them.
        let fonts = [
            Theme::FONT_SMALL,
            Theme::FONT_CONTROL,
            Theme::FONT_BODY,
            Theme::FONT_HEADING,
            Theme::FONT_DISPLAY,
        ];
        assert!(fonts.windows(2).all(|w| w[0] <= w[1]), "scale must ascend");
        assert_eq!(Theme::FONT_BODY, Theme::FONT_CONTROL, "one reading size");
        let icons = [
            Theme::ICON_SIZE_SMALL,
            Theme::ICON_SIZE,
            Theme::ICON_SIZE_LARGE,
        ];
        assert!(icons.windows(2).all(|w| w[0] < w[1]));
        // A glyph has to leave room inside the control it sits in.
        assert!(Theme::ICON_SIZE_LARGE < Theme::CONTROL_HEIGHT);
    }

    #[test]
    fn control_geometry_is_unified() {
        assert_eq!(Theme::CONTROL_HEIGHT, 40.0);
        assert_eq!(Theme::RADIUS_CONTROL, 16.0);
        assert_eq!(Theme::ICON_SIZE, 16.0);
        // A 16px glyph centred in a 40px control.
        assert_eq!(Theme::CONTROL_PADDING, 12.0);
    }

    #[test]
    fn every_control_colour_is_achromatic() {
        // The interface takes its colour from the desktop behind the glass,
        // so no control may introduce a hue of its own.
        for t in [Theme::light(), Theme::dark()] {
            for (name, colour) in [
                ("accent", t.accent),
                ("accent_hover", t.accent_hover),
                ("accent_text", t.accent_text),
                ("on_accent", t.on_accent),
                ("selection", t.selection),
                ("unchecked", t.unchecked),
                ("toggle_thumb", t.toggle_thumb),
            ] {
                assert_eq!(colour.s, 0.0, "{name} must be grey, not tinted");
            }
        }
    }

    #[test]
    fn the_filled_plate_inverts_with_the_appearance() {
        // Filled means near-black on light and near-white on dark, with the
        // glyph on it taking the opposite end.
        let light = Theme::light();
        assert!(light.accent.l < 0.2 && light.on_accent.l > 0.9);
        let dark = Theme::dark();
        assert!(dark.accent.l > 0.9 && dark.on_accent.l < 0.2);
        // Hover lifts the plate away from the background in both.
        assert!(light.accent_hover.l > light.accent.l);
        assert!(dark.accent_hover.l > dark.accent.l);
    }

    #[test]
    fn glass_is_on_so_the_translucent_tokens_composite() {
        assert!(Theme::WINDOW_GLASS_SUPPORTED);
        if !cfg!(target_os = "linux") {
            assert_eq!(
                Theme::light().window_background_appearance(),
                gpui::WindowBackgroundAppearance::Blurred
            );
        }
    }

    #[test]
    fn appearance_preference_overrides_the_window() {
        assert_eq!(
            Appearance::resolve("dark", WindowAppearance::Light),
            Appearance::Dark
        );
        assert_eq!(
            Appearance::resolve("system", WindowAppearance::VibrantDark),
            Appearance::Dark
        );
    }
}
