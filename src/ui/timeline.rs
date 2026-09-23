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

/// Which part of an empty lane the pointer is over: its name or its track.
const LANE_HOVER_LABEL: u8 = 1;
const LANE_HOVER_TRACK: u8 = 2;

/// What a click on an opened empty lane adds, and the region it names. The
/// clip lane adds a trim, since a clip comes only from splitting the take.
fn lane_add(label: &str) -> (&'static str, &'static str) {
    match label {
        "Zoom" => ("add-zoom", "a zoom"),
        "Clip" => ("add-trim", "a trim"),
        "Annotation" => ("add-text", "an annotation"),
        "Audio" => ("add-audio", "an audio"),
        _ => ("add-caption", "a caption"),
    }
}

/// How wide a label is in the small mono face, known before layout.
fn mono_small_width(text: &str) -> f32 {
    text.chars().count() as f32 * Theme::FONT_SMALL * Theme::MONO_ADVANCE
}

/// One name in the lane gutter, on the lane's own grid: the lane's height,
/// the label centred in it, and the gap that follows every lane below it.
fn lane_label(text: impl Into<SharedString>, height: f32, theme: Theme) -> Div {
    div()
        .h(px(height))
        .mb(px(Theme::LANE_GAP))
        .flex()
        .items_center()
        .flex_shrink_0()
        .text_size(px(Theme::FONT_SECONDARY))
        .text_color(theme.muted)
        .child(text.into())
}

/// How short and how tall the console's top edge can drag the lane region:
/// down to the source lane and one more, and up to half the window. The top
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
            Some(Gesture::Seek) => self.seek_at(event.position.x),
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
                    Gesture::Seek => self.seek_at(event.position.x),
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

    /// The pointer entering or leaving an empty lane's name or its track.
    /// The lane stays open while either holds it, and folds back a moment
    /// after both have let go.
    fn hover_lane(&mut self, lane: usize, part: u8, hovered: bool, cx: &mut Context<Self>) {
        let held = match self.lane_open {
            Some((open, held, _)) if open == lane => held,
            _ => 0,
        };
        let held = if hovered { held | part } else { held & !part };
        if hovered {
            self.lane_open = Some((lane, held, None));
        } else if matches!(self.lane_open, Some((open, _, _)) if open == lane) {
            self.lane_open = Some((lane, held, (held == 0).then(Instant::now)));
        }
        cx.notify();
    }

    /// A lane with nothing on it: a 16-tall outlined strip that opens to the
    /// full lane under the pointer, with a line saying what a click adds. A
    /// click on the opened lane adds a region of its kind at the playhead.
    fn empty_lane(
        &self,
        lane: Div,
        index: usize,
        label: &str,
        open: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let (action, noun) = lane_add(label);
        let surface = self.surface.clone();
        let lerp = subtake_ui::motion::lerp;
        lane.id(("lane-empty", index))
            .flex()
            .items_center()
            .px(px(Theme::LANE_PLACEHOLDER_PADDING))
            .overflow_hidden()
            .rounded(px(lerp(Theme::RADIUS_LANE_EMPTY, Theme::RADIUS_LANE, open)))
            .bg(subtake_ui::motion::blend(
                theme.hover.opacity(0.),
                theme.hover,
                open,
            ))
            .shadow(vec![hairline(theme.line, Theme::HAIRLINE_WIDTH * 2.)])
            .cursor(CursorStyle::PointingHand)
            .on_hover(cx.listener(move |s, hovered, _, cx| {
                s.hover_lane(index, LANE_HOVER_TRACK, *hovered, cx)
            }))
            .when(open > 0., |el| {
                el.child(
                    div()
                        .whitespace_nowrap()
                        .opacity(open)
                        .text_size(px(Theme::FONT_SMALL))
                        .text_color(theme.muted)
                        .child(format!("Click or drag to add {noun} region")),
                )
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if open >= 1. {
                    surface.action(action);
                    cx.stop_propagation();
                }
            })
            .into_any_element()
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
        // The playhead's chip, placed here because the ruler hides any
        // label it would crowd. Centred on the line, and held inside the
        // track at either end rather than cut off by it.
        let track_width = f32::from(self.timeline_bounds.get().size.width);
        let playhead = (window.get_playhead() - offset) / visible;
        let playhead_shown = (0. ..=1.).contains(&playhead);
        let chip_label = ruler_clock(window.get_playhead(), true);
        let chip_width = mono_small_width(&chip_label) + Theme::PLAYHEAD_CHIP_PADDING * 2.;
        let chip_left = (playhead * track_width - chip_width / 2.)
            .clamp(0., (track_width - chip_width).max(0.));
        // The ruler, in `m:ss` as the transport counts, at the smallest
        // interval that keeps its labels apart; zooming in picks a finer
        // one. A label that would come within a hair of the chip is left
        // out until the playhead moves on — hidden, not faded, so it never
        // shows half-covered.
        let interval = RULER_INTERVALS
            .into_iter()
            .find(|seconds| seconds / visible * track_width >= Theme::RULER_LABEL_SPACING)
            .unwrap_or(RULER_INTERVALS[RULER_INTERVALS.len() - 1]);
        let mut ruler = div().relative().h(px(Theme::RULER_HEIGHT));
        if track_width > 0. {
            let first = (offset / interval).ceil() as i64;
            let last = ((offset + visible) / interval).floor() as i64;
            for tick in first..=last {
                let seconds = tick as f32 * interval;
                let label = ruler_clock(seconds, false);
                let left = (seconds - offset) / visible * track_width;
                let right = left + mono_small_width(&label);
                let crowded = playhead_shown
                    && left - Theme::RULER_CHIP_CLEARANCE < chip_left + chip_width
                    && right + Theme::RULER_CHIP_CLEARANCE > chip_left;
                if right <= track_width && !crowded {
                    ruler = ruler.child(mono_small(label, theme).absolute().left(px(left)));
                }
            }
        }
        // The source lane: the recording's frames, on the same 30 as every
        // other lane. It carried the document's title over a shorter strip,
        // which the titlebar already names, and it stood taller than the
        // lanes for it.
        let mut source = div()
            .relative()
            .h(px(Theme::LANE_HEIGHT))
            .overflow_hidden()
            .rounded(px(Theme::RADIUS_REGION))
            .bg(theme.sunk);
        if let Some(image) = window.get_thumbnails().0 {
            source = source.child(
                img(image)
                    .absolute()
                    .top_0()
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(Theme::LANE_HEIGHT))
                    .object_fit(ObjectFit::Fill),
            );
        }
        let labels: Vec<String> = window.get_track_labels().iter().collect();
        let regions: Vec<Region> = window.get_regions().iter().collect();
        let audio_row = window.get_audio_row() as usize;
        let waveform = window.get_waveform().0;
        // An empty lane folds away after the pointer has been gone from it a
        // moment; until then frames keep coming so the fold is on time.
        if let Some((_, _, Some(left))) = self.lane_open {
            if left.elapsed() >= std::time::Duration::from_millis(Theme::LANE_COLLAPSE_DELAY_MS) {
                self.lane_open = None;
            } else {
                win.request_animation_frame();
            }
        }
        // Each lane's height and how far open it is: a lane with anything on
        // it is always the full 30, and an empty one folds to a 16 strip,
        // opening to 30 under the pointer. The audio lane counts as holding
        // the recording's waveform when there is one. Not drawn by the
        // design: the waveform in an otherwise empty audio lane.
        let lanes: Vec<(bool, f32)> = (0..labels.len())
            .map(|i| {
                let occupied = regions.iter().any(|r| r.row as usize == i)
                    || (i == audio_row && waveform.is_some());
                if occupied {
                    return (true, 1.);
                }
                let key = subtake_ui::motion::tween_key(
                    &SharedString::from(format!("lane-{}-{i}", labels[i])).into(),
                    "open",
                );
                let open = matches!(self.lane_open, Some((j, _, _)) if j == i)
                    || subtake_ui::motion::hover_pinned(&key);
                (false, subtake_ui::motion::state_fade(&key, open))
            })
            .collect();
        let height =
            |t: f32| subtake_ui::motion::lerp(Theme::LANE_EMPTY_HEIGHT, Theme::LANE_HEIGHT, t);
        // Where each lane starts: the lanes above it and the air under each.
        // Every `top` below comes from here, so a lane, its waveform and the
        // blocks on it cannot drift apart.
        let tops: Vec<f32> = lanes
            .iter()
            .scan(0., |top, &(_, t)| {
                let this = *top;
                *top += height(t) + Theme::LANE_GAP;
                Some(this)
            })
            .collect();
        let stack: f32 = lanes
            .iter()
            .map(|&(_, t)| height(t) + Theme::LANE_GAP)
            .sum();
        let mut tracks = div().relative().h(px(stack));
        for (i, &(occupied, t)) in lanes.iter().enumerate() {
            let lane = div().absolute().top(px(tops[i])).w_full().h(px(height(t)));
            tracks = tracks.child(if occupied {
                lane.rounded(px(Theme::RADIUS_LANE))
                    .bg(theme.lane_track)
                    .into_any_element()
            } else {
                self.empty_lane(lane, i, &labels[i], t, cx)
            });
        }
        for region in window.get_regions().iter() {
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
                .top(px(tops.get(region.row as usize).copied().unwrap_or(0.)))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(Theme::GAP))
                .h(px(Theme::LANE_HEIGHT))
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
                .active(|s| s.opacity(Theme::PRESSED_OPACITY))
                .tab_index(0)
                .focus_visible(move |s| s.shadow(edges.iter().cloned().chain([ring]).collect()))
                .cursor(CursorStyle::ClosedHand)
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
                            .h(px(Theme::LANE_HEIGHT - Theme::REGION_HANDLE_MARGIN * 2.0))
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
        if let Some(image) = waveform {
            tracks = tracks.child(
                img(image)
                    .absolute()
                    .top(px(
                        tops.get(audio_row).copied().unwrap_or(0.) + Theme::WAVEFORM_INSET
                    ))
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
            .gap_1()
            .overflow_hidden()
            .child(measure(self.timeline_bounds.clone()))
            .child(ruler)
            .child(source)
            .child(tracks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    s.gesture = Some(Gesture::Seek);
                    s.seek_at(event.position.x);
                    cx.stop_propagation();
                }),
            );
        if playhead_shown {
            // A 2px accent rule the full height of the stack, and its time
            // on an accent chip over the ruler. The chip replaced a round
            // head, which sat on the ruler label under it.
            //
            // Not drawn by the design: the chip at the stack's top edge
            // rather than 2 above it. The stack clips at its top, and 20 tall
            // from there it fills the ruler and the gap under it exactly.
            timeline = timeline
                .child(
                    div()
                        .absolute()
                        .left(relative(playhead))
                        .ml(px(-Theme::PLAYHEAD_WIDTH / 2.0))
                        .top_0()
                        .bottom_0()
                        .w(px(Theme::PLAYHEAD_WIDTH))
                        .rounded_full()
                        .bg(theme.accent),
                )
                .child(
                    mono(chip_label)
                        .absolute()
                        .top_0()
                        .left(px(chip_left))
                        .flex()
                        .items_center()
                        .h(px(Theme::PLAYHEAD_CHIP_HEIGHT))
                        .px(px(Theme::PLAYHEAD_CHIP_PADDING))
                        .rounded_full()
                        .bg(theme.accent)
                        .text_color(theme.on_accent)
                        .text_size(px(Theme::FONT_SMALL))
                        .font_weight(FontWeight::MEDIUM),
                );
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
                            .gap(px(Theme::LANE_GUTTER_GAP))
                            .items_start()
                            .h(px({
                                let (min, max) = lane_stack_range(win);
                                self.lane_height.clamp(min, max)
                            }))
                            .flex_none()
                            .overflow_y_scroll()
                            .track_scroll(&self.lane_scroll)
                            .child(
                                column()
                                    .w(px(Theme::LANE_GUTTER))
                                    .flex_shrink_0()
                                    .gap_0()
                                    // The gutter runs on the track column's own grid,
                                    // row for row: a spacer the height of the ruler
                                    // and the gap under it, then one box per lane at
                                    // that lane's height with the same gap below. A
                                    // label is centred on its lane rather than set at
                                    // its top, so the name and the blocks it names
                                    // read as one line.
                                    .child(div().h(px(Theme::RULER_HEIGHT + Theme::LANE_GAP)))
                                    .child(lane_label("Source", Theme::LANE_HEIGHT, theme))
                                    .children(labels.iter().enumerate().map(|(i, label)| {
                                        let (occupied, t) = lanes[i];
                                        let label = lane_label(label.clone(), height(t), theme);
                                        if occupied {
                                            return label.into_any_element();
                                        }
                                        // An empty lane's name is a step smaller, and
                                        // opens the lane as its track does.
                                        label
                                            .id(("lane-label", i))
                                            .when(t < 0.5, |el| el.text_size(px(Theme::FONT_SMALL)))
                                            .on_hover(cx.listener(move |s, hovered, _, cx| {
                                                s.hover_lane(i, LANE_HOVER_LABEL, *hovered, cx)
                                            }))
                                            .into_any_element()
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
