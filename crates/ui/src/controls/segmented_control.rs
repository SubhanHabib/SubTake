//! A row of mutually exclusive options, on one recessed track.
//!
//! The active segment is an `ink` pill, not an accent one. The accent marks
//! four things in this interface — the playhead, the selection, the active
//! tool and the primary action — and "which of these two views you are
//! looking at" is none of them, so the segmented control says it in the
//! achromatic fill instead. That is also why this is not built from `Button`:
//! a selected button fills with accent, which is the right answer everywhere
//! except here.

use gpui::{prelude::*, *};
use std::rc::Rc;
use subtake_theme::Theme;

use crate::{focus_ring, motion, perf};

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
        .flex_none()
        .h(px(Theme::CONTROL_HEIGHT_LARGE))
        .p(px(Theme::GAP_SMALL))
        .gap(px(Theme::GAP_SMALL))
        .rounded_full()
        .bg(theme.sunk)
        .children(
            options
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let pick = on_select.clone();
                    let element_id = ElementId::from((id.clone(), index));
                    let active = index == selected;
                    // The pill is meant to slide between positions. It
                    // cross-fades instead: gpui lays the segments out, so
                    // there is no single element to animate along the track,
                    // and a fade at the same 160ms reads as the same move.
                    let on = motion::state_fade(&motion::tween_key(&element_id, "segment"), active);
                    let hover_key = motion::tween_key(&element_id, "hover");
                    let idle = motion::hover_blend(&hover_key, theme.muted, theme.text);
                    let ring = focus_ring(theme);
                    let press = theme.press;
                    let click_id = element_id.clone();
                    div()
                        .id(element_id)
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .min_w_0()
                        .px(px(Theme::CONTROL_PADDING))
                        .rounded_full()
                        .bg(motion::blend(theme.ink.opacity(0.), theme.ink, on))
                        .text_color(motion::blend(idle, theme.on_ink, on))
                        .text_size(px(Theme::FONT_BODY))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .cursor_pointer()
                        // Pressed as a button presses: the `press` wash and a
                        // dim. The current segment keeps its `ink` pill and
                        // only dims, since a wash would show as a grey pill
                        // swapped in for a black one.
                        .active(move |s| {
                            if active { s } else { s.bg(press) }.opacity(Theme::PRESSED_OPACITY)
                        })
                        .tab_index(0)
                        .focus_visible(move |s| s.shadow(vec![ring]))
                        .on_hover(motion::hover_listener(hover_key))
                        .child(div().text_ellipsis().child(label))
                        .on_click(move |_, w, cx| {
                            perf::log(format_args!("click segment {click_id:?}"));
                            pick(index, w, cx)
                        })
                }),
        )
}
