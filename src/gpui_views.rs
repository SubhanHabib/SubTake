//! Native editor and recorder surfaces. Business commands remain in the UI facade.
use crate::ui_state::{EditorWindow, Field, RecordingLauncher, RecordingOptions, Region};
use base64::Engine;
use gpui::{prelude::*, *};
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Instant,
};
use subtake_theme::{FONT_SANS, PANEL_WIDTH, RAIL_WIDTH, TRACK_HEIGHT, Theme};

/// Title bar strip: tall enough to seat the 40px icon cluster with air.
const TITLEBAR_HEIGHT: f32 = 56.0;
/// The command palette card: wide enough for the longest Edit command
/// without wrapping, and capped so a long menu scrolls rather than filling
/// the window.
const PALETTE_WIDTH: f32 = 300.0;
const PALETTE_VISIBLE_ROWS: usize = 8;
/// Chip, input, footer and the card's own padding — everything in the card
/// that is not a command row.
const PALETTE_CHROME_HEIGHT: f32 =
    Theme::CHIP_HEIGHT + Theme::CONTROL_HEIGHT + Theme::FOOTER_HEIGHT + 4.0 * Theme::GAP_SMALL;

/// The command sets behind both the in-window palette and the native menu
/// bar, grouped so the menu bar can keep its separators. One table, because
/// the two used to carry verbatim copies of these lists that could drift.
///
/// A command starting with `@` opens that inspector panel instead of firing
/// an action; every consumer strips the prefix the same way.
pub fn menu_commands(name: &str) -> &'static [&'static [(&'static str, &'static str)]] {
    match name {
        "File" => &[&[
            ("Open…", "open"),
            ("Save", "save"),
            ("Save As…", "save-as"),
            ("Export…", "@Export"),
        ]],
        "Edit" => &[
            &[("Undo", "undo"), ("Redo", "redo")],
            &[
                ("Add marker", "add-marker"),
                ("Previous marker", "previous-marker"),
                ("Next marker", "next-marker"),
                ("Split clip at playhead", "split-clip"),
                ("Select all regions", "select-all"),
                ("Next overlapping annotation", "next-annotation"),
                ("Previous overlapping annotation", "previous-annotation"),
            ],
            &[
                ("Copy region", "copy"),
                ("Cut region", "cut"),
                ("Paste region", "paste"),
                ("Duplicate region", "duplicate"),
                ("Delete region", "delete"),
            ],
        ],
        "Add" => &[&[
            ("Zoom", "add-zoom"),
            ("Text", "add-text"),
            ("Arrow", "add-figure"),
            ("Blur", "add-blur"),
            ("Audio", "add-audio"),
            ("Caption", "add-caption"),
            ("Trim", "add-trim"),
            ("Speed", "add-speed"),
            ("Marker", "add-marker"),
        ]],
        _ => &[&[
            ("Keyboard shortcuts", "shortcut-reference"),
            ("Feedback and issues", "feedback"),
        ]],
    }
}


/// The glyph a numeric field wears in its scrub plate. Keyed on the field so
/// the inspector reads as a set of labelled dials rather than a list of rows.
/// How a slider's number should read. The model carries the raw value, so the
/// unit is presentation: a 0-1 factor reads as a percentage, a multiplier as
/// a cross, a length in points. Returned as (scale, suffix); anything not
/// listed keeps the bare number it has today.
fn field_unit(key: &str) -> (f32, &'static str) {
    match key.rsplit('.').next().unwrap_or(key) {
        "borderRadius" | "backgroundBlur" | "margin" => (1.0, " px"),
        "cursorSize" | "cursorSmoothing" | "cursorSway" | "cursorMotionBlur"
        | "cursorClickBounce" | "depth" | "speed" | "volume" => (1.0, "\u{00d7}"),
        // Roundness is already carried as 0-100, so it takes the suffix
        // without the rescale the other factors need.
        "roundness" => (1.0, "%"),
        "zoomSmoothness" | "shadowIntensity" | "shadow" | "cx" | "cy" => (100.0, "%"),
        _ => (1.0, ""),
    }
}

use subtake_ui::{
    Button, ButtonVariant, Dropdown, FADE_BAND, MENU_BLUR, Slider, Surface as UiSurface, TextInput,
    button, caps_label, choice_tile, column, composer_footer, context_chip, empty_state,
    fade_edges, frosted, group_card, icon, icon_button, measure, media_tile, menu_in,
    menu_list, menu_row, menu_surface, panel, panel_variant, progress_bar, rail_button, row,
    section_label, segmented_control, setting_card, status_dot, swatch, switch, tile_grid,
    timeline_scrubber, toggle, tooltip,
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
    /// Which window this is, for a perf timeline line.
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Editor(_) => "editor",
            Self::Launcher(_) => "recorder",
            Self::Options(_) => "options",
        }
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
    /// When this view last rendered; only read while `SUBTAKE_PERF` is set.
    last_render: Option<Instant>,
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
    /// Where the palette's trigger sits, so the card opens against it
    /// instead of at a fixed window coordinate.
    menu_anchor: Rc<Cell<Bounds<Pixels>>>,
    menu_filter: String,
    /// Set when the palette opens so the next render hands it the keyboard.
    menu_focus: bool,
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
            last_render: None,
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
            menu_anchor: Rc::new(Cell::new(Bounds::default())),
            menu_filter: String::new(),
            menu_focus: false,
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
        unit: (f32, &str),
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
            s.set_unit(unit.0, unit.1.to_owned());
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
        let mut body = column().gap(px(Theme::GAP_SMALL));
        if key == "cursorStyle" {
            // Five even tiles, four across: the reference's pickers are grids,
            // and a wrap puts a ragged last row under an even first one.
            let mut choices = tile_grid(4);
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
                        .items_center()
                        .justify_center()
                        .h(px(Theme::TILE_HEIGHT + Theme::GAP_LARGE))
                        .child(cursor)
                        .tooltip(move |_, cx| tooltip(label, t, cx))
                        .on_click(move |_, _, _| {
                            editor.defer_field("cursorStyle".into(), value.into())
                        }),
                );
            }
            body = body.child(caps_label(label, t)).child(choices);
            if field.choice >= 5 {
                body = body.child(format!("Current: {}", field.value));
            }
            return body.into_any_element();
        }
        if key == "webcam.positionPreset" {
            // Nine cells, three across — the grid IS the picture of where the
            // webcam lands, which is why the marker stays a dot rather than an
            // arrow glyph the icon set does not carry.
            let mut choices = tile_grid(3);
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
                        .h(px(Theme::CONTROL_HEIGHT))
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .h_full()
                                .map(|el| match i % 3 {
                                    0 => el.justify_start(),
                                    1 => el.justify_center(),
                                    _ => el.justify_end(),
                                })
                                .map(|el| match i / 3 {
                                    0 => el.items_start(),
                                    1 => el.items_center(),
                                    _ => el.items_end(),
                                })
                                .child(
                                    div()
                                        .size(px(Theme::DOT_SIZE + 2.0))
                                        .rounded(px(Theme::RADIUS_SMALL / 2.0))
                                        .bg(if field.value == value {
                                            t.accent
                                        } else {
                                            t.muted
                                        }),
                                ),
                        )
                        .tooltip(move |_, cx| tooltip(format!("Webcam position {value}"), t, cx))
                        .on_click(move |_, _, _| {
                            editor.defer_field("webcam.positionPreset".into(), value.into())
                        }),
                );
            }
            let editor = e.clone();
            return body
                .child(caps_label(label, t))
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
                // One row, not two: a label stacked over its control reads
                // as a heading plus a thing, when it is one setting.
                body = body.child(
                    row()
                        .h(px(Theme::CONTROL_HEIGHT))
                        .child(
                            div()
                                .flex_none()
                                .max_w(px(PANEL_WIDTH / 3.0))
                                .text_ellipsis()
                                .text_color(t.text)
                                .child(label),
                        )
                        .child(div().flex_1().min_w_0().child(control)),
                );
            }
            1 => {
                let e1 = e.clone();
                let k1 = key.clone();
                let min = field.minimum;
                let max = field.maximum;
                let _ = (&e1, &k1);
                let e2 = e.clone();
                // One control, not three: the plate carries the caption, the
                // level and the value together (the product's "unified
                // control geometry"). No glyph -- a stack of these is a list
                // of settings, and a pictogram on every row reads as
                // decoration rather than as information.
                let scrub = self.slider(
                    &id,
                    min,
                    max,
                    field.value.parse().unwrap_or(min),
                    (&label, ""),
                    field_unit(&key),
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
                body = body.child(
                    row()
                        .h(px(Theme::CONTROL_HEIGHT))
                        .child(
                            div()
                                .flex_none()
                                .max_w(px(PANEL_WIDTH / 3.0))
                                .text_ellipsis()
                                .text_color(t.text)
                                .child(label),
                        )
                        .child(div().flex_1().min_w_0().child(input)),
                );
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
        // The panel names itself the way the reference does: small caps
        // rather than a heading that competes with the controls under it. A
        // sub-panel keeps its way back, now as a caret rather than a button
        // whose caption was a single guillemet character.
        let mut heading = row().h(px(Theme::CONTROL_HEIGHT)).flex_none();
        if matches!(
            name.as_str(),
            "Crop" | "Wallpapers" | "Presets" | "Shortcuts"
        ) {
            let editor = e.clone();
            let back = if name == "Shortcuts" {
                "Preferences"
            } else {
                "Frame"
            };
            heading = heading.child(
                icon_button("inspector-back", "CaretLeft-regular", "Back", t)
                    .ghost()
                    .on_click(move |_, _, _| {
                        editor.set_panel(back.into());
                        editor.defer_panel(back.into());
                    }),
            );
        }
        heading = heading.child(caps_label(title.to_owned(), t)).child(div().flex_1());
        let mut content = column().gap(px(Theme::GAP));
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
                .child(caps_label("Choose a background", t))
                .child(wallpapers)
                .child(self.action(
                    "upload-background",
                    "Upload image or video",
                    "choose-background",
                    true,
                ));
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
            content = content.child(
                group_card(t, "Background color or gradient")
                    .child(swatches)
                    .child(input),
            );
        }
        if name == "Preferences" {
            // Appearance is one choice of three, so it is one segmented
            // control rather than three buttons that happen to sit together.
            let editor = e.clone();
            let appearances = ["light", "dark", "system"];
            let chosen = appearances
                .iter()
                .position(|v| *v == e.get_appearance())
                .unwrap_or(2);
            let e1 = e.clone();
            let e2 = e.clone();
            content = content
                .child(caps_label("Appearance", t))
                .child(segmented_control(
                    "appearance",
                    &["Light", "Dark", "System"],
                    chosen,
                    t,
                    move |index, _, _| {
                        editor.defer_field(
                            "prefs.appearance".into(),
                            appearances[index].to_owned(),
                        )
                    },
                ))
                .child(caps_label("Zooms", t))
                // A setting and the sentence that explains it are one thing,
                // so they share one plate. Loose muted lines under a control
                // read as unattached commentary.
                .child(
                    setting_card(
                        t,
                        "Automatic recording zooms",
                        "Suggest zooms when a new recording opens.",
                    )
                    .child(switch(
                        "auto-zooms",
                        e.get_auto_apply_zooms(),
                        true,
                        t,
                        move |v, _, _| {
                            e1.defer_field("prefs.auto_apply_zooms".into(), v.to_string())
                        },
                    )),
                )
                .child(
                    setting_card(
                        t,
                        "Connect zooms",
                        "Join nearby zooms into a continuous camera move.",
                    )
                    .child(switch(
                        "connect-zooms",
                        e.get_connect_zooms(),
                        e.get_has_video(),
                        t,
                        move |v, _, _| e2.defer_field("connectZooms".into(), v.to_string()),
                    )),
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
            content = content.child(caps_label("Choose a look", t)).child(looks);
        }
        if matches!(name.as_str(), "Cursor" | "Preferences" | "Presets") {
            // A preset is a choice you make once and live with, so it states
            // what it does rather than making the name carry it alone.
            let mut presets = tile_grid(2);
            for (value, title, glyph, detail) in [
                (
                    "focused",
                    "Focused",
                    "Cursor-regular",
                    "Snappier motion for demos, walkthroughs and everyday recordings.",
                ),
                (
                    "smooth",
                    "Smooth",
                    "FilmStrip-regular",
                    "Gentler motion for presentations, keynote-style videos and polished reveals.",
                ),
            ] {
                let surface = self.surface.clone();
                let command = format!("motion-{value}");
                presets = presets.child(
                    choice_tile(value, e.get_motion_choice() == value, e.get_has_video(), t)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_none()
                                .size(px(Theme::CONTROL_HEIGHT))
                                .rounded(px(Theme::RADIUS_SMALL))
                                .bg(t.surface)
                                .child(icon(glyph, t.text)),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(Theme::FONT_SMALL))
                                .text_color(t.muted)
                                .child(detail),
                        )
                        .on_click(move |_, _, _| surface.action(&command)),
                );
            }
            content = content.child(caps_label("Motion presets", t)).child(presets);
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
                .child(caps_label("Capture source", t))
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
                // Eight controls in one flat stack gave no clue which
                // dropdown belonged to which switch. They are groups now.
                .child(
                    group_card(t, "Camera")
                        .child(toggle(
                            "capture-camera",
                            "Record camera",
                            e.get_capture_camera(),
                            true,
                            t,
                            move |v, _, _| e1.set_capture_camera(v),
                        ))
                        .child(camera),
                )
                .child(
                    group_card(t, "Audio")
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
                        )),
                );
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

    fn menu_button(&self, name: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let control = button(name, name, self.theme)
            .selected(self.menu.as_deref() == Some(name))
            .glyph("Plus-regular")
            .on_click(cx.listener(move |s, _, _, cx| {
                s.menu = if s.menu.as_deref() == Some(name) {
                    None
                } else {
                    s.menu_filter.clear();
                    s.menu_focus = true;
                    Some(name.into())
                };
                cx.notify();
            }));
        // The palette opens against these bounds. Measuring costs a wrapper,
        // but the alternative is the fixed coordinate this replaced, which
        // put the card at the top of the window while its trigger sat in the
        // timeline strip at the bottom.
        div()
            .relative()
            .flex_none()
            .child(measure(self.menu_anchor.clone()))
            .child(control)
    }

    fn menu_commands(name: &str) -> &'static [&'static [(&'static str, &'static str)]] {
        menu_commands(name)
    }

    /// Run `command` and close the palette.
    fn run_command(&mut self, command: &str, cx: &mut Context<Self>) {
        if let Some(panel) = command.strip_prefix('@') {
            if let Surface::Editor(e) = &self.surface {
                e.set_panel(panel.into());
                e.defer_panel(panel.into());
            }
        } else {
            self.surface.action(command);
        }
        self.menu = None;
        self.menu_filter.clear();
        cx.notify();
    }

    /// The command palette: a context chip, a filter input, the command
    /// list, and a quiet row of the other menus beneath it.
    ///
    /// It anchors to the trigger's measured bounds and flips above it when
    /// the trigger sits in the lower half of the window — the "Add" button
    /// lives in the timeline strip, so a card that always dropped downward
    /// would open off the bottom edge.
    fn menu_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(name) = self.menu.clone() else {
            return div().into_any_element();
        };
        let t = self.theme;
        let filter = self.menu_filter.to_lowercase();
        let matches: Vec<_> = Self::menu_commands(&name)
            .iter()
            .flat_map(|group| group.iter())
            .filter(|(label, _)| filter.is_empty() || label.to_lowercase().contains(&filter))
            .collect();
        let first = matches.first().map(|(_, command)| command.to_string());

        let anchor = self.menu_anchor.get();
        let viewport = window.viewport_size();
        let left = f32::from(anchor.origin.x)
            .min(f32::from(viewport.width) - PALETTE_WIDTH - Theme::GAP)
            .max(Theme::GAP);
        // Card height is content-driven, so cap it and reserve that much when
        // deciding which way to open.
        let rows = matches.len().clamp(1, PALETTE_VISIBLE_ROWS) as f32;
        let height = PALETTE_CHROME_HEIGHT + rows * Theme::CONTROL_HEIGHT;
        let below = f32::from(anchor.origin.y + anchor.size.height) + Theme::GAP;
        let top = if below + height <= f32::from(viewport.height) - Theme::GAP {
            below
        } else {
            (f32::from(anchor.origin.y) - Theme::GAP - height).max(Theme::GAP)
        };

        let search = self.input("command-palette", "", window, cx, {
            let first = first.clone();
            move |_, _, _| {
                let _ = &first;
            }
        });
        // `cx.listener` hands the callback its event by reference; the input's
        // callbacks take the text by value, so they go through a weak handle.
        let view = cx.entity().downgrade();
        search.update(cx, |input, _| {
            input.set_placeholder("Type to filter commands");
            let filtering = view.clone();
            input.set_on_change(move |value, _, cx| {
                filtering
                    .update(cx, |s: &mut Self, cx| {
                        s.menu_filter = value.clone();
                        cx.notify();
                    })
                    .ok();
            });
            // Enter runs whatever is at the top of the filtered list, which
            // is the only reason the field commits at all.
            let running = view.clone();
            let run = first.clone();
            input.set_handler(move |_, _, cx| {
                let Some(command) = run.clone() else { return };
                running
                    .update(cx, |s: &mut Self, cx| s.run_command(&command, cx))
                    .ok();
            });
            // The field swallows escape, so it has to close the card itself.
            let dismissing = view.clone();
            input.set_on_cancel(move |_, cx| {
                dismissing
                    .update(cx, |s: &mut Self, cx| {
                        s.menu = None;
                        s.menu_filter.clear();
                        cx.notify();
                    })
                    .ok();
            });
        });
        if std::mem::take(&mut self.menu_focus) {
            search.update(cx, |input, cx| {
                input.reset("");
                input.focus(window, cx);
            });
        }

        let mut list = menu_list(
            "palette-list",
            PALETTE_VISIBLE_ROWS as f32 * Theme::CONTROL_HEIGHT,
        )
        .py(px(FADE_BAND));
        if matches.is_empty() {
            list = list.child(
                div()
                    .h(px(Theme::CONTROL_HEIGHT))
                    .flex()
                    .items_center()
                    .px(px(Theme::CONTROL_PADDING))
                    .text_color(t.muted)
                    .child("No matching command"),
            );
        }
        // Rows carry no glyph: a third of these commands have no icon in the
        // set, and inventing one per row reads worse than a clean list.
        for (label, command) in matches {
            let command = command.to_string();
            list = list.child(menu_row(
                SharedString::from(format!("palette-{command}")),
                *label,
                false,
                false,
                t,
                cx.listener(move |s, _, _, cx| s.run_command(&command, cx)),
            ));
        }

        // The footer doubles as the menu switcher, which is what finally
        // gives File, Edit and Help an entry point in the window itself.
        let mut footer = composer_footer(t);
        for (menu, glyph) in [
            ("File", "FolderOpen-regular"),
            ("Edit", "SlidersHorizontal-regular"),
            ("Add", "Plus-regular"),
            ("Help", "Question-regular"),
        ] {
            footer = footer.child(
                icon_button(SharedString::from(format!("palette-menu-{menu}")), glyph, menu, t)
                    .ghost()
                    .selected(name == menu)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.menu = Some(menu.into());
                        s.menu_filter.clear();
                        s.menu_focus = true;
                        cx.notify();
                    })),
            );
        }
        footer = footer.child(
            div()
                .flex_1()
                .text_ellipsis()
                .min_w_0()
                .child(SharedString::from(name.clone())),
        );

        deferred(frosted(
            Theme::RADIUS_CARD,
            MENU_BLUR,
            menu_in(
                "command-menu-in",
                top,
                menu_surface(t)
                    .id("command-menu")
                    .absolute()
                    .left(px(left))
                    .w(px(PALETTE_WIDTH))
                    .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                        s.menu = None;
                        cx.notify();
                    }))
                    .child(context_chip(t, &[&name, "Commands"]))
                    .child(search)
                    .child(fade_edges(list))
                    .child(footer),
            ),
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
        let editor = e.clone();
        let position = self.slider(
            "timeline-position",
            0.,
            (e.get_duration() - visible).max(0.001),
            offset,
            ("Position", "ArrowsOutSimple-regular"),
            (1.0, " s"),
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
            .child(self.menu_button("Add", cx))
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
        // Only the panel list scrolls. Settings and Help used to sit after a
        // `flex_1` spacer INSIDE the scroll region, which pins them to the
        // bottom only while the content fits — the moment it overflows the
        // spacer collapses and they scroll away with everything else, so at
        // 980x680 they were unreachable. They now live outside the scroller.
        let mut panels = div()
            .id("rail")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w_full()
            .flex_1()
            .min_h_0()
            .py(px(FADE_BAND))
            .overflow_y_scroll();
        // Icons only; the active panel keeps the accent plate and the marker.
        for (label, name, glyph) in [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Webcam", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ] {
            panels = panels.child(self.rail_panel_button(e, label, name, glyph));
        }
        let rail = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w(px(RAIL_WIDTH))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .child(fade_edges(panels))
            .child(self.rail_panel_button(e, "Settings", "Preferences", "Gear-regular"))
            .child(self.rail_panel_button(e, "Help", "shortcut-reference", "Question-regular"))
            .pb(px(Theme::GAP_SMALL));
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
                    .child(rail)
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
            .child(self.menu_overlay(window, cx))
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
        // The bar IS the window's plate — the window itself is transparent and
        // borderless, so there is nothing behind this to tint. An outer plate
        // around it only drew a second, square box.
        let mut bar = panel_variant(t, UiSurface::Overlay)
            .flex_row()
            .items_center()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .p(px(Theme::GAP))
            .gap(px(Theme::GAP_SMALL))
            .child(
                // Was a bare "⠿" text character: no size token, no colour
                // token and no control geometry, so it sat misaligned beside
                // the 40px controls. Same glyph, on the scale everything
                // else uses.
                div()
                    .id("launcher-drag")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(Theme::CONTROL_HEIGHT / 2.0))
                    .h(px(Theme::CONTROL_HEIGHT))
                    .cursor(CursorStyle::ClosedHand)
                    .child(icon("DotsSixVertical-regular", t.muted))
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
        // The quiet end of the bar. These were a bare "?" with no hit target
        // and a full button plate whose caption was the literal character
        // "×"; both are now icon controls at the shared geometry.
        let hint = s.get_status();
        bar = bar
            .child(
                div()
                    .id("recorder-status")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(Theme::CONTROL_HEIGHT))
                    .child(icon("Question-regular", t.muted))
                    .tooltip(move |_, cx| tooltip(hint.clone(), t, cx)),
            )
            .child(self.icon_action(
                "close",
                "X-regular",
                "Hide recorder",
                "hide-launcher",
                true,
            ));
        bar.into_any_element()
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
        // Composer structure: a context chip naming the surface, the controls
        // beneath it, and a quiet footer row. The close control is an icon at
        // the shared geometry rather than a button whose caption was the
        // literal character "×".
        let mut body = panel_variant(t, UiSurface::Overlay)
            .size_full()
            .p(px(Theme::GAP_LARGE))
            .gap(px(Theme::GAP))
            .child(
                row()
                    .child(context_chip(t, &["Recorder", title]).flex_1().min_w_0())
                    .child(
                        icon_button("close", "X-regular", "Close", t)
                            .ghost()
                            .on_click(move |_, _, _| options.defer_panel("".into())),
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
                    .child(section_label("Capture source", t))
                    .child(sources)
                    .child(self.action(
                        "refresh",
                        "Refresh displays and windows",
                        "sources",
                        !s.get_busy(),
                    ))
                    .child(div().flex_1())
                    .child(composer_footer(t).child(
                        "Choose a display or a visible window to record.",
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
                    .child(div().flex_1())
                    // The path and the control that changes it are one
                    // setting; they were a muted line and a row a gap apart,
                    // with the value drifting away from its own label.
                    .child(
                        group_card(t, "Recordings path").child(
                            row()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_ellipsis()
                                        .child(s.get_directory()),
                                )
                                .child(self.action(
                                    "folder",
                                    "Choose folder…",
                                    "recording-folder",
                                    true,
                                )),
                        ),
                    );
            }
        }
        body.into_any_element()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::new(&self.surface.appearance(), window.appearance());
        // Element ids are unique within a window; the tween store is one
        // thread-local shared by all three of ours. Scope it before anything
        // in this tree asks it for a value.
        subtake_ui::motion::enter_window(window.window_handle().window_id().as_u64());
        let render_began = Instant::now();
        if subtake_ui::perf::enabled() {
            let since = self.last_render.map(|t| t.elapsed().as_secs_f64() * 1000.0);
            subtake_ui::perf::log(format_args!(
                "render {} begin (since last {})",
                self.surface.kind_name(),
                since.map_or("-".to_string(), |ms| format!("{ms:.1}ms"))
            ));
            self.last_render = Some(render_began);
        }
        // Interface text is authored in rems against the reference's 16px
        // baseline, so `px(11.0)` resolves to 11px here.

        let surface = self.surface.clone();
        let content = match &surface {
            Surface::Editor(e) => self.editor(e, window, cx),
            Surface::Launcher(s) => self.launcher(s),
            Surface::Options(s) => self.options(s, cx),
        };
        // Keep frames coming while any wash or switch is mid-fade. This has
        // to run AFTER the tree is built, not before: hover fades are kicked
        // off by an event that refreshes the window anyway, but a tween that
        // a control starts from its own render — a switch flipping, a tile
        // being selected — is only visible to the store once that render has
        // happened, and nothing else would ask for the frames to finish it.
        //
        // `request_animation_frame`, not `cx.notify()`. We are inside the
        // draw: gpui's invalidator ignores a notify while a draw phase is
        // active (it records the view and requests no frame), so a fade ran
        // one frame and then froze until an unrelated event repainted the
        // window — the hover that "took a while" to arrive. The next-frame
        // callback schedules a real frame and notifies this view from it.
        let fading = subtake_ui::tick_hover_fades();
        if fading {
            window.request_animation_frame();
        }
        subtake_ui::perf::log_took(
            format_args!("render {} tree built (fading={fading})", surface.kind_name()),
            render_began,
            0.0,
        );
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
