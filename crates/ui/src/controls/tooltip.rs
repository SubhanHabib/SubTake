//! The frosted hover tip a control shows when its caption cannot be read.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{frost, motion};

struct Tooltip {
    text: SharedString,
    theme: Theme,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        frost::frosted(
            Theme::RADIUS_SMALL,
            frost::MENU_BLUR,
            motion::fade_in(
                "tooltip",
                div()
                    .max_w(px(320.))
                    .px(px(Theme::GAP))
                    .py(px(Theme::GAP_SMALL))
                    // .bg(t.popup)
                    .border_1()
                    .border_color(t.border)
                    .rounded(px(Theme::RADIUS_SMALL))
                    .shadow_lg()
                    .text_size(px(Theme::FONT_SMALL))
                    .text_color(t.text)
                    .child(self.text.clone()),
            ),
        )
    }
}

pub fn tooltip(text: impl Into<SharedString>, theme: Theme, cx: &mut App) -> AnyView {
    let text = text.into();
    cx.new(|_| Tooltip { text, theme }).into()
}
