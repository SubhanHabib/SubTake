//! A key cap: "22 min-square, radius 7, `--sunk` with
//! `inset 0 -1.5px 0 var(--line)` for the lip."

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, Theme};

const CAP_SIZE: f32 = 22.0;
const CAP_RADIUS: f32 = 7.0;
const CAP_PADDING: f32 = 6.0;
/// The lip: a hairline on the bottom edge only, so the cap reads as something
/// with a thickness rather than a flat tile.
const CAP_LIP: f32 = 1.5;

/// One key in a shortcut — `⌘`, `⇧`, `K`.
pub fn key_cap(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(CAP_SIZE))
        .min_w(px(CAP_SIZE))
        .px(px(CAP_PADDING))
        .rounded(px(CAP_RADIUS))
        .bg(theme.sunk)
        // An offset inset shadow, which is the one place in the kit an edge
        // is not symmetric: the lip is only on the bottom.
        .shadow(vec![BoxShadow {
            color: theme.line,
            offset: point(px(0.), px(-CAP_LIP)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        }])
        .font_family(FONT_MONO)
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
        .child(text.into())
}
