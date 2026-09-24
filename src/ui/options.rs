//! The recorder's option cards: batch 1, screen 2a of the round-2 handoff.
//!
//! One card per bar control — Source, Audio, Camera, Countdown, More — each
//! drawn in the options window, which floats over the control that opened it
//! (`set_launcher_options_anchor`). Only one is open at a time; opening
//! another replaces it in place with a short cross-fade.

use super::*;
use crate::ui_state::CaptureSource;
use subtake_ui::{icon_sized, layered};

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
        let height = self.card_fade(state, window);
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
        // the card and its window take that height in one step; the window
        // is clear whenever it moves or resizes (`card_fade`).
        panel_variant(theme, UiSurface::Overlay)
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(height))
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
                    .child(options_fit(state.clone(), self.card_measured.clone())),
            )
            .into_any_element()
    }

    /// Steps the card's window through its fade (`CardFade`) and gives the
    /// height to draw the card at: what its content last measured.
    ///
    /// The window's alpha carries the fade, not the card's paint: the paint
    /// and the frosted material under it reach the screen by different
    /// routes, and a card that grew or faded on its own showed its glass a
    /// frame early or late, and its rows sliding through a plate still
    /// growing to meet them. The window moves and resizes only while clear.
    fn card_fade(&mut self, state: &RecordingOptions, window: &mut Window) -> f32 {
        let natural = state.get_options_height();
        let now = Instant::now();
        let still = subtake_ui::motion::reduced_motion();
        let length = |ms: u64| if still { 0 } else { ms };
        let fade = |alpha: f64, ms: u64| {
            if let Some(view) = state.window().native_view() {
                unsafe { crate::platform::ui_fade_launcher_options(view, alpha, ms as f64 / 1000.) }
            }
        };
        let panel = state.get_panel();
        let open = !panel.is_empty();
        let wanted = (panel, state.window().opens());
        if !open {
            if !matches!(self.card_fade, CardFade::Gone | CardFade::Closing) {
                let out = length(CARD_OUT_MS);
                fade(0., out);
                self.card_fade = CardFade::Closing;
                let state = state.clone();
                crate::ui_runtime::Timer::single_shot(
                    std::time::Duration::from_millis(out),
                    move || {
                        if state.get_panel().is_empty() {
                            let _ = state.hide();
                        }
                    },
                );
            }
        } else if self.card_last != wanted {
            self.card_fade = match self.card_fade {
                // Replaced while in view: this card fades out first, still
                // drawn, and the window moves once it is clear.
                CardFade::Shown if self.card_last.1 == wanted.1 => {
                    let out = length(CARD_SWAP_MS / 2);
                    fade(0., out);
                    CardFade::Leaving(now + std::time::Duration::from_millis(out))
                }
                CardFade::Leaving(until) if now < until => CardFade::Leaving(until),
                before => {
                    self.card_last = wanted;
                    CardFade::Waiting(
                        false,
                        match before {
                            CardFade::Leaving(_) => CARD_SWAP_MS / 2,
                            CardFade::Waiting(_, ms) => ms,
                            _ => CARD_IN_MS,
                        },
                    )
                }
            };
        }
        match self.card_fade {
            CardFade::Leaving(_) if open => window.request_animation_frame(),
            // Not on the card's first frame, whose rows have yet to be
            // measured, and not before the window has taken their height.
            CardFade::Waiting(drawn, ms) if open => {
                let fits = |height: f32| (height - natural).abs() < 0.5;
                if drawn
                    && fits(self.card_measured.get())
                    && fits(f32::from(window.viewport_size().height))
                {
                    fade(1., length(ms));
                    self.card_fade = CardFade::Shown;
                } else {
                    self.card_fade = CardFade::Waiting(true, ms);
                    window.request_animation_frame();
                }
            }
            // Reopened as it faded out: it comes back from where it was.
            CardFade::Closing if open => {
                fade(1., length(CARD_IN_MS));
                self.card_fade = CardFade::Shown;
            }
            _ => {}
        }
        // The frosted material under the card is the card's height, not the
        // window's: the window is resized a moment before its paint catches
        // up, and glass that filled it would show past the card's edge.
        crate::platform::set_recorder_glass_height(state.window(), natural.max(0.01));
        natural
    }

    /// Displays as pictures, then windows as rows: you are choosing a
    /// picture, so the card shows pictures.
    fn source_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let selected = state.get_source_index();
        let enabled = !state.get_busy();
        let sources: Vec<_> = state.get_capture_sources().iter().enumerate().collect();
        let mut displays = row().gap(px(Theme::gap())).items_start();
        let mut windows = column().gap(px(Theme::list_gap()));
        for (index, source) in &sources {
            let chosen = *index as i32 == selected;
            let options = state.clone();
            let index = *index;
            let choose = move |_: &ClickEvent, _: &mut Window, _: &mut App| {
                options.defer_option("source".into(), index.to_string())
            };
            if source.kind == "display" {
                displays = displays.child(
                    display_tile(index, source, chosen, enabled, theme)
                        .when(enabled, |tile| tile.on_click(choose)),
                );
            } else {
                windows = windows.child(
                    window_row(index, source, chosen, enabled, theme)
                        .when(enabled, |tile| tile.on_click(choose)),
                );
            }
        }
        let has = |kind: &str| sources.iter().any(|(_, s)| s.kind == kind);
        let mut body = Vec::new();
        if has("display") {
            body.push(caps_label("Displays", theme).into_any_element());
            body.push(displays.into_any_element());
        }
        if has("window") {
            body.push(
                caps_label("Windows", theme)
                    .mt(px(Theme::list_gap()))
                    .into_any_element(),
            );
            body.push(windows.into_any_element());
        }
        // Not drawn by the design: no sources at all — before the first
        // refresh, or without the screen-recording permission. The status
        // line says which, where the list would be.
        if sources.is_empty() {
            body.push(helper(state.get_status(), theme).into_any_element());
        }
        body.push(if state.get_sources_loading() {
            refreshing(theme).into_any_element()
        } else {
            self.action(
                "refresh",
                "Refresh displays and windows",
                "sources",
                enabled,
            )
            .glyph("ArrowClockwise-regular")
            .standard()
            .into_any_element()
        });
        body.push(
            helper("Area selection starts after you press Record.", theme).into_any_element(),
        );
        body
    }

    fn audio_card(&mut self, state: &RecordingOptions, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = self.theme;
        let busy = state.get_busy();
        let on = state.get_microphone();
        let options = state.clone();
        let microphone = self.dropdown(
            "microphone",
            state.get_microphone_names().iter().collect(),
            state.get_microphone_index(),
            !busy && on,
            cx,
            move |i, _, _| options.defer_option("microphone-device".into(), i.to_string()),
        );
        microphone.update(cx, |d, _| d.glyph = Some("Microphone-regular".into()));
        let mic = state.clone();
        let system = state.clone();
        vec![
            caps_label("Microphone", theme).into_any_element(),
            microphone.into_any_element(),
            toggle(
                "mic-toggle",
                "Record microphone",
                on,
                !busy,
                theme,
                move |v, _, _| mic.defer_option("microphone".into(), v.to_string()),
            )
            .into_any_element(),
            self.level_meter(state.get_mic_level(), on)
                .into_any_element(),
            caps_label("System", theme)
                .mt(px(Theme::list_gap()))
                .into_any_element(),
            toggle(
                "system-toggle",
                "Record system audio",
                state.get_system_audio(),
                !busy,
                theme,
                move |v, _, _| system.defer_option("system-audio".into(), v.to_string()),
            )
            .into_any_element(),
            helper(
                "Microphone and system audio land on separate tracks, so you can mix them later.",
                theme,
            )
            .into_any_element(),
        ]
    }

    /// Twelve bars lit from the left as the level rises, in `ink` — the
    /// meter says "sound is arriving", not "this is selected" — and the
    /// peak beside them. Clipping turns the top two `rec` and holds them
    /// for a second. With the microphone off the meter dims and stops.
    fn level_meter(&mut self, level: f32, on: bool) -> Div {
        let theme = self.theme;
        let floor = Theme::meter_floor_db();
        let bars = Theme::METER_BARS.len();
        let lit = if on && level > floor {
            (((level - floor) / -floor) * bars as f32)
                .round()
                .clamp(0., bars as f32) as usize
        } else {
            0
        };
        let now = Instant::now();
        if on && level >= Theme::meter_clip_db() {
            self.mic_clipped = Some(now);
        }
        let clipped = on
            && self
                .mic_clipped
                .is_some_and(|at| now.duration_since(at) < std::time::Duration::from_secs(1));
        let mut meter = row()
            .h(px(Theme::meter_height()))
            .gap(px(Theme::meter_gap()))
            .opacity(if on { 1. } else { Theme::disabled_opacity() });
        for (i, height) in Theme::METER_BARS.into_iter().enumerate() {
            let fill = if clipped && i >= bars - 2 {
                theme.rec
            } else if i < lit {
                theme.ink
            } else {
                theme.sunk2
            };
            meter = meter.child(
                div()
                    .flex_none()
                    .w(px(Theme::meter_bar_width()))
                    .h(px(height))
                    .rounded(px(Theme::meter_bar_radius()))
                    .bg(fill),
            );
        }
        // Without microphone access, which the card never asks for, the
        // level stays at negative infinity and the peak reads as a dash
        // rather than as a made-up number.
        let peak = if level > floor {
            format!("−{:.0} dB", -level.min(0.))
        } else {
            "— dB".into()
        };
        meter.child(div().flex_1()).child(
            mono(peak)
                .text_size(px(Theme::font_small()))
                .text_color(theme.muted),
        )
    }

    fn camera_card(&mut self, state: &RecordingOptions, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = self.theme;
        let busy = state.get_busy();
        let on = state.get_camera();
        let toggle_state = state.clone();
        let mut body = vec![
            toggle(
                "camera-toggle",
                "Webcam overlay",
                on,
                !busy,
                theme,
                move |v, _, _| toggle_state.defer_option("camera".into(), v.to_string()),
            )
            .into_any_element(),
        ];
        let names: Vec<String> = state.get_camera_names().iter().collect();
        if names.is_empty() {
            // Not drawn by the design: a Mac with no camera. The plate keeps
            // the preview's size so the card does not jump when one appears.
            body.push(
                row()
                    .h(px(Theme::camera_preview_height()))
                    .justify_center()
                    .gap(px(Theme::icon_gap_row()))
                    .rounded(px(Theme::radius_menu()))
                    .bg(theme.sunk)
                    .text_color(theme.muted)
                    .child(icon("VideoCameraSlash-regular", theme.muted))
                    .child("No camera found")
                    .into_any_element(),
            );
        } else {
            let options = state.clone();
            let camera = self.dropdown(
                "camera",
                names,
                state.get_camera_index(),
                !busy && on,
                cx,
                move |i, _, _| options.defer_option("camera-device".into(), i.to_string()),
            );
            camera.update(cx, |d, _| d.glyph = Some("VideoCamera-regular".into()));
            body.push(camera.into_any_element());
            body.push(camera_preview(state.get_camera_preview(), on, theme).into_any_element());
        }
        body.push(
            helper(
                "Your camera is recorded separately and added to the project.",
                theme,
            )
            .into_any_element(),
        );
        body
    }

    fn countdown_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let mut choices = column().gap(px(Theme::list_gap()));
        for (label, value) in [
            ("No delay", 0),
            ("3 seconds", 3),
            ("5 seconds", 5),
            ("10 seconds", 10),
        ] {
            let chosen = state.get_countdown() == value;
            let options = state.clone();
            let id = ElementId::from(SharedString::from(format!("countdown-{value}")));
            let hover_key = subtake_ui::motion::tween_key(&id, "hover");
            let mut choice = div()
                .id(id)
                .relative()
                .flex()
                .items_center()
                .gap(px(Theme::icon_gap_row()))
                .h(px(Theme::control_height_large()))
                .px(px(Theme::control_padding()))
                .rounded_full()
                .map(|s| {
                    subtake_ui::pressable(
                        s,
                        theme,
                        (!chosen).then_some(theme.press),
                        hover_key.clone(),
                    )
                })
                .on_click(move |_, _, _| {
                    options.defer_option("countdown".into(), value.to_string())
                })
                .child(div().flex_1().child(label));
            if chosen {
                choice = choice
                    .bg(theme.sunk)
                    .font_weight(FontWeight::MEDIUM)
                    .child(icon_sized(
                        "Check-regular",
                        Theme::icon_size_medium(),
                        theme.accent,
                    ))
                    .child(selection_ring(None, theme));
            } else {
                choice = choice.bg(subtake_ui::motion::hover_blend(
                    &hover_key,
                    theme.sunk2.opacity(0.),
                    theme.sunk2,
                ));
            }
            choices = choices.child(choice);
        }
        vec![
            choices.into_any_element(),
            helper("Give yourself a moment before recording starts.", theme).into_any_element(),
        ]
    }

    fn more_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let items = column()
            .gap(px(Theme::list_gap()))
            .child(self.more_item(
                "open",
                "FolderOpen-regular",
                "Open video or project…",
                Some("⌘O"),
                "open",
                true,
            ))
            .child(self.more_item(
                "projects",
                "Stack-regular",
                "Projects",
                None,
                "projects",
                true,
            ))
            .child(self.more_item(
                "editor",
                "ArrowUpRight-regular",
                "Back to editor",
                None,
                "show-editor",
                state.get_has_project(),
            ))
            // Not drawn by the design: the storyboard spike. It is a
            // development entry point, so it stays last and unlabelled by
            // any shortcut rather than being dropped while it is in use.
            .child(self.more_item(
                "storyboard",
                "Sparkle-regular",
                "Create video · spike",
                None,
                "storyboard-spike",
                true,
            ));
        let path = row()
            .h(px(Theme::control_height_large()))
            .gap(px(Theme::icon_gap_row()))
            .pl(px(Theme::control_padding_large()))
            .pr(px(Theme::gap_block()))
            .rounded_full()
            .bg(theme.sunk)
            .child(
                mono(state.get_directory())
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_size(px(Theme::font_secondary()))
                    .text_color(theme.muted),
            )
            .child(
                self.action("folder", "Choose folder…", "recording-folder", true)
                    .raised()
                    .small(),
            );
        vec![
            items.into_any_element(),
            divider(theme).my(px(Theme::list_gap())).into_any_element(),
            caps_label("Recordings path", theme).into_any_element(),
            path.into_any_element(),
            helper(
                "New recordings are written here, then opened in the editor.",
                theme,
            )
            .into_any_element(),
        ]
    }

    /// One row of More: a muted glyph, the caption, and its shortcut.
    fn more_item(
        &self,
        id: &'static str,
        glyph: &str,
        label: &'static str,
        shortcut: Option<&'static str>,
        command: &str,
        enabled: bool,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let hover_key = subtake_ui::motion::tween_key(&id.into(), "hover");
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(Theme::icon_gap_row()))
            .h(px(Theme::control_height_small()))
            .px(px(Theme::control_padding_small()))
            .rounded(px(Theme::radius_lane()))
            .opacity(if enabled {
                1.
            } else {
                Theme::disabled_opacity()
            })
            .child(icon(glyph, theme.muted))
            .child(div().flex_1().child(label))
            .children(shortcut.map(|s| {
                mono(s)
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
            }))
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.sunk2.opacity(0.),
                theme.sunk2,
            ))
            .when(enabled, |s| {
                subtake_ui::pressable(s, theme, Some(theme.press), hover_key)
                    .on_click(self.command(command))
            })
    }
}

/// A display: its picture, and its name and resolution under it.
///
/// While the card is busy the sources cannot be changed, so they dim and
/// take no pointer, as a disabled control does.
fn display_tile(
    index: usize,
    source: &CaptureSource,
    chosen: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("source-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    // The chosen picture carries the accent twice, inside and out, so it
    // reads as picked even where the picture itself is mostly blue. Both
    // halves are one inset edge on a plate grown by the outer half: gpui
    // does not clip an outer shadow to what is outside its box, so a spread
    // shadow on a see-through overlay would fill the picture with accent.
    let grow = Theme::selected_width();
    let ring = div()
        .absolute()
        .top(px(-grow))
        .left(px(-grow))
        .right(px(-grow))
        .bottom(px(-grow))
        .rounded(px(Theme::radius_inner() + grow))
        .when(chosen, |s| {
            s.shadow(vec![hairline(theme.accent, Theme::selected_width() * 2.)])
        })
        .when(!chosen && enabled, |s| {
            s.shadow(vec![hairline(
                subtake_ui::motion::hover_blend(&hover_key, theme.line.opacity(0.), theme.line),
                Theme::selected_width(),
            )])
        });
    let picture = thumbnail(
        &source.thumbnail,
        "Monitor-regular",
        Theme::source_thumb_height(),
        Theme::radius_inner(),
        theme,
    )
    .w_full();
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(Theme::source_tile_gap()))
        // The radius is for the focus ring alone, which goes round the
        // picture and its caption together, as a wallpaper tile's does.
        .rounded(px(Theme::radius_inner()))
        .map(|s| {
            if enabled {
                subtake_ui::pressable(s, theme, None, hover_key)
            } else {
                s.opacity(Theme::disabled_opacity())
            }
        })
        .child(div().relative().child(picture).child(layered(ring)))
        .child(
            div()
                .text_size(px(Theme::font_secondary()))
                .text_ellipsis()
                .when(chosen, |s| s.font_weight(FontWeight::MEDIUM))
                .child(source.name.clone()),
        )
        .child(
            mono(source.detail.clone())
                .text_size(px(Theme::font_small()))
                .text_color(theme.muted),
        )
}

/// A window: a small picture of it and its title.
fn window_row(
    index: usize,
    source: &CaptureSource,
    chosen: bool,
    enabled: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("source-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(Theme::icon_gap_row()))
        .h(px(Theme::control_height_small()))
        .px(px(Theme::control_padding_small()))
        .rounded(px(Theme::radius_lane()))
        .map(|s| {
            if enabled {
                s.bg(subtake_ui::motion::hover_blend(
                    &hover_key,
                    theme.sunk2.opacity(0.),
                    theme.sunk2,
                ))
                .map(|s| subtake_ui::pressable(s, theme, Some(theme.press), hover_key))
            } else {
                s.opacity(Theme::disabled_opacity())
            }
        })
        .child(
            thumbnail(
                &source.thumbnail,
                "Image-regular",
                Theme::window_thumb_height(),
                Theme::window_thumb_radius(),
                theme,
            )
            .w(px(Theme::window_thumb_width())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .child(source.name.clone()),
        )
        .when(chosen, |s| {
            s.child(selection_ring(Some(Theme::radius_lane()), theme))
        })
}

/// A source's picture, or — not drawn by the design — its kind's glyph on
/// `sunk` when no still has been taken of it.
fn thumbnail(
    image: &crate::ui_runtime::Image,
    glyph: &str,
    height: f32,
    radius: f32,
    theme: Theme,
) -> Div {
    let plate = div()
        .flex_none()
        .h(px(height))
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    match &image.0 {
        // gpui clips overflow to the box, not its corners, so the picture
        // carries the radius itself.
        Some(image) => plate.child(
            img(image.clone())
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        None => plate
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                glyph,
                (height * 0.4).min(Theme::icon_size_large()),
                theme.muted,
            )),
    }
}

/// The camera's live picture, with the shape it will take set in its
/// corner. Off, it dims.
fn camera_preview(image: crate::ui_runtime::Image, on: bool, theme: Theme) -> Div {
    let plate = div()
        .relative()
        .h(px(Theme::camera_preview_height()))
        .rounded(px(Theme::radius_menu()))
        .overflow_hidden()
        .bg(theme.sunk)
        .opacity(if on { 1. } else { Theme::disabled_opacity() });
    let plate = match image.0 {
        Some(image) => plate.child(
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(Theme::radius_menu()))
                .object_fit(ObjectFit::Cover),
        ),
        // Not wired: the app does not stream the camera into this card yet,
        // so outside the gallery the plate shows the camera glyph.
        None => plate
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                "VideoCamera-regular",
                Theme::icon_size_large(),
                theme.muted,
            )),
    };
    // The swatch and the chip sit on the picture, not on chrome, so they
    // keep one fill in both appearances as the handoff draws them.
    plate
        .child(
            div()
                .absolute()
                .left(px(Theme::gap_large()))
                .bottom(px(Theme::gap_large()))
                .size(px(Theme::camera_swatch()))
                .rounded_full()
                .bg(rgb(0x1b2434))
                .shadow(theme.camera_swatch_shadow()),
        )
        // Palette churn: the handoff blurs what is under this chip by 18.
        // A backdrop blur inside the card would be a second frosted layer
        // over a live picture for one word, so the chip keeps the tint only.
        .child(
            div()
                .absolute()
                .right(px(Theme::gap_large()))
                .top(px(Theme::gap_large()))
                .flex()
                .items_center()
                .h(px(Theme::chip_height()))
                .px(px(Theme::gap_large()))
                .rounded_full()
                .bg(theme.scrim_chip())
                .text_size(px(Theme::font_small()))
                .text_color(gpui::white())
                .child("Preview"),
        )
}

/// Not drawn by the design: the Refresh button while a refresh runs — the
/// same plate and geometry, its arrow turning and its caption saying so.
fn refreshing(theme: Theme) -> Div {
    row()
        .justify_center()
        .gap(px(Theme::icon_gap()))
        .h(px(Theme::control_height()))
        .rounded_full()
        .bg(theme.sunk)
        .child(icon("ArrowClockwise-regular", theme.text).with_animation(
            "refresh-spin",
            Animation::new(std::time::Duration::from_secs(1)).repeat(),
            |svg, t| svg.with_transformation(Transformation::rotate(percentage(t))),
        ))
        .child("Refreshing…")
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
/// clips and never floats in an empty frame, whatever it holds.
fn options_fit(state: RecordingOptions, measured: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let height = f32::from(bounds.size.height).ceil();
            measured.set(height);
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
