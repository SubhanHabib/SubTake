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

use gpui::{Hsla, WindowAppearance};

mod appearance;
mod css;
mod metrics;
mod palette;
#[cfg(test)]
mod tests;

pub use appearance::Appearance;
use css::css;
#[cfg(test)]
use css::parse_css;
pub use metrics::*;
use palette::{DARK, LIGHT};

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
    // ---- construction ----------------------------------------------------

    pub fn new(preference: &str, appearance: WindowAppearance) -> Self {
        match Appearance::resolve(preference, appearance) {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }

    pub fn light() -> Self {
        *LIGHT
    }

    pub fn dark() -> Self {
        *DARK
    }
    // ---- derived ---------------------------------------------------------

    /// The fill painted over a filled slider's plate. Deliberately a white
    /// wash rather than the accent: a scrub field reads as a level, not as a
    /// coloured control.
    pub fn slider_fill(&self) -> Hsla {
        match self.appearance {
            Appearance::Dark => css("hsla(0, 0%, 100%, 0.188)"),
            Appearance::Light => css("hsla(0, 0%, 100%, 0.502)"),
        }
    }

    /// The hairline marking the fill edge in a scrub field.
    pub fn slider_marker(&self) -> Hsla {
        self.muted.opacity(0.6)
    }

    pub fn slider_focus(&self) -> Hsla {
        css("hsla(0, 0%, 0%, 0.502)")
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
