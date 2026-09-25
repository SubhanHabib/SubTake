//! The recorder's option cards: batch 1, screen 2a of the round-2 handoff.
//!
//! One card per bar control — Source, Audio, Camera, Countdown, More — each
//! drawn in the options window, which floats over the control that opened it
//! (`set_launcher_options_anchor`). Only one is open at a time; opening
//! another replaces it in place with a short cross-fade.

use super::*;
use crate::ui_state::CaptureSource;
use subtake_ui::{icon_sized, layered};

mod audio;
mod camera;
mod countdown;
mod more;
mod source;

impl RootView {
    pub(super) fn options(
        &mut self,
        state: &RecordingOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        // Closing or replaced, the card keeps what it last showed while it
        // fades out.
        let (height, opacity, rise) = self.card_fade(state, window);
        let name = self.card_last.0.clone();
        let title = match name.as_str() {
            "sources" => "Capture source",
            "audio" => "Audio",
            "camera" => "Camera",
            "countdown" => "Countdown delay",
            _ => "More",
        };
        let close = {
            let options = state.clone();
            move |_: &ClickEvent, _: &mut Window, _: &mut App| options.defer_panel("".into())
        };
        let header = row()
            .gap(px(Theme::icon_gap_row()))
            .child(context_chip(theme, &["Recorder", title]).min_w_0())
            .child(div().flex_1())
            .child(
                icon_button("close", "X-regular", "Close", theme)
                    .small()
                    .on_click(close),
            );
        let body = match name.as_str() {
            "sources" => self.source_card(state),
            "audio" => self.audio_card(state, cx),
            "camera" => self.camera_card(state, cx),
            "countdown" => self.countdown_card(state),
            _ => self.more_card(state),
        };
        let content = column()
            .gap(px(Theme::gap_large()))
            .child(header)
            .children(body);

        // The card's own fill is `card`, not the bar's `glass`: it holds
        // text. The window's material is masked to it at the panel radius.
        // The window cannot draw a drop shadow outside itself, so the panel
        // shadow the handoff gives is the window server's to draw.
        //
        // The content lays out at its own height and is measured there, and
        // the card and its window take that height in one step; the card is
        // clear whenever its window moves or resizes (`card_fade`). It rests
        // `MENU_IN_RISE` up its window, the room it rises through.
        panel_variant(theme, UiSurface::Overlay)
            .absolute()
            .bottom(px(rise))
            .left_0()
            .right_0()
            .h(px(height))
            .opacity(opacity)
            .overflow_hidden()
            .rounded(px(Theme::radius_panel()))
            .bg(theme.card)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .p(px(Theme::panel_padding()))
                    .child(content)
                    .child(options_fit(
                        state.clone(),
                        self.card_measured.clone(),
                        self.card_floor.clone(),
                    )),
            )
            .into_any_element()
    }

    /// Steps the card through its fade (`CardFade`) and gives the height to
    /// draw it at, what its content last measured, with its opacity and how
    /// far up its window it stands.
    ///
    /// It comes and goes as a menu does: the entrance `menu_in_above` draws,
    /// fading in and rising onto its place, then `Leave`'s fade out, and the
    /// window hides once it is clear. The frosted material under it is the
    /// window server's, not GPUI's, and is given the same opacity and place
    /// each frame. The window itself stays opaque: the window server blurs
    /// what is behind a window at its own pace rather than at the window's
    /// alpha, and a card faded by its window showed an empty pane of frost
    /// before its rows came in and after they had gone.
    fn card_fade(&mut self, state: &RecordingOptions, window: &mut Window) -> (f32, f32, f32) {
        let rise = subtake_ui::motion::MENU_IN_RISE;
        let natural = state.get_options_height() - rise;
        let now = Instant::now();
        let swap = if subtake_ui::motion::reduced_motion() {
            0
        } else {
            CARD_SWAP_MS
        };
        let panel = state.get_panel();
        let open = !panel.is_empty();
        let wanted = (panel, state.window().opens());
        let leave = self.card_leave.shown(open);
        if leave.is_none() {
            if self.card_fade != CardFade::Gone {
                self.card_fade = CardFade::Gone;
                let state = state.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    if state.get_panel().is_empty() {
                        let _ = state.hide();
                    }
                });
            }
        } else if open && self.card_last != wanted {
            self.card_fade = match self.card_fade {
                // Replaced while in view: this card fades out first, still
                // drawn, and the window moves once it is clear.
                CardFade::Shown(_) if self.card_last.1 == wanted.1 => {
                    CardFade::Leaving(now, self.card_opacity)
                }
                CardFade::Leaving(since, from)
                    if subtake_ui::motion::progress(since, swap.max(1), now) < 1. =>
                {
                    CardFade::Leaving(since, from)
                }
                _ => {
                    self.card_last = wanted;
                    CardFade::Waiting(0)
                }
            };
        }
        // Reopened as it faded out, it comes in afresh, as a menu does.
        if open
            && std::mem::replace(&mut self.card_opens, self.card_leave.opens)
                != self.card_leave.opens
        {
            if let CardFade::Shown(_) = self.card_fade {
                self.card_fade = CardFade::Shown(now);
            }
        }
        // Not before the window has taken the height of the card's measured
        // rows, and then not for a few frames more: a resized window's first
        // frames reach the screen late, and the frost under a card that set
        // off at once showed empty until they came.
        if let CardFade::Waiting(frames) = self.card_fade {
            let fits = |height: f32| (height - natural).abs() < 0.5;
            let fitted = fits(self.card_measured.get())
                && fits(f32::from(window.viewport_size().height) - rise);
            if fitted && frames >= CARD_SETTLE_FRAMES {
                // The window shows clear (`ui_runtime::Window::show`); the
                // card is clear too until its entrance begins.
                if let Some(view) = state.window().native_view() {
                    unsafe { crate::platform::ui_fade_launcher_options(view, 1., 0.) }
                }
                self.card_fade = CardFade::Shown(now);
            } else {
                self.card_fade = CardFade::Waiting(if fitted { frames + 1 } else { 0 });
            }
        }
        let (opacity, lift) = match self.card_fade {
            CardFade::Gone | CardFade::Waiting(_) => (0., 0.),
            CardFade::Shown(started) => {
                let t = subtake_ui::motion::menu_entrance(started, now);
                (t * leave.unwrap_or(0.), t)
            }
            CardFade::Leaving(since, from) => (
                subtake_ui::motion::ease_toward(from, 0., since, swap, now),
                1.,
            ),
        };
        if !matches!(self.card_fade, CardFade::Gone) && (opacity < 1. || lift < 1. || !open) {
            window.request_animation_frame();
        }
        self.card_opacity = opacity;
        let height = self.card_resize(natural, window);
        // The frosted material under the card is the card's height, not the
        // window's: the window is resized a moment before its paint catches
        // up, and glass that filled it would show past the card's edge.
        let bottom = rise * lift;
        crate::platform::set_recorder_glass_fade(state.window(), bottom, opacity);
        crate::platform::set_recorder_glass_height(state.window(), height.max(0.01));
        (height, opacity, bottom)
    }

    /// The height to draw a shown card at as its content changes — a
    /// source list arriving, an error line, the refresh row turning into
    /// its spinner: it eases there over `RESIZE_MS`, card and glass
    /// together, rather than snapping. Growing, the window takes the new
    /// height at once and the card grows up into it; shrinking, the window
    /// holds at least the card's drawn height (`card_floor`) until the card
    /// is down, then drops the clear space above it. A card that is not in view is simply its
    /// content's height, since its window is clear while that changes.
    fn card_resize(&mut self, natural: f32, window: &mut Window) -> f32 {
        let measured = self.card_measured.get();
        if !matches!(self.card_fade, CardFade::Shown(_)) || measured <= 0. {
            self.card_resize = None;
            self.card_floor.set(0.);
            return natural;
        }
        let now = Instant::now();
        let (from, to, started) = *self.card_resize.get_or_insert((measured, measured, now));
        let mut height = subtake_ui::motion::ease_toward(from, to, started, RESIZE_MS, now);
        if (measured - to).abs() >= 1. {
            if subtake_ui::motion::reduced_motion() {
                height = measured;
            }
            self.card_resize = Some((height, measured, now));
        }
        // The floor is set before the content under it is measured, so a
        // card that has just got shorter keeps its window for the frame
        // before its move begins, too.
        let (from, to, _) = self.card_resize.unwrap_or((height, height, now));
        if height == to {
            self.card_floor.set(height);
        } else {
            self.card_floor.set(from.max(to));
            window.request_animation_frame();
        }
        height
    }
}

/// The muted sentence a card ends on.
fn helper(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .text_size(px(Theme::font_secondary()))
        .line_height(relative(Theme::helper_leading()))
        .text_color(theme.muted)
        .child(text.into())
}

/// The 1.5 accent inset a chosen row carries, on its own layer over the
/// row's fill. `None` is a pill.
fn selection_ring(radius: Option<f32>, theme: Theme) -> impl IntoElement {
    let ring = div()
        .absolute()
        .inset_0()
        .shadow(vec![hairline(theme.accent, Theme::selected_width())]);
    layered(match radius {
        Some(r) => ring.rounded(px(r)),
        None => ring.rounded_full(),
    })
}

/// Sizes the options window to the card: the card lays out at its natural
/// height, this reads it back, and the window follows — so a card never
/// clips and never floats in an empty frame, whatever it holds. While the
/// card eases smaller the window holds at `floor`.
fn options_fit(
    state: RecordingOptions,
    measured: Rc<Cell<f32>>,
    floor: Rc<Cell<f32>>,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let content = f32::from(bounds.size.height).ceil();
            if (measured.replace(content) - content).abs() > 0.5 {
                // The card eases toward it from the next frame.
                window.request_animation_frame();
            }
            // With the room under the card it rises through.
            let height = content.max(floor.get()) + subtake_ui::motion::MENU_IN_RISE;
            if (state.get_options_height() - height).abs() > 0.5 {
                let state = state.clone();
                crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                    state.set_options_height(height)
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}
