//! A row of mutually exclusive buttons.

use gpui::{prelude::*, *};
use std::rc::Rc;
use subtake_theme::Theme;

use crate::button;

pub fn segmented_control(
    id: &str,
    options: &[&str],
    selected: usize,
    theme: Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);
    let id = SharedString::from(id.to_owned());
    div()
        .flex()
        .gap(px(Theme::GAP_SMALL))
        .h(px(Theme::CONTROL_HEIGHT))
        .children(
            options
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let pick = on_select.clone();
                    button((id.clone(), index), label, theme)
                        .selected(index == selected)
                        .stretch()
                        .on_click(move |_, w, cx| pick(index, w, cx))
                }),
        )
}
