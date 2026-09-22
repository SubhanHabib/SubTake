//! Native editor and recorder surfaces, one `RootView` per window kind.
//! Business commands stay behind the `ui_state` callbacks; this module and
//! its children only present state and forward intent.
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
use subtake_ui::{
    Button, Dropdown, FADE_BAND, MENU_BLUR, Slider, Surface as UiSurface, TextInput, button,
    caps_label, choice_tile, column, composer_footer, context_chip, empty_state, fade_edges,
    frosted, group_card, icon, icon_button, measure, media_tile, menu_in, menu_list, menu_row,
    menu_surface, mono, mono_small, panel, panel_variant, progress_bar, rail_button, row,
    section_label, segmented_control, setting_card, status_dot, swatch, switch, tile_grid,
    timeline_scrubber, toggle, tooltip,
};

/// Title bar strip: tall enough to seat the 40px icon cluster with air.
mod editor;
mod inspector;
mod menus;
mod preview;
mod recorder;
mod timeline;

pub use menus::menu_commands;

/// Title bar strip: tall enough to seat the 40px icon cluster with air.
const TITLEBAR_HEIGHT: f32 = 56.0;

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
        let (sans, mono, title) = subtake_ui::families_available(cx);
        if !sans || !mono || !title {
            eprintln!("SUBTAKE_FONTS_MISSING: Geist={sans} GeistMono={mono} SpaceGrotesk={title}");
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

    pub(super) fn action(
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
    pub(super) fn translate(&self, editor: &EditorWindow, label: &str) -> String {
        let translated = editor.invoke_translate(label.into(), editor.get_language());
        if translated.is_empty() {
            label.to_owned()
        } else {
            translated
        }
    }

    /// A command as a bare click handler, for buttons built inline.
    pub(super) fn command(
        &self,
        command: &str,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let surface = self.surface.clone();
        let command = command.to_owned();
        move |_, _, _| surface.action(&command)
    }

    /// A square icon-only title-bar action.
    pub(super) fn icon_action(
        &self,
        id: &str,
        glyph: &str,
        label: &str,
        command: &str,
        enabled: bool,
    ) -> Button {
        let s = self.surface.clone();
        let command = command.to_owned();
        icon_button(
            SharedString::from(id.to_owned()),
            glyph,
            label.to_owned(),
            self.theme,
        )
        .ghost()
        .enabled(enabled)
        .on_click(move |_, _, _| s.action(&command))
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
            format_args!(
                "render {} tree built (fading={fading})",
                surface.kind_name()
            ),
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
