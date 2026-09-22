//! The button and its variants, plus the editor rail's icon action.
//!
//! Every labelled button in the redesign is a full pill and every icon button
//! is a circle, so no variant sets a radius: the shape follows from whether
//! there is a caption. What a variant chooses is its fill, its height and how
//! much air sits around the label — and the height then picks the glyph size,
//! because a glyph tracks the control it sits in rather than deciding for
//! itself.

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, Theme};

use crate::{icon_sized, motion, perf, tooltip};

/// Private on purpose: every variant has a named builder, so a caller says
/// `.primary()` rather than naming an enum, and there is one way to ask for
/// each shape instead of two.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    Secondary,
    Primary,
    /// A control sitting directly on glass, with a hairline of its own so it
    /// separates from the material rather than from a plate.
    Raised,
    Ghost,
    Danger,
    Record,
    /// Play and pause — the one circular `ink` button. Achromatic on purpose:
    /// the accent already marks four things, and "the video is playing" is
    /// not one of them.
    Transport,
}

impl ButtonVariant {
    /// The height a variant takes unless the caller asks for another. Record
    /// is the tallest control in the app because it is the one that must
    /// never be hit by accident.
    fn height(self) -> f32 {
        match self {
            Self::Primary | Self::Secondary | Self::Danger => Theme::CONTROL_HEIGHT_LARGE,
            Self::Raised | Self::Ghost => Theme::CONTROL_HEIGHT,
            Self::Transport => Theme::TRANSPORT_SIZE,
            Self::Record => Theme::RECORD_HEIGHT,
        }
    }

    /// Side padding. A pill has no plate edge to speak of, so its padding is
    /// what gives it its width.
    fn padding(self) -> f32 {
        match self {
            Self::Primary => Theme::CONTROL_PADDING_PRIMARY,
            Self::Secondary | Self::Danger => Theme::CONTROL_PADDING_LARGE,
            Self::Raised => Theme::CONTROL_PADDING,
            Self::Ghost => Theme::CONTROL_PADDING_SMALL,
            Self::Transport | Self::Record => Theme::CONTROL_PADDING_HERO,
        }
    }
}

/// The glyph size for a control of this height — the pairing the icon scale
/// is built around. Anything taller than a hero button is Record-sized; the
/// transport asks for its own size, because it is the one control whose glyph
/// is the whole control.
fn glyph_for(height: f32) -> f32 {
    if height <= Theme::CONTROL_HEIGHT_SMALL {
        Theme::ICON_SIZE_SMALL
    } else if height <= Theme::CONTROL_HEIGHT {
        Theme::ICON_SIZE
    } else if height <= Theme::CONTROL_HEIGHT_LARGE {
        Theme::ICON_SIZE_MEDIUM
    } else {
        Theme::ICON_SIZE_LARGE
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    theme: Theme,
    variant: ButtonVariant,
    glyph: Option<SharedString>,
    glyph_size: Option<f32>,
    height: Option<f32>,
    /// Hide the caption and render a round icon-only control.
    icon_only: bool,
    /// A quieter second line under the caption: a display's resolution, a
    /// device's subtitle.
    detail: Option<SharedString>,
    /// A trailing caret: this control opens something.
    caret: bool,
    /// Set the caption in the mono face — a duration, a count, anything whose
    /// digits must not shove the glyph beside them as they change.
    mono: bool,
    /// Full-width, left-aligned: the shape a control takes as a menu row.
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
        glyph_size: None,
        height: None,
        icon_only: false,
        detail: None,
        caret: false,
        mono: false,
        selected: false,
        enabled: true,
        stretch: false,
        handler: None,
    }
}

/// A round icon-only control. It takes its variant's height as its diameter,
/// so an icon button beside a labelled one is the same size as it is tall.
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

    /// Override the glyph size. Rarely wanted: the height already picks one.
    pub fn glyph_size(mut self, size: f32) -> Self {
        self.glyph_size = Some(size);
        self
    }

    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// The dense size — a control in a packed row, a menu item, a pill inside
    /// a pod.
    /// A second line under the caption, muted and a step down, so the pill
    /// reads as one thing with a name and a detail rather than two labels.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A trailing caret. The control opens a list, a panel or a menu.
    pub fn caret(mut self) -> Self {
        self.caret = true;
        self
    }

    /// The caption in the mono face: a countdown, an elapsed clock, a count.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// The recorder bar's height. The bar is one row of the app's largest
    /// controls, so its pills and its round buttons stand as tall as the
    /// Record button beside them.
    pub fn bar(mut self) -> Self {
        self.height = Some(Theme::RECORD_HEIGHT);
        self
    }

    /// The 44 step: the height a Secondary control already takes, for a
    /// variant that would otherwise sit at 40.
    pub fn large(mut self) -> Self {
        self.height = Some(Theme::CONTROL_HEIGHT_LARGE);
        self
    }

    pub fn small(mut self) -> Self {
        self.height = Some(Theme::CONTROL_HEIGHT_SMALL);
        self
    }

    /// The hero size: the one action an otherwise empty screen is asking for.
    pub fn hero(mut self) -> Self {
        self.height = Some(Theme::CONTROL_HEIGHT_HERO);
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    /// Starts capture. The one red fill in the app.
    pub fn record(mut self) -> Self {
        self.variant = ButtonVariant::Record;
        self
    }

    pub fn transport(mut self) -> Self {
        self.variant = ButtonVariant::Transport;
        self
    }

    pub fn raised(mut self) -> Self {
        self.variant = ButtonVariant::Raised;
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

/// The focus ring: a 3px spread of `accent_soft` drawn as a shadow, not a
/// border, so gaining focus cannot move the control or its neighbours. This
/// is what the redesign means by "there are no real borders anywhere".
pub fn focus_ring(theme: Theme) -> BoxShadow {
    BoxShadow {
        color: theme.accent_soft,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(Theme::FOCUS_WIDTH),
        inset: false,
    }
}

/// A glow under a filled control. Only the accent and Record carry one — it
/// is how those two say they are the action, without another colour.
fn glow(color: Hsla, blur: f32, offset: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(offset)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// A hairline drawn inside the control's own edge.
pub fn hairline(color: Hsla, width: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset: true,
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = self.theme;
        let tip = self.label.clone();
        let height = self.height.unwrap_or_else(|| self.variant.height());
        let glyph_size = self.glyph_size.unwrap_or_else(|| match self.variant {
            ButtonVariant::Transport => Theme::ICON_SIZE_TRANSPORT,
            _ => glyph_for(height),
        });

        // Selection is not a fill swap. The handoff is explicit twice over —
        // "selected is `inset 0 0 0 1.5px --accent` over `--sunk` or
        // `--card`, never a fill swap", and again under button states, "not
        // a button state". A selected control keeps the plate it already had
        // and gains an accent edge, so a row of choices reads as one row of
        // choices with one of them marked, instead of one blue button beside
        // some grey ones.
        //
        // The one exception is the icon button's "active tool" state, which
        // the Icon card does give as `fill --accent · text --on-accent`.
        // That is the accent doing one of its four jobs: the tool pod, the
        // snap toggle. A glyph has no room for an edge to read against, and
        // an active tool is a mode the whole app is in rather than one item
        // picked from a list.
        let tool_active = self.selected && self.icon_only;
        let filled = matches!(
            self.variant,
            ButtonVariant::Primary | ButtonVariant::Record | ButtonVariant::Transport
        ) || tool_active;
        let marked = self.selected && !self.icon_only;
        let (fill_rest, fill_hover) = match self.variant {
            ButtonVariant::Transport => (theme.ink, theme.ink),
            ButtonVariant::Record => (theme.rec, theme.rec),
            // Palette churn: `--danger` at 10% resting, which is a fill the
            // handoff's "no fill until hover" line would forbid. That line is
            // written under menu items, about a destructive *row* in a list
            // of rows; this is a standalone button, where a bare red caption
            // on glass reads as text rather than as the thing that deletes.
            // Kept, and noted here so it is a decision rather than a drift.
            ButtonVariant::Danger => (theme.danger.opacity(0.10), theme.danger.opacity(0.22)),
            ButtonVariant::Ghost => (theme.hover.opacity(0.0), theme.hover),
            ButtonVariant::Raised => (theme.raise, theme.raise_hover()),
            _ => (theme.sunk, theme.sunk2),
        };

        // Selection is a state the control HOLDS, so it tweens from render;
        // hover is an event, so it tweens from a listener. Both run in the
        // same store, which is what lets a control that is hovered while it
        // is switched on cross-fade along both axes at once instead of
        // snapping to whichever the last frame happened to compute.
        let fill = motion::state_fade(&motion::tween_key(&self.id, "fill"), filled);
        let (accent, accent_hover, on_plate) = match self.variant {
            ButtonVariant::Record => (theme.rec, theme.rec, theme.thumb()),
            ButtonVariant::Transport => (theme.ink, theme.ink, theme.on_ink),
            _ => (theme.accent, theme.accent_hover, theme.on_accent),
        };
        let rest = motion::blend(fill_rest, accent, fill);
        let hover = motion::blend(fill_hover, accent_hover, fill);
        let hover_key = motion::tween_key(&self.id, "hover");
        let background = motion::hover_blend(&hover_key, rest, hover);
        // Idle icon-only controls sit at `muted` so a pod of six of them
        // reads as one object; anything with a caption is at full strength.
        let resting = match self.variant {
            ButtonVariant::Danger => theme.danger,
            ButtonVariant::Ghost => theme.muted,
            _ if self.icon_only => theme.muted,
            _ => theme.text,
        };
        let content = motion::blend(resting, on_plate, fill);

        let click_id = self.id.clone();
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(if self.variant == ButtonVariant::Record {
                Theme::ICON_GAP_RECORD
            } else {
                Theme::ICON_GAP
            }))
            .h(px(height))
            .rounded_full()
            .bg(background)
            .text_color(content)
            .text_size(px(if self.variant == ButtonVariant::Record {
                Theme::FONT_ACTION
            } else {
                Theme::FONT_BODY
            }))
            // 500 on a filled control and on Record, 400 elsewhere: weight is
            // the quiet half of what marks the primary action.
            .font_weight(if filled || marked {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            })
            .opacity(if self.enabled {
                1.
            } else {
                Theme::DISABLED_OPACITY
            });

        if self.icon_only {
            el = el.w(px(height));
        } else {
            el = el.px(px(self.variant.padding()));
            if self.stretch {
                el = el.flex_1().min_w_0();
            }
        }

        // Shadows, in the order the redesign layers them: a raised control's
        // hairline always, a glow only while the plate is filled and enabled.
        let mut shadows = Vec::new();
        if self.variant == ButtonVariant::Raised {
            shadows.push(hairline(theme.raise_line, Theme::BORDER_WIDTH));
        }
        // The mark a selected control carries: an inset edge, so gaining or
        // losing it cannot change what the control measures.
        if marked {
            shadows.push(hairline(theme.accent, Theme::SELECTED_WIDTH));
        }
        // Only the accent and Record glow. The transport is filled too, but
        // it is `ink` — a glow would make the quietest control in the player
        // bar look like the loudest.
        if self.enabled && filled {
            match self.variant {
                ButtonVariant::Transport => {}
                ButtonVariant::Record => shadows.push(glow(theme.rec.opacity(0.32), 26., 10.)),
                // A hero button glows one step wider than a primary one. It
                // is the same accent saying the same thing, on a screen with
                // nothing else on it to say it against.
                _ if height >= Theme::CONTROL_HEIGHT_HERO => {
                    shadows.push(glow(theme.accent_soft, 28., 12.))
                }
                _ => shadows.push(glow(theme.accent_soft, 20., 8.)),
            }
        }
        if !shadows.is_empty() {
            el = el.shadow(shadows.clone());
        }
        // A raised control lifts under the pointer: the same hairline plus a
        // `0 2 6`. gpui's `.shadow` replaces the whole stack rather than
        // appending to it, so the hovered state restates what the resting one
        // already carries.
        if self.variant == ButtonVariant::Raised && self.enabled {
            let mut lifted = shadows;
            lifted.push(theme.lift_shadow());
            el = el.hover(move |s| s.shadow(lifted.clone()));
        }

        // Record wears its state: a white dot ahead of whatever the caption
        // says.
        //
        // The card also gives a recording state — the dot pulsing on a 1s
        // cycle and the caption swapping to the elapsed time in mono. Neither
        // is reachable here: the recorder bar swaps its whole contents when
        // capture starts, so this button is never on screen while recording,
        // and the elapsed clock is rendered by the bar itself. Building the
        // pulse would be building something nothing can show.
        if self.variant == ButtonVariant::Record {
            el = el.child(
                div()
                    .flex_none()
                    .size(px(Theme::RECORD_DOT))
                    .rounded_full()
                    .bg(theme.thumb()),
            );
        }
        if let Some(name) = &self.glyph {
            el = el.child(icon_sized(name, glyph_size, content));
        }
        if !self.icon_only {
            let caption = div()
                .when(self.mono, |s| s.font_family(FONT_MONO))
                .text_ellipsis()
                .child(self.label.clone());
            el = el.child(match self.detail {
                // Two lines in one pill. The detail stays `muted` rather than
                // following the caption onto a filled plate: a pill that
                // carries a detail is a `sunk` one by construction — it is a
                // thing you pick, not the action you take.
                Some(detail) => div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .when(self.stretch, |s| s.flex_1())
                    .child(caption)
                    .child(
                        div()
                            .text_size(px(Theme::FONT_SMALL))
                            .text_color(theme.muted)
                            .text_ellipsis()
                            .child(detail),
                    )
                    .into_any_element(),
                None => caption
                    .when(self.stretch, |s| s.flex_1().min_w_0())
                    .into_any_element(),
            });
        }
        if self.caret {
            el = el.child(icon_sized(
                "CaretDown-regular",
                Theme::ICON_SIZE_CARET,
                theme.muted,
            ));
        }
        // Only a control whose caption cannot be read needs a tooltip to name
        // it: one with no caption at all, or a stretched one, which is
        // exactly the case that ellipsizes. "Export" hovering to reveal a
        // tooltip that says "Export" is noise.
        if self.icon_only || self.stretch {
            el = el.tooltip(move |_, cx| tooltip(tip.clone(), theme, cx));
        }

        if self.enabled {
            // GPUI synthesizes ClickEvent::Keyboard for Enter/Space on focused divs.
            let ring = focus_ring(theme);
            let press = theme.press;
            el = el
                .cursor_pointer()
                .tab_index(0)
                // `focus_visible`, not `focus`: the fork gates this on
                // `last_input_was_keyboard`, exactly like CSS
                // `:focus-visible`. With plain `focus` every click left a
                // ring behind on the control it just pressed.
                .focus_visible(move |s| s.shadow(vec![ring]))
                // The redesign presses with `scale(.97)`, which gpui at this
                // revision cannot do to a div, so the press is the `press`
                // fill plus a dim. Both land immediately rather than through
                // the tween store: the feedback has to arrive with the
                // finger, and only the release fades back.
                .active(move |s| s.bg(press).opacity(Theme::PRESSED_OPACITY))
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
/// A tool-pod entry: one round button carrying a glyph and nothing else.
///
/// The active tool is one of the four things the accent is allowed to mark, so
/// it takes the accent fill outright. That is the exception to "selection is an
/// inset edge, never a fill swap": the pod has no captions and no other state
/// to read, so the fill *is* the state.
pub fn tool_button(
    id: impl Into<ElementId>,
    glyph: &str,
    label: impl Into<SharedString>,
    active: bool,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    let mut el = button(id, label, theme).glyph(glyph).icon_only();
    el = if active { el.primary() } else { el.ghost() };
    el.on_click(on_click)
}
