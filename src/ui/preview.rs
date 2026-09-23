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
const ASPECT_TRIGGER_WIDTH: f32 = 96.0;

/// How far the pod's zoom out and in step, and the most the preview
/// magnifies.
const PREVIEW_ZOOM_STEP: f32 = 1.25;
pub(super) const PREVIEW_ZOOM_MAX: f32 = 8.;

/// A zoom step on its way: from and to, the pan it set out with, and when.
#[derive(Clone, Copy)]
pub(super) struct PreviewZoomMove {
    from: f32,
    to: f32,
    pan: Point<Pixels>,
    started: Instant,
}

/// The stage's right reserve: the inspector's inset, its width as last
/// dragged and the inset again, or only its toggle's while it is folded away.
pub(super) fn stage_reserve_right(window: &Window, inspector_width: f32) -> f32 {
    if inspector_collapsed(window) {
        STAGE_RESERVE_RIGHT_COLLAPSED
    } else {
        Theme::INSET * 2.0 + inspector_width
    }
}

/// Whether the window is too narrow to keep the inspector beside the stage.
pub(super) fn inspector_collapsed(window: &Window) -> bool {
    window.viewport_size().width < px(INSPECTOR_COLLAPSE_WIDTH)
}

fn stage_reserve(el: impl IntoElement, right: f32) -> Div {
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .pl(px(STAGE_RESERVE_LEFT))
        .pr(px(right))
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
                self.preview_zoom_move = None;
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
        // A pinch takes over from a step still easing.
        self.preview_zoom_move = None;
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
                let new_zoom = (old_zoom * (1. + delta).max(0.01)).clamp(1., PREVIEW_ZOOM_MAX);
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
    ) -> (AnyElement, Option<AnyElement>) {
        let theme = self.theme;
        self.advance_preview_zoom(e, window);
        self.sync_preview_context(e);
        if !e.get_has_video() {
            return (self.empty_stage(e), None);
        }
        let viewport = self.preview_viewport.get();
        let available_w = if viewport.size.width > px(0.) {
            f32::from(viewport.size.width)
        } else {
            (f32::from(window.viewport_size().width)
                - STAGE_RESERVE_LEFT
                - stage_reserve_right(window, self.inspector_width))
            .max(100.)
        };
        let available_h = if viewport.size.height > px(0.) {
            f32::from(viewport.size.height) - Theme::STAGE_PICTURE_MARGIN * 2.
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
        // The picture's rest centre is the stage's, and the pan moves it off
        // that. It is drawn in a layer the size of the window, so a picture
        // zoomed past the stage runs on under the pods, the titlebar and the
        // console instead of being cut off at the stage's edge. The stage is
        // still what it is sized to and what the pan is held inside, so every
        // part of it can be brought out into the clear.
        let layer = self.preview_layer.get();
        let centre = if viewport.size.width > px(0.) {
            viewport.center()
        } else {
            point(
                px(STAGE_RESERVE_LEFT + available_w / 2.),
                px(Theme::TITLEBAR_HEIGHT + Theme::STAGE_PICTURE_MARGIN + available_h / 2.),
            )
        };
        let left = centre.x - layer.origin.x + self.preview_pan.x - px(width / 2.);
        let top = centre.y - layer.origin.y + self.preview_pan.y - px(height / 2.);
        let mut picture = div()
            .id("preview-image")
            .absolute()
            .left(left)
            .top(top)
            .w(px(width))
            .h(px(height))
            .rounded(px(Theme::STAGE_PICTURE_RADIUS))
            .shadow(theme.picture_shadow())
            .overflow_hidden()
            .child(measure(self.preview_bounds.clone()));
        if let Some(image) = e.get_preview().0 {
            picture = picture.child(
                img(image)
                    .size_full()
                    .rounded(px(Theme::STAGE_PICTURE_RADIUS))
                    .object_fit(ObjectFit::Contain),
            );
        }
        picture = picture.on_mouse_down(
            MouseButton::Left,
            cx.listener(|s, event: &MouseDownEvent, _, cx| {
                // Past the stage the picture is only showing through: the
                // console, the titlebar and the pods above it take the press.
                if !s.preview_viewport.get().contains(&event.position) {
                    return;
                }
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
                            if !s.preview_viewport.get().contains(&event.position) {
                                return;
                            }
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
                            .right(px(Theme::SELECTION_HANDLE_OFFSET))
                            .bottom(px(Theme::SELECTION_HANDLE_OFFSET))
                            .size(px(Theme::SELECTION_HANDLE_SIZE))
                            .rounded(px(Theme::SELECTION_HANDLE_RADIUS))
                            .bg(theme.on_accent)
                            .border_1()
                            .border_color(theme.accent)
                            .cursor(CursorStyle::ResizeUpLeftDownRight)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|s, event: &MouseDownEvent, _, cx| {
                                    if !s.preview_viewport.get().contains(&event.position) {
                                        return;
                                    }
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
        // The viewport's height is what the picture is sized to, so the
        // column holding it has to take the stage's height rather than its
        // content's: sized by its content, it measured the picture, which
        // was sized by it, and the two shrank to nothing.
        let stage = stage_reserve(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .pb(px(STAGE_RESERVE_BOTTOM))
                .child(
                    div()
                        .id("preview-viewport")
                        .relative()
                        .flex_1()
                        .min_h_0()
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
                                s.clamp_preview_pan(
                                    f32::from(image.width),
                                    f32::from(image.height),
                                );
                            }
                            cx.stop_propagation();
                            cx.notify();
                        })),
                ),
            stage_reserve_right(window, self.inspector_width),
        )
        .into_any_element();
        let layer = div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(measure(self.preview_layer.clone()))
            .child(picture)
            .into_any_element();
        (stage, Some(layer))
    }

    /// Where the preview's zoom is headed: the end of a step still easing,
    /// or where it is. A second click steps on from the first's end rather
    /// than from wherever the ease had got to.
    fn preview_zoom_target(&self, e: &EditorWindow) -> f32 {
        self.preview_zoom_move
            .map_or(e.get_preview_zoom(), |m| m.to)
    }

    /// Step the preview's zoom, keeping whatever is at the stage's centre
    /// there. The picture's centre sits at the stage's plus the pan, so a
    /// point at the stage's centre is `-pan` from the picture's, and scaling
    /// the pan with the zoom keeps it where it was. The step eases over
    /// [`PREVIEW_ZOOM_MS`] rather than landing; a pinch is already as smooth
    /// as the hand.
    fn zoom_preview(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let Surface::Editor(e) = &self.surface else {
            return;
        };
        let from = e.get_preview_zoom().max(0.001);
        let to = zoom.clamp(1., PREVIEW_ZOOM_MAX);
        if matches!(self.pinch, Some((false, ..))) {
            self.pinch = None;
        }
        self.preview_zoom_move = Some(PreviewZoomMove {
            from,
            to,
            pan: self.preview_pan,
            started: Instant::now(),
        });
        if subtake_ui::motion::reduced_motion() {
            self.preview_zoom_move = None;
            self.preview_pan = if to <= 1. {
                point(px(0.), px(0.))
            } else {
                self.preview_pan * (to / from)
            };
            e.set_preview_zoom(to);
            self.preview_known_zoom = to;
        }
        cx.notify();
    }

    /// Carry a zoom step on by a frame. On the way to Fit the pan runs out
    /// with it, so the picture comes home to the centre as it shrinks; any
    /// other step keeps the stage's centre point where it is.
    fn advance_preview_zoom(&mut self, e: &EditorWindow, window: &mut Window) {
        let Some(m) = self.preview_zoom_move else {
            return;
        };
        let zoom = subtake_ui::motion::ease_toward(
            m.from,
            m.to,
            m.started,
            PREVIEW_ZOOM_MS,
            Instant::now(),
        );
        self.preview_pan = if m.to <= 1. {
            let t = if m.to == m.from {
                1.
            } else {
                (zoom - m.from) / (m.to - m.from)
            };
            m.pan * (1. - t)
        } else {
            m.pan * (zoom / m.from)
        };
        e.set_preview_zoom(zoom);
        self.preview_known_zoom = zoom;
        if zoom == m.to {
            self.preview_zoom_move = None;
        } else {
            window.request_animation_frame();
        }
    }

    /// The aspect pod: the frame's ratio, the crop tool and the zoom.
    ///
    /// It floats at the bottom centre of the stage, between the tool pod and
    /// the inspector, and the picture stops short of it, so at rest the two
    /// never overlap. It is the thin pod, at the 34 the handoff gives an
    /// aspect pill.
    ///
    /// Not drawn by the design: the handoff hangs it from the stage's top
    /// left, over the picture.
    ///
    /// Not drawn by the design: the zoom steps and the readout between them.
    /// The handoff has a single Fit pill; zoom out and in step by a quarter
    /// about the stage's centre, and Fit puts the picture back.
    pub(super) fn aspect_pod(
        &mut self,
        e: &EditorWindow,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !e.get_has_video() {
            return None;
        }
        let theme = self.theme;
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
        aspect_control.update(cx, |d, _| {
            d.compact = true;
            d.opens_up = true;
        });
        let zoom = e.get_preview_zoom();
        // The steps dim by where the zoom is going, so Fit greys out on the
        // click that sends it home rather than when it arrives.
        let target = self.preview_zoom_target(e);
        Some(
            div()
                .absolute()
                .left(px(STAGE_RESERVE_LEFT))
                .right(px(stage_reserve_right(window, self.inspector_width)))
                .bottom(px(Theme::INSET))
                .flex()
                .justify_center()
                .child(frosted(
                    Theme::RADIUS_ROW,
                    UiSurface::Pod.blur(),
                    pod_small(theme)
                        .child(
                            div()
                                .w(px(ASPECT_TRIGGER_WIDTH))
                                .flex_shrink_0()
                                .child(aspect_control),
                        )
                        .child(self.action("crop", "Crop", "visual-crop", true).compact())
                        .child(
                            zoom_control("preview-zoom", zoom, theme)
                                .can_zoom_out(target > 1.)
                                .can_zoom_in(target < PREVIEW_ZOOM_MAX)
                                .can_fit(target > 1.)
                                .on_zoom_out(cx.listener(|s, _, _, cx| {
                                    if let Surface::Editor(e) = &s.surface {
                                        let zoom = s.preview_zoom_target(e) / PREVIEW_ZOOM_STEP;
                                        s.zoom_preview(zoom, cx);
                                    }
                                }))
                                .on_zoom_in(cx.listener(|s, _, _, cx| {
                                    if let Surface::Editor(e) = &s.surface {
                                        let zoom = s.preview_zoom_target(e) * PREVIEW_ZOOM_STEP;
                                        s.zoom_preview(zoom, cx);
                                    }
                                }))
                                .on_fit(cx.listener(|s, _, _, cx| s.zoom_preview(1., cx))),
                        ),
                ))
                .into_any_element(),
        )
    }
}
