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

/// The document's status, as a chip at the end of the title pill: a
/// running job, an error, "Gallery mode". `sunk2` on the pill's `sunk`, in
/// the small face at `muted`, so it is read after the title and never
/// instead of it.
pub fn status_chip(theme: Theme) -> Div {
    div()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(Theme::GAP_SMALL))
        .h(px(Theme::CHIP_HEIGHT))
        .px(px(Theme::STATUS_CHIP_PADDING))
        .rounded_full()
        .bg(theme.sunk2)
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
        .whitespace_nowrap()
}

/// The quiet context strip that sits above a composer, a palette or a card
/// and names what the surface is acting on: a trail of short labels, the
/// last — where you are — in `text` and the rest muted, with a half-strength
/// "/" between them so "Recorder More" does not read as one phrase.
pub fn context_chip(theme: Theme, parts: &[&str]) -> Div {
    let last = parts.len().saturating_sub(1);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(Theme::GAP))
        .h(px(Theme::CHIP_HEIGHT))
        .px(px(Theme::GAP_LARGE))
        .rounded_full()
        .bg(theme.sunk)
        .text_size(px(Theme::FONT_SMALL))
        .text_color(theme.muted)
        .whitespace_nowrap()
        .children(parts.iter().enumerate().flat_map(|(i, part)| {
            let lead = (i > 0).then(|| div().flex_none().opacity(0.5).child("/"));
            let part = div().min_w_0().text_ellipsis().child(part.to_string());
            lead.into_iter().chain([if i == last {
                part.text_color(theme.text)
            } else {
                part
            }])
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

/// A determinate progress rule: a `sunk` track under an `accent` fill.
///
/// Accent, not `text`. Progress is one of the four things the accent marks —
/// it is the primary action, still running — and a near-black bar on a light
/// track read as a rule rather than as something moving.
pub fn progress_bar(fraction: f32, theme: Theme) -> Div {
    div()
        .flex_none()
        .h(px(Theme::PROGRESS_HEIGHT))
        .rounded(px(Theme::PROGRESS_RADIUS))
        .overflow_hidden()
        .bg(theme.sunk)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0., 1.)))
                .rounded(px(Theme::PROGRESS_RADIUS))
                .bg(theme.accent),
        )
}
