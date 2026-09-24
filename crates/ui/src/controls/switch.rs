//! The switch, bare or on its labelled plate.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{field_row, focus_ring, layered, motion, perf};

/// The bare switch, with no label and no plate of its own.
///
/// A 46 × 28 track with a 22px thumb inset 3, so the thumb travels 18 — the
/// geometry the redesign specifies, and the reason none of these numbers are
/// written here: they are derived from each other in the token set.
///
/// The redesign also widens the thumb to 26 while it is held. gpui at the
/// pinned revision cannot restyle a child from its parent's active state, so
/// that one state is not carried; everything else is.
pub fn switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    t: Theme,
    change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.into();
    // The thumb slides and the track fills on one progress value, so the
    // plate is never briefly filled under a thumb that has not moved yet.
    let on = motion::state_fade(&motion::tween_key(&id, "switch"), checked);
    let click_id = id.clone();
    let ring = focus_ring(t);
    // Hover lays a wash over the track rather than swapping its fill: the
    // off track is already `sunk2`, the top of the fill scale, so there is
    // no firmer tone to move to. On the mid-grey `switch_on` track the wash
    // is a lift of the thumb's white, which shows against it in both
    // appearances.
    let hover_key = motion::tween_key(&id, "hover");
    let wash = motion::blend(t.hover, t.thumb().opacity(Theme::switch_on_hover()), on);
    let mut switch = div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(Theme::toggle_width()))
        .h(px(Theme::toggle_height()))
        .rounded_full()
        .bg(motion::blend(t.sunk2, t.switch_on, on))
        .opacity(if enabled {
            1.
        } else {
            Theme::disabled_opacity()
        })
        // The hover wash and the thumb on it are on a layer over the track,
        // or the thumb's drop shadow goes under the track's fill inside a
        // frosted float — at one draw order gpui draws every shadow before
        // every fill.
        .child(layered(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .bg(motion::hover_blend(&hover_key, wash.opacity(0.), wash))
                .child(
                    div()
                        .absolute()
                        .top(px(Theme::toggle_inset()))
                        .left(px(motion::lerp(
                            Theme::toggle_inset(),
                            Theme::toggle_inset() + Theme::toggle_travel(),
                            on,
                        )))
                        .size(px(Theme::toggle_thumb()))
                        .rounded_full()
                        // White in both states and both appearances: on and off are
                        // the track's tone and the thumb's side, never the thumb's
                        // colour. An `ink` track with an `on_ink` thumb read as off
                        // in dark, where it was a black dot on white. On the light
                        // off track that is white on a near-white `sunk2`, so the
                        // thumb's own drop shadow is what separates it.
                        .bg(t.thumb())
                        .shadow(vec![BoxShadow {
                            color: hsla(0., 0., 0., 0.3),
                            offset: point(px(0.), px(1.)),
                            blur_radius: px(3.),
                            spread_radius: px(0.),
                            inset: false,
                        }]),
                ),
        ));
    if enabled {
        switch = switch
            .tab_index(0)
            .focus_visible(move |s| s.shadow(vec![ring]))
            .cursor_pointer()
            .active(|s| s.opacity(Theme::pressed_opacity()))
            .on_hover(motion::hover_listener(hover_key))
            .on_click(move |_, w, cx| {
                perf::log(format_args!("click switch {click_id:?} -> {}", !checked));
                change(!checked, w, cx)
            });
    }
    // A layer of its own for the focus ring, which falls outside the track
    // onto the plate the switch sits on and would otherwise draw under it.
    layered(switch)
}

/// The switch on its own plate with its label — the shape a setting takes when
/// it stands alone rather than inside a `setting_card`.
pub fn toggle(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    t: Theme,
    change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    field_row(t, label).child(switch(id, checked, enabled, t, change))
}
