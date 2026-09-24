//! The plate, list and row every transient menu is built from.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{Surface, column, icon_sized, motion, panel_variant, perf, row};

/// The plate a transient menu lands on: the dropdown's choices, the command
/// palette. One definition, so no menu can drift from another.
///
/// No `shadow_lg` of its own: `shadow` replaces the whole stack rather than
/// adding to it, so the preset was quietly dropping the hairline and the
/// float shadow that `panel_variant` had just set.
///
/// `occlude` is what makes it a menu rather than a picture of one: without it
/// the surface only paints over what is behind, so a wheel scrolled on an
/// open menu scrolled the panel underneath and a click on a gap between rows
/// reached whatever it was covering.
pub fn menu_surface(theme: Theme) -> Div {
    panel_variant(theme, Surface::Popup)
        .p(px(Theme::menu_padding()))
        .gap(px(Theme::menu_item_gap()))
        .min_w(px(Theme::menu_min_width()))
        .occlude()
}

/// The scrolling column of rows inside a menu surface. Rows sit a hair apart
/// rather than flush — they are one list, not separate controls.
pub fn menu_list(id: impl Into<ElementId>, max_height: f32) -> Stateful<Div> {
    column()
        .id(id)
        .gap(px(Theme::menu_item_gap()))
        .min_h_0()
        .max_h(px(max_height))
        .overflow_y_scroll()
}

/// A rule between two groups of items. Inset from both edges so it separates
/// the labels rather than cutting the plate in half.
pub fn menu_separator(theme: Theme) -> Div {
    div()
        .flex_none()
        .h(px(Theme::border_width()))
        .mx(px(Theme::menu_separator_inset()))
        .my(px(Theme::menu_separator_margin()))
        .bg(theme.line)
}

/// One row of a menu.
///
/// Not a `Button`. A button says "selected" by filling with the accent, which
/// is right for the four things the accent marks and wrong here: a menu's
/// current item is marked by a tick in a fixed gutter, and its fill is the
/// same `sunk` that hover uses. Building this out of `Button` is how every
/// open menu in the app came to show a solid blue pill where the design has a
/// quiet row and a checkmark.
pub fn menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    highlighted: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    menu_row_in(id, label, Some(selected), highlighted, theme, on_click)
}

/// A row of a list of actions, which has no current item: the command
/// palette's. It drops the tick's gutter, which in a list that can never
/// show a tick only pushed every label in by an empty column.
pub fn command_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    highlighted: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    menu_row_in(id, label, None, highlighted, theme, on_click)
}

/// `selected` is `None` for a row with no tick gutter at all.
fn menu_row_in(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: Option<bool>,
    highlighted: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = id.into();
    let click_id = id.clone();
    let hover_key = motion::tween_key(&id, "menu-row");
    let gutter = selected.is_some();
    let selected = selected.unwrap_or(false);
    // Hover and checked are the same fill, so a row that is both does not
    // stack two washes and come out darker than either. The keyboard cursor
    // joins them: it is the pointer's place in the list, not a third state.
    let resting = if selected || highlighted {
        theme.sunk
    } else {
        theme.sunk.opacity(0.)
    };
    let sunk = theme.sunk;
    row()
        .id(id)
        .flex_none()
        .h(px(Theme::menu_item_height()))
        .px(px(Theme::menu_item_padding()))
        .gap(px(Theme::icon_gap_row()))
        .rounded(px(Theme::menu_item_radius()))
        .bg(motion::hover_blend(&hover_key, resting, theme.sunk))
        .text_size(px(Theme::font_body()))
        .text_color(theme.text)
        .font_weight(if selected {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        // Pressed is the hover fill, dimmed: the row is already on `sunk`
        // under the pointer, and a second, deeper wash would read as a
        // different kind of row rather than this one being pushed.
        .active(move |s| s.bg(sunk).opacity(Theme::pressed_opacity()))
        .on_hover(motion::hover_listener(hover_key))
        // The gutter is held whether or not this row is the current one, so
        // the labels in a menu line up with each other instead of stepping in
        // and out as the selection moves.
        .when(gutter, |el| {
            el.child(
                div()
                    .flex()
                    .flex_none()
                    .w(px(Theme::check_gutter()))
                    .justify_center()
                    .when(selected, |el| {
                        el.child(icon_sized(
                            "Check-regular",
                            Theme::check_gutter(),
                            theme.accent,
                        ))
                    }),
            )
        })
        .child(div().flex_1().min_w_0().text_ellipsis().child(label.into()))
        .on_click(move |e, w, cx| {
            perf::log(format_args!("click menu row {click_id:?}"));
            on_click(e, w, cx)
        })
}
