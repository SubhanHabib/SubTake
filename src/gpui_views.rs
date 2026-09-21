//! Native editor and recorder surfaces. Business commands remain in the UI facade.
use crate::ui_state::{EditorWindow, Field, RecordingLauncher, RecordingOptions, Region};
use base64::Engine;
use gpui::{prelude::*, *};
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, OnceLock},
};
use subtake_theme::{FONT_SANS, PANEL_WIDTH, RAIL_WIDTH, TRACK_HEIGHT, Theme};

/// Title bar strip: tall enough to seat the 40px icon cluster with air.
const TITLEBAR_HEIGHT: f32 = 56.0;

/// The glyph a numeric field wears in its scrub plate. Keyed on the field so
/// the inspector reads as a set of labelled dials rather than a list of rows.
fn field_glyph(key: &str) -> &'static str {
    let k = key.rsplit('.').next().unwrap_or(key).to_ascii_lowercase();
    if k.contains("radius") || k.contains("corner") {
        "Selection-regular"
    } else if k.contains("shadow") {
        "Drop-regular"
    } else if k.contains("scale") || k.contains("zoom") || k.contains("size") {
        "MagnifyingGlassPlus-regular"
    } else if k.contains("volume") || k.contains("gain") || k.contains("audio") {
        "SpeakerHigh-regular"
    } else if k.contains("speed") || k.contains("duration") || k.contains("time") {
        "Timer-regular"
    } else if k.contains("opacity") {
        "EyeSlash-regular"
    } else {
        "SlidersHorizontal-regular"
    }
}
use subtake_ui::{
    Button, ButtonVariant, Dropdown, FADE_BAND, MENU_BLUR, Slider, Surface as UiSurface, TextInput,
    button, choice_tile, column, empty_state, fade_edges, frosted, icon_button, measure,
    media_tile, panel, panel_variant, progress_bar, rail_button, row, section_label,
    segmented_control, status_dot, swatch, timeline_scrubber, toggle, tooltip,
};

fn macos_cursor_image() -> Arc<gpui::Image> {
    static IMAGE: OnceLock<Arc<gpui::Image>> = OnceLock::new();
    IMAGE
        .get_or_init(|| {
            // Preserve the exact embedded raster from the existing approved cursor asset.
            let source =
                include_str!("../legacy-electron/src/assets/cursors/macos/pointer-1__34-24.svg");
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

#[derive(Clone)]
pub enum Surface {
    Editor(EditorWindow),
    Launcher(RecordingLauncher),
    Options(RecordingOptions),
}
impl Surface {
    fn action(&self, command: &str) {
        let surface = self.clone();
        let command = command.to_owned();
        crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || match surface {
            Self::Editor(s) => s.invoke_action(command),
            Self::Launcher(s) => s.invoke_action(command),
            Self::Options(s) => s.invoke_action(command),
        });
    }
    fn appearance(&self) -> String {
        match self {
            Self::Editor(s) => s.get_appearance(),
            Self::Launcher(s) => s.get_appearance(),
            Self::Options(s) => s.get_appearance(),
        }
    }
}

enum Gesture {
    Seek,
    Region {
        region: Region,
        origin: Point<Pixels>,
        mode: i32,
        delta: f32,
    },
    Canvas {
        origin: Point<Pixels>,
        resize: bool,
        dx: f32,
        dy: f32,
    },
}

// Kept independent of GPUI so geometry regressions can be tested without a window.
mod preview_geometry {
    pub fn clamp_axis(pan: f32, image: f32, viewport: f32) -> f32 {
        if !pan.is_finite() || !image.is_finite() || !viewport.is_finite() {
            return 0.;
        }
        let overflow = ((image - viewport) / 2.).max(0.);
        pan.clamp(-overflow, overflow)
    }

    #[cfg(test)]
    mod tests {
        use super::clamp_axis;

        #[test]
        fn edges_stop_at_half_the_centered_image_overflow() {
            assert_eq!(clamp_axis(900., 1200., 800.), 200.);
            assert_eq!(clamp_axis(-900., 1200., 800.), -200.);
            assert_eq!(clamp_axis(75., 1200., 800.), 75.);
        }

        #[test]
        fn letterboxed_axis_stays_centered() {
            assert_eq!(clamp_axis(100., 450., 600.), 0.);
            assert_eq!(clamp_axis(-100., 600., 600.), 0.);
        }

        #[test]
        fn resize_and_zoom_out_reclamp_existing_pan() {
            assert_eq!(clamp_axis(200., 900., 800.), 50.);
            assert_eq!(clamp_axis(200., 800., 1000.), 0.);
            assert_eq!(clamp_axis(f32::NAN, 1200., 800.), 0.);
        }
    }
}

struct PreviewContext {
    title: String,
    thumbnails: Option<Arc<RenderImage>>,
    has_video: bool,
    aspect: f32,
    aspect_index: i32,
}

impl PreviewContext {
    fn matches(&self, other: &Self) -> bool {
        self.title == other.title
            && self.has_video == other.has_video
            && self.aspect == other.aspect
            && self.aspect_index == other.aspect_index
            && match (&self.thumbnails, &other.thumbnails) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

// Modal operations run after GPUI releases its App borrow.
trait DeferredCommands {
    fn defer_action(&self, command: String);
    fn defer_panel(&self, panel: String);
    fn defer_field(&self, key: String, value: String);
    fn defer_option(&self, key: String, value: String);
}
macro_rules! deferred_commands {
    ($ty:ty) => {
        impl DeferredCommands for $ty {
            fn defer_action(&self, command: String) {
                let ui = self.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    ui.invoke_action(command)
                });
            }
            fn defer_panel(&self, panel: String) {
                let ui = self.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    ui.invoke_panel_change(panel)
                });
            }
            fn defer_field(&self, key: String, value: String) {
                let ui = self.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    ui.invoke_field_change(key, value)
                });
            }
            fn defer_option(&self, key: String, value: String) {
                let ui = self.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    ui.invoke_option(key, value)
                });
            }
        }
    };
}
deferred_commands!(EditorWindow);
deferred_commands!(RecordingLauncher);
deferred_commands!(RecordingOptions);

pub struct RootView {
    surface: Surface,
    focus: FocusHandle,
    inputs: HashMap<String, Entity<TextInput>>,
    dropdowns: HashMap<String, Entity<Dropdown>>,
    sliders: HashMap<String, Entity<Slider>>,
    timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    preview_bounds: Rc<Cell<Bounds<Pixels>>>,
    preview_viewport: Rc<Cell<Bounds<Pixels>>>,
    pinch: Option<(bool, f32, f32, Point<Pixels>)>,
    gesture: Option<Gesture>,
    menu: Option<String>,
    preview_pan: Point<Pixels>,
    preview_context: Option<PreviewContext>,
    preview_known_zoom: f32,
    theme: Theme,
}

impl RootView {
    pub fn new(surface: Surface, window: &mut Window, cx: &mut Context<Self>) -> Self {
        subtake_ui::init(cx);
        // A silent fallback to the system face still renders legible text and
        // would pass every other check, so surface it loudly instead.
        let (sans, mono) = subtake_ui::families_available(cx);
        if !sans || !mono {
            eprintln!("SUBTAKE_FONTS_MISSING: Geist={sans} GeistMono={mono}");
        }
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let theme = Theme::new(&surface.appearance(), window.appearance());
        Self {
            surface,
            focus,
            inputs: HashMap::new(),
            dropdowns: HashMap::new(),
            sliders: HashMap::new(),
            timeline_bounds: Rc::new(Cell::new(Bounds::default())),
            preview_bounds: Rc::new(Cell::new(Bounds::default())),
            preview_viewport: Rc::new(Cell::new(Bounds::default())),
            pinch: None,
            gesture: None,
            menu: None,
            preview_pan: point(px(0.), px(0.)),
            preview_context: None,
            preview_known_zoom: 1.,
            theme,
        }
    }

    fn sync_preview_context(&mut self, e: &EditorWindow) {
        // The frame image changes during playback; use the source thumbnail strip instead.
        let context = PreviewContext {
            title: e.get_document_title(),
            thumbnails: e.get_thumbnails().0,
            has_video: e.get_has_video(),
            aspect: e.get_preview_aspect(),
            aspect_index: e.get_aspect_index(),
        };
        let changed = self
            .preview_context
            .as_ref()
            .is_some_and(|old| !old.matches(&context));
        let controller_reset = e.get_preview_zoom() <= 1. && self.preview_known_zoom > 1.;
        if changed || controller_reset || !context.has_video {
            self.preview_pan = point(px(0.), px(0.));
            if matches!(self.pinch, Some((false, ..))) {
                self.pinch = None;
            }
            if matches!(self.gesture, Some(Gesture::Canvas { .. })) {
                self.gesture = None;
            }
            if changed {
                e.set_preview_zoom(1.);
            }
        }
        if e.get_preview_zoom() <= 1. {
            self.preview_pan = point(px(0.), px(0.));
        }
        self.preview_known_zoom = e.get_preview_zoom();
        self.preview_context = Some(context);
    }

    fn clamp_preview_pan(&mut self, image_width: f32, image_height: f32) {
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

    fn action(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        command: &str,
        enabled: bool,
    ) -> Button {
        let s = self.surface.clone();
        let command = command.to_owned();
        button(id, label, self.theme)
            .enabled(enabled)
            .on_click(move |_, _, _| s.action(&command))
    }

    /// A visible caption in the editor's language, falling back to the
    /// source string when the catalog has no entry.
    fn translate(&self, editor: &EditorWindow, label: &str) -> String {
        let translated = editor.invoke_translate(label.into(), editor.get_language());
        if translated.is_empty() {
            label.to_owned()
        } else {
            translated
        }
    }

    /// A command as a bare click handler, for buttons built inline.
    fn command(&self, command: &str) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let surface = self.surface.clone();
        let command = command.to_owned();
        move |_, _, _| surface.action(&command)
    }

    /// A square icon-only title-bar action.
    fn icon_action(
        &self,
        id: &str,
        glyph: &str,
        label: &str,
        command: &str,
        enabled: bool,
    ) -> Button {
        let s = self.surface.clone();
        let command = command.to_owned();
        icon_button(SharedString::from(id.to_owned()), glyph, label.to_owned(), self.theme)
            .ghost()
            .enabled(enabled)
            .on_click(move |_, _, _| s.action(&command))
    }

    /// A rail entry: round icon over its caption, accented while active.
    fn rail_panel_button(
        &self,
        editor: &EditorWindow,
        label: &str,
        name: &str,
        glyph: &str,
    ) -> AnyElement {
        let e = editor.clone();
        let target = name.to_owned();
        // "Help" opens a command rather than a panel.
        let is_panel = !target.contains('-');
        let active = is_panel && editor.get_panel() == target;
        let surface = self.surface.clone();
        let caption = editor.invoke_translate(label.into(), editor.get_language());
        let caption = if caption.is_empty() {
            label.to_owned()
        } else {
            caption
        };
        rail_button(
            SharedString::from(format!("rail-{target}")),
            glyph,
            caption,
            active,
            self.theme,
            move |_, _, _| {
                if is_panel {
                    e.set_panel(target.clone());
                    e.defer_panel(target.clone());
                } else {
                    surface.action(&target);
                }
            },
        )
        .into_any_element()
    }

    fn panel_button(&self, editor: &EditorWindow, label: &str, name: &str) -> Button {
        let e = editor.clone();
        let name = name.to_owned();
        button(
            SharedString::from(format!("panel-{name}-{label}")),
            self.translate(editor, label),
            self.theme,
        )
        .selected(editor.get_panel() == name)
        .on_click(move |_, _, _| {
            e.set_panel(name.clone());
            e.defer_panel(name.clone());
        })
    }

    fn input(
        &mut self,
        id: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        accept: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Entity<TextInput> {
        let theme = self.theme;
        let accept = Rc::new(accept);
        let initial = accept.clone();
        let input = self
            .inputs
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    TextInput::new(cx, value.to_owned(), theme, move |v, w, cx| {
                        initial(v, w, cx)
                    })
                })
            })
            .clone();
        input.update(cx, |s, _| {
            s.set_handler(move |v, w, cx| accept(v, w, cx));
            s.theme = theme;
            s.sync(value, window);
        });
        input
    }

    fn dropdown(
        &mut self,
        id: &str,
        items: Vec<String>,
        selected: i32,
        enabled: bool,
        cx: &mut Context<Self>,
        change: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Entity<Dropdown> {
        let theme = self.theme;
        let change = Rc::new(change);
        let initial = change.clone();
        let control = self
            .dropdowns
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    Dropdown::new(
                        cx,
                        items.clone(),
                        selected.max(0) as usize,
                        theme,
                        move |v, w, cx| initial(v, w, cx),
                    )
                })
            })
            .clone();
        control.update(cx, |s, _| {
            s.set_handler(move |v, w, cx| change(v, w, cx));
            s.items = items;
            s.selected = selected.max(0) as usize;
            s.enabled = enabled;
            s.theme = theme;
        });
        control
    }

    fn slider(
        &mut self,
        id: &str,
        minimum: f32,
        maximum: f32,
        value: f32,
        caption: (&str, &str),
        cx: &mut Context<Self>,
        change: impl Fn(f32, bool, &mut Window, &mut App) + 'static,
    ) -> Entity<Slider> {
        let theme = self.theme;
        let change = Rc::new(change);
        let initial = change.clone();
        let control = self
            .sliders
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|_| {
                    Slider::new(minimum, maximum, value, theme, move |v, commit, w, cx| {
                        initial(v, commit, w, cx)
                    })
                })
            })
            .clone();
        control.update(cx, |s, _| {
            s.set_handler(move |v, commit, w, cx| change(v, commit, w, cx));
            s.minimum = minimum;
            s.maximum = maximum;
            s.sync(value);
            s.theme = theme;
            s.set_caption(caption.0.to_owned(), caption.1.to_owned());
        });
        control
    }

    fn field(
        &mut self,
        e: &EditorWindow,
        field: Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = self.theme;
        let key = field.key.clone();
        let id = format!("{}:{}", e.get_panel(), key);
        let label = e.invoke_translate(field.label.clone(), e.get_language());
        let label = if label.is_empty() {
            field.label.clone()
        } else {
            label
        };
        let mut body = column().gap_1();
        if key == "cursorStyle" {
            let mut choices = row().flex_wrap();
            for (value, label, asset) in [
                (
                    "tahoe",
                    "Tahoe",
                    "legacy-electron/src/assets/cursors/tahoe/pointer-1__14-6.svg",
                ),
                (
                    "macos",
                    "macOS",
                    "legacy-electron/src/assets/cursors/macos/pointer-1__34-24.svg",
                ),
                (
                    "windows11",
                    "Windows",
                    "legacy-electron/src/assets/cursors/windows11/arrow__31-22.svg",
                ),
                ("dot", "Dot", "assets/icons/Record-fill.svg"),
                (
                    "figma",
                    "Minimal",
                    "legacy-electron/src/assets/cursors/custom/minimal-cursor.svg",
                ),
            ] {
                let editor = e.clone();
                let cursor = if value == "macos" {
                    img(macos_cursor_image())
                        .size(px(28.))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element()
                } else {
                    svg()
                        .path(asset)
                        .size(px(28.))
                        .text_color(t.text)
                        .into_any_element()
                };
                choices = choices.child(
                    choice_tile(value, field.value == value, true, t)
                        .w(px(70.))
                        .items_center()
                        .child(cursor)
                        .child(label)
                        .on_click(move |_, _, _| {
                            editor.defer_field("cursorStyle".into(), value.into())
                        }),
                );
            }
            body = body.child(label).child(choices);
            if field.choice >= 5 {
                body = body.child(format!("Current: {}", field.value));
            }
            return body.into_any_element();
        }
        if key == "webcam.positionPreset" {
            let mut choices = row().flex_wrap();
            for (i, value) in [
                "top-left",
                "top-center",
                "top-right",
                "center-left",
                "center",
                "center-right",
                "bottom-left",
                "bottom-center",
                "bottom-right",
            ]
            .iter()
            .enumerate()
            {
                let editor = e.clone();
                let value = *value;
                choices = choices.child(
                    choice_tile(value, field.value == value, true, t)
                        .w(px(70.))
                        .h(px(38.))
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .left(px(8. + (i % 3) as f32 * 22.))
                                .top(px(5. + (i / 3) as f32 * 9.))
                                .size(px(10.))
                                .rounded_sm()
                                .bg(if field.value == value {
                                    t.accent
                                } else {
                                    t.muted
                                }),
                        )
                        .tooltip(move |_, cx| tooltip(format!("Webcam position {value}"), t, cx))
                        .on_click(move |_, _, _| {
                            editor.defer_field("webcam.positionPreset".into(), value.into())
                        }),
                );
            }
            let editor = e.clone();
            return body
                .child(label)
                .child(choices)
                .child(
                    button("custom-position", "Custom position", t)
                        .selected(field.value == "custom")
                        .on_click(move |_, _, _| {
                            editor.defer_field("webcam.positionPreset".into(), "custom".into())
                        }),
                )
                .into_any_element();
        }
        match field.kind {
            5 => {
                // A small muted caption with a rule running out to the edge.
                return section_label(label, t).mt(px(Theme::GAP_SMALL)).into_any_element();
            }
            3 => {
                // An inspector action is a row in a stacked picker, not a
                // toolbar control, so it carries the recessed field plate.
                return self
                    .action(SharedString::from(id), field.value, &key, true)
                    
                    .into_any_element();
            }
            2 => {
                let e = e.clone();
                return toggle(
                    SharedString::from(id),
                    label,
                    field.value == "true",
                    true,
                    t,
                    move |v, _, _| e.defer_field(key.clone(), v.to_string()),
                )
                .into_any_element();
            }
            4 => {
                body = body.child(label);
                // The model remains authoritative, including custom cursor/position choices.
                let e = e.clone();
                let values: Vec<_> = field.values.iter().collect();
                let control = self.dropdown(
                    &id,
                    field.choices.iter().collect(),
                    field.choice,
                    true,
                    cx,
                    move |i, _, _| {
                        if let Some(v) = values.get(i) {
                            e.defer_field(key.clone(), v.clone());
                        }
                    },
                );
                body = body.child(control);
            }
            1 => {
                let e1 = e.clone();
                let k1 = key.clone();
                let min = field.minimum;
                let max = field.maximum;
                let _ = (&e1, &k1);
                let e2 = e.clone();
                // One control, not three: the plate carries glyph, caption,
                // level and value together (the product's "unified control
                // geometry").
                let scrub = self.slider(
                    &id,
                    min,
                    max,
                    field.value.parse().unwrap_or(min),
                    (&label, field_glyph(&key)),
                    cx,
                    move |v, commit, _, _| {
                        if commit {
                            e2.defer_field(key.clone(), v.to_string());
                        }
                    },
                );
                body = body.child(scrub);
            }
            _ => {
                let e = e.clone();
                let input = self.input(&id, &field.value, window, cx, move |v, _, _| {
                    e.defer_field(key.clone(), v)
                });
                body = body.child(label).child(input);
            }
        }
        body.into_any_element()
    }

    fn inspector(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = self.theme;
        let name = e.get_panel();
        let title = match name.as_str() {
            "Frame" => "Scene",
            "Preferences" => "Settings",
            "Recent" => "Projects",
            "Wallpapers" => "Background",
            _ => &name,
        };
        let mut heading = row();
        if matches!(
            name.as_str(),
            "Crop" | "Wallpapers" | "Presets" | "Shortcuts"
        ) {
            heading = heading.child(self.panel_button(
                e,
                "‹",
                if name == "Shortcuts" {
                    "Preferences"
                } else {
                    "Frame"
                },
            ));
        }
        heading = heading.child(
            div()
                .flex_1()
                .text_size(px(Theme::FONT_HEADING))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(title.to_owned()),
        );
        let mut content = column().gap_3();
        if name == "Frame" || name == "Wallpapers" {
            // Scene / Background is one segmented control, not two buttons.
            let editor = e.clone();
            let surface = self.surface.clone();
            content = content.child(segmented_control(
                "scene-background",
                &["Scene", "Background"],
                if name == "Wallpapers" { 1 } else { 0 },
                t,
                move |index, _, _| {
                    if index == 0 {
                        editor.set_panel("Frame".into());
                        editor.defer_panel("Frame".into());
                    } else {
                        surface.action("wallpapers");
                    }
                },
            ));
        }
        if name == "Recent" {
            content = content
                .child(
                    self.action(
                        "storyboard",
                        "Create video · spike",
                        "storyboard-spike",
                        true,
                    )
                    .primary(),
                )
                .child(self.action("import", "Import video or project", "open", true));
        }
        if name == "Selection" && e.get_selected_id().is_empty() {
            content = content.child("Select a clip or effect in the timeline to edit it.");
        }
        if name == "Wallpapers" {
            let mut wallpapers = row().flex_wrap();
            for tile in e.get_wallpapers().iter() {
                let editor = e.clone();
                let key = tile.key.clone();
                let mut item = media_tile(
                    SharedString::from(format!("wallpaper-{key}")),
                    tile.title,
                );
                if let Some(image) = tile.source.0 {
                    item = item.child(
                        img(image)
                            .w_full()
                            .h(px(Theme::TILE_HEIGHT))
                            .object_fit(ObjectFit::Cover),
                    );
                }
                wallpapers = wallpapers
                    .child(item.on_click(move |_, _, _| editor.defer_action(key.clone())));
            }
            content = content
                .child("Choose a background")
                .child(wallpapers)
                .child(self.action(
                    "upload-background",
                    "Upload image or video",
                    "choose-background",
                    true,
                ))
                .child("Color");
            let mut swatches = row();
            for value in [
                "#17171c", "#22364a", "#253c32", "#54324a", "#784832", "#ededed",
            ] {
                let editor = e.clone();
                swatches = swatches.child(
                    swatch(
                        value,
                        rgb(u32::from_str_radix(&value[1..], 16).unwrap()).into(),
                        e.get_background_value() == value,
                        t,
                    )
                    .on_click(move |_, _, _| {
                        editor.defer_field("wallpaper".into(), value.into())
                    }),
                );
            }
            let editor = e.clone();
            let input = self.input(
                "wallpaper-color",
                &e.get_background_value(),
                window,
                cx,
                move |v, _, _| editor.defer_field("wallpaper".into(), v),
            );
            content = content
                .child(swatches)
                .child("Background color or gradient")
                .child(input);
        }
        if name == "Preferences" {
            let mut appearances = row();
            for (label, value) in [("Light", "light"), ("Dark", "dark"), ("System", "system")] {
                let editor = e.clone();
                appearances = appearances.child(
                    button(value, label, t)
                        .selected(e.get_appearance() == value)
                        .on_click(move |_, _, _| {
                            editor.defer_field("prefs.appearance".into(), value.into())
                        }),
                );
            }
            let e1 = e.clone();
            let e2 = e.clone();
            content = content
                .child("Appearance")
                .child(appearances)
                .child(toggle(
                    "auto-zooms",
                    "Automatic recording zooms",
                    e.get_auto_apply_zooms(),
                    true,
                    t,
                    move |v, _, _| e1.defer_field("prefs.auto_apply_zooms".into(), v.to_string()),
                ))
                .child(
                    div()
                        .text_color(t.muted)
                        .child("Suggest zooms when a new recording opens."),
                )
                .child(toggle(
                    "connect-zooms",
                    "Connect zooms",
                    e.get_connect_zooms(),
                    e.get_has_video(),
                    t,
                    move |v, _, _| e2.defer_field("connectZooms".into(), v.to_string()),
                ))
                .child(
                    div()
                        .text_color(t.muted)
                        .child("Join nearby zooms into a continuous camera move."),
                );
        }
        if name == "Presets" {
            let mut looks = row();
            for (label, value) in [
                ("Studio", "studio"),
                ("Minimal", "minimal"),
                ("Bold", "bold"),
            ] {
                looks = looks.child(
                    self.action(value, label, &format!("look-{value}"), e.get_has_video())
                        .selected(e.get_look_choice() == value),
                );
            }
            content = content.child("Choose a look").child(looks);
        }
        if matches!(name.as_str(), "Cursor" | "Preferences" | "Presets") {
            content = content.child("Motion presets").child(
                row()
                    .child(
                        self.action("focused", "Focused", "motion-focused", e.get_has_video())
                            .selected(e.get_motion_choice() == "focused"),
                    )
                    .child(
                        self.action("smooth", "Smooth", "motion-smooth", e.get_has_video())
                            .selected(e.get_motion_choice() == "smooth"),
                    ),
            );
            if !e.get_has_video() {
                content = content.child(
                    div()
                        .text_color(t.muted)
                        .child("Open a project to adjust its motion."),
                );
            }
        }
        if name == "Recording" {
            let editor = e.clone();
            let source = self.dropdown(
                "editor-source",
                e.get_source_names().iter().collect(),
                e.get_source_index(),
                !e.get_busy(),
                cx,
                move |i, _, _| editor.set_source_index(i as i32),
            );
            let editor = e.clone();
            let camera = self.dropdown(
                "editor-camera",
                e.get_camera_names().iter().collect(),
                e.get_camera_index(),
                e.get_capture_camera(),
                cx,
                move |i, _, _| editor.set_camera_index(i as i32),
            );
            let editor = e.clone();
            let microphone = self.dropdown(
                "editor-microphone",
                e.get_microphone_names().iter().collect(),
                e.get_microphone_index(),
                e.get_capture_mic(),
                cx,
                move |i, _, _| editor.set_microphone_index(i as i32),
            );
            let e1 = e.clone();
            let e2 = e.clone();
            let e3 = e.clone();
            content = content
                .child("Capture source")
                .child(source)
                .child(self.action(
                    "sources",
                    if e.get_sources_loading() {
                        "Finding sources…"
                    } else {
                        "Refresh displays and windows"
                    },
                    "sources",
                    !e.get_busy(),
                ))
                .child(toggle(
                    "capture-camera",
                    "Camera",
                    e.get_capture_camera(),
                    true,
                    t,
                    move |v, _, _| e1.set_capture_camera(v),
                ))
                .child(camera)
                .child(toggle(
                    "capture-mic",
                    "Microphone",
                    e.get_capture_mic(),
                    true,
                    t,
                    move |v, _, _| e2.set_capture_mic(v),
                ))
                .child(microphone)
                .child(toggle(
                    "capture-system",
                    "System audio",
                    e.get_capture_system(),
                    true,
                    t,
                    move |v, _, _| e3.set_capture_system(v),
                ));
        }
        for field in e.get_fields().iter() {
            content = content.child(self.field(e, field, window, cx));
        }
        // Only these panels pin an action strip under the scroll region. An
        // always-present empty column still cost the panel's gap plus its
        // bottom padding, which is what left the dead band under the last row.
        let has_footer = matches!(
            name.as_str(),
            "Preferences" | "Recording" | "Selection" | "Export" | "Captions"
        );
        let mut footer = column();
        match name.as_str() {
            "Preferences" => {
                footer =
                    footer.child(self.panel_button(e, "Customize keyboard shortcuts…", "Shortcuts"))
            }
            "Recording" => {
                footer = footer.child(e.get_recording_hint()).child(
                    self.action(
                        "start-recording",
                        if e.get_recording() {
                            "Stop recording"
                        } else {
                            "Start recording"
                        },
                        if e.get_recording() {
                            "stop-recording"
                        } else {
                            "start-recording"
                        },
                        !e.get_busy()
                            && (e.get_recording() || e.get_source_names().row_count() > 0),
                    )
                    .primary(),
                )
            }
            "Selection" => {
                footer = footer.child(self.action(
                    "delete-region",
                    "Delete selected region",
                    "delete",
                    !e.get_selected_id().is_empty(),
                ))
            }
            "Export" => {
                footer = footer.child(
                    self.action(
                        "export-video",
                        "Export video",
                        "export",
                        e.get_has_video() && !e.get_busy(),
                    )
                    .primary(),
                )
            }
            "Captions" => {
                footer = footer.child(
                    row()
                        .child(self.action("import-srt", "Import SRT", "import-captions", true))
                        .child(
                            self.action("transcribe", "Transcribe", "transcribe", true)
                                .primary(),
                        ),
                )
            }
            _ => {}
        }
        // The scroll region runs to the panel's inner top and bottom edges so
        // the fade ramp sits exactly on the clip line; the panel's vertical
        // padding moves onto the heading and footer instead of stacking with
        // the band and pushing the first row down.
        let mut el = panel(t)
            .py_0()
            .w(px(PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .child(heading.pt(px(Theme::GAP_LARGE)))
            .child(fade_edges(
                div()
                    .id(SharedString::from(format!("inspector-{name}")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    // A band's worth of padding: at rest the ramp lands here,
                    // so a panel that fits is never dimmed, and one that
                    // overflows dissolves instead of slicing a row in half.
                    .py(px(FADE_BAND))
                    .child(content),
            ));
        if has_footer {
            el = el.child(footer.pb(px(Theme::GAP_LARGE)));
        }
        el.into_any_element()
    }

    fn menu_button(&self, name: &'static str, cx: &mut Context<Self>) -> Button {
        button(name, name, self.theme)
            .selected(self.menu.as_deref() == Some(name))
            .on_click(cx.listener(move |s, _, _, cx| {
                s.menu = if s.menu.as_deref() == Some(name) {
                    None
                } else {
                    Some(name.into())
                };
                cx.notify();
            }))
    }

    fn menu_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(name) = self.menu.as_deref() else {
            return div().into_any_element();
        };
        let commands: &[(&str, &str)] = match name {
            "File" => &[
                ("Open…", "open"),
                ("Save", "save"),
                ("Save As…", "save-as"),
                ("Export…", "@Export"),
            ],
            "Edit" => &[
                ("Undo", "undo"),
                ("Redo", "redo"),
                ("Add marker", "add-marker"),
                ("Previous marker", "previous-marker"),
                ("Next marker", "next-marker"),
                ("Split clip at playhead", "split-clip"),
                ("Select all regions", "select-all"),
                ("Next overlapping annotation", "next-annotation"),
                ("Previous overlapping annotation", "previous-annotation"),
                ("Copy region", "copy"),
                ("Cut region", "cut"),
                ("Paste region", "paste"),
                ("Duplicate region", "duplicate"),
                ("Delete region", "delete"),
            ],
            "Add" => &[
                ("Text", "add-text"),
                ("Image", "add-image"),
                ("Arrow", "add-figure"),
                ("Blur", "add-blur"),
                ("Audio", "add-audio"),
                ("Caption", "add-caption"),
                ("Trim", "add-trim"),
                ("Speed", "add-speed"),
                ("Marker", "add-marker"),
            ],
            _ => &[
                ("Keyboard shortcuts", "shortcut-reference"),
                ("Feedback and issues", "feedback"),
            ],
        };
        deferred(frosted(
            Theme::RADIUS_PANEL,
            MENU_BLUR,
            panel_variant(self.theme, UiSurface::Popup).p(px(Theme::GAP_SMALL))
                .id("command-menu")
                .absolute()
                .top(px(52.))
                .left(px(100.))
                .w(px(260.))
                .max_h(px(540.))
                .overflow_y_scroll()
                .shadow_lg()
                .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                    s.menu = None;
                    cx.notify();
                }))
                .children(commands.iter().map(|(label, command)| {
                    let command = command.to_string();
                    button(SharedString::from(command.clone()), *label, self.theme).on_click(
                        cx.listener(move |s, _, _, cx| {
                            if let Some(panel) = command.strip_prefix('@') {
                                if let Surface::Editor(e) = &s.surface {
                                    e.set_panel(panel.into());
                                    e.defer_panel(panel.into());
                                }
                            } else {
                                s.surface.action(&command);
                            }
                            s.menu = None;
                            cx.notify();
                        }),
                    )
                })),
        ))
        .with_priority(30)
        .into_any_element()
    }

    fn preview(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = self.theme;
        self.sync_preview_context(e);
        if !e.get_has_video() {
            return empty_state(t, "No Video Loaded", "Open a video or start a recording")
                .child(
                    row()
                        .child(self.action("open-video", "Open video", "open", !e.get_busy()))
                        .child(
                            self.action(
                                "new-recording",
                                "New recording",
                                "record",
                                !e.get_busy() && !e.get_recording(),
                            )
                            .primary(),
                        ),
                )
                .into_any_element();
        }
        let viewport = self.preview_viewport.get();
        let available_w = if viewport.size.width > px(0.) {
            f32::from(viewport.size.width) - 16.
        } else {
            (f32::from(window.viewport_size().width) - PANEL_WIDTH - 120.).max(100.)
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
                    .border_color(t.accent)
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
                            .bg(t.on_accent)
                            .border_1()
                            .border_color(t.accent)
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
        column()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                row()
                    .justify_center()
                    .child(div().w(px(108.)).flex_shrink_0().child(aspect_control))
                    .child(self.action("crop", "Crop", "visual-crop", true))
                    .child(
                        button(
                            "fit-preview",
                            format!("Fit · {}%", (e.get_preview_zoom() * 100.).round()),
                            t,
                        )
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
            )
            .child(
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
                        if let Surface::Editor(e) = &s.surface {
                            if e.get_preview_zoom() > 1. {
                                s.preview_pan.x += delta.x;
                                s.preview_pan.y += delta.y;
                                let image = s.preview_bounds.get().size;
                                s.clamp_preview_pan(
                                    f32::from(image.width),
                                    f32::from(image.height),
                                );
                            }
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
                    ),
            )
            // Transport: the time on the left, a centred icon cluster with a
            // filled play plate, and the audio panel on the right.
            .child(
                row()
                    .h(px(Theme::CONTROL_HEIGHT))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(t.muted)
                            .child(e.get_time_label()),
                    )
                    .child(
                        row()
                            .gap(px(Theme::GAP_SMALL))
                            .flex_none()
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
                                    if e.get_playing() {
                                        "Pause-fill"
                                    } else {
                                        "Play-fill"
                                    },
                                    if e.get_playing() { "Pause" } else { "Play" },
                                    t,
                                )
                                .round()
                                .glyph_size(Theme::ICON_SIZE_LARGE)
                                .variant(ButtonVariant::Primary)
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
                    .child(
                        div().flex_1().flex().justify_end().child(
                            self.panel_button(e, "Audio", "Audio")
                                .glyph("SpeakerHigh-regular")
                                .icon_only()
                                .ghost(),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn seek_at(&self, x: Pixels) {
        if let Surface::Editor(e) = &self.surface {
            let b = self.timeline_bounds.get();
            let fraction = f32::from(x - b.left()) / f32::from(b.size.width).max(1.);
            e.invoke_seek(
                (e.get_timeline_offset() + fraction * e.get_timeline_visible())
                    .clamp(0., e.get_duration()),
            );
        }
    }

    fn move_gesture(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn end_gesture(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn timeline(&mut self, e: &EditorWindow, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let visible = e.get_timeline_visible();
        let offset = e.get_timeline_offset();
        let editor = e.clone();
        let zoom = self.slider(
            "timeline-zoom",
            1.,
            100.,
            e.get_timeline_zoom(),
            ("Zoom", "MagnifyingGlassPlus-regular"),
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
        let editor = e.clone();
        let position = self.slider(
            "timeline-position",
            0.,
            (e.get_duration() - visible).max(0.001),
            offset,
            ("Position", "ArrowsOutSimple-regular"),
            cx,
            move |v, _, _, _| editor.set_timeline_offset(v),
        );
        let editor = e.clone();
        let editor_out = e.clone();
        let editor_in = e.clone();
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
                button("auto-zoom", "Suggest zooms", t)
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
            .child(self.menu_button("Add", cx).glyph("Plus-regular"))
            .child(div().flex_1())
            // Snap keeps the accent plate while engaged; the zoom cluster is
            // icon-only so the strip stays quiet.
            .child(
                icon_button("snap", "Magnet-regular", "Snap", t)
                    .ghost()
                    .selected(e.get_snap())
                    .on_click(move |_, _, _| editor.set_snap(!editor.get_snap())),
            )
            .child(
                icon_button("fit-timeline", "ArrowsOutSimple-regular", "Fit timeline", t)
                    .ghost()
                    .on_click(cx.listener(|s, _, _, _| {
                        if let Surface::Editor(e) = &s.surface {
                            e.set_timeline_zoom(1.);
                            e.set_timeline_offset(0.);
                        }
                    })),
            )
            .child(
                icon_button("zoom-out", "MagnifyingGlassMinus-regular", "Zoom out", t)
                    .ghost()
                    .on_click(move |_, _, _| {
                        editor_out.set_timeline_zoom((editor_out.get_timeline_zoom() / 1.5).max(1.))
                    }),
            )
            .child(
                icon_button("zoom-in", "MagnifyingGlassPlus-regular", "Zoom in", t)
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
                    .text_color(t.muted)
                    .child(format!("{:.1}s", offset + visible * i as f32 / 8.)),
            );
        }
        let mut source = div()
            .relative()
            .h(px(56.))
            .overflow_hidden()
            .rounded_lg()
            .bg(t.surface)
            .child(div().px_2().child(e.get_document_title()));
        if let Some(image) = e.get_thumbnails().0 {
            source = source.child(
                img(image)
                    .absolute()
                    .top(px(22.))
                    .left(relative(-offset / visible))
                    .w(relative(e.get_timeline_zoom()))
                    .h(px(34.))
                    .object_fit(ObjectFit::Fill),
            );
        }
        let labels: Vec<String> = e.get_track_labels().iter().collect();
        let mut tracks = div().relative().h(px(labels.len() as f32 * TRACK_HEIGHT));
        for i in 0..labels.len() {
            tracks = tracks.child(
                div()
                    .absolute()
                    .top(px(i as f32 * TRACK_HEIGHT))
                    .w_full()
                    .h(px(38.))
                    .rounded_lg()
                    .bg(t.surface),
            );
        }
        if let Some(image) = e.get_waveform().0 {
            tracks = tracks.child(
                img(image)
                    .absolute()
                    .top(px(e.get_audio_row() as f32 * TRACK_HEIGHT))
                    .left(relative(-offset / visible))
                    .w(relative(e.get_timeline_zoom()))
                    .h(px(38.))
                    .opacity(0.3)
                    .object_fit(ObjectFit::Fill),
            );
        }
        for region in e.get_regions().iter() {
            let (mut start, mut end) = (region.start, region.end);
            if let Some(Gesture::Region {
                region: dragged,
                delta,
                mode,
                ..
            }) = &self.gesture
            {
                if dragged.id == region.id && dragged.kind == region.kind {
                    if *mode != 1 {
                        start += delta;
                    }
                    if *mode != 2 {
                        end += delta;
                    }
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
                .border_color(if region.selected { tint } else { t.border })
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
                    if let Surface::Editor(e) = &s.surface {
                        e.invoke_select_region(
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
                        if let Surface::Editor(e) = &s.surface {
                            e.invoke_select_region(
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
        let playhead = (e.get_playhead() - offset) / visible;
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
                    .child(timeline_scrubber(t, 40.0)),
            );
        }
        panel(t)
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
                            .child(div().h(px(88.)).pt_8().text_color(t.muted).child("Source"))
                            .children(labels.into_iter().map(|label| {
                                div()
                                    .h(px(TRACK_HEIGHT))
                                    .pt_3()
                                    .text_size(px(Theme::FONT_SMALL))
                                    .text_color(t.muted)
                                    .child(label)
                            })),
                    )
                    .child(timeline),
            ))
            .child(div().ml(px(82.)).child(position))
            .on_scroll_wheel(cx.listener(|s, event: &ScrollWheelEvent, _, cx| {
                if let Surface::Editor(e) = &s.surface {
                    let delta = event.delta.pixel_delta(px(20.));
                    if event.modifiers.control || event.modifiers.platform {
                        let b = s.timeline_bounds.get();
                        let fraction = (f32::from(event.position.x - b.left())
                            / f32::from(b.size.width).max(1.))
                        .clamp(0., 1.);
                        let anchor = e.get_timeline_offset() + fraction * e.get_timeline_visible();
                        e.set_timeline_zoom(
                            (e.get_timeline_zoom() * (-f32::from(delta.y) * 0.01).exp())
                                .clamp(1., 100.),
                        );
                        e.set_timeline_offset(
                            (anchor - fraction * e.get_timeline_visible())
                                .clamp(0., (e.get_duration() - e.get_timeline_visible()).max(0.)),
                        );
                        cx.stop_propagation();
                    } else if delta.x != px(0.) || event.modifiers.shift {
                        let dx = if event.modifiers.shift {
                            delta.y
                        } else {
                            delta.x
                        };
                        e.set_timeline_offset(
                            (e.get_timeline_offset()
                                - f32::from(dx)
                                    / f32::from(s.timeline_bounds.get().size.width).max(1.)
                                    * e.get_timeline_visible())
                            .clamp(0., (e.get_duration() - e.get_timeline_visible()).max(0.)),
                        );
                        cx.stop_propagation();
                    }
                }
            }))
            .into_any_element()
    }

    fn editor(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = self.theme;
        // The unified window titlebar: a translucent strip tall enough for
        // the 40px icon cluster, on the `header` tone so the vibrancy
        // material reads through it.
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .h(px(TITLEBAR_HEIGHT))
            .px(px(Theme::GAP_LARGE))
            .flex_shrink_0()
            .bg(t.header);
        if e.get_mac_titlebar() {
            // Traffic lights sit at {14,15}; the cluster starts at 88px.
            header = header.pl(px(88.));
        }
        header = header
            .child(self.brand())
            .child(self.icon_action("open", "FolderOpen-regular", "Open projects", "Recent", true))
            .child(self.icon_action("save", "FloppyDisk-regular", "Save", "save", e.get_has_video()))
            .child(self.icon_action(
                "undo",
                "ArrowCounterClockwise-regular",
                "Undo",
                "undo",
                e.get_can_undo(),
            ))
            .child(self.icon_action(
                "redo",
                "ArrowClockwise-regular",
                "Redo",
                "redo",
                e.get_can_redo(),
            ))
            // Document title, with the unsaved dot as a coloured mark rather
            // than a bullet in the string.
            .child(
                div()
                    .id("title-drag")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(Theme::GAP_SMALL))
                    .when(e.get_dirty(), |el| el.child(status_dot(t)))
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(Theme::FONT_BODY))
                            .text_color(t.text)
                            .child(e.get_document_title()),
                    )
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            )
            .child(
                button(
                    "record",
                    self.translate(e, if e.get_recording() { "Stop" } else { "Record" }),
                    t,
                )
                .glyph(if e.get_recording() {
                    "Stop-fill"
                } else {
                    "Record-regular"
                })
                .ghost()
                .enabled(!e.get_busy())
                .on_click(self.command(if e.get_recording() {
                    "stop-recording"
                } else {
                    "record"
                })),
            );
        if e.get_recording() {
            header = header.child(
                button(
                    "pause-recording",
                    if e.get_recording_paused() {
                        "Resume"
                    } else {
                        "Pause"
                    },
                    t,
                )
                .glyph("Pause-regular")
                .ghost()
                .on_click(self.command("pause-recording")),
            );
        }
        header = header
            .child(
                self.panel_button(e, "Presets", "Presets")
                    .glyph("Stack-regular")
                    .ghost(),
            )
            .child(
                self.panel_button(e, "Export", "Export")
                    .glyph("Export-regular")
                    .primary()
                    .enabled(e.get_has_video() && !e.get_busy()),
            );
        let mut rail = div()
            .id("rail")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w(px(RAIL_WIDTH))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .py(px(FADE_BAND))
            .overflow_y_scroll();
        // Icon over caption; the active panel keeps the accent plate.
        for (label, name, glyph) in [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Webcam", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ] {
            rail = rail.child(self.rail_panel_button(e, label, name, glyph));
        }
        rail = rail
            .child(div().flex_1())
            .child(self.rail_panel_button(e, "Settings", "Preferences", "Gear-regular"))
            .child(self.rail_panel_button(e, "Help", "shortcut-reference", "Question-regular"));
        let preview = self.preview(e, window, cx);
        let inspector = self.inspector(e, window, cx);
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .gap_0()
            .child(header)
            .child(
                div()
                    .flex()
                    .items_start()
                    .flex_1()
                    .min_h_0()
                    .p(px(Theme::GAP))
                    .gap(px(Theme::GAP))
                    .child(fade_edges(rail))
                    .child(preview)
                    .child(inspector),
            );
        if e.get_has_video() {
            root = root.child(self.timeline(e, cx));
        }
        // Reserved status strip under the content outlet (reference
        // `Theme::CONTROL_HEIGHT`): reserving it keeps the timeline from
        // shifting when a status line appears.
        let mut status = div()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(Theme::GAP_SMALL))
            .min_h(px(Theme::CONTROL_HEIGHT))
            .px(px(Theme::GAP_LARGE))
            .text_size(px(Theme::FONT_SMALL));
        let mut status_line = row()
            .text_color(t.muted)
            .child(div().flex_1().text_ellipsis().child(e.get_status()));
        if e.get_busy() {
            status_line = status_line.child(self.action("cancel", "Cancel", "cancel", true));
        }
        status = status.child(status_line);
        if e.get_busy() {
            status = status.child(progress_bar(e.get_progress(), t));
        }
        root.child(status)
            .child(self.menu_overlay(cx))
            .into_any_element()
    }

    fn brand(&self) -> impl IntoElement {
        svg()
            .path("assets/branding/menu-bar.svg")
            .size(px(Theme::ICON_SIZE_LARGE))
            .flex_none()
            .text_color(self.theme.text)
    }

    fn launcher(&self, s: &RecordingLauncher) -> AnyElement {
        let t = self.theme;
        let mut bar = row()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .p(px(Theme::GAP))
            .gap(px(Theme::GAP_SMALL))
            .rounded(px(Theme::RADIUS_PANEL))
            .bg(t.panel)
            .border_1()
            .border_color(t.border)
            .child(
                div()
                    .id("launcher-drag")
                    .cursor(CursorStyle::ClosedHand)
                    .child("⠿")
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            );
        if !s.get_recording() && !s.get_busy() {
            bar = bar.child(self.brand());
            for (id, label, selected) in [
                (
                    "sources",
                    s.get_source_names()
                        .row_data(s.get_source_index().max(0) as usize)
                        .unwrap_or_else(|| "Choose a source".into()),
                    s.get_panel() == "sources",
                ),
                (
                    "audio",
                    "Audio".into(),
                    s.get_microphone() || s.get_system_audio(),
                ),
                ("camera", "Webcam".into(), s.get_camera()),
                (
                    "countdown",
                    format!("{}s", s.get_countdown()),
                    s.get_countdown() > 0,
                ),
                ("more", "More".into(), s.get_panel() == "more"),
            ] {
                let launcher = s.clone();
                let control = button(id, label, t)
                    .selected(selected)
                    .on_click(move |_, _, _| {
                        let value = if launcher.get_panel() == id { "" } else { id };
                        launcher.set_panel(value.into());
                        launcher.defer_panel(value.into());
                    });
                // The source name is the only variable-width control, so it
                // takes the slack and ellipsizes; a fixed width pushed the
                // rest of the bar past the window on long display names.
                bar = bar.child(if id == "sources" {
                    control.stretch()
                } else {
                    control
                });
            }
            let launcher = s.clone();
            bar = bar.child(
                button("record", "Record", t)
                    .primary()
                    .on_click(move |_, _, _| {
                        if launcher.get_source_names().row_count() == 0 {
                            launcher.set_panel("sources".into());
                            launcher.defer_panel("sources".into());
                            launcher.defer_action("sources".into());
                        } else {
                            launcher.defer_action("start-recording".into());
                        }
                    }),
            );
        } else {
            bar = bar.child(div().flex_1().child(if s.get_recording() {
                format!(
                    "{}  {}",
                    if s.get_paused() { "PAUSED" } else { "REC" },
                    s.get_elapsed()
                )
            } else {
                s.get_status()
            }));
            if s.get_recording() {
                bar = bar
                    .child(self.action(
                        "pause",
                        if s.get_paused() { "Resume" } else { "Pause" },
                        "pause-recording",
                        !s.get_busy(),
                    ))
                    .child(self.action("stop", "Stop", "stop-recording", !s.get_busy()));
            }
            if s.get_cancellable() {
                bar = bar.child(self.action("cancel", "Cancel", "cancel", true));
            }
        }
        let hint = s.get_status();
        bar = bar
            .child(
                div()
                    .id("recorder-status")
                    .child("?")
                    .tooltip(move |_, cx| tooltip(hint.clone(), t, cx)),
            )
            .child(self.action("close", "×", "hide-launcher", true));
        div()
            .size_full()
            .px_4()
            .py_2()
            .child(bar)
            .into_any_element()
    }

    fn options(&mut self, s: &RecordingOptions, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let name = s.get_panel();
        let title = match name.as_str() {
            "sources" => "Screens and windows",
            "audio" => "Microphone & system audio",
            "camera" => "Webcam",
            "countdown" => "Countdown delay",
            _ => "More",
        };
        let options = s.clone();
        let mut body = panel(t).size_full().p_5().gap_3().child(
            row()
                .child(
                    div()
                        .flex_1()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    button("close", "×", t).on_click(move |_, _, _| options.defer_panel("".into())),
                ),
        );
        match name.as_str() {
            "sources" => {
                let options = s.clone();
                let sources = self.dropdown(
                    "sources",
                    s.get_source_names().iter().collect(),
                    s.get_source_index(),
                    !s.get_busy(),
                    cx,
                    move |i, _, _| options.defer_option("source".into(), i.to_string()),
                );
                body = body
                    .child("Capture source")
                    .child(sources)
                    .child(
                        div()
                            .text_color(t.muted)
                            .child("Choose a display or a visible window to record."),
                    )
                    .child(self.action(
                        "refresh",
                        "Refresh displays and windows",
                        "sources",
                        !s.get_busy(),
                    ));
            }
            "audio" => {
                let options = s.clone();
                let microphone = self.dropdown(
                    "microphone",
                    s.get_microphone_names().iter().collect(),
                    s.get_microphone_index(),
                    !s.get_busy() && s.get_microphone(),
                    cx,
                    move |i, _, _| options.defer_option("microphone-device".into(), i.to_string()),
                );
                let s1 = s.clone();
                let s2 = s.clone();
                body = body
                    .child(toggle(
                        "mic-toggle",
                        "Microphone",
                        s.get_microphone(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| s1.defer_option("microphone".into(), v.to_string()),
                    ))
                    .child(microphone)
                    .child(toggle(
                        "system-toggle",
                        "System audio",
                        s.get_system_audio(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| s2.defer_option("system-audio".into(), v.to_string()),
                    ));
            }
            "camera" => {
                let options = s.clone();
                let camera = self.dropdown(
                    "camera",
                    s.get_camera_names().iter().collect(),
                    s.get_camera_index(),
                    !s.get_busy() && s.get_camera(),
                    cx,
                    move |i, _, _| options.defer_option("camera-device".into(), i.to_string()),
                );
                let options = s.clone();
                body = body
                    .child(toggle(
                        "camera-toggle",
                        "Webcam overlay",
                        s.get_camera(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| options.defer_option("camera".into(), v.to_string()),
                    ))
                    .child(camera)
                    .child(
                        div()
                            .text_color(t.muted)
                            .child("Your camera is recorded separately and added to the project."),
                    );
            }
            "countdown" => {
                let mut choices = row().flex_wrap();
                for (label, value) in [
                    ("No delay", 0),
                    ("3 seconds", 3),
                    ("5 seconds", 5),
                    ("10 seconds", 10),
                ] {
                    let options = s.clone();
                    choices = choices.child(
                        button(label, label, t)
                            .selected(s.get_countdown() == value)
                            .on_click(move |_, _, _| {
                                options.defer_option("countdown".into(), value.to_string())
                            }),
                    );
                }
                body = body
                    .child("Give yourself a moment before recording starts.")
                    .child(choices);
            }
            _ => {
                body = body
                    .child(self.action(
                        "storyboard",
                        "Create video · spike",
                        "storyboard-spike",
                        true,
                    ))
                    .child(
                        row()
                            .child(self.action("open", "Open video or project", "open", true))
                            .child(self.action("projects", "Projects", "projects", true))
                            .child(self.action(
                                "editor",
                                "Back to editor",
                                "show-editor",
                                s.get_has_project(),
                            )),
                    )
                    .child(div().text_color(t.muted).child("Recordings path"))
                    .child(
                        row()
                            .child(div().flex_1().text_ellipsis().child(s.get_directory()))
                            .child(self.action(
                                "folder",
                                "Choose folder…",
                                "recording-folder",
                                true,
                            )),
                    );
            }
        }
        body.into_any_element()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::new(&self.surface.appearance(), window.appearance());
        // Interface text is authored in rems against the reference's 16px
        // baseline, so `px(11.0)` resolves to 11px here.
        
        // Keep frames coming while any hover wash is mid-fade.
        if subtake_ui::tick_hover_fades() {
            cx.notify();
        }
        let surface = self.surface.clone();
        let content = match &surface {
            Surface::Editor(e) => self.editor(e, window, cx),
            Surface::Launcher(s) => self.launcher(s),
            Surface::Options(s) => self.options(s, cx),
        };
        let editor_surface = matches!(surface, Surface::Editor(_));
        div()
            .id("subtake-root")
            .relative()
            .size_full()
            .track_focus(&self.focus)
            .tab_group()
            .tab_stop(false)
            .font_family(FONT_SANS)
            .text_size(px(Theme::FONT_BODY))
            .text_color(self.theme.text)
            // `bg` carries its own alpha: it is a tint over the window's
            // vibrancy material, not a paint.
            .when(editor_surface, |s| s.bg(self.theme.bg))
            .on_mouse_move(cx.listener(Self::move_gesture))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::end_gesture))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::end_gesture))
            .on_key_down(cx.listener(|s, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                // An active text input owns editing shortcuts and IME composition.
                if s.inputs
                    .values()
                    .any(|input| input.focus_handle(cx).is_focused(window))
                    || s.dropdowns
                        .values()
                        .any(|input| input.read(cx).is_focused(window))
                {
                    return;
                }
                if !s.focus.is_focused(window)
                    && matches!(event.keystroke.key.as_str(), "space" | "enter")
                {
                    return;
                }
                if event.keystroke.key == "escape" {
                    s.gesture = None;
                    s.menu = None;
                    cx.notify();
                }
                if let Surface::Editor(e) = &s.surface {
                    let e = e.clone();
                    let k = event.keystroke.clone();
                    crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                        let key = if k.key == "space" {
                            " ".to_owned()
                        } else {
                            k.key_char.unwrap_or(k.key)
                        };
                        e.invoke_keyboard(
                            key,
                            k.modifiers.control || k.modifiers.platform,
                            k.modifiers.shift,
                            k.modifiers.alt,
                        );
                    });
                    cx.stop_propagation();
                }
            }))
            .on_drop(cx.listener(|s, paths: &ExternalPaths, _, _| {
                if let Surface::Editor(e) = &s.surface {
                    for path in paths.paths() {
                        let e = e.clone();
                        let path = path.clone();
                        crate::ui_runtime::Timer::single_shot(
                            std::time::Duration::ZERO,
                            move || e.window().dispatch_drop(path),
                        );
                    }
                }
            }))
            .child(content)
    }
}
