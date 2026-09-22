//! The switch, bare or on its labelled plate.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{field_row, motion, perf};

/// The bare switch, with no label and no plate of its own.
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
    // A 30px pill inside the 40px control slot, with a 24px thumb.
    let mut switch = div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(52.))
        .h(px(30.))
        .rounded(px(15.))
        .bg(motion::blend(t.sunk2, t.accent, on))
        .opacity(if enabled { 1. } else { Theme::DISABLED_OPACITY })
        .child(
            div()
                .absolute()
                .top(px(3.))
                .left(px(motion::lerp(3., 25., on)))
                .size(px(24.))
                .rounded(px(12.))
                // White on both tracks, as the redesign specifies. On the
                // off track that is white on a near-white `sunk2` in the
                // light appearance, so the thumb's own drop shadow is what
                // separates it — not a second fill.
                .bg(t.thumb())
                .shadow(vec![BoxShadow {
                    color: hsla(0., 0., 0., 0.3),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(3.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        );
    if enabled {
        switch = switch
            .tab_index(0)
            .focus_visible(move |s| s.border_2().border_color(t.accent))
            .cursor_pointer()
            .on_click(move |_, w, cx| {
                perf::log(format_args!("click switch {click_id:?} -> {}", !checked));
                change(!checked, w, cx)
            });
    }
    switch
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
