//! A mark that glides between the slots of a list — the open section of a
//! sidebar, the active tool in the pod, the row under the pointer — instead
//! of switching off in one place and on in another.
//!
//! Not drawn by the design: the handoff gives each state as a still.

use gpui::{prelude::*, *};

/// Where a glide mark sits this frame, in slots, and how much of it shows.
#[derive(Clone, Copy)]
pub struct Glide {
    pub at: f32,
    pub strength: f32,
}

/// Each slot's index weighted by its 0..1 tween toward holding the mark.
///
/// While the mark moves, the old slot's tween falls as the new one's rises
/// on the same curve, so the weighted mean slides from one to the other, as
/// the segmented control's thumb does. When no slot holds it, it fades out
/// where it last was; `None` once it has gone.
pub fn glide(progress: impl IntoIterator<Item = f32>) -> Option<Glide> {
    let (mut at, mut strength) = (0., 0.);
    for (index, t) in progress.into_iter().enumerate() {
        at += index as f32 * t;
        strength += t;
    }
    (strength > f32::EPSILON).then(|| Glide {
        at: at / strength,
        strength: strength.min(1.),
    })
}

/// The mark: a strip across its slots' container, `extent` tall, at the
/// glide's slot with slots `pitch` apart, shown at the glide's strength.
/// The container is `relative` and the strip goes in before the slots, so
/// they paint over it; the caller gives it its fill and shape.
pub fn glide_mark(glide: Glide, pitch: f32, extent: f32) -> Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .top(px(glide.at * pitch))
        .h(px(extent))
        .opacity(glide.strength)
}
