//! The button and its variants, plus the editor rail's icon action.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{icon_sized, motion, perf, row, tooltip};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Secondary,
    Primary,
    Ghost,
    Outline,
    Danger,
    Record,
    /// Text-only accent action — the reference's "Reset" and "Advanced".
    /// It carries no plate, so it never competes with the row it labels.
    Link,
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
    /// Full-width, left-aligned: the shape a control takes as a menu row.
    menu_item: bool,
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
        menu_item: false,
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
    /// A row in a menu: full width, caption left, ellipsised. Every list of
    /// choices in the app is built from this, so the dropdown's rows and the
    /// command palette's rows cannot drift apart.
    pub fn menu_item(mut self) -> Self {
        self.menu_item = true;
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

    pub fn link(mut self) -> Self {
        self.variant = ButtonVariant::Link;
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
        let theme = self.theme;
        let tip = self.label.clone();
        let danger = matches!(self.variant, ButtonVariant::Danger | ButtonVariant::Record);
        let link = self.variant == ButtonVariant::Link;

        // Two states, no hues: a control is either FILLED — the solid
        // achromatic plate with its glyph inverted on top — or a translucent
        // wash of the same grey. Primary actions and anything switched on
        // take the fill; everything else washes. Colour in the interface
        // comes from the desktop behind the glass, never from a control.
        let filled = self.variant == ButtonVariant::Primary || self.selected;
        let (washed, washed_hover) = if danger {
            (theme.danger.opacity(0.10), theme.danger.opacity(0.22))
        } else if matches!(
            self.variant,
            ButtonVariant::Ghost | ButtonVariant::Outline | ButtonVariant::Link
        ) {
            (theme.hover.opacity(0.0), theme.hover)
        } else {
            (theme.surface, theme.hover)
        };

        // Selection is a state the control HOLDS, so it tweens from render;
        // hover is an event, so it tweens from a listener. Both run in the
        // same store, which is what lets a control that is hovered while it
        // is switched on cross-fade along both axes at once instead of
        // snapping to whichever the last frame happened to compute.
        let fill = motion::state_fade(&motion::tween_key(&self.id, "fill"), filled);
        let rest = motion::blend(washed, theme.accent, fill);
        let hover = motion::blend(washed_hover, theme.accent_hover, fill);
        let hover_key = motion::tween_key(&self.id, "hover");
        let background = motion::hover_blend(&hover_key, rest, hover);
        let resting = if danger {
            theme.danger
        } else if link {
            theme.accent
        } else {
            theme.text
        };
        let content = motion::blend(resting, theme.on_accent, fill);
        // A ring drawn in the plate colour would vanish on a filled control.
        let focus_ring = motion::blend(theme.accent, theme.on_accent, fill);

        let radius = if self.round {
            Theme::CONTROL_HEIGHT / 2.0
        } else if link {
            Theme::RADIUS_SMALL
        } else {
            Theme::RADIUS_CONTROL
        };

        let click_id = self.id.clone();
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_shrink_0()
            .items_center()
            .map(|el| {
                if self.menu_item {
                    el.w_full().justify_start()
                } else {
                    el.justify_center()
                }
            })
            .gap(px(Theme::GAP))
            .h(px(if link {
                Theme::CHIP_HEIGHT
            } else {
                Theme::CONTROL_HEIGHT
            }))
            .rounded(px(radius))
            .bg(background)
            .text_color(content)
            .text_size(px(if link {
                Theme::FONT_SMALL
            } else {
                Theme::FONT_CONTROL
            }))
            .font_weight(FontWeight::MEDIUM)
            .opacity(if self.enabled {
                1.
            } else {
                Theme::DISABLED_OPACITY
            });

        if self.icon_only {
            el = el.w(px(Theme::CONTROL_HEIGHT));
        } else if link {
            el = el.px(px(Theme::GAP_SMALL));
        } else {
            el = el.px(px(Theme::CONTROL_PADDING));
            if self.stretch {
                el = el.flex_1().min_w_0();
            }
            // A row is already full-width, and `flex_1` inside the menu's
            // column would grow it along the wrong axis.
            if self.menu_item {
                el = el.flex_none();
            }
        }
        if self.variant == ButtonVariant::Outline {
            el = el.border_1().border_color(theme.border);
        }
        if danger {
            el = el.border_1().border_color(theme.danger.opacity(0.4));
        }

        if let Some(name) = &self.glyph {
            el = el.child(icon_sized(name, self.glyph_size, content));
        }
        if !self.icon_only {
            el = el.child(
                div()
                    .when(self.stretch || self.menu_item, |s| s.flex_1().min_w_0())
                    .text_ellipsis()
                    .child(self.label.clone()),
            );
        }
        // Only a control whose caption cannot be read needs a tooltip to name
        // it: one with no caption at all, or a stretched one, which is
        // exactly the case that ellipsizes. "Export" hovering to reveal a
        // tooltip that says "Export" is noise.
        if self.icon_only || self.stretch || self.menu_item {
            el = el.tooltip(move |_, cx| tooltip(tip.clone(), theme, cx));
        }

        if self.enabled {
            // GPUI synthesizes ClickEvent::Keyboard for Enter/Space on focused divs.
            el = el
                .cursor_pointer()
                .tab_index(0)
                // `focus_visible`, not `focus`: the fork gates this on
                // `last_input_was_keyboard`, exactly like CSS
                // `:focus-visible`. With plain `focus` every click left a
                // ring behind on the control it just pressed.
                .focus_visible(move |s| s.border_2().border_color(focus_ring))
                // A press should land the instant the finger does, so it
                // stays an immediate style; only the release fades back.
                .active(|s| s.opacity(Theme::PRESSED_OPACITY))
                .on_hover(motion::hover_listener(hover_key));
            if let Some(handler) = self.handler {
                let id = click_id;
                el = el.on_click(move |e, w, cx| {
                    perf::log(format_args!("click button {id:?}"));
                    handler(e, w, cx)
                });
            }
        }
        el
    }
}

// ---------------------------------------------------------------------------
// rail button
// ---------------------------------------------------------------------------

/// The editor's left rail: an icon action and its open-panel marker.
pub fn rail_button(
    id: impl Into<ElementId>,
    glyph: &str,
    label: impl Into<SharedString>,
    active: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    let id = id.into();
    // The rail is icons only: the captions cost a third of the rail's width
    // for text that repeats the tooltip, and the panel they open names itself
    // in its own heading. Which one is open reads from the marker instead.
    let lit = motion::state_fade(&motion::tween_key(&id, "rail"), active);
    row()
        .flex_none()
        .gap(px(Theme::GAP_SMALL))
        .h(px(Theme::RAIL_BUTTON_HEIGHT))
        .child(
            button(id, label, theme)
                .glyph(glyph)
                .icon_only()
                .selected(active)
                .on_click(on_click),
        )
        .child(
            div()
                .flex_none()
                .size(px(Theme::DOT_SIZE))
                .rounded(px(Theme::DOT_SIZE / 2.0))
                .bg(motion::blend(theme.accent.opacity(0.), theme.accent, lit)),
        )
}
