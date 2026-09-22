//! Native GPUI controls on SubTake's own design system.
//!
//! Geometry is carried over from the pre-GPUI Slint components
//! (`ui/components/*.slint`): one 40px control height everywhere, a 16px
//! radius, 16px glyphs inset 12px, and translucent plates that let the
//! window's vibrancy material read through. The distinctive pieces are the
//! scrub field (a filled slider that *is* the row, with its value shown in
//! place) and the timeline scrubber (cap, rule and six-dot grip).

mod controls;
mod fonts;
mod frost;
mod icon;
mod layout;
pub mod motion;
pub mod perf;

pub use controls::*;
pub use fonts::{families_available, register as register_fonts};
pub use frost::{FADE_BAND, MENU_BLUR, fade_edges, frosted, layered};
pub use icon::{icon, icon_sized};
pub use layout::{column, measure, row};
pub use motion::{
    HOVER_FADE_MS, MENU_IN_MS, blend, fade_in, hover_blend, hover_listener, menu_in, state_fade,
    tick_hover_fades, tween_key,
};
