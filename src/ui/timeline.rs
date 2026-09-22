//! The timeline: tracks, regions, the scrubber and its drag gestures.

use super::*;

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

    pub(super) fn timeline(&mut self, window: &EditorWindow, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let visible = window.get_timeline_visible();
        let offset = window.get_timeline_offset();
        let editor = window.clone();
        let zoom = self.slider(
            "timeline-zoom",
            1.,
            100.,
            window.get_timeline_zoom(),
            ("Zoom", "MagnifyingGlassPlus-regular"),
            (1.0, "\u{00d7}"),
            cx,
            move |v, _, _, _| {
                editor.set_timeline_zoom(v);
                editor.set_timeline_offset(
                    editor
                        .get_timeline_offset()
                        .min((editor.get_duration() - editor.get_timeline_visible()).max(0.)),
                );
            },
        );
        let editor = window.clone();
        let position = self.slider(
            "timeline-position",
            0.,
            (window.get_duration() - visible).max(0.001),
            offset,
            ("Position", "ArrowsOutSimple-regular"),
            (1.0, " s"),
            cx,
            move |v, _, _, _| editor.set_timeline_offset(v),
        );
        let editor = window.clone();
        let editor_out = window.clone();
        let editor_in = window.clone();
        let toolbar = row()
            .gap(px(Theme::GAP_SMALL))
            .child(self.icon_action(
                "add-zoom",
                "MagnifyingGlassPlus-regular",
                "Add zoom",
                "add-zoom",
                true,
            ))
            .child(
                button("auto-zoom", "Suggest zooms", theme)
                    .glyph("MagicWand-regular")
                    .ghost()
                    .on_click(self.command("auto-zoom")),
            )
            .child(self.icon_action(
                "split-clip",
                "Scissors-regular",
                "Split clip",
                "split-clip",
                true,
            ))
            .child(self.menu_button("Add", cx))
            .child(div().flex_1())
            // Snap keeps the accent plate while engaged; the zoom cluster is
            // icon-only so the strip stays quiet.
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
                    editor_out.set_timeline_zoom((editor_out.get_timeline_zoom() / 1.5).max(1.))
                }),
            )
            .child(
                icon_button("zoom-in", "MagnifyingGlassPlus-regular", "Zoom in", theme)
                    .ghost()
                    .on_click(move |_, _, _| {
                        editor_in.set_timeline_zoom((editor_in.get_timeline_zoom() * 1.5).min(100.))
                    }),
            );
        let _ = zoom;
        let mut ruler = div().relative().h(px(24.));
        for i in 0..8 {
            ruler = ruler.child(
                div()
                    .absolute()
                    .left(relative(i as f32 / 8.))
                    .text_color(theme.muted)
                    .child(format!("{:.1}s", offset + visible * i as f32 / 8.)),
            );
        }
        let mut source = div()
            .relative()
            .h(px(56.))
            .overflow_hidden()
            .rounded_lg()
            .bg(theme.surface)
            .child(div().px_2().child(window.get_document_title()));
        if let Some(image) = window.get_thumbnails().0 {
            source = source.child(
                img(image)
                    .absolute()
                    .top(px(22.))
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(34.))
                    .object_fit(ObjectFit::Fill),
            );
        }
        let labels: Vec<String> = window.get_track_labels().iter().collect();
        let mut tracks = div().relative().h(px(labels.len() as f32 * TRACK_HEIGHT));
        for i in 0..labels.len() {
            tracks = tracks.child(
                div()
                    .absolute()
                    .top(px(i as f32 * TRACK_HEIGHT))
                    .w_full()
                    .h(px(38.))
                    .rounded_lg()
                    .bg(theme.surface),
            );
        }
        if let Some(image) = window.get_waveform().0 {
            tracks = tracks.child(
                img(image)
                    .absolute()
                    .top(px(window.get_audio_row() as f32 * TRACK_HEIGHT))
                    .left(relative(-offset / visible))
                    .w(relative(window.get_timeline_zoom()))
                    .h(px(38.))
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
            let mut block = div()
                .id(SharedString::from(format!(
                    "region-{}-{}",
                    region.kind, region.id
                )))
                .absolute()
                .left(relative((start - offset) / visible))
                .top(px(region.row as f32 * TRACK_HEIGHT + 1.))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(8.))
                .h(px(36.))
                .rounded_lg()
                .overflow_hidden()
                .bg(tint.opacity(if region.selected { 0.24 } else { 0.11 }))
                .border_1()
                .border_color(if region.selected { tint } else { theme.border })
                .cursor(CursorStyle::ClosedHand)
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_ellipsis()
                        .child(region.label.clone()),
                );
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
                let mut handle = div()
                    .id(("resize", mode as usize))
                    .absolute()
                    .top_0()
                    .w(px(10.))
                    .h_full()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .child(
                        div()
                            .absolute()
                            .left(px(3.))
                            .top(px(10.))
                            .w(px(3.))
                            .h(px(16.))
                            .rounded_full()
                            .bg(tint.opacity(0.55)),
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
            // Cap, continuous rule and six-dot grip — the product's scrubber,
            // centred on the playhead so the rule sits on the exact frame.
            timeline = timeline.child(
                div()
                    .absolute()
                    .left(relative(playhead))
                    .ml(px(-Theme::SCRUBBER_WIDTH / 2.0))
                    .top_0()
                    .bottom_0()
                    .w(px(Theme::SCRUBBER_WIDTH))
                    .child(timeline_scrubber(theme, 40.0)),
            );
        }
        panel(theme)
            .id("timeline")
            .h(px(310.))
            .mx(px(Theme::GAP))
            .mb(px(Theme::GAP))
            .flex_shrink_0()
            .child(toolbar)
            .child(fade_edges(
                row()
                    .id("track-scroll")
                    .items_start()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb(px(FADE_BAND))
                    .child(
                        column()
                            .w(px(74.))
                            .flex_shrink_0()
                            .gap_0()
                            .child(
                                div()
                                    .h(px(88.))
                                    .pt_8()
                                    .text_color(theme.muted)
                                    .child("Source"),
                            )
                            .children(labels.into_iter().map(|label| {
                                div()
                                    .h(px(TRACK_HEIGHT))
                                    .pt_3()
                                    .text_size(px(Theme::FONT_SMALL))
                                    .text_color(theme.muted)
                                    .child(label)
                            })),
                    )
                    .child(timeline),
            ))
            .child(div().ml(px(82.)).child(position))
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
            .into_any_element()
    }
}
