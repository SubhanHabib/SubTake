//! Layout primitives shared by every control: the flex row and column at
//! the design system's gap, and the bounds probe gestures rely on.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

pub fn row() -> Div {
    div().flex().items_center().gap(px(Theme::gap()))
}

pub fn column() -> Div {
    div().flex().flex_col().gap(px(Theme::gap()))
}

/// Captures actual layout bounds for pixel-accurate canvas and timeline gestures.
pub fn measure(bounds: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(
        move |b, window, _| {
            let previous = bounds.replace(b);
            if previous.size != b.size {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
