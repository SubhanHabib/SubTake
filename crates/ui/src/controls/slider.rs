//! The scrub field: a filled slider that *is* the row, its value shown in place.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

use crate::{focus_ring, icon, layered, measure, motion};

/// A filled slider that *is* the row: glyph and label on the left, the level
/// painted as a flat fill from the left end of the 44px plate, and the value
/// shown in place on the right. A thumb marks the fill edge only while the
/// pointer is over the row or holding it.
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
    /// Words for a stepped slider: the value is an index into them, it
    /// lands on whole steps, and the row shows the word, not the number.
    pub steps: Vec<SharedString>,
    /// Off, the row dims to the disabled opacity and takes no input.
    pub enabled: bool,
    /// The row's height: 44, unless it sits among smaller controls.
    pub height: f32,
    /// On a recess of its own — the Microphone card's meter block — the
    /// track is that recess's `sunk` and the level is `sunk2`, since
    /// `slider_fill` is drawn for a track on glass.
    pub recessed: bool,
    /// The room the number is kept to, so it does not reflow while
    /// scrubbing, and how far in the words sit from the row's ends: 70 and
    /// 18, unless the row sits among smaller controls.
    pub value_width: f32,
    pub padding: f32,
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
            steps: Vec::new(),
            enabled: true,
            height: Theme::control_height_large(),
            recessed: false,
            value_width: Theme::scrub_value_width(),
            padding: Theme::control_padding_large(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            dragging: false,
            change: Box::new(change),
        }
    }

    /// The number the row shows is the number the row commits. A drag lands
    /// on a pixel, which is an arbitrary fraction of the range: committing
    /// that raw float wrote `1.7592592592593` into the project for a row
    /// reading `1.76`, and every place that shows the stored string rather
    /// than this slider's own rendering — a text field, a saved preset, the
    /// project file — carried the tail. The step is the one the display
    /// implies: two decimals when the number is shown as it is, and one part
    /// in `scale` when it is rescaled to whole units.
    fn quantise(&self, value: f32) -> f32 {
        if !self.steps.is_empty() {
            return value.round();
        }
        let step = if self.scale == 1.0 { 100.0 } else { self.scale };
        (value * step).round() / step
    }

    fn set(&mut self, x: Pixels, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let b = self.bounds.get();
        let f = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        self.value = self.quantise(self.minimum + f * (self.maximum - self.minimum));
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
        let display = if !self.steps.is_empty() {
            let index = (self.value.round().max(0.) as usize).min(self.steps.len() - 1);
            self.steps[index].to_string()
        } else if self.scale == 1.0 {
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

        let enabled = self.enabled;
        // A slider has no caller-supplied id, so its hover hangs off the
        // entity, as a dropdown's does.
        let hover_key = format!("slider-{:?}-hover", cx.entity_id());
        let scrub = div()
            .id("scrub")
            .relative()
            .h(px(self.height))
            .w_full()
            .rounded_full()
            // Track `sunk`, one step up to `sunk2` under the pointer. The row
            // is its own hit target across its whole width, so the track is
            // the only thing that can say it is live. On a recess `sunk2` is
            // the level, so there the thumb alone says it.
            .bg(if self.recessed {
                theme.sunk
            } else {
                motion::hover_blend(&hover_key, theme.sunk, theme.sunk2)
            })
            .overflow_hidden()
            .when(enabled, |el| {
                el.tab_index(0)
                    .on_hover(motion::hover_listener(hover_key.clone()))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .focus_visible(move |s| s.shadow(vec![focus_ring(theme)]))
            })
            .when(!enabled, |el| el.opacity(Theme::disabled_opacity()))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if !s.enabled {
                    return;
                }
                let step = if !s.steps.is_empty() {
                    1.
                } else {
                    (s.maximum - s.minimum)
                        / if e.keystroke.modifiers.shift {
                            10.
                        } else {
                            100.
                        }
                };
                let value = match e.keystroke.key.as_str() {
                    "left" | "down" => s.value - step,
                    "right" | "up" => s.value + step,
                    "home" => s.minimum,
                    "end" => s.maximum,
                    _ => return,
                };
                s.value = s.quantise(value.clamp(s.minimum, s.maximum));
                (s.change)(s.value, true, w, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(measure(self.bounds.clone()))
            // The level, painted across the plate rather than on a rail: full
            // height, left-anchored, square at its right edge and rounded
            // only where the track is. gpui clips to rectangles, so the fill
            // is a track-sized pill cut to the level's width rather than a
            // shape of its own — at a low level that leaves a sliver of the
            // track's left end, not a blob that reads as a knob.
            .when(fraction > 0., |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(relative(fraction))
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(relative(1. / fraction))
                                .rounded_full()
                                .bg(if self.recessed {
                                    theme.sunk2
                                } else {
                                    theme.slider_fill
                                }),
                        ),
                )
            })
            // The thumb: a short bar centred on the fill edge, `muted` under
            // the pointer and taller in `text` while held. At rest there is
            // nothing on the edge, so no line ever crosses the label.
            .child({
                let inset = if self.dragging {
                    Theme::slider_thumb_inset_held()
                } else {
                    Theme::slider_thumb_inset()
                };
                div()
                    .absolute()
                    .top(px(inset))
                    .bottom(px(inset))
                    .left(relative(fraction))
                    .ml(px(-Theme::slider_thumb_width() / 2.))
                    .w(px(Theme::slider_thumb_width()))
                    .rounded_full()
                    .bg(if self.dragging {
                        theme.text
                    } else {
                        motion::hover_blend(&hover_key, theme.muted.opacity(0.), theme.muted)
                    })
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .gap(px(Theme::gap()))
                    .px(px(self.padding))
                    .when(!self.glyph.is_empty(), |el| {
                        el.child(icon(&self.glyph, theme.text))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(Theme::font_control()))
                            .text_color(theme.text)
                            .child(self.label.clone()),
                    )
                    // The value in Geist Mono at `muted`, as the Slider row
                    // card gives it: the label names the setting and the
                    // number is what it currently is, so the number is the
                    // quieter of the two and must not reflow while scrubbing.
                    .child(
                        crate::mono(display)
                            .flex_none()
                            .w(px(self.value_width))
                            .whitespace_nowrap()
                            .text_right()
                            .text_size(px(Theme::font_control()))
                            .text_color(if self.dragging {
                                theme.text
                            } else {
                                theme.muted
                            })
                            .when(self.dragging, |el| el.font_weight(FontWeight::MEDIUM)),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, w, cx| {
                    if !s.enabled {
                        return;
                    }
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
            );
        // A layer of its own for the focus ring, which falls outside the
        // track and inside a frosted float would draw under the panel's fill.
        layered(scrub)
    }
}
