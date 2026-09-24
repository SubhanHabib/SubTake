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
    /// A second part in Geist Mono at a lower strength: a region's range.
    detail: Option<SharedString>,
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
                .gap(px(Theme::gap()))
                .max_w(px(Theme::tooltip_max_width()))
                .min_h(px(Theme::tooltip_height()))
                .py(px(Theme::tooltip_padding_y()))
                .px(px(Theme::icon_gap_row()))
                .rounded(px(Theme::radius_region()))
                .bg(theme.ink)
                .text_size(px(Theme::font_secondary()))
                .line_height(relative(Theme::message_leading()))
                .text_color(theme.on_ink)
                .child(self.text.clone())
                .when_some(self.detail.clone(), |el, detail| {
                    el.child(
                        crate::mono(detail)
                            .flex_none()
                            .opacity(Theme::tooltip_detail_alpha()),
                    )
                }),
        )
    }
}

pub fn tooltip(text: impl Into<SharedString>, theme: Theme, cx: &mut App) -> AnyView {
    let text = text.into();
    cx.new(|_| Tooltip {
        text,
        detail: None,
        theme,
    })
    .into()
}

/// A tooltip with a second, quieter part after the text: a timeline
/// region's name and then its range, `2× Speed  0:26–0:34`.
pub fn tooltip_detail(
    text: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    theme: Theme,
    cx: &mut App,
) -> AnyView {
    let (text, detail) = (text.into(), Some(detail.into()));
    cx.new(|_| Tooltip {
        text,
        detail,
        theme,
    })
    .into()
}
