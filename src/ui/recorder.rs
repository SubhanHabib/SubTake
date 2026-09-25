//! The recorder bar. Its option cards are `options.rs`.

use super::*;
use subtake_ui::{fade_in, icon_sized};

thread_local! {
    /// The anchor last handed to the options window, so the bar only moves
    /// the card when its control has actually moved.
    static OPTIONS_ANCHOR: Cell<f32> = const { Cell::new(-1.) };
    /// The width last asked of the bar's window, so it is resized only when
    /// that changes.
    static LAUNCHER_WIDTH: Cell<f32> = const { Cell::new(-1.) };
}

impl RootView {
    /// The count over the display the capture will record. The design fixes
    /// its colours — white on a dimmed screen — in both themes: it sits on
    /// the user's own desktop, not on the app's chrome.
    pub(super) fn countdown_overlay(&self, state: &RecordingCountdown) -> AnyElement {
        let count = state.get_counting();
        if count <= 0 {
            return div().into_any_element();
        }
        let white = white();
        // Keyed by the number, so each second starts its own shrink and fade.
        let numeral = mono(count.to_string())
            .font_weight(FontWeight::MEDIUM)
            .text_color(white)
            .line_height(relative(1.))
            .text_size(px(Theme::font_countdown()))
            // Palette churn: the design sets a soft shadow under the numeral;
            // gpui has no text shadow.
            .with_animation(
                SharedString::from(format!("count-{count}")),
                Animation::new(std::time::Duration::from_secs(1)),
                |numeral, t| {
                    let scale = 1. - (1. - Theme::countdown_end_scale()) * t;
                    numeral
                        .text_size(px(Theme::font_countdown() * scale))
                        .opacity(1. - (1. - Theme::countdown_end_opacity()) * t)
                },
            );
        // Palette churn: the design blurs what is under the chip; a
        // click-through overlay has no material of its own, so it is only
        // the tint.
        let chip = row()
            .h(px(Theme::countdown_chip_height()))
            .gap(px(Theme::countdown_chip_gap()))
            .px(px(Theme::countdown_chip_padding()))
            .rounded_full()
            .bg(self.theme.scrim_chip())
            .text_color(white)
            .text_size(px(Theme::font_secondary()))
            .whitespace_nowrap()
            .child("Press")
            .child(mono("esc"))
            .child("to cancel");
        div()
            .relative()
            .size_full()
            .bg(hsla(225. / 360., 0.286, 0.055, 0.42))
            .child(
                column()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(Theme::countdown_gap()))
                    .child(numeral)
                    .child(chip),
            )
            .child(
                div()
                    .absolute()
                    .inset(px(Theme::countdown_frame_inset()))
                    .rounded(px(Theme::countdown_frame_radius()))
                    .shadow(vec![hairline(
                        hsla(0., 0., 1., 0.55),
                        Theme::countdown_frame_width(),
                    )]),
            )
            .into_any_element()
    }

    pub(super) fn launcher(
        &mut self,
        state: &RecordingLauncher,
        window: &mut Window,
    ) -> AnyElement {
        let theme = self.theme;
        let full = Theme::recorder_width();
        let width = self.bar_width(state, window);
        // "Hide bar while recording" leaves the clock alone on the bar while
        // it runs, until the pointer comes back to it.
        let pill = width.1 < full;
        // The bar IS the window's plate — the window itself is transparent and
        // borderless, and the native material under it is masked to this
        // plate's shape (`update_recorder_glass`), so the plate is only the
        // tint over that glass. An outer plate around it only drew a second,
        // square box.
        //
        // It is one row of `RECORDER_CONTROL` controls in every phase, so
        // the bar keeps its size when it turns from one job to the next.
        //
        // Its row is laid out at the width it is easing to and centred on
        // the plate, so a bar growing from its pill uncovers its controls
        // from the middle out rather than squeezing them.
        let plate = panel_variant(theme, UiSurface::Overlay)
            .flex_none()
            .h_full()
            .w(px(width.0))
            .items_center()
            .overflow_hidden();
        // The controls sit on their own row inside the plate, so when the bar
        // turns from one job to the next its controls can fade in while the
        // plate stays still.
        let mut bar = row()
            .flex_none()
            .h_full()
            .w(px(width.1))
            .p(px(Theme::recorder_padding()))
            .gap(px(Theme::recorder_gap()));
        let phase = if pill {
            "pill"
        } else if state.get_recording() {
            "recording"
        } else if state.get_counting() > 0 {
            "counting"
        } else if state.get_busy() {
            "working"
        } else {
            "ready"
        };
        let swap = |bar: Div| {
            row()
                .size_full()
                .justify_center()
                .child(
                    plate.child(
                        bar.with_animation(
                            SharedString::from(format!("bar-{phase}")),
                            Animation::new(std::time::Duration::from_millis(BAR_SWAP_MS))
                                .with_easing(|t| subtake_ui::motion::EASE_OUT.eval(t)),
                            |bar, t| bar.opacity(t),
                        ),
                    ),
                )
                .into_any_element()
        };
        if pill {
            // The pill is picked up and moved as the grip would be.
            return swap(
                bar.justify_center()
                    .cursor(CursorStyle::ClosedHand)
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move())
                    .child(self.measured_clock(state)),
            );
        }
        // The grip is drawn on the idle and capturing bars only: while the
        // count runs or the file is written the bar is a message, and a
        // message is not something to pick up and move.
        let counting = !state.get_recording() && state.get_busy();
        if !counting {
            bar = bar.child(
                div()
                    .id("launcher-drag")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(Theme::recorder_handle()))
                    .h(px(Theme::recorder_control()))
                    .cursor(CursorStyle::ClosedHand)
                    .child(icon_sized(
                        "DotsSixVertical-regular",
                        Theme::icon_size(),
                        theme.text,
                    ))
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            );
        }
        if !state.get_recording() && !state.get_busy() {
            // Where each control that opens a card sits, so its card can
            // float over it rather than over the middle of the bar. The
            // children are the handle, then these five in this order.
            let panel = state.get_panel();
            let open = panel.clone();
            bar = bar.on_children_prepainted(move |bounds, _, _| {
                let slot = ["sources", "audio", "camera", "countdown", "more"]
                    .iter()
                    .position(|id| *id == open);
                // A closing card stays over its control while it fades.
                let Some(anchor) = slot
                    .and_then(|i| bounds.get(i + 1))
                    .map(|b| f32::from(b.center().x))
                else {
                    return;
                };
                if OPTIONS_ANCHOR.replace(anchor) != anchor {
                    crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                        crate::platform::set_launcher_options_anchor(anchor)
                    });
                }
            });
            bar = bar.child(self.source_pill(state, panel == "sources"));
            // Audio and camera say their state with the glyph and the plate,
            // not with an accent edge: a struck-through microphone in `muted`
            // on no plate is off, a microphone in `text` on `sunk` is on. The
            // accent has four jobs in this design and "the mic is live" is
            // not one of them.
            // One the system refuses, or none plugged in, reads as off, as
            // its card says.
            let mic = (state.get_microphone() && super::options::microphone_usable(state))
                || state.get_system_audio();
            let camera = state.get_camera() && super::options::camera_usable(state);
            for (id, glyph, label, on) in [
                (
                    "audio",
                    if mic {
                        "Microphone-regular"
                    } else {
                        "MicrophoneSlash-regular"
                    },
                    "Microphone",
                    mic,
                ),
                (
                    "camera",
                    if camera {
                        "VideoCamera-regular"
                    } else {
                        "VideoCameraSlash-regular"
                    },
                    "Camera",
                    camera,
                ),
            ] {
                let launcher = state.clone();
                bar = bar.child(
                    self.bar_control(id, panel == id, if on { theme.sunk } else { clear(theme) })
                        .w(px(Theme::recorder_control()))
                        .tooltip(move |_, cx| tooltip(label, theme, cx))
                        .child(icon_sized(
                            glyph,
                            Theme::icon_size_pod(),
                            if on { theme.text } else { theme.muted },
                        ))
                        .on_click(move |_, _, _| Self::toggle_panel(&launcher, id)),
                );
            }
            let countdown = state.get_countdown();
            let launcher = state.clone();
            bar = bar.child(
                self.bar_control("countdown", panel == "countdown", theme.sunk)
                    .w(px(Theme::recorder_countdown_width()))
                    .gap(px(Theme::recorder_countdown_gap()))
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .child(icon_sized(
                        "Timer-regular",
                        Theme::icon_size_medium(),
                        theme.text,
                    ))
                    .child(if countdown > 0 {
                        format!("{countdown}s")
                    } else {
                        "Off".to_owned()
                    })
                    .on_click(move |_, _, _| Self::toggle_panel(&launcher, "countdown")),
            );
            let launcher = state.clone();
            bar = bar.child(
                self.bar_control("more", panel == "more", clear(theme))
                    .w(px(Theme::recorder_control()))
                    .tooltip(move |_, cx| tooltip("More", theme, cx))
                    .child(icon_sized(
                        "DotsThree-regular",
                        Theme::recorder_more_glyph(),
                        theme.text,
                    ))
                    .on_click(move |_, _, _| Self::toggle_panel(&launcher, "more")),
            );
            let launcher = state.clone();
            // Red, not accent. `rec` is the only red fill in the app and this
            // is the control it exists for; a blue Record button would make
            // the one destructive-adjacent action look like Export.
            bar = bar.child(self.record_button().on_click(move |_, _, _| {
                if launcher.get_source_names().row_count() == 0 {
                    launcher.set_panel("sources".into());
                    launcher.defer_panel("sources".into());
                    launcher.defer_action("sources".into());
                } else {
                    launcher.defer_action("start-recording".into());
                }
            }));
        } else if state.get_recording() {
            bar = self.capture_controls(bar, state);
        } else if state.get_counting() > 0 {
            bar = self.counting_controls(bar, state);
        } else {
            bar = self.working_controls(bar, state);
        }
        if state.get_recording() || state.get_busy() {
            return swap(bar);
        }
        // The bar's last control. The handoff draws a 36 close button and
        // nothing else after Record.
        //
        // What used to sit here as well: the app's own mark, and a "?" whose
        // only job was to hold the status text in a tooltip. Neither is
        // drawn — a logo on a five-control bar is a sixth thing to read, and
        // the status has the bar itself to speak in while a capture runs.
        bar = bar.child(
            self.bar_control("close", false, clear(theme))
                .w(px(Theme::recorder_close_width()))
                .tooltip(move |_, cx| tooltip("Hide recorder", theme, cx))
                .child(icon_sized("X-regular", Theme::icon_size(), theme.text))
                .on_click(self.command("hide-launcher")),
        );
        swap(bar)
    }

    /// The clock: the pulsing dot and the time on `rec` while the capture
    /// runs; on `sunk`, dimmed, with PAUSED while it is held.
    fn clock(&self, state: &RecordingLauncher) -> Div {
        let theme = self.theme;
        let paused = state.get_paused();
        let dot = div()
            .flex_none()
            .size(px(Theme::record_dot()))
            .rounded_full();
        let dot = if paused {
            dot.bg(theme.rec)
                .opacity(Theme::paused_dot_opacity())
                .into_any_element()
        } else {
            dot.bg(white())
                .with_animation(
                    "rec-pulse",
                    Animation::new(std::time::Duration::from_secs(1))
                        .repeat()
                        .with_easing(ease_in_out),
                    |dot, t| dot.opacity(1. - 0.65 * (1. - (2. * t - 1.).abs())),
                )
                .into_any_element()
        };
        // Pausing eases the clock from the capture's red to `sunk` and dims
        // its count, rather than cutting between the two.
        let held = subtake_ui::motion::state_fade("clock-paused", paused);
        let mut clock = row()
            .flex_none()
            .gap(px(Theme::icon_gap_record()))
            .h(px(Theme::recorder_control()))
            .px(px(Theme::recorder_pill_padding()))
            .rounded_full()
            .whitespace_nowrap()
            .child(dot)
            .child(
                mono(state.get_elapsed())
                    .text_size(px(Theme::font_clock()))
                    .font_weight(FontWeight::MEDIUM)
                    .opacity(subtake_ui::motion::lerp(1., PAUSED_CLOCK_OPACITY, held)),
            );
        clock = if paused {
            // Palette churn: the handoff tracks PAUSED out by .09em; gpui
            // sets no letter spacing, so it is the caps alone.
            clock.child(fade_in(
                "clock-paused-label",
                div()
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
                    .child("PAUSED"),
            ))
        } else {
            clock
        };
        clock
            .bg(subtake_ui::motion::blend(theme.rec, theme.sunk, held))
            .text_color(subtake_ui::motion::blend(white(), theme.text, held))
    }

    /// The clock, its width kept for the pill "Hide bar while recording"
    /// draws round it.
    fn measured_clock(&self, state: &RecordingLauncher) -> Div {
        let measured = self.bar_pill.clone();
        div()
            .flex()
            .flex_none()
            .child(self.clock(state))
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(clock) = bounds.first() {
                    let width = f32::from(clock.size.width);
                    if measured.replace(width) != width {
                        window.request_animation_frame();
                    }
                }
            })
    }

    /// Where the bar's plate stands as it eases between its whole width and
    /// its recording pill's, and the width it is easing to. The window keeps
    /// the wider of the two while the plate moves, and the frosted material
    /// follows the plate; once the plate is there, the window takes its
    /// size about its centre.
    fn bar_width(&mut self, state: &RecordingLauncher, window: &mut Window) -> (f32, f32) {
        let full = Theme::recorder_width();
        let measured = self.bar_pill.get();
        let hidden = state.get_recording()
            && !state.get_stopping()
            && state.get_recorder_flag("hide-bar")
            && !state.get_bar_hovered()
            && measured > 0.;
        let target = if hidden {
            measured + 2. * Theme::recorder_padding()
        } else {
            full
        };
        let now = Instant::now();
        let at = |(from, to, since): (f32, f32, Instant)| {
            subtake_ui::motion::ease_toward(from, to, since, BAR_HIDE_MS, now)
        };
        let moving = *self.bar_width.get_or_insert((target, target, now));
        if moving.1 != target {
            self.bar_width = Some((at(moving), target, now));
        }
        let moving = self.bar_width.unwrap_or((target, target, now));
        let drawn = at(moving);
        if drawn != target {
            window.request_animation_frame();
        }
        let wanted = if drawn == target {
            target
        } else {
            moving.0.max(target)
        };
        if LAUNCHER_WIDTH.replace(wanted) != wanted {
            let launcher = state.window().clone();
            crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                crate::platform::set_launcher_width(&launcher, wanted)
            });
        }
        let room = f32::from(window.viewport_size().width);
        let drawn = drawn.min(room);
        crate::platform::set_recorder_glass_width(
            state.window(),
            if drawn < room - 0.5 { drawn } else { 0. },
        );
        (drawn, target)
    }

    /// Recording and paused. The Record button's place becomes the clock —
    /// red while capture runs, `sunk` while it is held — and the controls
    /// after it keep one position across both, so Stop never moves under
    /// the pointer. Only Pause trades places with Resume.
    fn capture_controls(&self, bar: Div, state: &RecordingLauncher) -> Div {
        let theme = self.theme;
        let paused = state.get_paused();
        let enabled = !state.get_busy();
        let clock = self.measured_clock(state);
        // Not drawn by the design: Resume's hover, a white lift over `rec`.
        let resume_hover = subtake_ui::motion::tween_key(&"pause".into(), "hover");
        let pause = if paused {
            // Resume takes Pause's place in `rec`: the one way back into
            // the capture, in the capture's own colour.
            row()
                .id("pause")
                .flex_none()
                .gap(px(Theme::icon_gap_row()))
                .h(px(Theme::recorder_control()))
                .px(px(Theme::recorder_pill_padding()))
                .rounded_full()
                .bg(subtake_ui::motion::hover_blend(
                    &resume_hover,
                    theme.rec,
                    theme.rec.blend(white().opacity(Theme::rec_hover_lift())),
                ))
                .text_color(white())
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .when(enabled, |s| {
                    subtake_ui::pressable(s, theme, Some(theme.press), resume_hover)
                })
                .when(!enabled, |s| s.opacity(Theme::disabled_opacity()))
                .child(icon_sized("Play-fill", Theme::icon_size_pod(), white()))
                .child("Resume")
                .when(enabled, |s| s.on_click(self.command("pause-recording")))
                .into_any_element()
        } else {
            self.bar_round(
                "pause",
                "Pause-fill",
                "Pause",
                theme.text,
                true,
                Some(enabled),
            )
            .when(enabled, |s| s.on_click(self.command("pause-recording")))
            .into_any_element()
        };
        // Pause and Resume fade in as they trade places.
        let pause = fade_in(
            if paused { "resume-in" } else { "pause-in" },
            div().flex().flex_none().child(pause),
        );
        // Whether the microphone and the camera are in this capture is fixed
        // when it starts, so these say it rather than change it, by the rule
        // the idle bar uses: on is a `sunk` plate and a `text` glyph, off no
        // plate, the struck-through glyph and `muted`.
        let mic = (state.get_microphone() && super::options::microphone_usable(state))
            || state.get_system_audio();
        let camera = state.get_camera() && super::options::camera_usable(state);
        bar.child(clock)
            .child(pause)
            .child(
                self.bar_round("stop", "Stop-fill", "Stop", theme.text, true, Some(enabled))
                    .when(enabled, |s| s.on_click(self.command("stop-recording"))),
            )
            .child(div().flex_1())
            .child(self.bar_round(
                "capture-mic",
                if mic {
                    "Microphone-regular"
                } else {
                    "MicrophoneSlash-regular"
                },
                if mic {
                    "Audio is recording"
                } else {
                    "No audio"
                },
                if mic { theme.text } else { theme.muted },
                mic,
                None,
            ))
            .child(self.bar_round(
                "capture-camera",
                if camera {
                    "VideoCamera-regular"
                } else {
                    "VideoCameraSlash-regular"
                },
                if camera {
                    "Camera is recording"
                } else {
                    "No camera"
                },
                if camera { theme.text } else { theme.muted },
                camera,
                None,
            ))
            .child(
                self.bar_round(
                    "discard",
                    "X-regular",
                    "Discard recording",
                    theme.danger,
                    false,
                    Some(enabled),
                )
                .when(enabled, |s| s.on_click(self.command("discard-recording"))),
            )
    }

    /// The count before capture. The bar becomes the count, so the control
    /// that was just pressed is the thing that answers.
    fn counting_controls(&self, bar: Div, state: &RecordingLauncher) -> Div {
        let theme = self.theme;
        let count = state.get_counting();
        let source = state
            .get_source_names()
            .row_data(state.get_source_index().max(0) as usize)
            .map(|name| name.split(" · ").next().unwrap_or_default().to_owned())
            .unwrap_or_default();
        let mic = if state.get_microphone() && super::options::microphone_usable(state) {
            "mic on"
        } else {
            "mic off"
        };
        let cancel_hover = subtake_ui::motion::tween_key(&"cancel".into(), "hover");
        bar.child(
            round_plate(theme).child(
                mono(count.to_string())
                    .text_size(px(Theme::font_count()))
                    .font_weight(FontWeight::MEDIUM),
            ),
        )
        .child(message(
            format!("Recording starts in {count}…"),
            format!("{source} · {mic}"),
            theme,
        ))
        .child(
            // Cancel names its key, in the mono face a shortcut is set in.
            row()
                .id("cancel")
                .flex_none()
                .gap(px(Theme::icon_gap_row()))
                .h(px(Theme::recorder_control()))
                .px(px(Theme::recorder_plate_padding()))
                .rounded_full()
                .bg(subtake_ui::motion::hover_blend(
                    &cancel_hover,
                    theme.sunk,
                    theme.sunk2,
                ))
                .map(|s| subtake_ui::pressable(s, theme, Some(theme.press), cancel_hover))
                .whitespace_nowrap()
                .child("Cancel")
                .child(
                    mono("esc")
                        .text_size(px(Theme::font_small()))
                        .text_color(theme.muted),
                )
                .on_click(self.command("cancel")),
        )
    }

    /// Writing the recording out, and — not drawn by the design — any other
    /// wait the bar has to sit through: finding displays, starting capture,
    /// holding or resuming it. Stopping ends on a plate rather than a
    /// button, because once capture has ended there is nothing to cancel.
    fn working_controls(&self, bar: Div, state: &RecordingLauncher) -> Div {
        let theme = self.theme;
        let bar = bar.child(round_plate(theme).child(spinner(Theme::spinner_size(), theme)));
        if state.get_stopping() {
            return bar
                .child(
                    message(
                        "Finishing your recording",
                        format!("{} captured · writing to disk", state.get_elapsed()),
                        theme,
                    )
                    .min_w(px(Theme::stopping_text_width())),
                )
                .child(
                    row()
                        .flex_none()
                        .h(px(Theme::recorder_control()))
                        .px(px(Theme::recorder_plate_padding()))
                        .rounded_full()
                        .bg(theme.sunk)
                        .text_color(theme.muted)
                        .whitespace_nowrap()
                        .child("Opens in the editor"),
                );
        }
        let bar = bar.child(message(state.get_status(), SharedString::default(), theme));
        if state.get_cancellable() {
            bar.child(
                button("cancel", "Cancel", theme)
                    .bar()
                    .on_click(self.command("cancel")),
            )
        } else {
            bar
        }
    }

    /// A round control on the capturing bar. `plate` is the `sunk` fill Pause and
    /// Stop stand on; without it the control is bare glass until hovered.
    /// `enabled` is `None` for an indicator: the microphone and camera say
    /// what the capture holds and cannot change it, so they take the shape
    /// and the tooltip and none of a control's states.
    fn bar_round(
        &self,
        id: &'static str,
        glyph: &'static str,
        label: &'static str,
        color: Hsla,
        plate: bool,
        enabled: Option<bool>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        // The hover wash fades in and out, as every other control's does.
        let hover_key = subtake_ui::motion::tween_key(&id.into(), "hover");
        let (rest, hover) = if plate {
            (theme.sunk, theme.sunk2)
        } else {
            (theme.sunk2.opacity(0.), theme.sunk2)
        };
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(Theme::recorder_control()))
            .rounded_full()
            .bg(subtake_ui::motion::hover_blend(&hover_key, rest, hover))
            .when(enabled == Some(true), |s| {
                subtake_ui::pressable(s, theme, Some(theme.press), hover_key)
            })
            .when(enabled == Some(false), |s| {
                s.opacity(Theme::disabled_opacity())
            })
            .tooltip(move |_, cx| tooltip(label, theme, cx))
            .child(icon_sized(glyph, Theme::icon_size_pod(), color))
    }

    /// A control on the idle bar, on `rest` until the pointer finds it. The
    /// control whose card is open stands on `sunk2` inside a `line` edge
    /// until the card closes.
    fn bar_control(&self, id: &'static str, open: bool, rest: Hsla) -> Stateful<Div> {
        let theme = self.theme;
        let hover_key = subtake_ui::motion::tween_key(&id.into(), "hover");
        let control = row()
            .id(id)
            .flex_none()
            .justify_center()
            .h(px(Theme::recorder_control()))
            .rounded_full()
            .bg(if open {
                theme.sunk2
            } else {
                subtake_ui::motion::hover_blend(&hover_key, rest, theme.sunk2)
            })
            .when(open, |s| {
                s.shadow(vec![hairline(theme.line, Theme::hairline_width())])
            });
        subtake_ui::pressable(control, theme, Some(theme.press), hover_key)
    }

    /// The source pill: the source's glyph, its name over its size — or a
    /// window's app over its title — and the caret that says it opens.
    fn source_pill(&self, state: &RecordingLauncher, open: bool) -> Stateful<Div> {
        let theme = self.theme;
        let source = usize::try_from(state.get_source_index())
            .ok()
            .and_then(|i| state.get_capture_sources().iter().nth(i));
        let (glyph, (name, detail)) = match &source {
            Some(source) => (
                match source.kind.as_str() {
                    "window" => "AppWindow-regular",
                    "area" => "Selection-regular",
                    _ => "Monitor-regular",
                },
                super::options::source::caption(source),
            ),
            None => (
                "Monitor-regular",
                ("Choose a source".to_owned(), String::new()),
            ),
        };
        let launcher = state.clone();
        self.bar_control("sources", open, theme.sunk)
            .w(px(Theme::recorder_source_width()))
            .gap(px(Theme::recorder_source_gap()))
            .pl(px(Theme::recorder_source_padding_left()))
            .pr(px(Theme::recorder_source_padding_right()))
            .child(icon_sized(glyph, Theme::icon_size_medium(), theme.text))
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap_0()
                    .line_height(relative(Theme::recorder_source_leading()))
                    .whitespace_nowrap()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_ellipsis()
                            .child(name),
                    )
                    .when(!detail.is_empty(), |s| {
                        s.child(
                            mono(detail)
                                .text_size(px(Theme::font_tiny()))
                                .text_color(theme.muted)
                                .text_ellipsis(),
                        )
                    }),
            )
            .child(icon_sized(
                "CaretDown-regular",
                Theme::icon_size_caret(),
                theme.muted,
            ))
            .on_click(move |_, _, _| Self::toggle_panel(&launcher, "sources"))
    }

    /// Record: a white dot and the word, on the one red fill in the app.
    fn record_button(&self) -> Stateful<Div> {
        let theme = self.theme;
        // Not drawn by the design: the hover, a white lift over `rec`, as
        // Resume's.
        let hover_key = subtake_ui::motion::tween_key(&"record".into(), "hover");
        let control = row()
            .id("record")
            .flex_none()
            .justify_center()
            .w(px(Theme::recorder_record_width()))
            .h(px(Theme::recorder_control()))
            .gap(px(Theme::recorder_record_gap()))
            .rounded_full()
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.rec,
                theme.rec.blend(white().opacity(Theme::rec_hover_lift())),
            ))
            .shadow(vec![theme.record_glow()])
            .text_color(white())
            .text_size(px(Theme::font_record()))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .child(
                div()
                    .flex_none()
                    .size(px(Theme::recorder_record_dot()))
                    .rounded_full()
                    .bg(white()),
            )
            .child("Record");
        subtake_ui::pressable(control, theme, Some(theme.press), hover_key)
    }

    /// A bar control that opens its panel, or closes it if it is the open one.
    fn toggle_panel(state: &RecordingLauncher, id: &str) {
        let value = if state.get_panel() == id { "" } else { id };
        state.set_panel(value.into());
        state.defer_panel(value.into());
    }
}

/// The bar's round `sunk` plate: it holds the count, or the spinner.
fn round_plate(theme: Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(Theme::recorder_control()))
        .rounded_full()
        .bg(theme.sunk)
}

/// What the bar is doing, in a line and a muted line under it.
fn message(title: impl Into<SharedString>, detail: impl Into<SharedString>, theme: Theme) -> Div {
    let detail = detail.into();
    column()
        .flex_1()
        .min_w_0()
        .justify_center()
        .h(px(Theme::recorder_control()))
        .px(px(Theme::recorder_text_inset()))
        .line_height(relative(Theme::message_leading()))
        .whitespace_nowrap()
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_ellipsis()
                .child(title.into()),
        )
        .when(!detail.is_empty(), |s| {
            s.child(
                div()
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
                    .text_ellipsis()
                    .child(detail),
            )
        })
}

/// A control's fill before the pointer finds it: none, as a fade's start.
fn clear(theme: Theme) -> Hsla {
    theme.sunk2.opacity(0.)
}

/// A `sunk2` ring with an accent arc turning round it once a second.
pub(super) fn spinner(size: f32, theme: Theme) -> Div {
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded_full()
        .shadow(vec![hairline(theme.sunk2, Theme::spinner_width())])
        .child(
            icon_sized("SpinnerArc", size, theme.accent)
                .absolute()
                .inset_0()
                .with_animation(
                    "stop-spin",
                    Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |svg, t| svg.with_transformation(Transformation::rotate(percentage(t))),
                ),
        )
}
