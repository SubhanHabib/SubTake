//! Small status marks: the state dot, context chip, composer footer and
//! progress rule.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

/// The small filled dot that marks a state on a label — an unsaved document,
/// a live source. Achromatic like everything else, so it reads as emphasis
/// rather than as a status colour.
pub fn status_dot(theme: Theme) -> Div {
    div()
        .flex_none()
        .size(px(Theme::DOT_SIZE))
        .rounded(px(Theme::DOT_SIZE / 2.0))
        .bg(theme.accent)
}

/// The quiet context strip that sits above a composer or palette input and
/// names what the surface is acting on. Several short labels in one plate,
/// separated by spacing rather than punctuation.
pub fn context_chip(theme: Theme, parts: &[&str]) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Theme::GAP_SMALL))
        .h(px(Theme::CHIP_HEIGHT))
        .px(px(Theme::GAP_LARGE))
        .rounded(px(Theme::RADIUS_SMALL))
        .bg(theme.sunk)
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
        // A separator between the parts, or "Recorder More" reads as one
        // phrase rather than as a trail.
        .children(parts.iter().enumerate().flat_map(|(i, part)| {
            let lead = (i > 0).then(|| {
                div()
                    .flex_none()
                    .text_color(theme.muted.opacity(0.6))
                    .child("/")
            });
            lead.into_iter()
                .chain([div().text_ellipsis().min_w_0().child(part.to_string())])
        }))
}

/// The muted strip of affordances under a composer input: small quiet
/// controls on the left, a plain status word on the right of them. It is
/// deliberately not a toolbar — nothing here carries a plate at rest.
pub fn composer_footer(theme: Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Theme::GAP_SMALL))
        .px(px(Theme::GAP_SMALL))
        .h(px(Theme::FOOTER_HEIGHT))
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
}

/// A determinate progress rule: a wash track with a filled bar over it.
pub fn progress_bar(fraction: f32, theme: Theme) -> Div {
    div()
        .h(px(Theme::PROGRESS_HEIGHT))
        .rounded_full()
        .overflow_hidden()
        .bg(theme.sunk2)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0., 1.)))
                .bg(theme.text),
        )
}
