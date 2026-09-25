//! The recorder's option cards: batch 1, screen 2a of the round-2 handoff.
//!
//! One card per bar control — Source, Audio, Camera, Countdown, More — each
//! drawn in the options window, which floats over the control that opened it
//! (`set_launcher_options_anchor`). Only one is open at a time; opening
//! another replaces it in place with a short cross-fade.

use super::*;
use crate::ui_state::{CaptureSource, UiHandle};
use subtake_ui::{icon_sized, layered};

mod audio;
mod camera;
mod countdown;
mod more;
mod parts;
pub(super) mod source;

use source::SOURCE_SEARCH;

impl RootView {
    pub(super) fn options(
        &mut self,
        state: &RecordingOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        self.card_blur(state, window);
        // Closing or replaced, the card keeps what it last showed while it
        // fades out.
        let (height, opacity, rise) = self.card_fade(state, window, cx);
        let name = self.card_last.0.clone();
        let face = CardFace::of(&name);
        let header = self.card_header(&face, state);
        let body = match name.as_str() {
            "sources" => self.source_card(state, window, cx),
            "audio" => self.audio_card(state, cx),
            "camera" => self.camera_card(state, cx),
            "countdown" => self.countdown_card(state),
            _ => self.more_card(state, cx),
        };
        let content = column()
            .gap(px(Theme::recorder_card_gap()))
            .child(header)
            .children(body);

        // The card is the bar's own material, `glass` over the window's
        // frost, as the handoff draws both. The window cannot draw a drop
        // shadow outside itself, so the card shadow the handoff gives is the
        // window server's to draw.
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
            .bg(theme.glass)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .p(px(Theme::recorder_card_padding()))
                    .child(content)
                    .child(options_fit(
                        state.clone(),
                        self.card_measured.clone(),
                        self.card_floor.clone(),
                    )),
            )
            .into_any_element()
    }

    /// The row every card opens with: its glyph on a plate, its name, what
    /// it is set to now, and the one control the card keeps at its top
    /// right. There is no breadcrumb and no close button: a card closes
    /// with Esc, a click away from it, or its control on the bar again.
    fn card_header(&self, face: &CardFace, state: &RecordingOptions) -> Div {
        let theme = self.theme;
        let busy = state.get_busy();
        let slot = match face.name {
            "sources" => Some(self.refresh_button(state).into_any_element()),
            "audio" => {
                let options = state.clone();
                // Refused by the system or with none plugged in, the
                // microphone reads as off and cannot be turned on here.
                let allowed = microphone_usable(state);
                Some(
                    switch(
                        "mic-toggle",
                        state.get_microphone() && allowed,
                        !busy && allowed,
                        theme,
                        move |v, _, _| options.defer_option("microphone".into(), v.to_string()),
                    )
                    .into_any_element(),
                )
            }
            "camera" => {
                let options = state.clone();
                let allowed = camera_usable(state);
                Some(
                    switch(
                        "camera-toggle",
                        state.get_camera() && allowed,
                        !busy && allowed,
                        theme,
                        move |v, _, _| options.defer_option("camera".into(), v.to_string()),
                    )
                    .into_any_element(),
                )
            }
            "countdown" => None,
            _ => Some(studio_button(self.command("show-editor"), theme).into_any_element()),
        };
        row()
            .flex_none()
            .h(px(Theme::card_header_height() + Theme::card_header_inset()))
            .pt(px(Theme::card_header_inset()))
            .px(px(Theme::card_header_inset()))
            .gap(px(Theme::card_header_gap()))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(Theme::card_header_plate()))
                    .rounded_full()
                    .bg(theme.sunk)
                    .child(icon_sized(
                        face.glyph,
                        Theme::card_header_icon(),
                        theme.text,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    // Set close, so the two lines sit inside the plate's 40.
                    .line_height(relative(Theme::message_leading()))
                    // Not carried: the title's −0.01em tracking. gpui at the
                    // pinned revision has no letter-spacing.
                    .child(title(face.title, Theme::font_card_title()).text_ellipsis())
                    .child(
                        div()
                            .text_size(px(Theme::font_secondary()))
                            .text_color(theme.muted)
                            .text_ellipsis()
                            .child(self.card_summary(face.name, state)),
                    ),
            )
            .children(slot)
    }

    /// What a card's header says it is set to, live.
    fn card_summary(&self, name: &str, state: &RecordingOptions) -> String {
        let pick = |names: crate::ui_runtime::ModelRc<String>, index: i32| {
            usize::try_from(index)
                .ok()
                .and_then(|i| names.iter().nth(i))
                .unwrap_or_default()
        };
        match name {
            "sources" => usize::try_from(state.get_source_index())
                .ok()
                .and_then(|i| state.get_capture_sources().iter().nth(i))
                .map(|source| {
                    let (name, detail) = source::caption(&source);
                    [name, detail]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ")
                })
                .unwrap_or_else(|| "Nothing to capture yet".into()),
            "audio" if !state.get_microphone_access() => "Not allowed".into(),
            "camera" if !state.get_camera_access() => "Not allowed".into(),
            "audio" if !state.get_microphone_notice().is_empty() => state.get_microphone_notice(),
            "camera" if !state.get_camera_notice().is_empty() => state.get_camera_notice(),
            "audio" if !microphone_usable(state) => "None found".into(),
            "camera" if !camera_usable(state) => "None found".into(),
            "audio" => pick(state.get_microphone_names(), state.get_microphone_index()),
            "camera" => pick(state.get_camera_names(), state.get_camera_index()),
            "countdown" => match state.get_countdown() {
                0 => "Starts immediately".into(),
                n => format!("{n} seconds before capture"),
            },
            _ => "Recorder".into(),
        }
    }

    /// Source's refresh: it lists the displays and windows again, its
    /// arrow turning while it does.
    fn refresh_button(&self, state: &RecordingOptions) -> AnyElement {
        let theme = self.theme;
        let refresh = icon_button("refresh", "ArrowClockwise-regular", "Refresh", theme)
            .small()
            .edged()
            .glyph_size(Theme::icon_size_card())
            .enabled(!state.get_busy() && !state.get_sources_loading())
            .on_click(self.command("sources"));
        if !state.get_sources_loading() {
            return refresh.into_any_element();
        }
        // The handoff turns the arrow once, over 500ms. A refresh can run
        // longer than that, so the arrow keeps turning at that pace until the
        // list is back, and the button takes no second press meanwhile.
        div()
            .relative()
            .child(refresh)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme.sunk)
                    .child(
                        icon_sized(
                            "ArrowClockwise-regular",
                            Theme::icon_size_card(),
                            theme.text,
                        )
                        .with_animation(
                            "refresh-spin",
                            Animation::new(std::time::Duration::from_millis(
                                subtake_theme::REFRESH_SPIN_MS,
                            ))
                            .repeat(),
                            |svg, t| svg.with_transformation(Transformation::rotate(percentage(t))),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// A card closes when its window loses focus — a click on the desktop,
    /// another app, the bar's handle — but only after
    /// `CARD_BLUR_GRACE_MS`, and only if it is still the card that was
    /// open: a click on the bar's controls closes or swaps the card itself,
    /// and takes the focus from it on the way.
    fn card_blur(&mut self, state: &RecordingOptions, window: &Window) {
        let active = window.is_window_active();
        let was = self.card_active.replace(active);
        let panel = state.get_panel();
        if was && !active && !panel.is_empty() && !self.gallery_card {
            let (options, focused) = (state.clone(), self.card_active.clone());
            crate::ui_runtime::Timer::single_shot(
                std::time::Duration::from_millis(subtake_theme::CARD_BLUR_GRACE_MS),
                move || {
                    if !focused.get() && options.get_panel() == panel {
                        options.defer_panel("".into());
                    }
                },
            );
        }
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
    fn card_fade(
        &mut self,
        state: &RecordingOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> (f32, f32, f32) {
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
                // The Source card opens afresh each time: on the chosen
                // source's tab, with nothing in its search.
                self.source_tab = None;
                self.source_query.clear();
                self.source_searched = false;
                if let Some(search) = self.inputs.get(SOURCE_SEARCH) {
                    search.update(cx, |input, _| input.reset(""));
                }
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
                    // The window takes the new card's width while there is
                    // nothing in it to see.
                    let width = CardFace::of(&wanted.0).width;
                    let options = state.clone();
                    crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                        options.set_options_width(width)
                    });
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
                && fits(f32::from(window.viewport_size().height) - rise)
                && (f32::from(window.viewport_size().width)
                    - CardFace::of(&self.card_last.0).width)
                    .abs()
                    < 0.5;
            if fitted && frames >= CARD_SETTLE_FRAMES {
                // The window shows clear (`ui_runtime::Window::show`); the
                // card is clear too until its entrance begins.
                if let Some(view) = state.window().native_view() {
                    unsafe { crate::platform::ui_fade_launcher_options(view, 1., 0.) }
                }
                self.card_fade = CardFade::Shown(now);
                // Focus goes to the card's first control, as a dialog's
                // does; Tab then runs round the card, the one thing in its
                // window.
                window.defer(cx, |window, cx| window.focus_next(cx));
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

/// What sets one card apart from the others: the name the view knows it
/// by, its glyph, its title and its width.
struct CardFace {
    name: &'static str,
    glyph: &'static str,
    title: &'static str,
    width: f32,
}

impl CardFace {
    fn of(name: &str) -> Self {
        let (name, glyph, title, width) = match name {
            "sources" => (
                "sources",
                "Monitor-regular",
                "Capture source",
                Theme::recorder_card_width_source(),
            ),
            "audio" => (
                "audio",
                "Microphone-regular",
                "Microphone",
                Theme::recorder_card_width_microphone(),
            ),
            "camera" => (
                "camera",
                "VideoCamera-regular",
                "Camera",
                Theme::recorder_card_width_camera(),
            ),
            "countdown" => (
                "countdown",
                "Timer-regular",
                "Countdown",
                Theme::recorder_card_width_countdown(),
            ),
            _ => (
                "more",
                "DotsThree-regular",
                "SubTake",
                Theme::recorder_card_width_more(),
            ),
        };
        Self {
            name,
            glyph,
            title,
            width,
        }
    }
}

/// More's way back to the editor: "Studio" and an arrow out, on a 34 pill.
/// Whether the recording can take a microphone: the system allows it and
/// one is plugged in. The platform lists the system default first, which
/// alone is no microphone at all.
pub(in crate::ui) fn microphone_usable(state: &UiHandle) -> bool {
    state.get_microphone_access() && state.get_microphone_names().row_count() > 1
}

/// Whether the recording can take a camera, as [`microphone_usable`] asks.
pub(in crate::ui) fn camera_usable(state: &UiHandle) -> bool {
    state.get_camera_access() && state.get_camera_names().row_count() > 1
}

fn studio_button(
    open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    theme: Theme,
) -> impl IntoElement {
    let id = ElementId::from("studio");
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    layered(
        div()
            .id(id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(Theme::icon_gap()))
            .h(px(Theme::control_height_small()))
            .pl(px(Theme::gap_block()))
            .pr(px(Theme::control_padding_small()))
            .rounded_full()
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.sunk,
                theme.sunk2,
            ))
            .font_weight(FontWeight::MEDIUM)
            .map(|s| subtake_ui::pressable(s, theme, Some(theme.press), hover_key))
            .on_click(open)
            .child("Studio")
            .child(icon_sized(
                "ArrowUpRight-regular",
                Theme::icon_size_small(),
                theme.text,
            ))
            .child(subtake_ui::pill_edge(vec![hairline(
                theme.line,
                Theme::hairline_width(),
            )])),
    )
}

/// The muted sentence a card ends on, 6 in and 2 under what it follows.
fn helper(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .pt(px(Theme::card_helper_top()))
        .px(px(Theme::card_text_inset()))
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
