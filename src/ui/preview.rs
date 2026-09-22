//! The canvas: the composed frame, its zoom and pan, and the crop and
//! camera gestures drawn over it.

use super::*;

pub(super) fn macos_cursor_image() -> Arc<gpui::Image> {
    static IMAGE: OnceLock<Arc<gpui::Image>> = OnceLock::new();
    IMAGE
        .get_or_init(|| {
            // Preserve the exact embedded raster from the existing approved cursor asset.
            let source =
                include_str!("../../legacy-electron/src/assets/cursors/macos/pointer-1__34-24.svg");
            let data = source
                .split_once("data:image/png;base64,")
                .expect("bundled macOS cursor PNG")
                .1
                .split('"')
                .next()
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .expect("valid bundled cursor PNG");
            Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes))
        })
        .clone()
}

/// The picture area. The handoff docks nothing over the stage, so the tool pod
/// and the inspector float on top of it and the stage keeps each one's width
/// clear instead. The preview centres in what is left, which reads slightly
/// left of the window's true centre — the handoff's own choice, over centring
/// in the window and letting the inspector cover the picture's right edge.
/// The aspect dropdown's trigger. A dropdown sizes to its widest option, and
/// the aspect list's options are two to six characters, so without a width the
/// pod changed shape every time the ratio changed.
const ASPECT_TRIGGER_WIDTH: f32 = 108.0;

fn stage_reserve(el: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .pl(px(STAGE_RESERVE_LEFT))
        .pr(px(STAGE_RESERVE_RIGHT))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(el),
        )
}

impl RootView {
    pub(super) fn sync_preview_context(&mut self, editor: &EditorWindow) {
        // The frame image changes during playback; use the source thumbnail strip instead.
        let context = PreviewContext {
            title: editor.get_document_title(),
            thumbnails: editor.get_thumbnails().0,
            has_video: editor.get_has_video(),
            aspect: editor.get_preview_aspect(),
            aspect_index: editor.get_aspect_index(),
        };
        let changed = self
            .preview_context
            .as_ref()
            .is_some_and(|old| !old.matches(&context));
        let controller_reset = editor.get_preview_zoom() <= 1. && self.preview_known_zoom > 1.;
        if changed || controller_reset || !context.has_video {
            self.preview_pan = point(px(0.), px(0.));
            if matches!(self.pinch, Some((false, ..))) {
                self.pinch = None;
            }
            if matches!(self.gesture, Some(Gesture::Canvas { .. })) {
                self.gesture = None;
            }
            if changed {
                editor.set_preview_zoom(1.);
            }
        }
        if editor.get_preview_zoom() <= 1. {
            self.preview_pan = point(px(0.), px(0.));
        }
        self.preview_known_zoom = editor.get_preview_zoom();
        self.preview_context = Some(context);
    }

    pub(super) fn clamp_preview_pan(&mut self, image_width: f32, image_height: f32) {
        let viewport = self.preview_viewport.get().size;
        self.preview_pan.x = px(preview_geometry::clamp_axis(
            f32::from(self.preview_pan.x),
            image_width,
            f32::from(viewport.width),
        ));
        self.preview_pan.y = px(preview_geometry::clamp_axis(
            f32::from(self.preview_pan.y),
            image_height,
            f32::from(viewport.height),
        ));
    }

    /// NSEvent magnification is incremental. The bridge supplies GPUI window coordinates.
    pub fn magnify(
        &mut self,
        x: f32,
        y: f32,
        delta: f32,
        phase: u8,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Surface::Editor(e) = self.surface.clone() else {
            return;
        };
        self.sync_preview_context(&e);
        if !e.get_has_video() {
            return;
        }
        let pointer = point(px(x), px(y));
        if phase == 0 || self.pinch.is_none() {
            let timeline = self.timeline_bounds.get().contains(&pointer);
            if !timeline && !self.preview_viewport.get().contains(&pointer) {
                return;
            }
            self.pinch = Some((
                timeline,
                if timeline {
                    e.get_timeline_zoom()
                } else {
                    e.get_preview_zoom()
                },
                e.get_timeline_offset(),
                self.preview_pan,
            ));
        }
        let Some((timeline, initial_zoom, initial_offset, initial_pan)) = self.pinch else {
            return;
        };
        if phase == 3 {
            if timeline {
                e.set_timeline_zoom(initial_zoom);
                e.set_timeline_offset(initial_offset);
            } else {
                e.set_preview_zoom(initial_zoom);
                self.preview_pan = initial_pan;
                self.preview_known_zoom = initial_zoom;
            }
            self.pinch = None;
            cx.notify();
            return;
        }
        if delta.is_finite() && delta != 0. {
            if timeline {
                let b = self.timeline_bounds.get();
                let fraction = (f32::from(pointer.x - b.left()) / f32::from(b.size.width).max(1.))
                    .clamp(0., 1.);
                let anchor = e.get_timeline_offset() + fraction * e.get_timeline_visible();
                e.set_timeline_zoom(
                    (e.get_timeline_zoom() * (1. + delta).max(0.01)).clamp(1., 100.),
                );
                e.set_timeline_offset(
                    (anchor - fraction * e.get_timeline_visible())
                        .clamp(0., (e.get_duration() - e.get_timeline_visible()).max(0.)),
                );
            } else {
                let old_zoom = e.get_preview_zoom();
                let new_zoom = (old_zoom * (1. + delta).max(0.01)).clamp(1., 8.);
                let center = self.preview_viewport.get().center();
                let ratio = new_zoom / old_zoom.max(0.001);
                self.preview_pan.x =
                    pointer.x - center.x - (pointer.x - center.x - self.preview_pan.x) * ratio;
                self.preview_pan.y =
                    pointer.y - center.y - (pointer.y - center.y - self.preview_pan.y) * ratio;
                if new_zoom <= 1. {
                    self.preview_pan = point(px(0.), px(0.));
                }
                e.set_preview_zoom(new_zoom);
                self.preview_known_zoom = new_zoom;
                let viewport = self.preview_viewport.get().size;
                let aspect = e.get_preview_aspect().max(0.01);
                let width = (f32::from(viewport.width) - 16.)
                    .max(1.)
                    .min((f32::from(viewport.height) - 16.).max(1.) * aspect)
                    * new_zoom;
                self.clamp_preview_pan(width, width / aspect);
            }
        }
        if phase == 2 {
            self.pinch = None;
        }
        cx.notify();
    }

    pub(super) fn preview(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        self.sync_preview_context(e);
        if !e.get_has_video() {
            return stage_reserve(
                empty_state(
                    theme,
                    "No Video Loaded",
                    "Open a video or start a recording",
                )
                .child(
                    row()
                        .gap(px(Theme::GAP))
                        .child(
                            self.action("open-video", "Open video", "open", !e.get_busy())
                                .raised()
                                .hero(),
                        )
                        // The hero size exists for exactly this: the one action an
                        // otherwise empty screen is asking for. It had been built
                        // and never used, so the emptiest screen in the app wore
                        // the same 44px button as a dialog's footer.
                        .child(
                            self.action(
                                "new-recording",
                                "New recording",
                                "record",
                                !e.get_busy() && !e.get_recording(),
                            )
                            .primary()
                            .hero(),
                        ),
                ),
            )
            .into_any_element();
        }
        let viewport = self.preview_viewport.get();
        let available_w = if viewport.size.width > px(0.) {
            f32::from(viewport.size.width) - 16.
        } else {
            (f32::from(window.viewport_size().width) - STAGE_RESERVE_LEFT - STAGE_RESERVE_RIGHT)
                .max(100.)
        };
        let available_h = if viewport.size.height > px(0.) {
            f32::from(viewport.size.height) - 16.
        } else {
            (f32::from(window.viewport_size().height) - 492.).max(80.)
        };
        let aspect = e.get_preview_aspect().max(0.01);
        let width = available_w.max(1.).min(available_h.max(1.) * aspect) * e.get_preview_zoom();
        let height = width / aspect;
        self.clamp_preview_pan(width, height);
        if (e.get_preview_pixel_width() - width).abs() > 0.5 {
            e.set_preview_pixel_width(width);
        }
        let editor = e.clone();
        let aspect_control = self.dropdown(
            "aspect",
            ["Native", "16:9", "9:16", "1:1", "4:3", "3:2"]
                .map(str::to_owned)
                .to_vec(),
            e.get_aspect_index(),
            true,
            cx,
            move |i, _, _| {
                editor.set_aspect_index(i as i32);
                editor.defer_field(
                    "aspectRatio".into(),
                    ["native", "16:9", "9:16", "1:1", "4:3", "3:2"][i].into(),
                );
            },
        );
        let mut picture = div()
            .id("preview-image")
            .relative()
            .flex_shrink_0()
            .w(px(width))
            .h(px(height))
            .rounded_xl()
            .overflow_hidden()
            .child(measure(self.preview_bounds.clone()));
        if let Some(image) = e.get_preview().0 {
            picture = picture.child(img(image).size_full().object_fit(ObjectFit::Contain));
        }
        picture = picture.on_mouse_down(
            MouseButton::Left,
            cx.listener(|s, event: &MouseDownEvent, _, cx| {
                if let Surface::Editor(e) = &s.surface {
                    let b = s.preview_bounds.get();
                    e.invoke_preview_click(
                        (f32::from(event.position.x - b.left()) / f32::from(b.size.width).max(1.))
                            .clamp(0., 1.),
                        (f32::from(event.position.y - b.top()) / f32::from(b.size.height).max(1.))
                            .clamp(0., 1.),
                    );
                }
                cx.stop_propagation();
            }),
        );
        if e.get_edit_visible() && !e.get_playing() {
            let (dx, dy, resize) = match &self.gesture {
                Some(Gesture::Canvas { dx, dy, resize, .. }) => (*dx, *dy, *resize),
                _ => (0., 0., false),
            };
            picture = picture.child(
                div()
                    .id("preview-selection")
                    .absolute()
                    .left(relative(e.get_edit_x() + if resize { 0. } else { dx }))
                    .top(relative(e.get_edit_y() + if resize { 0. } else { dy }))
                    .w(relative(
                        (e.get_edit_width() + if resize { dx } else { 0. }).max(0.005),
                    ))
                    .h(relative(
                        (e.get_edit_height() + if resize { dy } else { 0. }).max(0.005),
                    ))
                    .border_2()
                    .border_color(theme.accent)
                    .cursor(CursorStyle::ClosedHand)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, event: &MouseDownEvent, _, cx| {
                            s.gesture = Some(Gesture::Canvas {
                                origin: event.position,
                                resize: false,
                                dx: 0.,
                                dy: 0.,
                            });
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .id("selection-resize")
                            .absolute()
                            .right(px(-5.))
                            .bottom(px(-5.))
                            .size(px(Theme::ICON_SIZE_SMALL))
                            .bg(theme.on_accent)
                            .border_1()
                            .border_color(theme.accent)
                            .cursor(CursorStyle::ResizeUpLeftDownRight)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|s, event: &MouseDownEvent, _, cx| {
                                    s.gesture = Some(Gesture::Canvas {
                                        origin: event.position,
                                        resize: true,
                                        dx: 0.,
                                        dy: 0.,
                                    });
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            );
        }
        // The aspect pod. The handoff floats it at the stage's top left, not
        // over its centre: the stage's left reserve already ends where the
        // tool pod's air does, so the pod sits flush with the picture area's
        // own left edge and reads as belonging to the frame under it.
        //
        // It is an absolute child of the viewport, which is already
        // `relative`, so nothing about the stage's own measurement — the zoom
        // and the pan both depend on it — changes.
        let aspect_pod = div()
            .absolute()
            .left_0()
            .top(px(Theme::INSET))
            .child(frosted(
                UiSurface::Pod.radius(),
                UiSurface::Pod.blur(),
                pod(theme)
                    .child(
                        div()
                            .w(px(ASPECT_TRIGGER_WIDTH))
                            .flex_shrink_0()
                            .child(aspect_control),
                    )
                    .child(self.action("crop", "Crop", "visual-crop", true))
                    .child(
                        button(
                            "fit-preview",
                            format!("Fit · {}%", (e.get_preview_zoom() * 100.).round()),
                            theme,
                        )
                        .ghost()
                        .on_click(cx.listener(|s, _, _, cx| {
                            if let Surface::Editor(e) = &s.surface {
                                e.set_preview_zoom(1.);
                            }
                            s.preview_pan = point(px(0.), px(0.));
                            s.preview_known_zoom = 1.;
                            if matches!(s.pinch, Some((false, ..))) {
                                s.pinch = None;
                            }
                            cx.notify();
                        })),
                    ),
            ));
        stage_reserve(
            div().flex().flex_col().child(
                div()
                    .id("preview-viewport")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(measure(self.preview_viewport.clone()))
                    .on_scroll_wheel(cx.listener(|s, event: &ScrollWheelEvent, window, cx| {
                        let delta = event.delta.pixel_delta(px(20.));
                        if event.modifiers.control || event.modifiers.platform {
                            s.magnify(
                                f32::from(event.position.x),
                                f32::from(event.position.y),
                                (-f32::from(delta.y) * 0.01).exp() - 1.,
                                2,
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                            return;
                        }
                        if let Surface::Editor(e) = &s.surface
                            && e.get_preview_zoom() > 1.
                        {
                            s.preview_pan.x += delta.x;
                            s.preview_pan.y += delta.y;
                            let image = s.preview_bounds.get().size;
                            s.clamp_preview_pan(f32::from(image.width), f32::from(image.height));
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .relative()
                            .left(self.preview_pan.x)
                            .top(self.preview_pan.y)
                            .child(picture),
                    )
                    .child(aspect_pod),
            ),
        )
        .into_any_element()
    }
}
