//! The Source card: what the recorder captures, as three tabs — a whole
//! display, one window, or an area drawn on the screen.

use super::parts::{CardRow, access_off, group, section_label};
use super::*;
use subtake_theme::FONT_MONO;
use subtake_ui::{edge, fade_edges, pill_edge, segmented};

/// The Window tab's search field, by the name `RootView::input` keeps it
/// under.
pub(super) const SOURCE_SEARCH: &str = "source-search";

/// The tabs, in order: their glyphs and words.
const TABS: [(&str, &str); 3] = [
    ("Monitor-regular", "Display"),
    ("AppWindow-regular", "Window"),
    ("Selection-regular", "Area"),
];
const DISPLAY: usize = 0;
const WINDOW: usize = 1;

/// Area's aspect locks, as they are kept and as they read.
const ASPECTS: [(&str, &str); 5] = [
    ("free", "Free"),
    ("16:9", "16:9"),
    ("4:3", "4:3"),
    ("1:1", "1:1"),
    ("9:16", "9:16"),
];

/// A source's two lines: a display's name and resolution, or a window's
/// app and title. The platform names a window "App — Title", and a window
/// with no title just "App".
pub(in crate::ui) fn caption(source: &CaptureSource) -> (String, String) {
    if source.kind == "window" {
        match source.name.split_once(" — ") {
            Some((app, title)) => (app.to_owned(), title.to_owned()),
            None => (source.name.clone(), String::new()),
        }
    } else {
        (source.name.clone(), source.detail.clone())
    }
}

impl RootView {
    /// The tabs, then the tab's own body. The card opens on the tab of the
    /// source that is chosen; moving between tabs changes nothing until a
    /// source on one is picked.
    pub(super) fn source_card(
        &mut self,
        state: &RecordingOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        if !state.get_screen_access() {
            return vec![
                access_off("Screen", self.command("access-screen"), theme).into_any_element(),
            ];
        }
        let sources: Vec<CaptureSource> = state.get_capture_sources().iter().collect();
        let chosen = usize::try_from(state.get_source_index()).ok();
        let tab =
            self.source_tab
                .unwrap_or_else(|| match chosen.and_then(|index| sources.get(index)) {
                    Some(source) if source.kind == "window" => WINDOW,
                    _ => DISPLAY,
                });
        let view = cx.entity().downgrade();
        let tabs = segmented(
            "source-tabs",
            TABS.len(),
            tab,
            Theme::control_height(),
            theme,
            move |index, _, cx| {
                view.update(cx, |s: &mut Self, cx| {
                    s.source_tab = Some(index);
                    cx.notify();
                })
                .ok();
            },
            |index, color| {
                row()
                    .gap(px(Theme::source_tab_gap()))
                    .child(icon_sized(TABS[index].0, Theme::icon_size_card(), color))
                    .child(TABS[index].1)
                    .into_any_element()
            },
        );
        let mut body = vec![tabs.into_any_element()];
        let enabled = !state.get_busy();
        let listed = |kind: &str| -> Vec<(usize, &CaptureSource)> {
            sources
                .iter()
                .enumerate()
                .filter(|(_, source)| source.kind == kind)
                .collect()
        };
        match tab {
            DISPLAY => {
                let displays = listed("display");
                if displays.is_empty() {
                    // Not drawn by the design: no displays — before the
                    // first refresh, or without the screen-recording
                    // permission. The status line says which.
                    body.push(helper(state.get_status(), theme).into_any_element());
                } else {
                    body.push(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap(px(Theme::display_grid_gap()))
                            .children(displays.into_iter().map(|(index, source)| {
                                source_tile(
                                    index,
                                    source,
                                    chosen == Some(index),
                                    enabled,
                                    state,
                                    theme,
                                )
                            }))
                            .into_any_element(),
                    );
                }
                body.push(display_settings(state, theme).into_any_element());
            }
            WINDOW => self.window_tab(
                state,
                &listed("window"),
                chosen,
                enabled,
                &mut body,
                window,
                cx,
            ),
            _ => body.extend(area_tab(state, &listed("display"), chosen, theme)),
        }
        body
    }

    /// The search, then the windows it matches two across, scrolling under
    /// a fade past `WINDOW_GRID_MAX_HEIGHT`.
    ///
    /// Not carried: the handoff orders the windows most recently used
    /// first. The platform lists them in the window server's order, front
    /// to back, which is close to that but not the same.
    #[allow(clippy::too_many_arguments)]
    fn window_tab(
        &mut self,
        state: &RecordingOptions,
        windows: &[(usize, &CaptureSource)],
        chosen: Option<usize>,
        enabled: bool,
        body: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let theme = self.theme;
        if windows.is_empty() {
            // Not drawn by the design, as the Display tab's.
            body.push(helper(state.get_status(), theme).into_any_element());
            return;
        }
        let search = self.input(
            SOURCE_SEARCH,
            &self.source_query.clone(),
            window,
            cx,
            |_, _, _| {},
        );
        let view = cx.entity().downgrade();
        let options = state.clone();
        let count = windows.len();
        search.update(cx, |input, _| {
            input.set_placeholder(format!(
                "Search {count} window{}",
                if count == 1 { "" } else { "s" }
            ));
            input.set_leading("MagnifyingGlass-regular");
            let filtering = view.clone();
            input.set_on_change(move |value, _, cx| {
                filtering
                    .update(cx, |s: &mut Self, cx| {
                        s.source_query = value.clone();
                        cx.notify();
                    })
                    .ok();
            });
            // The field swallows escape, so it closes the card itself.
            input.set_on_cancel(move |_, _| options.defer_panel("".into()));
        });
        // Typing is quicker than scrolling once there are more than a
        // few, so the search takes the keys as soon as the tab opens.
        if !std::mem::replace(&mut self.source_searched, true) {
            search.update(cx, |input, cx| input.focus(window, cx));
        }
        body.push(
            div()
                .relative()
                .child(search)
                .child(pill_edge(vec![hairline(
                    theme.line,
                    Theme::hairline_width(),
                )]))
                .into_any_element(),
        );

        let query = self.source_query.trim().to_lowercase();
        let matching: Vec<_> = windows
            .iter()
            .filter(|(_, source)| query.is_empty() || source.name.to_lowercase().contains(&query))
            .collect();
        if matching.is_empty() {
            body.push(
                div()
                    .flex()
                    .justify_center()
                    .p(px(Theme::window_empty_padding()))
                    .text_size(px(Theme::font_body()))
                    .text_color(theme.muted)
                    .child(format!("No windows match “{}”", self.source_query.trim()))
                    .into_any_element(),
            );
        } else {
            let grid = div()
                .grid()
                .grid_cols(2)
                .gap_x(px(Theme::window_grid_gap_column()))
                .gap_y(px(Theme::window_grid_gap_row()))
                .children(matching.into_iter().map(|&(index, source)| {
                    source_tile(index, source, chosen == Some(index), enabled, state, theme)
                }));
            let list = div()
                .id("source-windows")
                .max_h(px(Theme::window_grid_max_height()))
                .overflow_y_scroll()
                .track_scroll(&self.source_scroll)
                .child(grid);
            body.push(
                fade_edges(list)
                    .band(Theme::window_grid_fade())
                    .top(false)
                    .tracking(&self.source_scroll)
                    .into_any_element(),
            );
        }
        body.push(
            helper(
                "Only this window is captured. Anything that moves in front of it stays out of the recording.",
                theme,
            )
            .into_any_element(),
        );
    }
}

/// Hide desktop icons and Show recorder in capture, under the displays.
fn display_settings(state: &RecordingOptions, theme: Theme) -> Div {
    let setting = |key: &'static str| {
        let options = state.clone();
        switch(
            SharedString::from(format!("{key}-toggle")),
            state.get_recorder_flag(key),
            true,
            theme,
            move |on, _, _| options.defer_option(key.into(), on.to_string()),
        )
    };
    group(
        vec![
            // Not wired: the setting is kept, but the capture does not hide
            // the desktop's icons yet.
            CardRow::new("hide-desktop-icons", "Hide desktop icons")
                .plate("EyeSlash-regular")
                .subtitle("While recording")
                .trailing(setting("hide-desktop-icons")),
            CardRow::new("show-recorder", "Show recorder in capture")
                .plate("Record-regular")
                .subtitle("Off keeps the bar out of the video")
                .trailing(setting("show-recorder")),
        ],
        theme,
    )
}

/// A display or a window: its picture at 16:10, and its two lines under
/// it. Picking a display keeps the card open, to see it chosen; picking a
/// window closes it, since the window is what comes next.
///
/// While the card is busy the sources cannot be changed, so they dim and
/// take no pointer, as a disabled control does.
fn source_tile(
    index: usize,
    source: &CaptureSource,
    chosen: bool,
    enabled: bool,
    state: &RecordingOptions,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("source-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let is_window = source.kind == "window";
    let (name, detail) = caption(source);
    let radius = Theme::radius_inner();
    let picture = div()
        .relative()
        .w_full()
        .aspect_ratio(Theme::source_picture_aspect())
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    let picture = match (&source.thumbnail.0, is_window) {
        // gpui clips overflow to the box, not its corners, so a display's
        // picture carries the radius itself.
        (Some(image), false) => picture.child(
            img(image.clone())
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        // A window stands in its picture as it does on the desktop, off
        // the sides and the top and running out at the foot.
        (Some(image), true) => picture.child(
            div()
                .absolute()
                .left(relative(Theme::window_still_inset_x()))
                .right(relative(Theme::window_still_inset_x()))
                .top(relative(Theme::window_still_inset_top()))
                .bottom_0()
                .rounded_t(px(Theme::window_thumb_radius()))
                .shadow(vec![theme.window_still_shadow()])
                .child(
                    img(image.clone())
                        .size_full()
                        .rounded_t(px(Theme::window_thumb_radius()))
                        .object_fit(ObjectFit::Cover),
                ),
        ),
        // Not drawn by the design: no still taken of it yet, so its kind's
        // glyph stands in.
        (None, _) => picture
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                if is_window {
                    "AppWindow-regular"
                } else {
                    "Monitor-regular"
                },
                Theme::icon_size_large(),
                theme.muted,
            )),
    };
    let picture = picture.child(edge(
        radius,
        vec![hairline(
            subtake_ui::motion::hover_blend(&hover_key, theme.line, theme.muted),
            Theme::hairline_width(),
        )],
    ));
    // The accent over a pale halo reads as picked on any picture, even one
    // that is mostly the accent's blue. Each is a layer of its own so the
    // accent lands on top.
    let picture = if chosen {
        picture
            .child(edge(
                radius,
                vec![hairline(
                    white().opacity(Theme::source_halo_alpha()),
                    Theme::source_halo_width(),
                )],
            ))
            .child(edge(
                radius,
                vec![hairline(theme.accent, Theme::selected_width())],
            ))
            .child(
                div()
                    .absolute()
                    .top(px(Theme::source_check_inset()))
                    .right(px(Theme::source_check_inset()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(Theme::source_check()))
                    .rounded_full()
                    .bg(theme.accent)
                    .shadow(vec![theme.source_check_shadow()])
                    .child(icon_sized(
                        "Check-regular",
                        Theme::source_check_glyph(),
                        theme.on_accent,
                    )),
            )
    } else {
        picture
    };

    let second = div()
        .text_size(px(Theme::font_small()))
        .text_color(theme.muted)
        .text_ellipsis()
        .when(!is_window, |s| s.font_family(FONT_MONO))
        // An untitled window keeps the line, so the row's names align.
        .child(if detail.is_empty() {
            "\u{a0}".to_owned()
        } else {
            detail
        });
    let tile = div()
        .id(id)
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(Theme::source_tile_gap()))
        .child(picture)
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .px(px(Theme::source_caption_inset()))
                .gap(px(Theme::source_caption_gap()))
                .child(
                    div()
                        .text_size(px(Theme::font_body()))
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .child(name),
                )
                .child(second),
        );
    if !enabled {
        return tile.opacity(Theme::disabled_opacity());
    }
    let options = state.clone();
    subtake_ui::pressable(tile, theme, None, hover_key).on_click(move |_, _, _| {
        options.defer_option("source".into(), index.to_string());
        if is_window {
            options.defer_panel("".into());
        }
    })
}

/// Area: the display the area is drawn on, the aspect to hold it to, and
/// the button that starts drawing it.
///
/// Not wired: the recorder cannot capture an area yet, so nothing is ever
/// drawn on the picture and Draw area on screen does nothing. The aspect
/// is kept for when it can.
fn area_tab(
    state: &RecordingOptions,
    displays: &[(usize, &CaptureSource)],
    chosen: Option<usize>,
    theme: Theme,
) -> Vec<AnyElement> {
    let display = displays
        .iter()
        .find(|(index, _)| Some(*index) == chosen)
        .or(displays.first())
        .map(|(_, source)| *source);
    let radius = Theme::area_preview_radius();
    let preview = div()
        .relative()
        .w_full()
        .aspect_ratio(Theme::source_picture_aspect())
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    let preview = match display.and_then(|source| source.thumbnail.0.clone()) {
        Some(image) => preview.child(
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        None => preview
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                "Monitor-regular",
                Theme::camera_off_icon(),
                theme.muted,
            )),
    }
    .child(edge(
        radius,
        vec![hairline(theme.line, Theme::hairline_width())],
    ));

    let current = state.get_recorder_setting("area-aspect");
    let options = state.clone();
    let aspect = segmented(
        "area-aspect",
        ASPECTS.len(),
        ASPECTS
            .iter()
            .position(|(key, _)| *key == current)
            .unwrap_or(0),
        Theme::control_height(),
        theme,
        move |index, _, _| options.defer_option("area-aspect".into(), ASPECTS[index].0.into()),
        |index, _| {
            div()
                .text_size(px(Theme::font_secondary()))
                .child(ASPECTS[index].1)
                .into_any_element()
        },
    );

    let hover_key = subtake_ui::motion::tween_key(&ElementId::from("area-draw"), "hover");
    let draw = div()
        .id("area-draw")
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(Theme::area_draw_gap()))
        .h(px(Theme::area_draw_height()))
        .rounded_full()
        .bg(theme.sunk2)
        .shadow(vec![theme.resting_shadow()])
        .text_size(px(Theme::font_area_draw()))
        .font_weight(FontWeight::MEDIUM)
        .child(icon_sized(
            "Selection-regular",
            Theme::area_draw_icon(),
            theme.text,
        ))
        .child("Draw area on screen")
        .child(pill_edge(vec![hairline(
            theme.line,
            Theme::hairline_width(),
        )]));
    let draw = subtake_ui::pressable(draw, theme, Some(theme.press), hover_key);

    vec![
        preview.into_any_element(),
        section_label("Aspect", theme).into_any_element(),
        aspect.into_any_element(),
        draw.into_any_element(),
        helper(
            "Drag across any display. Hold ⇧ to keep the ratio, press Esc to cancel.",
            theme,
        )
        .into_any_element(),
    ]
}
