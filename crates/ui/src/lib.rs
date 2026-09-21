//! Native GPUI controls on SubTake's own design system.
//!
//! Geometry is carried over from the pre-GPUI Slint components
//! (`ui/components/*.slint`): one 40px control height everywhere, a 16px
//! radius, 16px glyphs inset 12px, and translucent plates that let the
//! window's vibrancy material read through. The distinctive pieces are the
//! scrub field (a filled slider that *is* the row, with its value shown in
//! place) and the timeline scrubber (cap, rule and six-dot grip).

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

mod fonts;
mod frost;
mod input;
mod motion;
pub use fonts::{families_available, register as register_fonts};
pub use frost::{FADE_BAND, MENU_BLUR, fade_edges, frosted, layered};
pub use input::TextInput;
pub use input::init;
pub use motion::{HOVER_FADE_MS, hover_blend, hover_listener, tick_hover_fades};

// ---------------------------------------------------------------------------
// icons
// ---------------------------------------------------------------------------

/// A themed glyph at the shared 16px icon size.
pub fn icon(name: &str, color: Hsla) -> Svg {
    icon_sized(name, Theme::ICON_SIZE, color)
}

/// A glyph at a caller-chosen size (the record dot and play triangle run larger).
pub fn icon_sized(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(format!("assets/icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

// ---------------------------------------------------------------------------
// layout primitives
// ---------------------------------------------------------------------------

pub fn row() -> Div {
    div().flex().items_center().gap(px(Theme::GAP))
}

pub fn column() -> Div {
    div().flex().flex_col().gap(px(Theme::GAP))
}

/// Surface planes, matching the original `Panel` variants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Inspector / timeline plane.
    Panel,
    /// Inline card on a panel.
    Card,
    /// Menu or popover.
    Popup,
    /// Floating overlay; the material supplies the blur, this is only a tint.
    Overlay,
}

pub fn panel_variant(theme: Theme, variant: Surface) -> Div {
    let radius = match variant {
        Surface::Overlay => Theme::RADIUS_OVERLAY,
        Surface::Card | Surface::Popup => Theme::RADIUS_CARD,
        Surface::Panel => Theme::RADIUS_PANEL,
    };
    let background = match variant {
        Surface::Popup => theme.popup,
        Surface::Card => theme.surface,
        Surface::Overlay => theme.overlay,
        Surface::Panel => theme.panel,
    };
    let el = div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP))
        .rounded(px(radius))
        .bg(background);
    if variant == Surface::Overlay {
        el.shadow_lg()
    } else {
        el.border_1().border_color(theme.border)
    }
}

/// The default plane: an inspector / timeline panel.
pub fn panel(theme: Theme) -> Div {
    panel_variant(theme, Surface::Panel).p(px(Theme::GAP_LARGE))
}

/// A hairline rule.
pub fn divider(theme: Theme) -> Div {
    div().h(px(Theme::BORDER_WIDTH)).flex_1().bg(theme.border)
}

/// A small muted section caption ("Frame", "Padding", "Animation").
pub fn section_label(text: impl Into<SharedString>, theme: Theme) -> Div {
    row()
        .child(
            div()
                .flex_none()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(theme.muted)
                .child(text.into()),
        )
        .child(divider(theme))
}

// ---------------------------------------------------------------------------
// button
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Secondary,
    Primary,
    Ghost,
    Outline,
    Danger,
    Record,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    theme: Theme,
    variant: ButtonVariant,
    glyph: Option<SharedString>,
    glyph_size: f32,
    /// Hide the caption and render a square icon-only control.
    icon_only: bool,
    /// Pill the control fully (the rail's round actions).
    round: bool,
    selected: bool,
    enabled: bool,
    stretch: bool,
    handler: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, theme: Theme) -> Button {
    Button {
        id: id.into(),
        label: label.into(),
        theme,
        variant: ButtonVariant::Secondary,
        glyph: None,
        glyph_size: Theme::ICON_SIZE,
        icon_only: false,
        round: false,
        selected: false,
        enabled: true,
        stretch: false,
        handler: None,
    }
}

/// A square icon-only control at the shared 40px height.
pub fn icon_button(
    id: impl Into<ElementId>,
    glyph: &str,
    label: impl Into<SharedString>,
    theme: Theme,
) -> Button {
    button(id, label, theme).glyph(glyph).icon_only()
}

impl Button {
    pub fn glyph(mut self, name: &str) -> Self {
        self.glyph = Some(name.to_owned().into());
        self
    }
    pub fn glyph_size(mut self, size: f32) -> Self {
        self.glyph_size = size;
        self
    }
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }
    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }
    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn stretch(mut self) -> Self {
        self.stretch = true;
        self
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = self.theme;
        let tip = self.label.clone();
        let danger = matches!(self.variant, ButtonVariant::Danger | ButtonVariant::Record);

        // Two states, no hues: a control is either FILLED — the solid
        // achromatic plate with its glyph inverted on top — or a translucent
        // wash of the same grey. Primary actions and anything switched on
        // take the fill; everything else washes. Colour in the interface
        // comes from the desktop behind the glass, never from a control.
        let filled = self.variant == ButtonVariant::Primary || self.selected;
        let (rest, hover) = if filled {
            (t.accent, t.accent_hover)
        } else if danger {
            (t.danger.opacity(0.10), t.danger.opacity(0.22))
        } else if matches!(self.variant, ButtonVariant::Ghost | ButtonVariant::Outline) {
            (t.hover.opacity(0.0), t.hover)
        } else {
            (t.surface, t.hover)
        };
        let content = if filled {
            t.on_accent
        } else if danger {
            t.danger
        } else {
            t.text
        };
        // A ring drawn in the plate colour would vanish on a filled control.
        let focus_ring = if filled { t.on_accent } else { t.accent };

        let radius = if self.round {
            Theme::CONTROL_HEIGHT / 2.0
        } else {
            Theme::RADIUS_CONTROL
        };

        let mut el = div()
            .id(self.id)
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(Theme::GAP))
            .h(px(Theme::CONTROL_HEIGHT))
            .rounded(px(radius))
            .bg(rest)
            .text_color(content)
            .text_size(px(Theme::FONT_CONTROL))
            .font_weight(FontWeight::MEDIUM)
            .opacity(if self.enabled {
                1.
            } else {
                Theme::DISABLED_OPACITY
            });

        if self.icon_only {
            el = el.w(px(Theme::CONTROL_HEIGHT));
        } else {
            el = el.px(px(Theme::CONTROL_PADDING));
            if self.stretch {
                el = el.flex_1().min_w_0();
            }
        }
        if self.variant == ButtonVariant::Outline {
            el = el.border_1().border_color(t.border);
        }
        if danger {
            el = el.border_1().border_color(t.danger.opacity(0.4));
        }

        if let Some(name) = &self.glyph {
            el = el.child(icon_sized(name, self.glyph_size, content));
        }
        if !self.icon_only {
            el = el.child(
                div()
                    .when(self.stretch, |s| s.flex_1().min_w_0())
                    .text_ellipsis()
                    .child(self.label.clone()),
            );
        }
        el = el.tooltip(move |_, cx| tooltip(tip.clone(), t, cx));

        if self.enabled {
            // GPUI synthesizes ClickEvent::Keyboard for Enter/Space on focused divs.
            el = el
                .cursor_pointer()
                .tab_index(0)
                .focus(move |s| s.border_2().border_color(focus_ring))
                .hover(move |s| s.bg(hover));
            if let Some(handler) = self.handler {
                el = el.on_click(move |e, w, cx| handler(e, w, cx));
            }
        }
        el
    }
}

// ---------------------------------------------------------------------------
// rail button
// ---------------------------------------------------------------------------

/// A round action with its caption beneath — the editor's left rail.
pub fn rail_button(
    id: impl Into<ElementId>,
    glyph: &str,
    label: impl Into<SharedString>,
    active: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .w(px(Theme::RAIL_BUTTON_WIDTH))
        .h(px(Theme::RAIL_BUTTON_HEIGHT))
        .flex_none()
        .child(
            button(id, label.clone(), theme)
                .glyph(glyph)
                .icon_only()
                .round()
                .ghost()
                .selected(active)
                .on_click(on_click),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_ellipsis()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(if active { theme.text } else { theme.muted })
                .child(label),
        )
}

// ---------------------------------------------------------------------------
// segmented control
// ---------------------------------------------------------------------------

pub fn segmented_control(
    id: &str,
    options: &[&str],
    selected: usize,
    theme: Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);
    let id = SharedString::from(id.to_owned());
    div()
        .flex()
        .gap(px(Theme::GAP_SMALL))
        .h(px(Theme::CONTROL_HEIGHT))
        .children(
            options
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let pick = on_select.clone();
                    button((id.clone(), index), label, theme)
                        .selected(index == selected)
                        .stretch()
                        .on_click(move |_, w, cx| pick(index, w, cx))
                }),
        )
}

// ---------------------------------------------------------------------------
// tooltip
// ---------------------------------------------------------------------------

struct Tooltip {
    text: SharedString,
    theme: Theme,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        frost::frosted(
            Theme::RADIUS_SMALL,
            frost::MENU_BLUR,
            div()
                .max_w(px(320.))
                .px(px(Theme::GAP))
                .py(px(Theme::GAP_SMALL))
                .bg(t.popup)
                .border_1()
                .border_color(t.border)
                .rounded(px(Theme::RADIUS_SMALL))
                .shadow_lg()
                .text_size(px(Theme::FONT_SMALL))
                .text_color(t.text)
                .child(self.text.clone()),
        )
    }
}

pub fn tooltip(text: impl Into<SharedString>, theme: Theme, cx: &mut App) -> AnyView {
    let text = text.into();
    cx.new(|_| Tooltip { text, theme }).into()
}

// ---------------------------------------------------------------------------
// marks
// ---------------------------------------------------------------------------

/// The small filled dot that marks a state on a label — an unsaved document,
/// a live source. Achromatic like everything else, so it reads as emphasis
/// rather than as a status colour.
pub fn status_dot(theme: Theme) -> Div {
    div()
        .flex_none()
        .size(px(Theme::DOT_SIZE))
        .rounded(px(Theme::DOT_SIZE / 2.0))
        .bg(theme.accent)
}

/// A determinate progress rule: a wash track with a filled bar over it.
pub fn progress_bar(fraction: f32, theme: Theme) -> Div {
    div()
        .h(px(Theme::PROGRESS_HEIGHT))
        .rounded_full()
        .overflow_hidden()
        .bg(theme.unchecked)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0., 1.)))
                .bg(theme.text),
        )
}

/// A flat colour sample — the one place a literal colour is the content
/// rather than the styling, so it carries a full-strength outline when picked.
pub fn swatch(id: impl Into<ElementId>, colour: Hsla, selected: bool, theme: Theme) -> Stateful<Div> {
    div()
        .id(id.into())
        .size(px(Theme::SWATCH_SIZE))
        .rounded(px(Theme::RADIUS_SMALL))
        .bg(colour)
        .border_2()
        .border_color(if selected { theme.accent } else { theme.border })
        .cursor_pointer()
}

/// A captioned thumbnail in a picker grid (backgrounds, presets).
pub fn media_tile(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP_SMALL))
        .id(id.into())
        .w(px(Theme::TILE_WIDTH))
        .overflow_hidden()
        .rounded(px(Theme::RADIUS_SMALL))
        .cursor_pointer()
        .child(
            div()
                .text_size(px(Theme::FONT_SMALL))
                .text_ellipsis()
                .child(title.into()),
        )
}

/// The "nothing here yet" plane: one display headline, one muted line, and
/// the actions that get the user out of it.
pub fn empty_state(
    theme: Theme,
    headline: impl Into<SharedString>,
    detail: impl Into<SharedString>,
) -> Div {
    column()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(Theme::GAP_LARGE))
        .child(
            div()
                .text_size(px(Theme::FONT_DISPLAY))
                .font_weight(FontWeight::SEMIBOLD)
                .child(headline.into()),
        )
        .child(div().text_color(theme.muted).child(detail.into()))
}

// ---------------------------------------------------------------------------
// selection tile
// ---------------------------------------------------------------------------

pub fn choice_tile(
    id: impl Into<ElementId>,
    selected: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP_SMALL))
        .id(id.into())
        .min_w_0()
        .p(px(Theme::GAP))
        .rounded(px(Theme::RADIUS_CARD))
        // A tile is too big to invert wholesale, so "selected" reads as the
        // deeper grey wash plus a full-strength outline — the outlined half
        // of the same filled/outlined language the buttons use.
        .bg(if selected { theme.selection } else { theme.surface })
        .border_1()
        .border_color(if selected { theme.accent } else { theme.border })
        .opacity(if enabled {
            1.
        } else {
            Theme::DISABLED_OPACITY
        })
        .tab_index(0)
        .tab_stop(enabled)
        .focus(move |s| s.border_2().border_color(theme.accent))
        .when(enabled, |s| {
            s.cursor_pointer().hover(move |s| s.bg(theme.hover))
        })
}

// ---------------------------------------------------------------------------
// toggle
// ---------------------------------------------------------------------------

pub fn toggle(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    t: Theme,
    change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    // A 30px pill inside the 40px control slot, with a 24px thumb.
    let mut switch = div()
        .id(id.into())
        .relative()
        .flex_none()
        .w(px(52.))
        .h(px(30.))
        .rounded(px(15.))
        .bg(if checked { t.accent } else { t.unchecked })
        .opacity(if enabled {
            1.
        } else {
            Theme::DISABLED_OPACITY
        })
        .child(
            div()
                .absolute()
                .top(px(3.))
                .left(px(if checked { 25. } else { 3. }))
                .size(px(24.))
                .rounded(px(12.))
                // On a filled track the thumb takes the glyph colour, or it
                // would be white-on-white in the dark appearance.
                .bg(if checked { t.on_accent } else { t.toggle_thumb }),
        );
    if enabled {
        switch = switch
            .tab_index(0)
            .focus(move |s| s.border_2().border_color(t.accent))
            .cursor_pointer()
            .on_click(move |_, w, cx| change(!checked, w, cx));
    }
    row()
        .justify_between()
        .h(px(Theme::CONTROL_HEIGHT))
        .text_size(px(Theme::FONT_CONTROL))
        .text_color(t.text)
        .child(div().flex_1().min_w_0().text_ellipsis().child(label))
        .child(switch)
}

// ---------------------------------------------------------------------------
// measurement
// ---------------------------------------------------------------------------

/// Captures actual layout bounds for pixel-accurate canvas and timeline gestures.
pub fn measure(bounds: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(
        move |b, window, _| {
            let previous = bounds.replace(b);
            if previous.size != b.size {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

// ---------------------------------------------------------------------------
// scrub field — the unified control geometry
// ---------------------------------------------------------------------------

/// A filled slider that *is* the row: glyph and label on the left, the level
/// painted as a fill across the whole 40px plate, a hairline at the fill edge,
/// and the value shown in place on the right.
pub struct Slider {
    pub value: f32,
    pub minimum: f32,
    pub maximum: f32,
    pub theme: Theme,
    /// Glyph shown at the left of the plate.
    pub glyph: SharedString,
    /// Caption shown beside the glyph.
    pub label: SharedString,
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
            glyph: "SlidersHorizontal-regular".into(),
            label: SharedString::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            dragging: false,
            change: Box::new(change),
        }
    }
    fn set(&mut self, x: Pixels, commit: bool, w: &mut Window, cx: &mut Context<Self>) {
        let b = self.bounds.get();
        let f = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        self.value = self.minimum + f * (self.maximum - self.minimum);
        (self.change)(self.value, commit, w, cx);
        cx.notify();
    }
}

impl Render for Slider {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let fraction =
            ((self.value - self.minimum) / (self.maximum - self.minimum).max(0.001)).clamp(0., 1.);
        let rounded = (self.value * 100.0).round() / 100.0;
        let display = if rounded.fract() == 0.0 {
            format!("{}", rounded as i64)
        } else {
            format!("{rounded}")
        };

        div()
            .id("scrub")
            .relative()
            .tab_index(0)
            .h(px(Theme::CONTROL_HEIGHT))
            .w_full()
            .rounded(px(Theme::RADIUS_CONTROL))
            .bg(t.surface)
            .overflow_hidden()
            .cursor(CursorStyle::ResizeLeftRight)
            .focus(move |s| s.border_1().border_color(t.slider_focus()))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                let step =
                    (s.maximum - s.minimum) / if e.keystroke.modifiers.shift { 10. } else { 100. };
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
                    .bg(t.slider_fill()),
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
                    .bg(t.slider_marker()),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .gap(px(Theme::GAP))
                    .px(px(Theme::CONTROL_PADDING))
                    .child(icon(&self.glyph, t.text))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(t.text)
                            .child(self.label.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(Theme::SCRUB_VALUE_WIDTH))
                            .text_right()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(t.text)
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

// ---------------------------------------------------------------------------
// timeline scrubber
// ---------------------------------------------------------------------------

/// The playhead: a rounded cap, a continuous rule, and a six-dot grip the
/// pointer can take hold of anywhere down the track stack.
pub fn timeline_scrubber(theme: Theme, grip_top: f32) -> impl IntoElement {
    let dot = move |row: usize, col: f32| {
        div()
            .absolute()
            .left(px(col))
            .top(px(22.0 + row as f32 * 7.0))
            .size(px(3.))
            .rounded(px(1.5))
            .bg(theme.toggle_thumb)
    };
    div()
        .w(px(Theme::SCRUBBER_WIDTH))
        .h_full()
        .relative()
        // Continuous rule down the whole track stack.
        .child(
            div()
                .absolute()
                .left(px(12.))
                .top(px(17.))
                .bottom_0()
                .w(px(2.))
                .bg(theme.playhead),
        )
        // Rounded cap.
        .child(
            div()
                .absolute()
                .left(px(6.))
                .top_0()
                .w(px(14.))
                .h(px(17.))
                .rounded(px(6.))
                .bg(theme.playhead)
                .shadow_sm(),
        )
        // Grip: the handle the pointer takes hold of.
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(grip_top))
                .w(px(Theme::SCRUBBER_WIDTH))
                .h(px(Theme::SCRUBBER_GRIP_HEIGHT))
                .rounded(px(13.))
                .bg(theme.playhead.opacity(0.55))
                .border_1()
                .border_color(theme.toggle_thumb.opacity(0.35))
                .shadow_lg()
                .children((0..3).map(|r| dot(r, 8.0)))
                .children((0..3).map(|r| dot(r, 15.0))),
        )
}

// ---------------------------------------------------------------------------
// dropdown
// ---------------------------------------------------------------------------

/// A retained dropdown: keyboard navigation, selected state, and a native GPUI popover.
pub struct Dropdown {
    focus: FocusHandle,
    pub items: Vec<String>,
    pub selected: usize,
    pub enabled: bool,
    pub theme: Theme,
    open: bool,
    highlighted: usize,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    change: Box<dyn Fn(usize, &mut Window, &mut App)>,
}

impl Dropdown {
    pub fn set_handler(&mut self, change: impl Fn(usize, &mut Window, &mut App) + 'static) {
        self.change = Box::new(change);
    }
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }
    pub fn new(
        cx: &mut Context<Self>,
        items: Vec<String>,
        selected: usize,
        theme: Theme,
        change: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            focus: cx.focus_handle(),
            items,
            selected,
            enabled: true,
            theme,
            open: false,
            highlighted: selected,
            bounds: Rc::new(Cell::new(Bounds::default())),
            change: Box::new(change),
        }
    }
    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.enabled && index < self.items.len() {
            self.selected = index;
            (self.change)(index, window, cx);
        }
        self.open = false;
        cx.notify();
    }
}

impl Render for Dropdown {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let open = self.open;
        let label = self
            .items
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("—")
            .to_owned();

        let mut root = div()
            .flex()
            .flex_col()
            .id("dropdown")
            .relative()
            .w_full()
            .min_w_0()
            .track_focus(&self.focus)
            .tab_index(0)
            .tab_stop(self.enabled)
            .child(measure(self.bounds.clone()))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if !this.enabled {
                    return;
                }
                match e.keystroke.key.as_str() {
                    "escape" => this.open = false,
                    "down" => {
                        this.open = true;
                        this.highlighted =
                            (this.highlighted + 1).min(this.items.len().saturating_sub(1));
                    }
                    "up" => {
                        this.open = true;
                        this.highlighted = this.highlighted.saturating_sub(1);
                    }
                    "enter" | "space" => {
                        if this.open {
                            this.choose(this.highlighted, w, cx);
                        } else {
                            this.open = true;
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .id("trigger")
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(Theme::GAP))
                    .h(px(Theme::CONTROL_HEIGHT))
                    .px(px(Theme::CONTROL_PADDING))
                    .rounded(px(Theme::RADIUS_CONTROL))
                    .bg(if open { t.hover } else { t.surface })
                    .text_size(px(Theme::FONT_CONTROL))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .opacity(if self.enabled {
                        1.
                    } else {
                        Theme::DISABLED_OPACITY
                    })
                    .when(self.enabled, |s| {
                        s.cursor_pointer().hover(move |s| s.bg(t.hover))
                    })
                    .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                    .child(icon_sized(
                        "CaretDown-regular",
                        Theme::ICON_SIZE_SMALL,
                        t.muted,
                    ))
                    .on_click(cx.listener(|this, _, w, cx| {
                        if !this.enabled {
                            return;
                        }
                        w.focus(&this.focus, cx);
                        this.open = !this.open;
                        this.highlighted = this.selected;
                        cx.notify();
                    })),
            );

        if open {
            root = root.child(
                deferred(frost::frosted(
                    Theme::RADIUS_CARD,
                    frost::MENU_BLUR,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .id("choices")
                        .absolute()
                        .top(px(Theme::CONTROL_HEIGHT + Theme::GAP_SMALL))
                        .left_0()
                        .min_w(px(180.))
                        .max_h(px(280.))
                        .overflow_y_scroll()
                        .p(px(Theme::GAP_SMALL))
                        .bg(t.popup)
                        .border_1()
                        .border_color(t.border)
                        .rounded(px(Theme::RADIUS_CARD))
                        .shadow_lg()
                        .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            if !this.bounds.get().contains(&event.position) {
                                this.open = false;
                                cx.notify();
                            }
                        }))
                        .children(self.items.iter().enumerate().map(|(i, label)| {
                            let highlighted = i == self.highlighted;
                            let selected = i == self.selected;
                            div()
                                .id(("choice", i))
                                .flex()
                                .items_center()
                                .h(px(32.))
                                .px(px(Theme::GAP_LARGE))
                                .rounded(px(Theme::RADIUS_SMALL))
                                .text_size(px(Theme::FONT_CONTROL))
                                .text_color(if selected { t.accent_text } else { t.text })
                                .bg(if selected {
                                    t.selection
                                } else if highlighted {
                                    t.hover
                                } else {
                                    t.hover.opacity(0.0)
                                })
                                .cursor_pointer()
                                .hover(move |s| s.bg(t.hover))
                                .child(label.clone())
                                .on_click(cx.listener(move |this, _, w, cx| this.choose(i, w, cx)))
                        })),
                ))
                .with_priority(20),
            );
        }
        root
    }
}
