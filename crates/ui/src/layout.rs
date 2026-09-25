//! Layout primitives shared by every control: the flex row and column at
//! the design system's gap, the bounds probe gestures rely on, and the box
//! that eases to its content's height.

use crate::motion::{ease_toward, reduced_motion};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc, time::Instant};
use subtake_theme::{RESIZE_MS, Theme};

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

/// A box that eases to its content's height over [`RESIZE_MS`] instead of
/// snapping to it, for a dialog, palette or menu whose content changes
/// while it is open. The content lays out at its own height inside, and the
/// box follows it a frame behind, clipping the content only while it is
/// still growing into it.
///
/// Its first frame, and the first after [`FitHeight::opening`] sees a new
/// opening, takes the content's height as it is: a surface arrives at its
/// size, and only changes from there move.
#[derive(Default)]
pub struct FitHeight {
    measured: Rc<Cell<Bounds<Pixels>>>,
    /// From, to, and when the move began.
    shown: Option<(f32, f32, Instant)>,
    opens: usize,
}

impl FitHeight {
    /// Start over when the surface has opened again since the last frame,
    /// by its [`crate::motion::Leave`] count.
    pub fn opening(&mut self, opens: usize) -> &mut Self {
        if opens != self.opens {
            self.opens = opens;
            self.reset();
        }
        self
    }

    /// Forget the height, so the next frame takes the content's as it is.
    pub fn reset(&mut self) {
        self.measured.set(Bounds::default());
        self.shown = None;
    }

    pub fn wrap(&mut self, content: impl IntoElement, window: &mut Window) -> Div {
        let natural = f32::from(self.measured.get().size.height);
        let now = Instant::now();
        let height = match self.shown {
            // Not measured yet: the box is the content's own height.
            None if natural <= 0. => None,
            None => {
                self.shown = Some((natural, natural, now));
                Some(natural)
            }
            Some((from, to, started)) => {
                let at = ease_toward(from, to, started, RESIZE_MS, now);
                if (natural - to).abs() >= 1. {
                    let from = if reduced_motion() { natural } else { at };
                    self.shown = Some((from, natural, now));
                    Some(from)
                } else {
                    Some(at)
                }
            }
        };
        let settled = self.shown.is_none_or(|(_, to, _)| height == Some(to));
        if !settled {
            window.request_animation_frame();
        }
        div()
            .flex()
            .flex_col()
            .when_some(height, |d, h| d.h(px(h)))
            .when(height.is_some_and(|h| h < natural), |d| d.overflow_hidden())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(content)
                    .child(fit_probe(self.measured.clone())),
            )
    }
}

/// [`measure`] for [`FitHeight`]: a new height asks for the next frame, so
/// the box starts toward it at once. `window.refresh()` from prepaint is
/// cleared with the rest of the frame's dirt, and the move would wait for
/// whatever next redrew the window.
fn fit_probe(bounds: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(
        move |b, window, _| {
            if bounds.replace(b).size != b.size {
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
