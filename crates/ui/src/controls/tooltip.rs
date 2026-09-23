//! The hover tip a control shows when its caption cannot be read.
//!
//! An `ink` plate, not a frosted one. A tooltip is the one surface with no
//! business showing the desktop through it: it appears for 400ms over
//! whatever the pointer is on, so it has to be legible immediately, and the
//! redesign gives it the same achromatic fill the transport button uses.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::motion;

struct Tooltip {
    text: SharedString,
    theme: Theme,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        motion::fade_in(
            "tooltip",
            div()
                .flex()
                .items_center()
                .max_w(px(Theme::TOOLTIP_MAX_WIDTH))
                .h(px(Theme::TOOLTIP_HEIGHT))
                .px(px(Theme::ICON_GAP_ROW))
                .rounded(px(Theme::RADIUS_REGION))
                .bg(theme.ink)
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.on_ink)
                .child(self.text.clone()),
        )
    }
}

pub fn tooltip(text: impl Into<SharedString>, theme: Theme, cx: &mut App) -> AnyView {
    let text = text.into();
    cx.new(|_| Tooltip { text, theme }).into()
}
