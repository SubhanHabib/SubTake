//! Native GPUI controls on SubTake's own design system.
//!
//! Every number here comes from the "Stage" redesign handoff, extracted into
//! `docs/DESIGN-PRIMITIVES.md`: four nesting materials over the user's
//! desktop, a control ladder of 26 / 28 / 34 / 40 / 44 / 52 / 56 / 60, a
//! radius ladder with a deliberate gap between 14 and 20, and one blue accent
//! that marks exactly four things. There are no real borders anywhere — every
//! edge is an inset shadow, so an edge can never change what a control
//! measures.
//!
//! The distinctive pieces are the scrub field (a filled slider that *is* the
//! row, with its value shown in place) and the frosted float, which paints a
//! whole popover subtree inside one scene layer so a hover repaint elsewhere
//! cannot reorder its quads under its own backdrop blur.

mod controls;
mod fonts;
mod frost;
mod icon;
mod layout;
pub mod motion;
pub mod perf;
mod typography;
pub mod unused;

pub use controls::*;
pub use fonts::{families_available, register as register_fonts};
pub use frost::{
    BAR_BLUR, FADE_BAND, MENU_BLUR, PANEL_BLUR, POD_BLUR, edge, fade_edges, frosted, layered,
    pill_edge,
};
pub use icon::{icon, icon_sized};
pub use layout::{FitHeight, column, measure, row};
pub use motion::{
    HOVER_FADE_MS, Leave, MENU_IN_MS, MENU_OUT_MS, blend, fade_in, hover_blend, hover_listener,
    hover_progress, menu_in, menu_in_above, state_fade, tick_hover_fades, tween_key,
};
pub use typography::{heading, mono, mono_small, panel_title, title};
