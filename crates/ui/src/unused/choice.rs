//! Checkbox and radio: "18px, radius 6 for checkbox and round for radio, 10px
//! to the label. Checked is a solid accent fill."

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{hairline, icon_sized, row};

const BOX_SIZE: f32 = 18.0;
const BOX_RADIUS: f32 = 6.0;
const LABEL_GAP: f32 = 10.0;
/// The tick inside the box. One step below the box so it does not touch the
/// corners it sits in.
const TICK_SIZE: f32 = 12.0;

fn mark(round: bool, checked: bool, theme: Theme) -> Div {
    let mut el = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(BOX_SIZE));
    el = if round {
        el.rounded_full()
    } else {
        el.rounded(px(BOX_RADIUS))
    };
    if checked {
        el.bg(theme.accent)
    } else {
        // Unchecked is a recess with an edge, not an empty square: on glass,
        // `sunk` alone is close enough to the surface behind it to vanish.
        el.bg(theme.sunk)
            .shadow(vec![hairline(theme.line, Theme::hairline_width())])
    }
}

/// A checkbox and its label, as one row.
pub fn checkbox(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    theme: Theme,
) -> Stateful<Div> {
    row()
        .id(id)
        .gap(px(LABEL_GAP))
        .child(mark(false, checked, theme).when(checked, |el| {
            el.child(icon_sized("Check-regular", TICK_SIZE, theme.on_accent))
        }))
        .child(
            div()
                .text_size(px(Theme::font_control()))
                .text_color(theme.text)
                .child(label.into()),
        )
}

/// A radio and its label. The same box, round, with a dot instead of a tick —
/// a tick would say "this one is on", and a radio says "this one, of these".
pub fn radio(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    theme: Theme,
) -> Stateful<Div> {
    row()
        .id(id)
        .gap(px(LABEL_GAP))
        .child(mark(true, selected, theme).when(selected, |el| {
            el.child(
                div()
                    .size(px(Theme::dot_size()))
                    .rounded_full()
                    .bg(theme.on_accent),
            )
        }))
        .child(
            div()
                .text_size(px(Theme::font_control()))
                .text_color(theme.text)
                .child(label.into()),
        )
}
