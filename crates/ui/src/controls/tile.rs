//! Picker tiles: colour swatches, captioned thumbnails, selectable choice
//! cards and the empty-state plane.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{column, focus_ring, motion};

/// A flat colour sample — the one place a literal colour is the content
/// rather than the styling, so it carries a full-strength outline when picked.
pub fn swatch(
    id: impl Into<ElementId>,
    colour: Hsla,
    selected: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = id.into();
    let pick = motion::state_fade(&motion::tween_key(&id, "fill"), selected);
    // The pointer firms the outline up to `muted`, as it does a wallpaper
    // tile's ring: the swatch's own fill is the content, so it is the one
    // thing hover must not tint.
    let hover_key = motion::tween_key(&id, "hover");
    let ring = focus_ring(theme);
    div()
        .id(id)
        .size(px(Theme::SWATCH_SIZE))
        .rounded(px(Theme::RADIUS_LANE))
        .bg(colour)
        .border_2()
        .border_color(motion::blend(
            motion::hover_blend(&hover_key, theme.line, theme.muted),
            theme.accent,
            pick,
        ))
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(move |s| s.shadow(vec![ring]))
        .active(|s| s.opacity(Theme::PRESSED_OPACITY))
        .on_hover(motion::hover_listener(hover_key))
}

/// A captioned thumbnail in a picker grid: 48 tall at radius 14, as the
/// handoff draws the wallpaper row. The one in use takes the accent ring the
/// handoff gives the picked colour, and the pointer brings up a faint one.
///
/// Not drawn by the design: the caption, which the handoff leaves off, and
/// the hover ring.
pub fn media_tile(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    picture: Option<Img>,
    selected: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = id.into();
    let pick = motion::state_fade(&motion::tween_key(&id, "ring"), selected);
    let hover_key = motion::tween_key(&id, "hover");
    let ring = motion::blend(
        motion::hover_blend(&hover_key, theme.line, theme.muted),
        theme.accent,
        pick,
    );
    // The border sits inside the frame, so the picture's own corners are
    // rounded to the border's inner edge: gpui's clip is square, and would
    // leave the picture's corners poking past a rounded frame.
    let inner = Theme::RADIUS_INNER - Theme::TILE_RING_WIDTH;
    let frame = div()
        .h(px(Theme::TILE_HEIGHT))
        .w_full()
        .rounded(px(Theme::RADIUS_INNER))
        .border_2()
        .border_color(ring)
        .bg(theme.sunk)
        .when_some(picture, |frame, picture| {
            frame.child(
                picture
                    .size_full()
                    .rounded(px(inner))
                    .object_fit(ObjectFit::Cover),
            )
        });
    let focus = focus_ring(theme);
    div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP_SMALL))
        .id(id)
        .w(px(Theme::TILE_WIDTH))
        // The radius is for the focus ring alone, which goes round the
        // picture and its caption together: the tile is both.
        .rounded(px(Theme::RADIUS_INNER))
        .tab_index(0)
        .focus_visible(move |s| s.shadow(vec![focus]))
        .cursor_pointer()
        .active(|s| s.opacity(Theme::PRESSED_OPACITY))
        .on_hover(motion::hover_listener(hover_key))
        .child(frame)
        .child(
            div()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(if selected { theme.text } else { theme.muted })
                .text_ellipsis()
                .child(title.into()),
        )
}

/// The "nothing here yet" plane: one display headline, one muted line, and
/// the actions that get the user out of it.
pub fn empty_state(
    theme: Theme,
    headline: impl Into<SharedString>,
    detail: impl Into<SharedString>,
) -> Div {
    column()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(Theme::EMPTY_GAP))
        .px(px(Theme::EMPTY_PADDING_X))
        .pb(px(Theme::EMPTY_PADDING_BOTTOM))
        // Space Grotesk at 40, the one place the redesign is loud. It was
        // Geist SemiBold: the face was bundled and registered and nothing had
        // ever asked for it.
        .child(crate::title(headline, Theme::FONT_DISPLAY))
        .child(
            div()
                .max_w(px(Theme::EMPTY_TEXT_WIDTH))
                .text_center()
                .text_size(px(Theme::FONT_ACTION))
                .text_color(theme.muted)
                .child(detail.into()),
        )
}

// ---------------------------------------------------------------------------
// selection tile
// ---------------------------------------------------------------------------

pub fn choice_tile(
    id: impl Into<ElementId>,
    selected: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = id.into();
    let pick = motion::state_fade(&motion::tween_key(&id, "fill"), selected);
    let hover_key = motion::tween_key(&id, "hover");
    let wash = motion::blend(theme.sunk, theme.accent_soft, pick);
    div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP_SMALL))
        .id(id)
        .min_w_0()
        .p(px(Theme::GAP))
        .rounded(px(Theme::RADIUS_MENU))
        // A tile is too big to invert wholesale, so "selected" reads as the
        // deeper grey wash plus a full-strength outline — the outlined half
        // of the same filled/outlined language the buttons use.
        .bg(motion::hover_blend(&hover_key, wash, theme.hover))
        .border_1()
        .border_color(motion::blend(theme.line, theme.accent, pick))
        .opacity(if enabled { 1. } else { Theme::DISABLED_OPACITY })
        .tab_index(0)
        .tab_stop(enabled)
        // The ring thickens inward, as a second hairline inside the border,
        // rather than by widening the border: a wider border is layout, and
        // took a pixel off every side of what the tile holds.
        .focus_visible(move |s| {
            s.border_color(theme.accent)
                .shadow(vec![crate::hairline(theme.accent, Theme::BORDER_WIDTH)])
        })
        .when(enabled, |s| {
            s.cursor_pointer()
                .active(|s| s.opacity(Theme::PRESSED_OPACITY))
                .on_hover(motion::hover_listener(hover_key))
        })
}
