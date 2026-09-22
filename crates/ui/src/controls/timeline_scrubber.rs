//! The timeline playhead: cap, rule and six-dot grip.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

/// The playhead: a rounded cap, a continuous rule, and a six-dot grip the
/// pointer can take hold of anywhere down the track stack.
pub fn timeline_scrubber(theme: Theme, grip_top: f32) -> impl IntoElement {
    let dot = move |row: usize, col: f32| {
        div()
            .absolute()
            .left(px(col))
            .top(px(22.0 + row as f32 * 7.0))
            .size(px(3.))
            .rounded(px(1.5))
            .bg(theme.toggle_thumb)
    };
    div()
        .w(px(Theme::SCRUBBER_WIDTH))
        .h_full()
        .relative()
        // Continuous rule down the whole track stack.
        .child(
            div()
                .absolute()
                .left(px(12.))
                .top(px(17.))
                .bottom_0()
                .w(px(2.))
                .bg(theme.playhead),
        )
        // Rounded cap.
        .child(
            div()
                .absolute()
                .left(px(6.))
                .top_0()
                .w(px(14.))
                .h(px(17.))
                .rounded(px(6.))
                .bg(theme.playhead)
                .shadow_sm(),
        )
        // Grip: the handle the pointer takes hold of.
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(grip_top))
                .w(px(Theme::SCRUBBER_WIDTH))
                .h(px(Theme::SCRUBBER_GRIP_HEIGHT))
                .rounded(px(13.))
                .bg(theme.playhead.opacity(0.55))
                .border_1()
                .border_color(theme.toggle_thumb.opacity(0.35))
                .shadow_lg()
                .children((0..3).map(|r| dot(r, 8.0)))
                .children((0..3).map(|r| dot(r, 15.0))),
        )
}
