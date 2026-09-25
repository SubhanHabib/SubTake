//! A row of mutually exclusive options, on one recessed track.
//!
//! The active segment is a raised `seg_active` pill — white on light, a lift
//! of white on dark — with a faint shadow and a hairline, and its label goes
//! to `text` at weight 500. It was an `ink` pill, and that made it the
//! heaviest mark in any panel it sat in, heavier than Export. Accent is not
//! used either: it marks four things in this interface — the playhead, the
//! selection, the active tool and the primary action — and "which of these
//! two views you are looking at" is none of them. That is also why this is
//! not built from `Button`: a selected button fills with accent.

use gpui::{prelude::*, *};
use std::rc::Rc;
use subtake_theme::Theme;

use crate::{focus_ring, hairline, layered, motion, perf, pill_edge};

pub fn segmented_control(
    id: &str,
    options: &[&str],
    selected: usize,
    theme: Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let labels: Vec<SharedString> = options.iter().map(|s| s.to_string().into()).collect();
    segmented(
        id,
        labels.len(),
        selected,
        Theme::control_height_large(),
        theme,
        on_select,
        move |index, _| {
            div()
                .px(px(Theme::control_padding()))
                .text_ellipsis()
                .child(labels[index].clone())
                .into_any_element()
        },
    )
}

/// [`segmented_control`] at any height, drawing each segment's content
/// itself: `content(index, color)` returns what segment `index` shows, in
/// `color` — `text` on the current segment, `muted` easing to `text` on
/// hover on the rest. The segment already sets that colour, the body size
/// and the weight, so plain text needs no styling of its own; a glyph is
/// the one thing that has to be handed the colour.
pub fn segmented(
    id: &str,
    count: usize,
    selected: usize,
    height: f32,
    theme: Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    content: impl Fn(usize, Hsla) -> AnyElement,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);
    let id = SharedString::from(id.to_owned());
    let options = 0..count;
    let count = count.max(1) as f32;
    // Each segment runs a 0..1 tween toward being the current one. Summing
    // index × progress gives the pill's position along the track: while it
    // moves, the old segment's progress falls as the new one's rises on the
    // same curve, so the sum slides from one index to the other.
    let position: f32 = options
        .clone()
        .map(|index| {
            let key = motion::tween_key(&ElementId::from((id.clone(), index)), "segment");
            index as f32 * motion::state_fade(&key, index == selected)
        })
        .sum();
    // The track's padding is split: half on the track, half inside each
    // slot. The slots then divide the track exactly, so the pill can be
    // placed and sized as a fraction of it, and half a gap on each side of
    // neighbouring slots is the whole gap between them.
    let half_gap = px(Theme::gap_small() / 2.);
    let pill = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(position / count))
        .w(relative(1. / count))
        .px(half_gap)
        .child(
            div()
                .size_full()
                .rounded_full()
                .bg(theme.seg_active)
                .shadow(vec![theme.segment_shadow()])
                .child(pill_edge(vec![hairline(
                    theme.line,
                    Theme::hairline_width(),
                )])),
        );
    div()
        .flex()
        .flex_none()
        .h(px(height))
        .py(px(Theme::gap_small()))
        .px(half_gap)
        .rounded_full()
        .bg(theme.sunk)
        .child(
            div()
                .relative()
                .flex()
                .flex_1()
                .min_w_0()
                .child(layered(pill))
                .children(
                    options
                        .map(|index| {
                            let pick = on_select.clone();
                            let element_id = ElementId::from((id.clone(), index));
                            let active = index == selected;
                            let hover_key = motion::tween_key(&element_id, "hover");
                            let color = if active {
                                theme.text
                            } else {
                                motion::hover_blend(&hover_key, theme.muted, theme.text)
                            };
                            let ring = focus_ring(theme);
                            let press = theme.press;
                            let click_id = element_id.clone();
                            div()
                                .id(element_id)
                                .flex_1()
                                .flex()
                                .min_w_0()
                                .px(half_gap)
                                .rounded_full()
                                .cursor_pointer()
                                // Pressed as a button presses: the `press`
                                // wash and a dim. The current segment keeps
                                // its pill and only dims, since a wash would
                                // show as a grey pill over a white one.
                                .active(move |s| {
                                    if active { s } else { s.bg(press) }
                                        .opacity(Theme::pressed_opacity())
                                })
                                .tab_index(0)
                                .focus_visible(move |s| s.shadow(vec![ring]))
                                .on_hover(motion::hover_listener(hover_key))
                                .on_click(move |_, w, cx| {
                                    perf::log(format_args!("click segment {click_id:?}"));
                                    pick(index, w, cx)
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .min_w_0()
                                        .text_color(color)
                                        .text_size(px(Theme::font_body()))
                                        .font_weight(if active {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .child(content(index, color)),
                                )
                        })
                        // Each segment on a layer over the pill and the
                        // track: its focus ring is a shadow, and inside a
                        // frosted float a shadow at the track's draw order
                        // goes under the track's fill.
                        .map(layered),
                ),
        )
}
