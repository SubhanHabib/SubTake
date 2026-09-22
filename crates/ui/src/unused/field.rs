//! A text field and a numeric stepper: the two `sunk` shells the handoff
//! gives that this build has no call site for.

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, Theme};

use crate::{focus_ring, icon_sized, row};

/// "40 tall, fully round, `--sunk`, 15px padding, 14px leading icon at
/// `--muted`."
const FIELD_HEIGHT: f32 = 40.0;
const FIELD_PADDING: f32 = 15.0;
const FIELD_ICON: f32 = 14.0;
const FIELD_GAP: f32 = 10.0;

/// "40 tall `--sunk` shell, 6px padding, two 28px round `--raise` buttons,
/// value in Geist Mono between them."
const STEPPER_PADDING: f32 = 6.0;
const STEPPER_BUTTON: f32 = 28.0;
const STEPPER_GLYPH: f32 = 14.0;

/// A filter or search field: a recess with a glyph leading it.
///
/// The glyph is the caller's, not a magnifier. The icon set carries no plain
/// magnifier, which is deliberate in the handoff — "a filter leads with the
/// icon of what it filters", so a preset filter leads with the preset glyph
/// and a source filter with the monitor.
pub fn field(
    id: impl Into<ElementId>,
    glyph: &str,
    placeholder: impl Into<SharedString>,
    value: impl Into<SharedString>,
    theme: Theme,
) -> Stateful<Div> {
    let value = value.into();
    let empty = value.is_empty();
    row()
        .id(id)
        .tab_index(0)
        .h(px(FIELD_HEIGHT))
        .px(px(FIELD_PADDING))
        .gap(px(FIELD_GAP))
        .rounded_full()
        .bg(theme.sunk)
        .hover(|s| s.bg(theme.sunk2))
        .focus_visible(move |s| s.shadow(vec![focus_ring(theme)]))
        .child(icon_sized(glyph, FIELD_ICON, theme.muted))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .text_size(px(Theme::FONT_CONTROL))
                .text_color(if empty { theme.muted } else { theme.text })
                .child(if empty { placeholder.into() } else { value }),
        )
}

/// A numeric stepper: a recess holding a down button, the value, an up
/// button.
///
/// The glyphs are the two magnifier variants rather than a plus and a minus.
/// The set has no Minus, and the handoff pairs them for exactly that reason —
/// a stepper reads as "less of this / more of this" either way.
pub fn stepper(
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    theme: Theme,
) -> Stateful<Div> {
    // `.active` needs a stateful element, so each button carries its own id.
    let step = |button: &'static str, glyph: &str| {
        div()
            .id(button)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(STEPPER_BUTTON))
            .rounded_full()
            .bg(theme.raise)
            .hover(|s| s.bg(theme.raise_hover()))
            .active(|s| s.bg(theme.press))
            .cursor_pointer()
            .child(icon_sized(glyph, STEPPER_GLYPH, theme.text))
    };
    row()
        .id(id)
        .h(px(FIELD_HEIGHT))
        .p(px(STEPPER_PADDING))
        .gap(px(STEPPER_PADDING))
        .rounded_full()
        .bg(theme.sunk)
        .child(step("down", "MagnifyingGlassMinus-regular"))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_center()
                .font_family(FONT_MONO)
                .text_size(px(Theme::FONT_CONTROL))
                .text_color(theme.text)
                .child(value.into()),
        )
        .child(step("up", "MagnifyingGlassPlus-regular"))
}
