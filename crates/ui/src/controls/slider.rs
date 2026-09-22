//! The scrub field: a filled slider that *is* the row, its value shown in place.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

use crate::{icon, measure};

/// A filled slider that *is* the row: glyph and label on the left, the level
/// painted as a fill across the whole 40px plate, a hairline at the fill edge,
/// and the value shown in place on the right.
pub struct Slider {
    pub value: f32,
    pub minimum: f32,
    pub maximum: f32,
    pub theme: Theme,
    /// Glyph shown at the left of the plate. Empty for an inspector row: the
    /// caption already names the setting, and a glyph per row turns a stack
    /// of them into a column of pictograms.
    pub glyph: SharedString,
    /// Caption shown inside the plate.
    pub label: SharedString,
    /// What the number means. The model carries a raw value, so the unit is
    /// presentation: `scale` takes it to display terms (a 0–1 factor reads as
    /// a percentage) and `unit` is the suffix.
    pub scale: f32,
    pub unit: SharedString,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    dragging: bool,
    change: Box<dyn Fn(f32, bool, &mut Window, &mut App)>,
}

impl Slider {
    pub fn set_handler(&mut self, change: impl Fn(f32, bool, &mut Window, &mut App) + 'static) {
        self.change = Box::new(change);
    }
    pub fn sync(&mut self, value: f32) {
        if !self.dragging {
            self.value = value;
        }
    }
    pub fn set_caption(&mut self, label: impl Into<SharedString>, glyph: impl Into<SharedString>) {
        self.label = label.into();
        self.glyph = glyph.into();
    }
    pub fn set_unit(&mut self, scale: f32, unit: impl Into<SharedString>) {
        self.scale = scale;
        self.unit = unit.into();
    }
    pub fn new(
        minimum: f32,
        maximum: f32,
        value: f32,
        theme: Theme,
        change: impl Fn(f32, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            minimum,
            maximum,
            value,
            theme,
            glyph: SharedString::default(),
            label: SharedString::default(),
            scale: 1.0,
            unit: SharedString::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            dragging: false,
            change: Box::new(change),
        }
    }
    fn set(&mut self, x: Pixels, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let b = self.bounds.get();
        let f = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        self.value = self.minimum + f * (self.maximum - self.minimum);
        (self.change)(self.value, commit, window, cx);
        cx.notify();
    }
}

impl Render for Slider {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let fraction =
            ((self.value - self.minimum) / (self.maximum - self.minimum).max(0.001)).clamp(0., 1.);
        let shown = self.value * self.scale;
        let display = if self.scale == 1.0 {
            let rounded = (shown * 100.0).round() / 100.0;
            if rounded.fract() == 0.0 {
                format!("{}{}", rounded as i64, self.unit)
            } else {
                format!("{rounded}{}", self.unit)
            }
        } else {
            // A rescaled value is already coarse; two decimals of a
            // percentage is noise the row has no room for.
            format!("{}{}", shown.round() as i64, self.unit)
        };

        div()
            .id("scrub")
            .relative()
            .tab_index(0)
            .h(px(Theme::CONTROL_HEIGHT))
            .w_full()
            .rounded(px(Theme::RADIUS_CONTROL))
            .bg(theme.surface)
            .overflow_hidden()
            .cursor(CursorStyle::ResizeLeftRight)
            .focus_visible(move |s| s.border_1().border_color(theme.slider_focus()))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                let step = (s.maximum - s.minimum)
                    / if e.keystroke.modifiers.shift {
                        10.
                    } else {
                        100.
                    };
                let value = match e.keystroke.key.as_str() {
                    "left" | "down" => s.value - step,
                    "right" | "up" => s.value + step,
                    "home" => s.minimum,
                    "end" => s.maximum,
                    _ => return,
                };
                s.value = value.clamp(s.minimum, s.maximum);
                (s.change)(s.value, true, w, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(measure(self.bounds.clone()))
            // The level, painted across the plate rather than on a rail.
            .child(
                div()
                    .absolute()
                    .top(px(Theme::SLIDER_FILL_INSET))
                    .left(px(Theme::SLIDER_FILL_INSET))
                    .h(px(Theme::CONTROL_HEIGHT - 2.0 * Theme::SLIDER_FILL_INSET))
                    .w(relative(fraction))
                    .rounded(px(Theme::SLIDER_FILL_RADIUS))
                    .bg(theme.slider_fill()),
            )
            // Hairline at the fill edge so the exact level stays readable.
            .child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(relative(fraction))
                    .ml(px(-1.))
                    .w(px(2.))
                    .h(px(Theme::CONTROL_HEIGHT - 24.0))
                    .rounded(px(1.))
                    .bg(theme.slider_marker()),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .gap(px(Theme::GAP))
                    .px(px(Theme::CONTROL_PADDING))
                    .when(!self.glyph.is_empty(), |el| {
                        el.child(icon(&self.glyph, theme.text))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(theme.text)
                            .child(self.label.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(Theme::SCRUB_VALUE_WIDTH))
                            .text_right()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(theme.text)
                            .child(display),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, w, cx| {
                    s.dragging = true;
                    s.set(e.position.x, false, w, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, w, cx| {
                if s.dragging {
                    s.set(e.position.x, false, w, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, e: &MouseUpEvent, w, cx| {
                    if s.dragging {
                        s.dragging = false;
                        s.set(e.position.x, true, w, cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, e: &MouseUpEvent, w, cx| {
                    if s.dragging {
                        s.dragging = false;
                        s.set(e.position.x, true, w, cx);
                    }
                }),
            )
    }
}
