//! The timeline: tracks, regions, the scrubber and its drag gestures.

use super::*;

/// How far the console's zoom out and in step, and the most the timeline
/// magnifies: a hundredth of the take across the lanes.
const TIMELINE_ZOOM_STEP: f32 = 1.5;
const TIMELINE_ZOOM_MAX: f32 = 100.;

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
        // The ruler: eight ticks, which is the redesign's every-12.5%, set in
        // Geist Mono so a tick's width does not change with its digits.
        let mut ruler = div().relative().h(px(Theme::RULER_HEIGHT));
        for i in 0..8 {
            ruler = ruler.child(
                mono_small(format!("{:.1}s", offset + visible * i as f32 / 8.), theme)
                    .absolute()
                    .left(relative(i as f32 / 8.)),
            );
        }
        // The source lane: the recording's frames, on the same 36 as every
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
        // One pitch per lane: the lane itself plus the air under it. Every
        // `top` below is a multiple of it, so a lane, its waveform and the
        // blocks on it cannot drift apart.
        let mut tracks = div()
            .relative()
            .h(px(labels.len() as f32 * Theme::LANE_PITCH));
        for i in 0..labels.len() {
            tracks = tracks.child(
                div()
                    .absolute()
                    .top(px(i as f32 * Theme::LANE_PITCH))
                    .w_full()
                    .h(px(Theme::LANE_HEIGHT))
                    .rounded(px(Theme::RADIUS_REGION))
                    .bg(theme.sunk),
            );
        }
        if let Some(image) = window.get_waveform().0 {
            tracks = tracks.child(
                img(image)
                    .absolute()
                    .top(px(window.get_audio_row() as f32 * Theme::LANE_PITCH))
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(Theme::LANE_HEIGHT))
                    .opacity(0.3)
                    .object_fit(ObjectFit::Fill),
            );
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
            let tint = region.tint.to_gpui();
            // Fill and edge are the lane's own tint at four strengths. The
            // edge is an inset shadow rather than a border: a border adds to
            // what the block measures, so a region would have grown by two
            // pixels the moment it was selected and shifted its own label.
            let fill = tint.opacity(if region.selected {
                Theme::REGION_FILL_SELECTED
            } else {
                Theme::REGION_FILL
            });
            let edge = if region.selected {
                tint
            } else {
                tint.opacity(Theme::REGION_EDGE)
            };
            let width = (f32::from(self.timeline_bounds.get().size.width) * (end - start)
                / visible)
                .max(Theme::GAP);
            let block_id: ElementId =
                SharedString::from(format!("region-{}-{}", region.kind, region.id)).into();
            let hover_key = subtake_ui::motion::tween_key(&block_id, "hover");
            let mark_key = hover_key.clone();
            let hover_fill = tint.opacity(if region.selected {
                Theme::REGION_FILL_SELECTED_HOVER
            } else {
                Theme::REGION_FILL_HOVER
            });
            let ring = subtake_ui::focus_ring(theme);
            let mut block = div()
                .id(block_id)
                .absolute()
                .left(relative((start - offset) / visible))
                .top(px(region.row as f32 * Theme::LANE_PITCH))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(Theme::GAP))
                .h(px(Theme::LANE_HEIGHT))
                .rounded(px(Theme::RADIUS_REGION))
                .overflow_hidden()
                .bg(subtake_ui::motion::hover_blend(
                    &hover_key, fill, hover_fill,
                ))
                .on_hover(subtake_ui::motion::hover_listener(hover_key))
                .shadow(vec![hairline(edge, Theme::BORDER_WIDTH)])
                // Held, a region dims as every pressed control does, and
                // stays dimmed while it is dragged. Tab reaches it, and Enter
                // or Space selects it, which is what a click does.
                .active(|s| s.opacity(Theme::PRESSED_OPACITY))
                .tab_index(0)
                .focus_visible(move |s| s.shadow(vec![hairline(edge, Theme::BORDER_WIDTH), ring]))
                .cursor(CursorStyle::ClosedHand)
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .px(px(Theme::REGION_PADDING))
                        .text_size(px(Theme::FONT_SMALL))
                        .text_color(theme.text)
                        .overflow_hidden()
                        // On one line, in a box that may shrink below it: a
                        // flex row gives bare text its full width, and a
                        // short region cut its label off mid-letter instead.
                        .when(width >= Theme::REGION_LABEL_MIN_WIDTH, |el| {
                            el.child(
                                div()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(region.label.clone()),
                            )
                        }),
                );
            // Not drawn by the design: the tooltip, which names a region
            // whose label is cut short or left off.
            if !region.label.is_empty() {
                let label = region.label.clone();
                block = block.tooltip(move |_, cx| tooltip(label.clone(), theme, cx));
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
                                tint.opacity(if selected {
                                    Theme::REGION_HANDLE_ALPHA
                                } else {
                                    0.
                                }),
                                tint.opacity(Theme::REGION_HANDLE_ALPHA),
                            ))
                            // Not drawn by the design: a held handle's mark
                            // goes to the full tint, so the grab reads before
                            // the edge has moved.
                            .group_active("region-handle", move |s| s.bg(tint)),
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
        let playhead = (window.get_playhead() - offset) / visible;
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
        if (0. ..=1.).contains(&playhead) {
            // A 2px accent rule the full height of the stack, with a dot at
            // its head. The dot carries a soft accent ring so it stays
            // legible where it crosses a region painted in its lane's tint.
            let dot = Theme::PLAYHEAD_DOT;
            timeline = timeline.child(
                div()
                    .absolute()
                    .left(relative(playhead))
                    .ml(px(-Theme::PLAYHEAD_WIDTH / 2.0))
                    .top_0()
                    .bottom_0()
                    .w(px(Theme::PLAYHEAD_WIDTH))
                    .bg(theme.accent)
                    .child(
                        div()
                            .absolute()
                            .top(px(-dot / 2.0))
                            .left(px((Theme::PLAYHEAD_WIDTH - dot) / 2.0))
                            .size(px(dot))
                            .rounded_full()
                            .bg(theme.accent)
                            .shadow(vec![BoxShadow {
                                color: theme.accent_soft,
                                offset: point(px(0.), px(0.)),
                                blur_radius: px(0.),
                                spread_radius: px(Theme::PLAYHEAD_RING),
                                inset: false,
                            }]),
                    ),
            );
        }
        let console =
            panel(theme)
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
                                        .children(labels.into_iter().map(|label| {
                                            lane_label(label, Theme::LANE_HEIGHT, theme)
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
                            let anchor = window.get_timeline_offset()
                                + fraction * window.get_timeline_visible();
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
