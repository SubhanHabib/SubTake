//! Inspector row plates: a setting's label beside its control, with the
//! card and grid variants that group several of them.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{column, row};

/// A setting as a plate: its label on the left, its control on the right, at
/// the shared control geometry. This is the shape almost every inspector row
/// takes — a bare label above a bare control reads as two things, not one.
pub fn field_row(theme: Theme, label: impl Into<SharedString>) -> Div {
    row()
        .h(px(Theme::CONTROL_HEIGHT))
        .flex_none()
        .px(px(Theme::CONTROL_PADDING))
        .gap(px(Theme::GAP))
        .rounded(px(Theme::RADIUS_CONTROL))
        .bg(theme.sunk)
        .text_size(px(Theme::FONT_CONTROL))
        .text_color(theme.text)
        .child(div().flex_1().min_w_0().text_ellipsis().child(label.into()))
}

/// A setting that needs a sentence to explain it: title, detail and the
/// control that changes it, together on one plate instead of a control with a
/// loose muted line drifting underneath it.
pub fn setting_card(
    theme: Theme,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
) -> Div {
    row()
        .items_start()
        .flex_none()
        .p(px(Theme::GAP_LARGE))
        .gap(px(Theme::GAP))
        .rounded(px(Theme::RADIUS_CARD))
        .bg(theme.sunk)
        .child(
            column()
                .flex_1()
                .min_w_0()
                .gap(px(Theme::GAP_SMALL))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_size(px(Theme::FONT_SMALL))
                        .text_color(theme.muted)
                        .child(detail.into()),
                ),
        )
}

/// Related controls gathered under their own quiet label, on a recessed
/// plate — the reference's "Webcam Crop", "Position", "Webcam Footage".
pub fn group_card(theme: Theme, label: impl Into<SharedString>) -> Div {
    column()
        .flex_none()
        .gap(px(Theme::GAP))
        .p(px(Theme::GAP_LARGE))
        .rounded(px(Theme::RADIUS_CARD))
        .bg(theme.sunk)
        .child(
            div()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(theme.muted)
                .child(label.into()),
        )
}

/// An even grid of tiles `columns` across — a picker reads as a grid or as a
/// ragged wrap, and the reference's are grids.
pub fn tile_grid(columns: u16) -> Div {
    div().grid().grid_cols(columns).gap(px(Theme::GAP_SMALL))
}
