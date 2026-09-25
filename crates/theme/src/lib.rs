//! SubTake presentation tokens.
//!
//! The token set of the "Stage" redesign: a translucent shell over the user's
//! desktop, four nesting materials (`bg` → `glass`/`card` → `sunk`), and a
//! single blue accent that marks exactly four things — the playhead, the
//! selected region or row, the active tool, and the primary action. Everything
//! else is grey. Red (`rec`) is reserved for Record and destructive actions.
//!
//! Alpha is part of the colour here, not applied at the call site. The window
//! sits on a real vibrancy material (see [`Theme::WINDOW_GLASS_SUPPORTED`]),
//! so every surface tone is chosen to sit *on* the blurred desktop rather
//! than to hide it.
//!
//! The redesign spells `saturate()` alongside each material's blur. gpui at
//! the pinned revision has no saturation filter, so only the blur is carried;
//! see `crates/ui/src/frost.rs`.

use gpui::{Hsla, WindowAppearance};

mod appearance;
mod css;
mod metrics;
mod palette;
#[cfg(test)]
mod tests;
pub mod tune;

pub use appearance::Appearance;
#[cfg(test)]
use css::parse_css;
pub use metrics::*;
use palette::{DARK, LIGHT};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub appearance: Appearance,

    // ---- materials (alpha baked in — these ride the vibrancy material) ----
    //
    // Four, and they nest in this order. Only the first three carry a
    // backdrop blur, because only they have the desktop behind them.
    /// The window shell. One per screen.
    pub bg: Hsla,
    /// The solid the window's material stands on, under `bg`, for when macOS
    /// draws it without its live blur: Mission Control's thumbnails, and the
    /// frames while it rebuilds the blur going into and out of them. Opaque,
    /// and near what the frost reads as, so those frames match the theme
    /// rather than flash black.
    ///
    /// Not drawn by the design.
    pub ground: Hsla,
    /// Anything that floats and holds controls: console, inspector, tool
    /// pod, player bar, recorder bar.
    pub glass: Hsla,
    /// Anything that floats and holds text: dialog, selected row.
    pub card: Hsla,
    /// A dialog's plate: `card`, but on dark it stands over the stage's
    /// bright picture and needs a fill of its own.
    pub dialog: Hsla,
    /// A recess inside one of the above: lanes, sliders, segmented backings.
    pub sunk: Hsla,
    /// A recess, hovered or in its filled portion.
    pub sunk2: Hsla,
    /// A control sitting directly on glass.
    pub raise: Hsla,
    /// The hairline on a raised control.
    pub raise_line: Hsla,
    /// The active segment of a segmented control: a raised white pill on
    /// light, a lift of white on dark.
    pub seg_active: Hsla,
    /// The filled part of a slider row — flat and faint, so the level reads
    /// without the row becoming a control of its own.
    pub slider_fill: Hsla,
    /// The round plate an icon sits on inside a timeline region or a clip's
    /// label chip: a white wash, so it lifts off any lane tint alike.
    pub plate: Hsla,
    /// A toggle's track while it is on. A mid-grey, not `ink` and not the
    /// accent: on and off are the plate and the thumb's side.
    pub switch_on: Hsla,
    /// A toggle's track while it is off. Light `sunk2` is nearly white and
    /// the track vanished on recorder glass, so off is a faint grey of its
    /// own.
    pub switch_off: Hsla,
    /// A chip laid over a picture — the camera preview's, an area's size —
    /// always with a blur under it.
    pub frost: Hsla,

    // ---- state washes ----
    /// Hover overlay laid over a switch's track. Every other control and
    /// row lifts to `sunk2` under the pointer instead.
    pub hover: Hsla,
    /// Pressed overlay.
    pub press: Hsla,

    // ---- lines and text ----
    pub text: Hsla,
    /// Secondary text and idle icons.
    pub muted: Hsla,
    pub line: Hsla,

    // ---- accent ----
    //
    // One blue, four jobs: the playhead, the selected region or row, the
    // active tool, and the primary action. A control that is none of those is
    // grey, which is what lets the four read at all.
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_press: Hsla,
    /// Focus ring, region fill, glow.
    pub accent_soft: Hsla,
    pub on_accent: Hsla,
    /// The soft light around the timeline's playhead: its bubble, line and
    /// grab handle. The accent at a third of its strength or so.
    pub accent_glow: Hsla,

    // ---- ink ----
    //
    // The achromatic filled plate: the transport button and the active
    // segment of a segmented control. Near-black on light, near-white on dark.
    pub ink: Hsla,
    pub on_ink: Hsla,

    // ---- red ----
    /// Record fill — the only red fill in the app.
    pub rec: Hsla,
    /// Destructive text.
    pub danger: Hsla,

    // ---- shadows ----
    //
    // Colour and strength; how far each falls is in `metrics.rs`. A glow is
    // not here: it takes the colour of what glows, at an opacity kept with
    // its size.
    /// The wide layer under a dialog, a menu or a dropdown.
    pub shadow_far: Hsla,
    /// The tight layer under a dialog, a menu or a dropdown, and a raised
    /// control's lift.
    pub shadow_near: Hsla,
    /// The wide layer under the window's own planes: the console, the
    /// inspector, the tool rail and the stage's pods. Not drawn by the design:
    /// kept apart from `shadow_far` so the planes tune as one without moving a
    /// menu's or a dialog's shadow.
    pub shadow_plane_far: Hsla,
    /// The tight layer under the window's own planes.
    pub shadow_plane_near: Hsla,
    /// The picture's shadow on the stage.
    pub shadow_picture: Hsla,
    /// Under a segmented control's active pill.
    pub shadow_segment: Hsla,
    /// Under a toggle's thumb.
    pub shadow_thumb: Hsla,
}

impl Theme {
    // ---- construction ----------------------------------------------------

    pub fn new(preference: &str, appearance: WindowAppearance) -> Self {
        match Appearance::resolve(preference, appearance) {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }

    /// The light palette, with any colours the catalogue is tuning.
    pub fn light() -> Self {
        tune::tinted(*LIGHT)
    }

    /// The dark palette, with any colours the catalogue is tuning.
    pub fn dark() -> Self {
        tune::tinted(*DARK)
    }
    // ---- derived ---------------------------------------------------------

    pub fn slider_focus(&self) -> Hsla {
        self.accent_soft
    }

    /// The shadow under anything that floats and is not one of the window's
    /// own planes (see `plane_shadow`): a dialog, a menu, a dropdown. Two layers — a wide soft one that lifts the
    /// surface off the desktop and a tight one that seats its edge — and both
    /// go deeper on dark, where there is less contrast to do the lifting.
    pub fn panel_shadow(&self) -> Vec<gpui::BoxShadow> {
        let (far, near) = match self.appearance {
            Appearance::Light => (
                (Self::panel_shadow_far_y(), Self::panel_shadow_far_blur()),
                (Self::panel_shadow_near_y(), Self::panel_shadow_near_blur()),
            ),
            Appearance::Dark => (
                (
                    Self::panel_shadow_far_y_dark(),
                    Self::panel_shadow_far_blur_dark(),
                ),
                (
                    Self::panel_shadow_near_y(),
                    Self::panel_shadow_near_blur_dark(),
                ),
            ),
        };
        vec![
            drop_shadow(self.shadow_far, far),
            drop_shadow(self.shadow_near, near),
        ]
    }

    /// The shadow under the window's own planes — the console, the
    /// inspector, the tool rail, the stage's pods and the titlebar's pills.
    /// It starts as `panel_shadow`'s two layers, but has its own tokens so the
    /// planes can be tuned as one without touching a menu, a dropdown or a
    /// dialog.
    pub fn plane_shadow(&self) -> Vec<gpui::BoxShadow> {
        let (far, near) = match self.appearance {
            Appearance::Light => (
                (Self::plane_shadow_far_y(), Self::plane_shadow_far_blur()),
                (Self::plane_shadow_near_y(), Self::plane_shadow_near_blur()),
            ),
            Appearance::Dark => (
                (
                    Self::plane_shadow_far_y_dark(),
                    Self::plane_shadow_far_blur_dark(),
                ),
                (
                    Self::plane_shadow_near_y(),
                    Self::plane_shadow_near_blur_dark(),
                ),
            ),
        };
        vec![
            drop_shadow(self.shadow_plane_far, far),
            drop_shadow(self.shadow_plane_near, near),
        ]
    }

    /// The picture's shadow on the stage: one soft layer, short enough to
    /// fade out inside the stage's margin rather than being cut off at the
    /// timeline's edge the way a panel's deep shadow would be.
    pub fn picture_shadow(&self) -> Vec<gpui::BoxShadow> {
        vec![drop_shadow(
            self.shadow_picture,
            (Self::picture_shadow_y(), Self::picture_shadow_blur()),
        )]
    }

    /// The single `0 2 6` a raised control gains under the pointer — the
    /// near layer of `panel_shadow` and nothing else, so a button lifting on
    /// hover reads as the same material as a panel that is already lifted.
    pub fn lift_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            self.shadow_near,
            (Self::panel_shadow_near_y(), Self::panel_shadow_near_blur()),
        )
    }

    /// The shadow under a segmented control's active pill.
    pub fn segment_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            self.shadow_segment,
            (Self::segment_shadow_y(), Self::segment_shadow_blur()),
        )
    }

    /// The chosen source's check badge, off its picture.
    pub fn source_check_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            gpui::hsla(0., 0., 0., Self::source_check_shadow_alpha()),
            (
                Self::source_check_shadow_y(),
                Self::source_check_shadow_blur(),
            ),
        )
    }

    /// A window's still, standing in its picture as a window does on the
    /// desktop.
    pub fn window_still_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            gpui::hsla(0., 0., 0., Self::window_still_shadow_alpha()),
            (
                Self::window_still_shadow_y(),
                Self::window_still_shadow_blur(),
            ),
        )
    }

    /// A pill resting a step above its card, as Draw area on screen does:
    /// the segment's reach in the near shadow's tone.
    pub fn resting_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            self.shadow_near,
            (Self::segment_shadow_y(), Self::segment_shadow_blur()),
        )
    }

    /// The shadow under a toggle's thumb, which is what separates a white
    /// thumb from the near-white off track on light.
    pub fn thumb_shadow(&self) -> gpui::BoxShadow {
        drop_shadow(
            self.shadow_thumb,
            (Self::thumb_shadow_y(), Self::thumb_shadow_blur()),
        )
    }

    /// The glow under a filled accent control, one step wider at hero
    /// height, and Record's in its own red. Only those two glow: it is how
    /// they say they are the action, without another colour.
    pub fn action_glow(&self, hero: bool) -> gpui::BoxShadow {
        let fall = if hero {
            (Self::glow_hero_y(), Self::glow_hero_blur())
        } else {
            (Self::glow_y(), Self::glow_blur())
        };
        drop_shadow(self.accent_soft, fall)
    }

    pub fn record_glow(&self) -> gpui::BoxShadow {
        drop_shadow(
            self.rec.opacity(Self::record_glow_opacity()),
            (Self::record_glow_y(), Self::record_glow_blur()),
        )
    }

    /// A finished export's glow, `swell` of the way to its peak: an even
    /// accent light around the pill, spreading half as far as it blurs.
    pub fn export_done_glow(&self, swell: f32) -> gpui::BoxShadow {
        let blur = Self::export_glow() * swell;
        gpui::BoxShadow {
            color: self.accent.opacity(Self::export_glow_opacity() * swell),
            offset: gpui::point(gpui::px(0.), gpui::px(0.)),
            blur_radius: gpui::px(blur),
            spread_radius: gpui::px(blur / 2.),
            inset: false,
        }
    }

    /// The shadow a preset's frame casts in its preview, at the look's own
    /// strength: it is a picture of that look, not a piece of chrome.
    pub fn preset_shadow(&self, strength: f32) -> gpui::BoxShadow {
        drop_shadow(
            gpui::hsla(0., 0., 0., strength),
            (Self::preset_shadow_y(), Self::preset_shadow_blur()),
        )
    }

    /// A raised control, hovered: its own tone one step up the fill scale.
    /// The redesign spells this `white .16`, which is the dark value — the
    /// factor below reproduces it exactly and does the right thing on light,
    /// where `raise` is already a near-opaque white and a fixed `.16` would
    /// make the control fainter on hover instead of firmer.
    pub fn raise_hover(&self) -> Hsla {
        self.raise.opacity((self.raise.a * 1.6).min(1.))
    }

    /// The plate under a label set over live screen content — the
    /// countdown's "Press esc" hint, the camera preview's tag. The same in
    /// both appearances: what is under it is somebody's screen or face, not
    /// our palette, so it is a fixed dark tint rather than a theme fill.
    pub fn scrim_chip(&self) -> Hsla {
        gpui::hsla(225. / 360., 0.25, 0.063, 0.55)
    }

    /// A timeline region's fill and ink, from its lane's hue. Light regions
    /// are a clear pastel with a deep ink, dark ones a muted mid-tone with
    /// a pale ink; both put the label past 4.5:1 on its fill.
    pub fn region_tones(&self, tint: Hsla) -> (Hsla, Hsla) {
        let ((fill_s, fill_l), (ink_s, ink_l)) = match self.appearance {
            Appearance::Light => ((0.75, 0.82), (0.65, 0.25)),
            Appearance::Dark => ((0.35, 0.34), (0.60, 0.88)),
        };
        (
            gpui::hsla(tint.h, fill_s, fill_l, 1.),
            gpui::hsla(tint.h, ink_s, ink_l, 1.),
        )
    }

    /// A toggle's thumb, on or off, and the scrubber's dots. White in both
    /// appearances — the review specifies `#ffffff` on the `switch_off` and
    /// `switch_on` tracks in both, so this is not a palette entry.
    pub fn thumb(&self) -> Hsla {
        gpui::hsla(0., 0., 1., 1.)
    }

    /// How the platform should composite the window behind our paint. On
    /// macOS the frost is SubTake's own material under a transparent window,
    /// not gpui's `Blurred`, so its blur and saturation follow the theme.
    pub fn window_background_appearance(&self) -> gpui::WindowBackgroundAppearance {
        if Self::WINDOW_GLASS_SUPPORTED {
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        }
    }
}

/// A shadow straight down: `(offset, blur)` below its box, no spread.
fn drop_shadow(color: Hsla, (offset, blur): (f32, f32)) -> gpui::BoxShadow {
    gpui::BoxShadow {
        color,
        offset: gpui::point(gpui::px(0.), gpui::px(offset)),
        blur_radius: gpui::px(blur),
        spread_radius: gpui::px(0.),
        inset: false,
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}
