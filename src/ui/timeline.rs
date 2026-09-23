//! The timeline: tracks, regions, the scrubber and its drag gestures.

use super::*;

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

impl RootView {
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Surface::Editor(e) = &self.surface else {
            return;
        };
        match &mut self.gesture {
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
        status: Option<AnyElement>,
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
            // Snap keeps the accent plate while engaged; the zoom cluster is
            // icon-only so the strip stays quiet.
            .child(
                row()
                    .gap(px(Theme::GAP_SMALL))
                    .flex_none()
                    .child(
                        icon_button("snap", "Magnet-regular", "Snap", theme)
                            .ghost()
                            .selected(window.get_snap())
                            .on_click(move |_, _, _| editor.set_snap(!editor.get_snap())),
                    )
                    .child(
                        icon_button(
                            "fit-timeline",
                            "ArrowsOutSimple-regular",
                            "Fit timeline",
                            theme,
                        )
                        .ghost()
                        .on_click(cx.listener(|s, _, _, _| {
                            if let Surface::Editor(window) = &s.surface {
                                window.set_timeline_zoom(1.);
                                window.set_timeline_offset(0.);
                            }
                        })),
                    )
                    .child(
                        icon_button(
                            "zoom-out",
                            "MagnifyingGlassMinus-regular",
                            "Zoom out",
                            theme,
                        )
                        .ghost()
                        .on_click(move |_, _, _| {
                            editor_out
                                .set_timeline_zoom((editor_out.get_timeline_zoom() / 1.5).max(1.))
                        }),
                    )
                    .child(
                        icon_button("zoom-in", "MagnifyingGlassPlus-regular", "Zoom in", theme)
                            .ghost()
                            .on_click(move |_, _, _| {
                                editor_in.set_timeline_zoom(
                                    (editor_in.get_timeline_zoom() * 1.5).min(100.),
                                )
                            }),
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
        // The source lane: 42, against 30 for every other lane. It is the
        // only lane that carries a picture rather than blocks, and the strip
        // fills it below the title.
        let mut source = div()
            .relative()
            .h(px(Theme::LANE_SOURCE_HEIGHT))
            .overflow_hidden()
            .rounded(px(Theme::RADIUS_REGION))
            .bg(theme.sunk)
            .child(
                div()
                    .px(px(Theme::REGION_PADDING))
                    .text_size(px(Theme::FONT_SMALL))
                    .text_color(theme.muted)
                    .child(window.get_document_title()),
            );
        if let Some(image) = window.get_thumbnails().0 {
            let strip = Theme::LANE_SOURCE_HEIGHT - Theme::RULER_HEIGHT;
            source = source.child(
                img(image)
                    .absolute()
                    .top(px(Theme::RULER_HEIGHT))
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(strip))
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
            let mut block = div()
                .id(SharedString::from(format!(
                    "region-{}-{}",
                    region.kind, region.id
                )))
                .group("region")
                .absolute()
                .left(relative((start - offset) / visible))
                .top(px(region.row as f32 * Theme::LANE_PITCH))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(Theme::GAP))
                .h(px(Theme::LANE_HEIGHT))
                .rounded(px(Theme::RADIUS_REGION))
                .overflow_hidden()
                .bg(fill)
                .when(!region.selected, |el| {
                    el.hover(move |s| s.bg(tint.opacity(Theme::REGION_FILL_HOVER)))
                })
                .shadow(vec![hairline(edge, Theme::BORDER_WIDTH)])
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
                    .cursor(CursorStyle::ResizeLeftRight)
                    .child(
                        div()
                            .absolute()
                            .left(px(Theme::REGION_HANDLE_INSET))
                            .top(px(Theme::REGION_HANDLE_MARGIN))
                            .w(px(Theme::REGION_HANDLE_WIDTH))
                            .h(px(Theme::LANE_HEIGHT - Theme::REGION_HANDLE_MARGIN * 2.0))
                            .rounded(px(Theme::REGION_HANDLE_RADIUS))
                            .bg(tint.opacity(if selected {
                                Theme::REGION_HANDLE_ALPHA
                            } else {
                                0.
                            }))
                            .group_hover("region", move |s| {
                                s.bg(tint.opacity(Theme::REGION_HANDLE_ALPHA))
                            }),
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
        let console = panel(theme)
            .id("timeline")
            .mx(px(Theme::INSET))
            .mb(px(Theme::INSET))
            .flex_shrink_0()
            .child(toolbar)
            .child(fade_edges(
                row()
                    .id("track-scroll")
                    .gap(px(Theme::LANE_GUTTER_GAP))
                    .items_start()
                    .h(px(Theme::LANE_STACK_HEIGHT))
                    .flex_none()
                    .overflow_y_scroll()
                    .pb(px(FADE_BAND))
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
                            .child(lane_label("Source", Theme::LANE_SOURCE_HEIGHT, theme))
                            .children(
                                labels
                                    .into_iter()
                                    .map(|label| lane_label(label, Theme::LANE_HEIGHT, theme)),
                            ),
                    )
                    .child(timeline),
            ))
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
                                .clamp(1., 100.),
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
            // The export/transcription line, when there is one. It sits
            // inside the console rather than under it so the console keeps
            // the shell's own inset on all three of its edges.
            .when_some(status, |el, status| el.child(divider(theme)).child(status))
            .into_any_element();
        frosted(UiSurface::Panel.radius(), UiSurface::Panel.blur(), console).into_any_element()
    }
}
