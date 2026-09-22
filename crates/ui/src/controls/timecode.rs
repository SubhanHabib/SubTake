//! The timecode field: a time on a `sunk` plate that is typed or scrubbed.
//!
//! Click it and the whole value is selected, so typing replaces it; Return
//! or clicking away keeps what was typed, Esc puts the old value back. Drag
//! across it instead and it scrubs, the value going Geist Mono 500 for as
//! long as the hand is on it.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::focus_ring;

/// How far one pixel of drag moves the time, and how far with Shift held.
const SCRUB_MS_PER_PIXEL: f64 = 10.0;
const SCRUB_MS_PER_PIXEL_COARSE: f64 = 100.0;
/// How far the pointer travels before a press becomes a scrub rather than
/// a click into the field.
const SCRUB_SLOP: f32 = 3.0;

pub struct TimecodeField {
    pub label: SharedString,
    /// The time, in milliseconds.
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub enabled: bool,
    pub theme: Theme,
    focus: FocusHandle,
    /// What has been typed since the field was clicked. Empty means the
    /// whole value is still selected and the next key replaces it.
    typed: Option<String>,
    /// Where a press started, the value then, and whether it has moved far
    /// enough to be a scrub.
    press: Option<(Pixels, f64, bool)>,
    blur_observer: Option<Subscription>,
    change: Box<dyn Fn(f64, &mut Window, &mut App)>,
}

impl TimecodeField {
    pub fn new(
        cx: &mut Context<Self>,
        value: f64,
        theme: Theme,
        change: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: SharedString::default(),
            value,
            minimum: 0.,
            maximum: f64::MAX,
            enabled: true,
            theme,
            focus: cx.focus_handle(),
            typed: None,
            press: None,
            blur_observer: None,
            change: Box::new(change),
        }
    }

    pub fn set_handler(&mut self, change: impl Fn(f64, &mut Window, &mut App) + 'static) {
        self.change = Box::new(change);
    }

    /// Take the model's value, unless the user is in the middle of changing
    /// it here.
    pub fn sync(&mut self, value: f64) {
        if self.typed.is_none() && !self.scrubbing() {
            self.value = value;
        }
    }

    fn scrubbing(&self) -> bool {
        self.press.is_some_and(|(_, _, moved)| moved)
    }

    fn commit(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value.clamp(self.minimum, self.maximum);
        (self.change)(self.value, window, cx);
        cx.notify();
    }

    /// Keep what was typed, if it reads as a time.
    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(typed) = self.typed.take() {
            if let Some(value) = parse(&typed) {
                self.commit(value, window, cx);
            }
            cx.notify();
        }
    }
}

/// "0:04.1": minutes, seconds and tenths.
pub fn format_timecode(ms: f64) -> String {
    let tenths = (ms.max(0.) / 100.).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, (tenths / 10) % 60, tenths % 10)
}

/// A typed time: "1:04.5", "64.5" or "64", in minutes and seconds.
fn parse(text: &str) -> Option<f64> {
    let text = text.trim();
    let (minutes, seconds) = match text.rsplit_once(':') {
        Some((m, s)) => (m.trim().parse::<f64>().ok()?, s),
        None => (0., text),
    };
    let seconds: f64 = seconds.trim().parse().ok()?;
    (minutes >= 0. && seconds >= 0.).then(|| (minutes * 60. + seconds) * 1000.)
}

impl Render for TimecodeField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.blur_observer.is_none() {
            self.blur_observer = Some(cx.on_blur(&self.focus, window, |s, w, cx| s.finish(w, cx)));
        }
        let theme = self.theme;
        let scrubbing = self.scrubbing();
        let value = match &self.typed {
            // The whole value, selected: the next key replaces it.
            Some(typed) if typed.is_empty() => crate::mono(format_timecode(self.value))
                .px(px(Theme::TIMECODE_SELECTION_PADDING))
                .rounded(px(Theme::TIMECODE_SELECTION_RADIUS))
                .bg(theme.accent_soft)
                .text_color(theme.text),
            Some(typed) => crate::mono(typed.clone())
                .text_color(theme.text)
                .border_r(px(Theme::BORDER_WIDTH))
                .border_color(theme.accent),
            None => crate::mono(format_timecode(self.value))
                .text_color(theme.text)
                .when(scrubbing, |el| el.font_weight(FontWeight::MEDIUM)),
        };
        div()
            .id("timecode")
            .track_focus(&self.focus)
            .tab_index(0)
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(Theme::GAP))
            .h(px(Theme::CONTROL_HEIGHT_LARGE))
            .px(px(Theme::CONTROL_PADDING_LARGE))
            .rounded_full()
            .bg(theme.sunk)
            .when(self.enabled, |el| {
                el.hover(|s| s.bg(theme.sunk2))
                    .cursor(CursorStyle::ResizeLeftRight)
            })
            .when(!self.enabled, |el| el.opacity(Theme::DISABLED_OPACITY))
            .focus_visible(move |s| s.shadow(vec![focus_ring(theme)]))
            .when(self.typed.is_some(), move |el| {
                el.shadow(vec![focus_ring(theme)])
            })
            .child(
                div()
                    .flex_1()
                    .text_size(px(Theme::FONT_SECONDARY))
                    .text_color(theme.muted)
                    .child(self.label.clone()),
            )
            .child(value.text_size(px(Theme::FONT_CONTROL)))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if !s.enabled {
                    return;
                }
                let key = e.keystroke.key.as_str();
                match key {
                    "enter" => {
                        if s.typed.is_some() {
                            s.finish(w, cx);
                        } else {
                            s.typed = Some(String::new());
                        }
                    }
                    "escape" => s.typed = None,
                    "backspace" => {
                        if let Some(typed) = &mut s.typed {
                            typed.pop();
                        }
                    }
                    _ => {
                        let Some(ch) = e.keystroke.key_char.as_deref() else {
                            return;
                        };
                        if e.keystroke.modifiers.platform
                            || !ch
                                .chars()
                                .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
                        {
                            return;
                        }
                        s.typed.get_or_insert_with(String::new).push_str(ch);
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, w, cx| {
                    if !s.enabled {
                        return;
                    }
                    w.focus(&s.focus, cx);
                    s.finish(w, cx);
                    s.press = Some((e.position.x, s.value, false));
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                let Some((origin, start, moved)) = s.press else {
                    return;
                };
                let dx = f32::from(e.position.x - origin);
                if !moved && dx.abs() < SCRUB_SLOP {
                    return;
                }
                let rate = if e.modifiers.shift {
                    SCRUB_MS_PER_PIXEL_COARSE
                } else {
                    SCRUB_MS_PER_PIXEL
                };
                s.value = (start + dx as f64 * rate).clamp(s.minimum, s.maximum);
                s.press = Some((origin, start, true));
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _: &MouseUpEvent, w, cx| s.release(w, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _: &MouseUpEvent, w, cx| s.release(w, cx)),
            )
    }
}

impl TimecodeField {
    /// A press that never moved is a click into the field; one that did is
    /// the end of a scrub, which is kept.
    fn release(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.press.take() {
            Some((_, _, true)) => self.commit(self.value, window, cx),
            Some((_, _, false)) => {
                self.typed = Some(String::new());
                cx.notify();
            }
            None => {}
        }
    }
}

impl Focusable for TimecodeField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{format_timecode, parse};

    #[test]
    fn reads_and_writes_the_same_times() {
        assert_eq!(format_timecode(4_100.), "0:04.1");
        assert_eq!(format_timecode(64_560.), "1:04.6");
        assert_eq!(parse("1:04.5"), Some(64_500.));
        assert_eq!(parse("4.1"), Some(4_100.));
        assert_eq!(parse("12"), Some(12_000.));
        assert_eq!(parse("1:x"), None);
    }
}
