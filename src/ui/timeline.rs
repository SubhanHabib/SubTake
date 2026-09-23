//! The timeline: tracks, regions, the scrubber and its drag gestures.

use super::*;

/// How far the console's zoom out and in step, and the most the timeline
/// magnifies: a hundredth of the take across the lanes.
const TIMELINE_ZOOM_STEP: f32 = 1.5;
const TIMELINE_ZOOM_MAX: f32 = 100.;

/// The ruler's tick intervals, in seconds. It takes the smallest that keeps
/// its labels `RULER_LABEL_SPACING` apart at the current zoom.
const RULER_INTERVALS: [f32; 9] = [1., 2., 5., 10., 15., 30., 60., 120., 300.];

/// A time as the ruler and the playhead chip write it: `m:ss`, or `m:ss.cc`
/// with hundredths — the transport's own format, so the two agree.
fn ruler_clock(seconds: f32, hundredths: bool) -> String {
    let centis = (seconds.max(0.) * 100.).round() as u64;
    let (minutes, rest) = (centis / 6000, centis % 6000);
    if hundredths {
        format!("{minutes}:{:02}.{:02}", rest / 100, rest % 100)
    } else {
        format!("{minutes}:{:02}", rest / 100)
    }
}

/// The icon a region of this kind shows beside its label, and alone when the
/// region is too short for one. Zoom, clip and audio regions have none.
fn region_icon(kind: &str) -> Option<&'static str> {
    match kind {
        "speedRegions" => Some("Timer-regular"),
        "trimRegions" => Some("Scissors-regular"),
        "annotationRegions" => Some("TextT-regular"),
        "autoCaptions" => Some("ClosedCaptioning-regular"),
        _ => None,
    }
}

/// The glyph on a lane's header: what the lane holds.
fn lane_icon(label: &str) -> &'static str {
    match label {
        "Zoom" => "MagnifyingGlassPlus-regular",
        "Clip" => "FilmStrip-regular",
        "Annotation" => "TextT-regular",
        "Caption" => "ClosedCaptioning-regular",
        _ => "MusicNotes-regular",
    }
}

/// A lane's height: the clip lane is taller, for the frames it carries.
fn lane_height(label: &str) -> f32 {
    if label == "Clip" {
        Theme::CLIP_LANE_HEIGHT
    } else {
        Theme::LANE_HEIGHT
    }
}

/// How wide a label is in Geist Mono at `size`, known before layout.
fn mono_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * Theme::MONO_ADVANCE
}

/// The playhead's soft light: `accent_glow`, spread evenly around a part.
fn glow(theme: Theme, blur: f32) -> BoxShadow {
    BoxShadow {
        color: theme.accent_glow,
        offset: point(px(0.), px(0.)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// The bubble's tail: a small downward triangle, which no div can be.
fn playhead_tail(color: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut path = PathBuilder::fill();
            path.move_to(bounds.origin);
            path.line_to(bounds.top_right());
            path.line_to(point(bounds.center().x, bounds.bottom()));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
}

/// A lane's header: a 44 circle on `sunk` with a hairline and the lane's
/// glyph, centred in a row the lane's own height so the two line up.
fn lane_header(label: &str, theme: Theme) -> Div {
    div()
        .h(px(lane_height(label)))
        .flex()
        .flex_none()
        .items_center()
        .child(
            div()
                .size(px(Theme::LANE_HEADER))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.sunk)
                .shadow(vec![hairline(theme.line, Theme::HAIRLINE_WIDTH)])
                .child(subtake_ui::icon_sized(
                    lane_icon(label),
                    Theme::LANE_HEADER_ICON,
                    theme.text,
                )),
        )
}

/// How short and how tall the console's top edge can drag the lane region:
/// down to the ruler and two lanes, and up to half the window. The top
/// is not held to the lanes the project has, so a console at rest on a short
/// project still drags taller, and room above the lanes waits for new ones.
pub(super) fn lane_stack_range(window: &Window) -> (f32, f32) {
    let share = f32::from(window.viewport_size().height) * Theme::LANE_STACK_MAX_SHARE;
    (Theme::LANE_STACK_MIN, share.max(Theme::LANE_STACK_MIN))
}

impl RootView {
    /// A strip along a float's edge that takes a drag to resize it, with a
    /// grip that shows under the pointer and while the drag runs. A double
    /// click puts the float back at its resting size.
    ///
    /// Not drawn by the design: the handoff's floats are fixed, and it draws
    /// no grip.
    pub(super) fn resize_edge(&self, edge: ResizeEdge, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let id: SharedString = match edge {
            ResizeEdge::Console => "resize-console".into(),
            ResizeEdge::Inspector => "resize-inspector".into(),
        };
        let hover = subtake_ui::motion::tween_key(&id.clone().into(), "hover");
        let dragging = matches!(
            self.gesture,
            Some(Gesture::Resize { edge: active, .. }) if active == edge
        );
        let grip = theme.muted.opacity(0.5);
        let grip = if dragging {
            grip
        } else {
            subtake_ui::motion::hover_blend(&hover, grip.opacity(0.), grip)
        };
        let across = edge == ResizeEdge::Console;
        div()
            .id(id)
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .when(across, |el| {
                el.top_0()
                    .left_0()
                    .right_0()
                    .h(px(Theme::RESIZE_HANDLE))
                    .cursor(CursorStyle::ResizeUpDown)
            })
            .when(!across, |el| {
                el.left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(Theme::RESIZE_HANDLE))
                    .cursor(CursorStyle::ResizeLeftRight)
            })
            .on_hover(subtake_ui::motion::hover_listener(hover))
            .child(
                div()
                    .rounded_full()
                    .bg(grip)
                    .when(across, |el| {
                        el.w(px(Theme::RESIZE_GRIP_LENGTH))
                            .h(px(Theme::RESIZE_GRIP_WIDTH))
                    })
                    .when(!across, |el| {
                        el.w(px(Theme::RESIZE_GRIP_WIDTH))
                            .h(px(Theme::RESIZE_GRIP_LENGTH))
                    }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, event: &MouseDownEvent, _, cx| {
                    let rest = event.click_count >= 2;
                    let (origin, start) = match edge {
                        ResizeEdge::Console => {
                            if rest {
                                s.lane_height = Theme::LANE_STACK_HEIGHT;
                            }
                            (f32::from(event.position.y), s.lane_height)
                        }
                        ResizeEdge::Inspector => {
                            if rest {
                                s.inspector_width = PANEL_WIDTH;
                            }
                            (f32::from(event.position.x), s.inspector_width)
                        }
                    };
                    if !rest {
                        s.gesture = Some(Gesture::Resize {
                            edge,
                            origin,
                            start,
                        });
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    /// Follows a drag wherever the pointer goes. The floats occlude what is
    /// behind them, so a listener on the root lost the pointer the moment it
    /// crossed onto one: a drag held only while it stayed over the stage.
    /// The window's own mouse events reach this whatever is under them.
    pub(super) fn gesture_follower(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let moved = cx.listener(Self::move_gesture);
        let released = cx.listener(Self::end_gesture);
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        moved(event, window, cx);
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        released(event, window, cx);
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    pub(super) fn seek_at(&self, x: Pixels) {
        if let Surface::Editor(e) = &self.surface {
            let b = self.timeline_bounds.get();
            let fraction = f32::from(x - b.left()) / f32::from(b.size.width).max(1.);
            e.invoke_seek(
                (e.get_timeline_offset() + fraction * e.get_timeline_visible())
                    .clamp(0., e.get_duration()),
            );
        }
    }

    pub(super) fn move_gesture(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Surface::Editor(e) = &self.surface else {
            return;
        };
        match &mut self.gesture {
            Some(Gesture::Resize {
                edge,
                origin,
                start,
            }) => match edge {
                ResizeEdge::Console => {
                    let (min, max) = lane_stack_range(window);
                    self.lane_height =
                        (*start + *origin - f32::from(event.position.y)).clamp(min, max);
                }
                ResizeEdge::Inspector => {
                    self.inspector_width = (*start + *origin - f32::from(event.position.x))
                        .clamp(PANEL_WIDTH_MIN, PANEL_WIDTH_MAX);
                }
            },
            Some(Gesture::Seek { grab }) => {
                let at = event.position.x - px(*grab);
                self.seek_at(at)
            }
            Some(Gesture::Region { origin, delta, .. }) => {
                *delta = f32::from(event.position.x - origin.x)
                    / f32::from(self.timeline_bounds.get().size.width).max(1.)
                    * e.get_timeline_visible();
            }
            Some(Gesture::Canvas { origin, dx, dy, .. }) => {
                let b = self.preview_bounds.get();
                *dx = f32::from(event.position.x - origin.x) / f32::from(b.size.width).max(1.);
                *dy = f32::from(event.position.y - origin.y) / f32::from(b.size.height).max(1.);
            }
            None => return,
        }
        cx.notify();
    }

    pub(super) fn end_gesture(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(gesture) = self.gesture.take() {
            if let Surface::Editor(e) = &self.surface {
                match gesture {
                    Gesture::Seek { grab } => self.seek_at(event.position.x - px(grab)),
                    Gesture::Region {
                        region,
                        origin,
                        mode,
                        ..
                    } => {
                        let delta = f32::from(event.position.x - origin.x)
                            / f32::from(self.timeline_bounds.get().size.width).max(1.)
                            * e.get_timeline_visible();
                        if delta.abs() > 0.00001 {
                            e.invoke_move_region(region.kind, region.id, delta, mode);
                        }
                    }
                    Gesture::Resize { .. } => {}
                    Gesture::Canvas { origin, resize, .. } => {
                        let b = self.preview_bounds.get();
                        e.invoke_canvas_edit(
                            f32::from(event.position.x - origin.x)
                                / f32::from(b.size.width).max(1.),
                            f32::from(event.position.y - origin.y)
                                / f32::from(b.size.height).max(1.),
                            resize,
                        );
                    }
                }
            }
            cx.notify();
        }
    }

    pub(super) fn timeline(
        &mut self,
        window: &EditorWindow,
        win: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let visible = window.get_timeline_visible();
        let offset = window.get_timeline_offset();
        let editor = window.clone();
        let editor_out = window.clone();
        let editor_in = window.clone();
        let (elapsed, total) = window
            .get_time_label()
            .split_once(" / ")
            .map(|(a, b)| (a.to_owned(), format!(" / {b}")))
            .unwrap_or_else(|| (window.get_time_label(), String::new()));
        let toolbar = row()
            .gap(px(Theme::GAP_LARGE))
            // The transport belongs to the console, not to the stage. It had
            // been floated over the picture on a pod of its own, which is the
            // one thing the handoff does NOT float: the pods carry the tools
            // and the framing, and the thing that moves the playhead sits on
            // the same surface as the playhead.
            .child(
                row()
                    .gap(px(Theme::GAP_LARGE))
                    .flex_none()
                    .child(
                        row()
                            .gap(px(Theme::GAP_SMALL))
                            .child(self.icon_action(
                                "previous-frame",
                                "SkipBack-fill",
                                "Previous frame",
                                "previous-frame",
                                true,
                            ))
                            .child(
                                icon_button(
                                    "play",
                                    if window.get_playing() {
                                        "Pause-fill"
                                    } else {
                                        "Play-fill"
                                    },
                                    if window.get_playing() {
                                        "Pause"
                                    } else {
                                        "Play"
                                    },
                                    theme,
                                )
                                .transport()
                                .on_click(self.command("play")),
                            )
                            .child(self.icon_action(
                                "next-frame",
                                "SkipForward-fill",
                                "Next frame",
                                "next-frame",
                                true,
                            )),
                    )
                    // Geist Mono, not Geist. A timecode counts, and
                    // proportional digits reflow as it does — every glyph
                    // beside the seconds shifted each time they ticked from 9
                    // to 10. The position is the reading and the duration is
                    // the context, so only the position is at full strength.
                    .child(
                        mono(elapsed)
                            .flex_none()
                            .text_size(px(Theme::FONT_TIMECODE))
                            .text_color(theme.text)
                            .child(
                                div()
                                    .text_size(px(Theme::FONT_TIMECODE))
                                    .text_color(theme.muted)
                                    .child(total),
                            )
                            .flex()
                            .items_baseline(),
                    ),
            )
            .child(div().flex_1())
            .child(
                row()
                    .gap(px(Theme::GAP))
                    .flex_none()
                    .child(
                        button("auto-zoom", "Suggest zooms", theme)
                            .glyph("MagicWand-regular")
                            .raised()
                            .on_click(self.command("auto-zoom")),
                    )
                    .child(
                        self.action("split-clip", "Split", "split-clip", true)
                            .glyph("Scissors-regular")
                            .raised(),
                    )
                    .child(self.menu_button("Add", cx)),
            )
            .child(div().flex_1())
            // Snap is an on/off setting, so it takes a `sunk` plate while
            // engaged, not the accent; the zoom is the stage's own control,
            // so the two zooms read alike.
            .child(
                row()
                    .gap(px(Theme::GAP_SMALL))
                    .flex_none()
                    .child(
                        icon_button("snap", "Magnet-regular", "Snap", theme)
                            .ghost()
                            .toggled(window.get_snap())
                            .on_click(move |_, _, _| editor.set_snap(!editor.get_snap())),
                    )
                    .child(
                        zoom_control("timeline-zoom", window.get_timeline_zoom(), theme)
                            .can_zoom_out(window.get_timeline_zoom() > 1.)
                            .can_zoom_in(window.get_timeline_zoom() < TIMELINE_ZOOM_MAX)
                            .can_fit(window.get_timeline_zoom() > 1.)
                            .on_zoom_out(move |_, _, _| {
                                editor_out.set_timeline_zoom(
                                    (editor_out.get_timeline_zoom() / TIMELINE_ZOOM_STEP).max(1.),
                                )
                            })
                            .on_zoom_in(move |_, _, _| {
                                editor_in.set_timeline_zoom(
                                    (editor_in.get_timeline_zoom() * TIMELINE_ZOOM_STEP)
                                        .min(TIMELINE_ZOOM_MAX),
                                )
                            })
                            .on_fit(cx.listener(|s, _, _, _| {
                                if let Surface::Editor(window) = &s.surface {
                                    window.set_timeline_zoom(1.);
                                    window.set_timeline_offset(0.);
                                }
                            })),
                    ),
            );
        let track_width = f32::from(self.timeline_bounds.get().size.width);
        let playhead_time = window.get_playhead();
        let playhead = (playhead_time - offset) / visible;
        let playhead_shown = (0. ..=1.).contains(&playhead);
        let scrubbing = matches!(self.gesture, Some(Gesture::Seek { .. }));
        // The ruler: a recessed band with a label every major interval and a
        // dot at every fifth of one between them. The major interval is the
        // smallest that keeps its labels 80 apart, so zooming in picks a
        // finer one. What the playhead has passed is at full strength and
        // what is ahead of it muted, every frame. The playhead's bubble sits
        // above the band, so no label needs hiding for it. A label that
        // would run off either end of the band is left out.
        let interval = RULER_INTERVALS
            .into_iter()
            .find(|seconds| seconds / visible * track_width >= Theme::RULER_LABEL_SPACING)
            .unwrap_or(RULER_INTERVALS[RULER_INTERVALS.len() - 1]);
        let minor = interval / Theme::RULER_MINOR_STEPS;
        let mut ruler = div()
            .id("ruler")
            .relative()
            .h(px(Theme::RULER_HEIGHT))
            .rounded_full()
            .bg(theme.sunk)
            .shadow(vec![hairline(theme.line, Theme::HAIRLINE_WIDTH)]);
        if track_width > 0. {
            let first = (offset / minor).ceil() as i64;
            let last = ((offset + visible) / minor).floor() as i64;
            for tick in first..=last {
                let seconds = tick as f32 * minor;
                let x = (seconds - offset) / visible * track_width;
                let past = seconds <= playhead_time;
                if tick % Theme::RULER_MINOR_STEPS as i64 == 0 {
                    let label = ruler_clock(seconds, false);
                    let width =
                        mono_width(&label, Theme::FONT_SMALL) + 2. * Theme::RULER_LABEL_PADDING;
                    if x - width / 2. >= 0. && x + width / 2. <= track_width {
                        ruler = ruler.child(
                            mono(label)
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(px(x - width / 2.))
                                .w(px(width))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(Theme::FONT_SMALL))
                                .text_color(if past { theme.text } else { theme.muted }),
                        );
                    }
                } else {
                    // Not drawn by the design: a dot that would touch the
                    // label beside it is left out. At the closest spacing
                    // the design allows the dot either side of a label ran
                    // into its text.
                    let steps = Theme::RULER_MINOR_STEPS as i64;
                    let near = (tick as f32 / steps as f32).round() * interval;
                    let half = mono_width(&ruler_clock(near, false), Theme::FONT_SMALL) / 2.
                        + Theme::RULER_LABEL_PADDING;
                    if ((seconds - near) / visible * track_width).abs() < half + Theme::RULER_DOT {
                        continue;
                    }
                    let dot = if past {
                        theme.text.opacity(Theme::RULER_DOT_PAST)
                    } else {
                        theme.muted.opacity(Theme::RULER_DOT_AHEAD)
                    };
                    ruler = ruler.child(
                        div()
                            .absolute()
                            .left(px(x - Theme::RULER_DOT / 2.))
                            .top(px((Theme::RULER_HEIGHT - Theme::RULER_DOT) / 2.))
                            .size(px(Theme::RULER_DOT))
                            .rounded_full()
                            .bg(dot),
                    );
                }
            }
        }
        let labels: Vec<String> = window.get_track_labels().iter().collect();
        let regions: Vec<Region> = window.get_regions().iter().collect();
        let audio_row = window.get_audio_row() as usize;
        let waveform = window.get_waveform().0;
        // The lanes drawn: every row with something on it, in order. A row
        // with nothing on it is not drawn at all, header included; adding a
        // region of its kind brings it back. Where each drawn lane starts
        // comes from here, so a lane, its header and the blocks on it cannot
        // drift apart.
        let shown: Vec<usize> = (0..labels.len())
            .filter(|&row| regions.iter().any(|r| r.row as usize == row))
            .collect();
        let mut tops = vec![None; labels.len()];
        let mut stack = 0.;
        for &row in &shown {
            tops[row] = Some(stack);
            stack += lane_height(&labels[row]) + Theme::LANE_GAP;
        }
        let stack = (stack - Theme::LANE_GAP).max(0.);
        let mut tracks = div().relative().h(px(stack));
        for region in regions.iter() {
            let Some(top) = tops.get(region.row as usize).copied().flatten() else {
                continue;
            };
            let height = lane_height(&labels[region.row as usize]);
            let take = region.is_take();
            let (mut start, mut end) = (region.start, region.end);
            if let Some(Gesture::Region {
                region: dragged,
                delta,
                mode,
                ..
            }) = &self.gesture
                && dragged.id == region.id
                && dragged.kind == region.kind
            {
                if *mode != 1 {
                    start += delta;
                }
                if *mode != 2 {
                    end += delta;
                }
            }
            // A solid fill and an ink from the lane's hue, with no edge at
            // rest. Selected, it gains an accent edge and nothing else — an
            // inset shadow rather than a border, since a border adds to
            // what the block measures and would shift its own label.
            let (fill, ink) = theme.region_tones(region.tint.to_gpui());
            let edges = if region.selected {
                vec![hairline(theme.accent, Theme::SELECTED_WIDTH)]
            } else {
                Vec::new()
            };
            let width = (f32::from(self.timeline_bounds.get().size.width) * (end - start)
                / visible)
                .max(Theme::GAP);
            let block_id: ElementId =
                SharedString::from(format!("region-{}-{}", region.kind, region.id)).into();
            let hover_key = subtake_ui::motion::tween_key(&block_id, "hover");
            let mark_key = hover_key.clone();
            let hover_fill = subtake_ui::motion::blend(fill, ink, Theme::REGION_HOVER_INK);
            let ring = subtake_ui::focus_ring(theme);
            let mut block = div()
                .id(block_id)
                .absolute()
                .left(relative((start - offset) / visible))
                .top(px(top))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(Theme::GAP))
                .h(px(height))
                .rounded(px(Theme::RADIUS_REGION))
                .overflow_hidden()
                .bg(subtake_ui::motion::hover_blend(
                    &hover_key, fill, hover_fill,
                ))
                .on_hover(subtake_ui::motion::hover_listener(hover_key))
                .shadow(edges.clone())
                // Held, a region dims as every pressed control does, and
                // stays dimmed while it is dragged. Tab reaches it, and Enter
                // or Space selects it, which is what a click does.
                .when(!take, |el| {
                    el.active(|s| s.opacity(Theme::PRESSED_OPACITY))
                        .tab_index(0)
                        .focus_visible(move |s| {
                            s.shadow(edges.iter().cloned().chain([ring]).collect())
                        })
                        .cursor(CursorStyle::ClosedHand)
                })
                .child({
                    // Below the label width a region shows only its kind's
                    // icon, centred; at or above it the icon, if the kind
                    // has one, then the label. A label that would get fewer
                    // than four letters is left off rather than cut to a
                    // fragment, and the tooltip names the region either way.
                    let icon = region_icon(&region.kind);
                    let room = width
                        - 2. * Theme::REGION_PADDING
                        - icon.map_or(0., |_| Theme::REGION_ICON_SIZE + Theme::REGION_ICON_GAP);
                    let labelled = width >= Theme::REGION_LABEL_MIN_WIDTH
                        && room >= Theme::REGION_LABEL_MIN_ROOM;
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .when(!labelled, |el| el.justify_center())
                        .gap(px(Theme::REGION_ICON_GAP))
                        .when(labelled, |el| el.px(px(Theme::REGION_PADDING)))
                        .text_size(px(Theme::FONT_SMALL))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ink)
                        .overflow_hidden()
                        .when_some(icon, |el, name| {
                            el.child(
                                subtake_ui::icon_sized(name, Theme::REGION_ICON_SIZE, ink)
                                    .flex_none(),
                            )
                        })
                        // On one line, in a box that may shrink below it: a
                        // flex row gives bare text its full width, and a
                        // short region cut its label off mid-letter instead.
                        .when(labelled, |el| {
                            el.child(
                                div()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(region.label.clone()),
                            )
                        })
                });
            // The tooltip names the region and gives its range, for one
            // shown as an icon or with its label cut short. Not drawn by the
            // design: it shows on every region, since whether a label is cut
            // is only known after layout, and it sits under the pointer
            // rather than 8 above the region, centred, as gpui places a
            // tooltip.
            if !region.label.is_empty() {
                let label = region.label.clone();
                let range = format!(
                    "{}\u{2013}{}",
                    ruler_clock(region.start, false),
                    ruler_clock(region.end, false)
                );
                block = block.tooltip(move |_, cx| {
                    subtake_ui::tooltip_detail(label.clone(), range.clone(), theme, cx)
                });
            }
            // The take's own clip and sound take no selection and no drag:
            // a press on one seeks, as a press on the bare lane does.
            if take {
                tracks = tracks.child(block);
                continue;
            }
            let key_region = region.clone();
            block = block.on_click(cx.listener(move |s, event: &ClickEvent, _, cx| {
                if !event.is_keyboard() {
                    return;
                }
                if let Surface::Editor(window) = &s.surface {
                    window.invoke_select_region(
                        key_region.kind.clone(),
                        key_region.id.clone(),
                        false,
                    );
                }
                cx.notify();
            }));
            let drag_region = region.clone();
            block = block.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    if let Surface::Editor(window) = &s.surface {
                        window.invoke_select_region(
                            drag_region.kind.clone(),
                            drag_region.id.clone(),
                            event.modifiers.shift,
                        );
                    }
                    s.gesture = Some(Gesture::Region {
                        region: drag_region.clone(),
                        origin: event.position,
                        mode: 0,
                        delta: 0.,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
            for (mode, right) in [(2, false), (1, true)] {
                let drag_region = region.clone();
                // The grab area is wide; the mark inside it is 3px. The mark
                // is only drawn when the region is selected or under the
                // pointer — a timeline of twenty regions showing forty
                // handles is a texture, not a set of controls.
                let selected = region.selected;
                let mut handle = div()
                    .id(("resize", mode as usize))
                    .absolute()
                    .top_0()
                    .w(px(Theme::REGION_HANDLE_TARGET))
                    .h_full()
                    .group("region-handle")
                    .cursor(CursorStyle::ResizeLeftRight)
                    .child(
                        div()
                            .id("mark")
                            .absolute()
                            .left(px(Theme::REGION_HANDLE_INSET))
                            .top(px(Theme::REGION_HANDLE_MARGIN))
                            .w(px(Theme::REGION_HANDLE_WIDTH))
                            .h(px(height - Theme::REGION_HANDLE_MARGIN * 2.0))
                            .rounded(px(Theme::REGION_HANDLE_RADIUS))
                            .bg(subtake_ui::motion::hover_blend(
                                &mark_key,
                                ink.opacity(if selected {
                                    Theme::REGION_HANDLE_ALPHA
                                } else {
                                    0.
                                }),
                                ink.opacity(Theme::REGION_HANDLE_ALPHA),
                            ))
                            // Not drawn by the design: a held handle's mark
                            // goes to the full ink, so the grab reads before
                            // the edge has moved.
                            .group_active("region-handle", move |s| s.bg(ink)),
                    );
                handle = if right {
                    handle.right_0()
                } else {
                    handle.left_0()
                };
                block = block.child(handle.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, event: &MouseDownEvent, w, cx| {
                        w.focus(&s.focus, cx);
                        if let Surface::Editor(window) = &s.surface {
                            window.invoke_select_region(
                                drag_region.kind.clone(),
                                drag_region.id.clone(),
                                event.modifiers.shift,
                            );
                        }
                        s.gesture = Some(Gesture::Region {
                            region: drag_region.clone(),
                            origin: event.position,
                            mode,
                            delta: 0.,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ));
            }
            tracks = tracks.child(block);
        }
        // The waveform over the audio lane, drawn after the regions so the
        // opaque region under it does not hide it, 6 in from the lane's top
        // and bottom at `55`. Not wired: the waveform in the region's ink.
        // It is an image ffmpeg paints in one colour, and gpui cannot tint
        // an image, so it keeps that colour at the review's strength.
        if let Some(image) = waveform
            && let Some(top) = tops.get(audio_row).copied().flatten()
        {
            tracks = tracks.child(
                img(image)
                    .absolute()
                    .top(px(top + Theme::WAVEFORM_INSET))
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(Theme::LANE_HEIGHT - Theme::WAVEFORM_INSET * 2.))
                    .opacity(Theme::WAVEFORM_ALPHA)
                    .object_fit(ObjectFit::Fill),
            );
        }
        let mut timeline = column()
            .id("timeline-content")
            .relative()
            .flex_1()
            .min_w_0()
            .pt(px(Theme::BUBBLE_ZONE))
            .gap(px(Theme::RULER_GAP))
            // Cut at the ends of the take, not above it, so the bubble's
            // glow is not sheared off at the band's top.
            .overflow_x_hidden()
            .child(measure(self.timeline_bounds.clone()))
            .child(ruler)
            .child(tracks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    s.gesture = Some(Gesture::Seek { grab: 0. });
                    s.seek_at(event.position.x);
                    cx.stop_propagation();
                }),
            );
        if playhead_shown {
            let x = playhead * track_width;
            // Where the grab handle rides: centred on the clip lane, or on
            // the first lane when there is no clip lane. Not drawn by the
            // design, which always has one.
            let handle_lane = shown
                .iter()
                .copied()
                .find(|&row| labels[row] == "Clip")
                .or(shown.first().copied());
            let handle_top = handle_lane.map(|row| {
                Theme::LANE_STACK_TOP
                    + tops[row].unwrap_or(0.)
                    + (lane_height(&labels[row]) - Theme::PLAYHEAD_HANDLE_HEIGHT) / 2.
            });
            let chip_label = ruler_clock(playhead_time, true);
            let bubble_width = mono_width(&chip_label, Theme::FONT_SECONDARY)
                + 2. * Theme::PLAYHEAD_BUBBLE_PADDING;
            let bubble_left =
                (x - bubble_width / 2.).clamp(0., (track_width - bubble_width).max(0.));
            let handle_width = if scrubbing {
                Theme::PLAYHEAD_HANDLE_WIDTH_HELD
            } else {
                Theme::PLAYHEAD_HANDLE_WIDTH
            };
            // A press on any part of the playhead picks it up where it is.
            let grab = || {
                cx.listener(move |s: &mut Self, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    let b = s.timeline_bounds.get();
                    let at = f32::from(b.left()) + x;
                    s.gesture = Some(Gesture::Seek {
                        grab: f32::from(event.position.x) - at,
                    });
                    cx.stop_propagation();
                    cx.notify();
                })
            };
            // Bottom to top: the line over the regions, the dot on the
            // ruler's top edge, the bubble with its tail, and the handle.
            // The line keeps a 12-wide hit area that scrubs and shows the
            // resize cursor, since 1.5 is not a target.
            timeline = timeline
                .child(
                    div()
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_LINE_WIDTH / 2.))
                        .top(px(Theme::BUBBLE_ZONE))
                        .bottom_0()
                        .w(px(Theme::PLAYHEAD_LINE_WIDTH))
                        .rounded(px(Theme::PLAYHEAD_LINE_RADIUS))
                        .bg(theme.accent)
                        .shadow(vec![glow(theme, Theme::PLAYHEAD_LINE_GLOW)]),
                )
                .child(
                    div()
                        .id("playhead-line")
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_HIT))
                        .top(px(Theme::BUBBLE_ZONE))
                        .bottom_0()
                        .w(px(Theme::PLAYHEAD_HIT * 2.))
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .child(
                    div()
                        .id("playhead-dot")
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_DOT / 2.))
                        .top(px(Theme::BUBBLE_ZONE - Theme::PLAYHEAD_DOT / 2.))
                        .size(px(Theme::PLAYHEAD_DOT))
                        .rounded_full()
                        .bg(theme.accent)
                        .shadow(vec![BoxShadow {
                            color: theme.accent_soft,
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: px(Theme::PLAYHEAD_DOT_RING),
                            inset: false,
                        }])
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .child(
                    playhead_tail(theme.accent)
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_TAIL_WIDTH / 2.))
                        .top(px(Theme::PLAYHEAD_BUBBLE_HEIGHT))
                        .w(px(Theme::PLAYHEAD_TAIL_WIDTH))
                        .h(px(Theme::PLAYHEAD_TAIL_HEIGHT)),
                )
                // Held inside the track at either end rather than cut off by
                // it; the tail stays on the true time.
                .child(
                    mono(chip_label)
                        .id("playhead-bubble")
                        .absolute()
                        .top_0()
                        .left(px(bubble_left))
                        .w(px(bubble_width))
                        .h(px(Theme::PLAYHEAD_BUBBLE_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(theme.accent)
                        .shadow(vec![glow(
                            theme,
                            if scrubbing {
                                Theme::PLAYHEAD_BUBBLE_GLOW_HELD
                            } else {
                                Theme::PLAYHEAD_BUBBLE_GLOW
                            },
                        )])
                        .text_color(theme.on_accent)
                        .text_size(px(Theme::FONT_SECONDARY))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .when_some(handle_top, |el, top| {
                    el.child(
                        div()
                            .id("playhead-handle")
                            .absolute()
                            .left(px(x - handle_width / 2.))
                            .top(px(top))
                            .w(px(handle_width))
                            .h(px(Theme::PLAYHEAD_HANDLE_HEIGHT))
                            .rounded(px(Theme::PLAYHEAD_HANDLE_RADIUS))
                            .bg(theme.accent)
                            .shadow(vec![glow(
                                theme,
                                if scrubbing {
                                    Theme::PLAYHEAD_HANDLE_GLOW_HELD
                                } else {
                                    Theme::PLAYHEAD_HANDLE_GLOW
                                },
                            )])
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(Theme::PLAYHEAD_GRIP_GAP))
                            .cursor(CursorStyle::ResizeLeftRight)
                            .children((0..3).map(|_| {
                                div()
                                    .size(px(Theme::PLAYHEAD_GRIP_DOT))
                                    .rounded_full()
                                    .bg(theme.on_accent)
                            }))
                            .on_mouse_down(MouseButton::Left, grab()),
                    )
                });
        }
        let console = panel(theme)
            .id("timeline")
            .relative()
            // A zoomed picture runs on under the console; the console's
            // presses are its own.
            .occlude()
            .mx(px(Theme::INSET))
            .mb(px(Theme::INSET))
            .flex_shrink_0()
            .child(toolbar)
            // The lanes and the export/transcription line share one box, so
            // the line folds away without leaving the console's gap behind.
            // It sits inside the console rather than under it so the console
            // keeps the shell's own inset on all three of its edges.
            .child(
                column().gap_0().flex_none().child(
                    fade_edges(
                        row()
                            .id("track-scroll")
                            .gap(px(Theme::LANE_HEADER_GAP))
                            .items_start()
                            .h(px({
                                let (min, max) = lane_stack_range(win);
                                self.lane_height.clamp(min, max)
                            }))
                            .flex_none()
                            .overflow_y_scroll()
                            .track_scroll(&self.lane_scroll)
                            // The headers run on the track column's own grid,
                            // row for row: down past the bubble's band, the
                            // ruler and the air under it, then one row per
                            // lane at that lane's height with the same gap.
                            // A lane that overflows onto a second row shares
                            // the first row's header. Not drawn by the
                            // design, which gives every lane one row.
                            .child(
                                column()
                                    .w(px(Theme::LANE_HEADER))
                                    .flex_shrink_0()
                                    .gap(px(Theme::LANE_GAP))
                                    .pt(px(Theme::LANE_STACK_TOP))
                                    .children(shown.iter().map(|&row| {
                                        let label = &labels[row];
                                        let first = row == 0 || labels[row - 1] != *label;
                                        if first {
                                            lane_header(label, theme)
                                        } else {
                                            div().h(px(lane_height(label)))
                                        }
                                    })),
                            )
                            .child(timeline),
                    )
                    // The top fades by what is scrolled past it, so a
                    // stack that fits ends at its last lane rather than
                    // on a band of air kept for the fade to rest on. The
                    // bottom cuts hard: the console's own edge already
                    // closes it.
                    .tracking(&self.lane_scroll)
                    .bottom(false),
                ),
            )
            // Its top edge takes a drag, trading lane height for stage.
            .child(self.resize_edge(ResizeEdge::Console, cx))
            .on_scroll_wheel(cx.listener(|s, event: &ScrollWheelEvent, _, cx| {
                if let Surface::Editor(window) = &s.surface {
                    let delta = event.delta.pixel_delta(px(20.));
                    if event.modifiers.control || event.modifiers.platform {
                        let b = s.timeline_bounds.get();
                        let fraction = (f32::from(event.position.x - b.left())
                            / f32::from(b.size.width).max(1.))
                        .clamp(0., 1.);
                        let anchor =
                            window.get_timeline_offset() + fraction * window.get_timeline_visible();
                        window.set_timeline_zoom(
                            (window.get_timeline_zoom() * (-f32::from(delta.y) * 0.01).exp())
                                .clamp(1., TIMELINE_ZOOM_MAX),
                        );
                        window.set_timeline_offset(
                            (anchor - fraction * window.get_timeline_visible()).clamp(
                                0.,
                                (window.get_duration() - window.get_timeline_visible()).max(0.),
                            ),
                        );
                        cx.stop_propagation();
                    } else if delta.x != px(0.) || event.modifiers.shift {
                        let dx = if event.modifiers.shift {
                            delta.y
                        } else {
                            delta.x
                        };
                        window.set_timeline_offset(
                            (window.get_timeline_offset()
                                - f32::from(dx)
                                    / f32::from(s.timeline_bounds.get().size.width).max(1.)
                                    * window.get_timeline_visible())
                            .clamp(
                                0.,
                                (window.get_duration() - window.get_timeline_visible()).max(0.),
                            ),
                        );
                        cx.stop_propagation();
                    }
                }
            }))
            .into_any_element();
        frosted(UiSurface::Panel.radius(), UiSurface::Panel.blur(), console).into_any_element()
    }
}
