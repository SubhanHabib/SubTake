//! A toast: "radius 20, 14/16 padding, a 8px status dot, one optional action
//! at 30 tall. Never more than one line of text."

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{Surface, panel_variant, row};

const TOAST_PADDING_Y: f32 = 14.0;
const TOAST_PADDING_X: f32 = 16.0;
const TOAST_GAP: f32 = 12.0;
const TOAST_DOT: f32 = 8.0;
const TOAST_ACTION_HEIGHT: f32 = 30.0;
const TOAST_ACTION_PADDING: f32 = 12.0;

/// A transient notice. `tint` is the dot's colour — the caller picks it, so
/// the same plate serves "exported", "capture failed" and "preset saved"
/// without three variants.
///
/// One line, and the signature enforces it: there is no body slot. The
/// handoff is flat about this, and a toast that can hold a paragraph becomes
/// the place errors get dumped instead of handled.
pub fn toast(
    message: impl Into<SharedString>,
    tint: Hsla,
    action: Option<(
        SharedString,
        Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
    )>,
    theme: Theme,
) -> Div {
    let mut el = panel_variant(theme, Surface::Popup)
        .flex_row()
        .items_center()
        .py(px(TOAST_PADDING_Y))
        .px(px(TOAST_PADDING_X))
        .gap(px(TOAST_GAP))
        .child(
            div()
                .flex_none()
                .size(px(TOAST_DOT))
                .rounded_full()
                .bg(tint),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .text_size(px(Theme::font_control()))
                .text_color(theme.text)
                .child(message.into()),
        );
    if let Some((label, on_click)) = action {
        el = el.child(
            row()
                .id("toast-action")
                .flex_none()
                .h(px(TOAST_ACTION_HEIGHT))
                .px(px(TOAST_ACTION_PADDING))
                .rounded_full()
                .bg(theme.raise)
                .hover(|s| s.bg(theme.raise_hover()))
                .text_size(px(Theme::font_control()))
                .text_color(theme.text)
                .cursor_pointer()
                .child(label)
                .on_click(move |e, w, cx| on_click(e, w, cx)),
        );
    }
    el
}
