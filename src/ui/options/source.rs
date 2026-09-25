//! The Source card: what the recorder captures.

use super::*;

impl RootView {
    /// Displays as pictures, then windows as rows: you are choosing a
    /// picture, so the card shows pictures.
    pub(super) fn source_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let selected = state.get_source_index();
        let enabled = !state.get_busy();
        let sources: Vec<_> = state.get_capture_sources().iter().enumerate().collect();
        let mut displays = row().gap(px(Theme::gap())).items_start();
        let mut windows = column().gap(px(Theme::list_gap()));
        for (index, source) in &sources {
            let chosen = *index as i32 == selected;
            let options = state.clone();
            let index = *index;
            let choose = move |_: &ClickEvent, _: &mut Window, _: &mut App| {
                options.defer_option("source".into(), index.to_string())
            };
            if source.kind == "display" {
                displays = displays.child(
                    display_tile(index, source, chosen, enabled, theme)
                        .when(enabled, |tile| tile.on_click(choose)),
                );
            } else {
                windows = windows.child(
                    window_row(index, source, chosen, enabled, theme)
                        .when(enabled, |tile| tile.on_click(choose)),
                );
            }
        }
        let has = |kind: &str| sources.iter().any(|(_, s)| s.kind == kind);
        let mut body = Vec::new();
        if has("display") {
            body.push(caps_label("Displays", theme).into_any_element());
            body.push(displays.into_any_element());
        }
        if has("window") {
            body.push(
                caps_label("Windows", theme)
                    .mt(px(Theme::list_gap()))
                    .into_any_element(),
            );
            body.push(windows.into_any_element());
        }
        // Not drawn by the design: no sources at all — before the first
        // refresh, or without the screen-recording permission. The status
        // line says which, where the list would be.
        if sources.is_empty() {
            body.push(helper(state.get_status(), theme).into_any_element());
        }
        body.push(if state.get_sources_loading() {
            refreshing(theme).into_any_element()
        } else {
            self.action(
                "refresh",
                "Refresh displays and windows",
                "sources",
                enabled,
            )
            .glyph("ArrowClockwise-regular")
            .standard()
            .into_any_element()
        });
        body.push(
            helper("Area selection starts after you press Record.", theme).into_any_element(),
        );
        body
    }
}

/// A display: its picture, and its name and resolution under it.
///
/// While the card is busy the sources cannot be changed, so they dim and
/// take no pointer, as a disabled control does.
fn display_tile(
    index: usize,
    source: &CaptureSource,
    chosen: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("source-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    // The chosen picture carries the accent twice, inside and out, so it
    // reads as picked even where the picture itself is mostly blue. Both
    // halves are one inset edge on a plate grown by the outer half: gpui
    // does not clip an outer shadow to what is outside its box, so a spread
    // shadow on a see-through overlay would fill the picture with accent.
    let grow = Theme::selected_width();
    let ring = div()
        .absolute()
        .top(px(-grow))
        .left(px(-grow))
        .right(px(-grow))
        .bottom(px(-grow))
        .rounded(px(Theme::radius_inner() + grow))
        .when(chosen, |s| {
            s.shadow(vec![hairline(theme.accent, Theme::selected_width() * 2.)])
        })
        .when(!chosen && enabled, |s| {
            s.shadow(vec![hairline(
                subtake_ui::motion::hover_blend(&hover_key, theme.line.opacity(0.), theme.line),
                Theme::selected_width(),
            )])
        });
    let picture = thumbnail(
        &source.thumbnail,
        "Monitor-regular",
        Theme::source_thumb_height(),
        Theme::radius_inner(),
        theme,
    )
    .w_full();
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(Theme::source_tile_gap()))
        // The radius is for the focus ring alone, which goes round the
        // picture and its caption together, as a wallpaper tile's does.
        .rounded(px(Theme::radius_inner()))
        .map(|s| {
            if enabled {
                subtake_ui::pressable(s, theme, None, hover_key)
            } else {
                s.opacity(Theme::disabled_opacity())
            }
        })
        .child(div().relative().child(picture).child(layered(ring)))
        .child(
            div()
                .text_size(px(Theme::font_secondary()))
                .text_ellipsis()
                .when(chosen, |s| s.font_weight(FontWeight::MEDIUM))
                .child(source.name.clone()),
        )
        .child(
            mono(source.detail.clone())
                .text_size(px(Theme::font_small()))
                .text_color(theme.muted),
        )
}

/// A window: a small picture of it and its title.
fn window_row(
    index: usize,
    source: &CaptureSource,
    chosen: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("source-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(Theme::icon_gap_row()))
        .h(px(Theme::control_height_small()))
        .px(px(Theme::control_padding_small()))
        .rounded(px(Theme::radius_lane()))
        .map(|s| {
            if enabled {
                s.bg(subtake_ui::motion::hover_blend(
                    &hover_key,
                    theme.sunk2.opacity(0.),
                    theme.sunk2,
                ))
                .map(|s| subtake_ui::pressable(s, theme, Some(theme.press), hover_key))
            } else {
                s.opacity(Theme::disabled_opacity())
            }
        })
        .child(
            thumbnail(
                &source.thumbnail,
                "Image-regular",
                Theme::window_thumb_height(),
                Theme::window_thumb_radius(),
                theme,
            )
            .w(px(Theme::window_thumb_width())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .child(source.name.clone()),
        )
        .when(chosen, |s| {
            s.child(selection_ring(Some(Theme::radius_lane()), theme))
        })
}

/// A source's picture, or — not drawn by the design — its kind's glyph on
/// `sunk` when no still has been taken of it.
fn thumbnail(
    image: &crate::ui_runtime::Image,
    glyph: &str,
    height: f32,
    radius: f32,
    theme: Theme,
) -> Div {
    let plate = div()
        .flex_none()
        .h(px(height))
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    match &image.0 {
        // gpui clips overflow to the box, not its corners, so the picture
        // carries the radius itself.
        Some(image) => plate.child(
            img(image.clone())
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        None => plate
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                glyph,
                (height * 0.4).min(Theme::icon_size_large()),
                theme.muted,
            )),
    }
}

/// Not drawn by the design: the Refresh button while a refresh runs — the
/// same plate and geometry, its arrow turning and its caption saying so.
fn refreshing(theme: Theme) -> Div {
    row()
        .justify_center()
        .gap(px(Theme::icon_gap()))
        .h(px(Theme::control_height()))
        .rounded_full()
        .bg(theme.sunk)
        .child(icon("ArrowClockwise-regular", theme.text).with_animation(
            "refresh-spin",
            Animation::new(std::time::Duration::from_secs(1)).repeat(),
            |svg, t| svg.with_transformation(Transformation::rotate(percentage(t))),
        ))
        .child("Refreshing…")
}
