//! A list row: a Moments entry, a recent file, a preset.
//!
//! The app draws its presets as a grid of tiles, so nothing calls this yet.
//! It is the one primitive here that is a whole layout rather than a control,
//! and the states it carries — cut, dragging — are the ones a list of clips
//! needs and a grid of tiles does not.

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, Theme};

use crate::{column, hairline, row};

const ROW_PADDING_Y: f32 = 11.0;
const ROW_PADDING_X: f32 = 14.0;
const ROW_GAP: f32 = 14.0;
/// 84 x 48 for a compact list, 96 x 54 where the row has the width. This is
/// the compact one; a caller wanting the larger overrides the two.
const THUMB_WIDTH: f32 = 84.0;
const THUMB_HEIGHT: f32 = 48.0;
const THUMB_RADIUS: f32 = 12.0;
/// "meta — Geist 400 / 12" and "timecode — Geist Mono 400 / 12": one step
/// below the title and one above the caps label.
const META_SIZE: f32 = 12.0;

/// How a row is being treated, beyond selected.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RowState {
    #[default]
    Normal,
    /// Cut to the clipboard: half opacity, title struck through.
    Cut,
    /// The source of a drag in progress. The row being dragged carries the
    /// shadow and the scale; this is the hole it left.
    DragSource,
}

/// One row. `thumbnail` is whatever the caller paints in the 84 x 48 plate —
/// an image, a colour, a glyph — so this does not have to know what a moment
/// looks like.
pub fn list_row(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    meta: impl Into<SharedString>,
    timecode: impl Into<SharedString>,
    thumbnail: impl IntoElement,
    selected: bool,
    state: RowState,
    theme: Theme,
) -> Stateful<Div> {
    let mut el = row()
        .id(id)
        .py(px(ROW_PADDING_Y))
        .px(px(ROW_PADDING_X))
        .gap(px(ROW_GAP))
        .rounded(px(Theme::RADIUS_ROW))
        .hover(|s| s.bg(theme.hover));
    if selected {
        // Selected is a step up in material plus an accent edge — the same
        // rule every other selected thing in the kit follows, and never an
        // accent fill.
        el = el.bg(theme.card).shadow(
            [hairline(theme.accent, Theme::SELECTED_WIDTH)]
                .into_iter()
                .chain(theme.panel_shadow())
                .collect::<Vec<_>>(),
        );
    }
    if state == RowState::Cut {
        el = el.opacity(0.5);
    } else if state == RowState::DragSource {
        el = el.opacity(0.3);
    }
    el.child(
        div()
            .flex_none()
            .w(px(THUMB_WIDTH))
            .h(px(THUMB_HEIGHT))
            .rounded(px(THUMB_RADIUS))
            .overflow_hidden()
            .bg(theme.sunk)
            .child(thumbnail),
    )
    .child(
        column()
            .flex_1()
            .min_w_0()
            .gap(px(Theme::GAP_SMALL))
            .child(
                div()
                    .text_ellipsis()
                    .text_size(px(Theme::FONT_CONTROL))
                    .text_color(theme.text)
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .when(state == RowState::Cut, |s| s.line_through())
                    .child(title.into()),
            )
            .child(
                div()
                    .text_ellipsis()
                    .text_size(px(META_SIZE))
                    .text_color(theme.muted)
                    .child(meta.into()),
            ),
    )
    .child(
        div()
            .flex_none()
            .font_family(FONT_MONO)
            .text_size(px(META_SIZE))
            // The one place a timecode takes the accent: a selected row's
            // position is what the selection is *for*.
            .text_color(if selected { theme.accent } else { theme.muted })
            .child(timecode.into()),
    )
}
