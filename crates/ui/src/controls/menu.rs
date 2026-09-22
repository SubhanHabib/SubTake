//! The plate, list and row every transient menu is built from.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{Surface, button, column, panel_variant};

/// The plate a transient menu lands on: the dropdown's choices, the command
/// palette. One definition, so no menu can drift from another.
pub fn menu_surface(theme: Theme) -> Div {
    panel_variant(theme, Surface::Popup)
        .p(px(Theme::GAP_SMALL))
        .gap(px(Theme::GAP_SMALL))
        .shadow_lg()
}

/// The scrolling column of rows inside a menu surface. Rows sit closer than
/// the surface's own gap — they are one list, not separate controls.
pub fn menu_list(id: impl Into<ElementId>, max_height: f32) -> Stateful<Div> {
    column()
        .id(id)
        .gap(px(2.0))
        .min_h_0()
        .max_h(px(max_height))
        .overflow_y_scroll()
}

/// One row of a menu, with the keyboard cursor drawn behind it. The row
/// itself is an ordinary ghost control; the wrapper only carries the cursor,
/// which is a property of the list rather than of the control.
pub fn menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    highlighted: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .flex_none()
        .rounded(px(Theme::RADIUS_CONTROL))
        .when(highlighted && !selected, |el| el.bg(theme.hover))
        .child(
            button(id, label, theme)
                .ghost()
                .menu_item()
                .selected(selected)
                .on_click(on_click),
        )
}
