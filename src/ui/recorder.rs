//! The recorder bar. Its option cards are `options.rs`.

use super::*;
use subtake_ui::{fade_in, icon_sized};

thread_local! {
    /// The anchor last handed to the options window, so the bar only moves
    /// the card when its control has actually moved.
    static OPTIONS_ANCHOR: Cell<f32> = const { Cell::new(-1.) };
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
            .text_size(px(Theme::FONT_COUNTDOWN))
            // Palette churn: the design sets a soft shadow under the numeral;
            // gpui has no text shadow.
            .with_animation(
                SharedString::from(format!("count-{count}")),
                Animation::new(std::time::Duration::from_secs(1)),
                |numeral, t| {
                    let scale = 1. - (1. - Theme::COUNTDOWN_END_SCALE) * t;
                    numeral
                        .text_size(px(Theme::FONT_COUNTDOWN * scale))
                        .opacity(1. - (1. - Theme::COUNTDOWN_END_OPACITY) * t)
                },
            );
        // Palette churn: the design blurs what is under the chip; a
        // click-through overlay has no material of its own, so it is only
        // the tint.
        let chip = row()
            .h(px(Theme::COUNTDOWN_CHIP_HEIGHT))
            .gap(px(Theme::COUNTDOWN_CHIP_GAP))
            .px(px(Theme::COUNTDOWN_CHIP_PADDING))
            .rounded_full()
            .bg(self.theme.scrim_chip())
            .text_color(white)
            .text_size(px(Theme::FONT_SECONDARY))
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
                    .gap(px(Theme::COUNTDOWN_GAP))
                    .child(numeral)
                    .child(chip),
            )
            .child(
                div()
                    .absolute()
                    .inset(px(Theme::COUNTDOWN_FRAME_INSET))
                    .rounded(px(Theme::COUNTDOWN_FRAME_RADIUS))
                    .shadow(vec![hairline(
                        hsla(0., 0., 1., 0.55),
                        Theme::COUNTDOWN_FRAME_WIDTH,
                    )]),
            )
            .into_any_element()
    }

    pub(super) fn launcher(&self, state: &RecordingLauncher) -> AnyElement {
        let theme = self.theme;
        // The bar IS the window's plate — the window itself is transparent and
        // borderless, and the native material under it is masked to this
        // plate's shape (`update_recorder_glass`), so the plate is only the
        // tint over that glass. An outer plate around it only drew a second,
        // square box.
        //
        // It is one row of `RECORD_HEIGHT` controls: the handoff draws the
        // bar as a single line of the app's largest controls, so a 40px or
        // 44px control anywhere on it reads as a control that shrank.
        let plate = panel_variant(theme, UiSurface::Overlay)
            .size_full()
            .min_w_0()
            .overflow_hidden();
        // The controls sit on their own row inside the plate, so when the bar
        // turns from one job to the next its controls can fade in while the
        // plate stays still.
        let mut bar = row()
            .size_full()
            .min_w_0()
            .p(px(Theme::RECORDER_PADDING))
            .gap(px(Theme::GAP));
        let phase = if state.get_recording() {
            "recording"
        } else if state.get_counting() > 0 {
            "counting"
        } else if state.get_busy() {
            "working"
        } else {
            "ready"
        };
        let swap = |bar: Div| {
            plate
                .child(
                    bar.with_animation(
                        SharedString::from(format!("bar-{phase}")),
                        Animation::new(std::time::Duration::from_millis(BAR_SWAP_MS))
                            .with_easing(|t| subtake_ui::motion::EASE_OUT.eval(t)),
                        |bar, t| bar.opacity(t),
                    ),
                )
                .into_any_element()
        };
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
                    .w(px(Theme::RECORDER_HANDLE))
                    .h(px(Theme::RECORD_HEIGHT))
                    .cursor(CursorStyle::ClosedHand)
                    .child(icon("DotsSixVertical-regular", theme.muted))
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            );
        }
        if !state.get_recording() && !state.get_busy() {
            // Where each control that opens a card sits, so its card can
            // float over it rather than over the middle of the bar. The
            // children are the handle, then these five in this order.
            let panel = state.get_panel();
            bar = bar.on_children_prepainted(move |bounds, _, _| {
                let slot = ["sources", "audio", "camera", "countdown", "more"]
                    .iter()
                    .position(|id| *id == panel);
                let anchor = slot
                    .and_then(|i| bounds.get(i + 1))
                    .map(|b| f32::from(b.center().x))
                    .unwrap_or(-1.);
                if OPTIONS_ANCHOR.replace(anchor) != anchor {
                    crate::ui_runtime::Timer::single_shot(std::time::Duration::ZERO, move || {
                        crate::platform::set_launcher_options_anchor(anchor)
                    });
                }
            });
            // The source pill. The platform layer composes a display's name
            // as "<name> · <width>×<height>"; the handoff sets the name and
            // the resolution on two lines, so the pill splits it back apart.
            // A window source carries no separator and takes no second line.
            let name = state
                .get_source_names()
                .row_data(state.get_source_index().max(0) as usize)
                .unwrap_or_else(|| "Choose a source".into());
            let (name, detail) = match name.split_once(" · ") {
                Some((name, detail)) => (name.to_owned(), Some(detail.to_owned())),
                None => (name.to_string(), None),
            };
            let launcher = state.clone();
            let mut sources = button("sources", name, theme)
                .bar()
                .glyph("Monitor-regular")
                .caret()
                .stretch()
                .on_click(move |_, _, _| Self::toggle_panel(&launcher, "sources"));
            if let Some(detail) = detail {
                sources = sources.detail(detail);
            }
            bar = bar.child(sources);
            // Audio and camera say their state with the glyph and the plate,
            // not with an accent edge: a struck-through microphone on no
            // plate is off, a microphone on `sunk` is on. The accent has four
            // jobs in this design and "the mic is live" is not one of them.
            for (id, glyph, label, on) in [
                (
                    "audio",
                    if state.get_microphone() || state.get_system_audio() {
                        "Microphone-regular"
                    } else {
                        "MicrophoneSlash-regular"
                    },
                    "Audio",
                    state.get_microphone() || state.get_system_audio(),
                ),
                (
                    "camera",
                    if state.get_camera() {
                        "VideoCamera-regular"
                    } else {
                        "VideoCameraSlash-regular"
                    },
                    "Webcam",
                    state.get_camera(),
                ),
            ] {
                let launcher = state.clone();
                let mut control = button(id, label, theme).bar().glyph(glyph).icon_only();
                if !on {
                    control = control.ghost();
                }
                bar = bar.child(control.on_click(move |_, _, _| Self::toggle_panel(&launcher, id)));
            }
            let launcher = state.clone();
            bar = bar.child(
                button("countdown", format!("{}s", state.get_countdown()), theme)
                    .bar()
                    .glyph("Timer-regular")
                    .mono()
                    .on_click(move |_, _, _| Self::toggle_panel(&launcher, "countdown")),
            );
            let launcher = state.clone();
            bar = bar.child(
                button("more", "More", theme)
                    .bar()
                    .glyph("DotsThree-regular")
                    .icon_only()
                    .ghost()
                    .on_click(move |_, _, _| Self::toggle_panel(&launcher, "more")),
            );
            let launcher = state.clone();
            // Red, not accent. `rec` is the only red fill in the app and this
            // is the control it exists for; a blue Record button would make
            // the one destructive-adjacent action look like Export.
            bar = bar.child(
                button("record", "Record", theme)
                    .record()
                    .on_click(move |_, _, _| {
                        if launcher.get_source_names().row_count() == 0 {
                            launcher.set_panel("sources".into());
                            launcher.defer_panel("sources".into());
                            launcher.defer_action("sources".into());
                        } else {
                            launcher.defer_action("start-recording".into());
                        }
                    }),
            );
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
        // The bar's last control. The handoff draws a 44 close button and
        // nothing else after Record.
        //
        // What used to sit here as well: the app's own mark, and a "?" whose
        // only job was to hold the status text in a tooltip. Neither is
        // drawn — a logo on a five-control bar is a sixth thing to read, and
        // the status has the bar itself to speak in while a capture runs.
        bar = bar.child(
            icon_button("close", "X-regular", "Hide recorder", theme)
                .large()
                .ghost()
                .on_click(self.command("hide-launcher")),
        );
        swap(bar)
    }

    /// Recording and paused. The Record button's place becomes the clock —
    /// red while capture runs, `sunk` while it is held — and the controls
    /// after it keep one position across both, so Stop never moves under
    /// the pointer. Only Pause trades places with Resume.
    fn capture_controls(&self, bar: Div, state: &RecordingLauncher) -> Div {
        let theme = self.theme;
        let paused = state.get_paused();
        let enabled = !state.get_busy();
        let dot = div().flex_none().size(px(Theme::RECORD_DOT)).rounded_full();
        let dot = if paused {
            dot.bg(theme.rec)
                .opacity(Theme::PAUSED_DOT_OPACITY)
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
            .gap(px(Theme::ICON_GAP_RECORD))
            .h(px(Theme::RECORD_HEIGHT))
            .px(px(Theme::RECORDER_PILL_PADDING))
            .rounded_full()
            .whitespace_nowrap()
            .child(dot)
            .child(
                mono(state.get_elapsed())
                    .text_size(px(Theme::FONT_CLOCK))
                    .font_weight(FontWeight::MEDIUM)
                    .opacity(subtake_ui::motion::lerp(1., PAUSED_CLOCK_OPACITY, held)),
            );
        clock = if paused {
            // Palette churn: the handoff tracks PAUSED out by .09em; gpui
            // sets no letter spacing, so it is the caps alone.
            clock.child(fade_in(
                "clock-paused-label",
                div()
                    .text_size(px(Theme::FONT_SMALL))
                    .text_color(theme.muted)
                    .child("PAUSED"),
            ))
        } else {
            clock
        };
        let clock = clock
            .bg(subtake_ui::motion::blend(theme.rec, theme.sunk, held))
            .text_color(subtake_ui::motion::blend(white(), theme.text, held));
        let pause = if paused {
            // Resume takes Pause's place in `rec`: the one way back into
            // the capture, in the capture's own colour.
            row()
                .id("pause")
                .flex_none()
                .gap(px(Theme::ICON_GAP_ROW))
                .h(px(Theme::RECORD_HEIGHT))
                .px(px(Theme::RECORDER_PILL_PADDING))
                .rounded_full()
                .bg(theme.rec)
                .text_color(white())
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .when(enabled, |s| s.cursor_pointer())
                .when(!enabled, |s| s.opacity(Theme::DISABLED_OPACITY))
                .child(icon_sized("Play-fill", Theme::ICON_SIZE_POD, white()))
                .child("Resume")
                .when(enabled, |s| s.on_click(self.command("pause-recording")))
                .into_any_element()
        } else {
            self.bar_round("pause", "Pause-fill", "Pause", theme.text, true, enabled)
                .when(enabled, |s| s.on_click(self.command("pause-recording")))
                .into_any_element()
        };
        // Pause and Resume fade in as they trade places.
        let pause = fade_in(
            if paused { "resume-in" } else { "pause-in" },
            div().flex().flex_none().child(pause),
        );
        // Whether the microphone and the camera are in this capture is fixed
        // when it starts, so these say it rather than change it: on or off
        // is the glyph, never the colour.
        let mic = state.get_microphone() || state.get_system_audio();
        let camera = state.get_camera();
        bar.child(clock)
            .child(pause)
            .child(
                self.bar_round("stop", "Stop-fill", "Stop", theme.text, true, enabled)
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
                theme.muted,
                false,
                true,
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
                theme.muted,
                false,
                true,
            ))
            .child(
                self.bar_round(
                    "discard",
                    "X-regular",
                    "Discard recording",
                    theme.danger,
                    false,
                    enabled,
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
        let mic = if state.get_microphone() {
            "mic on"
        } else {
            "mic off"
        };
        bar.child(
            round_plate(theme).child(
                mono(count.to_string())
                    .text_size(px(Theme::FONT_COUNT))
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
                .gap(px(Theme::ICON_GAP_ROW))
                .h(px(Theme::RECORD_HEIGHT))
                .px(px(Theme::RECORDER_PLATE_PADDING))
                .rounded_full()
                .bg(theme.sunk)
                .hover(move |s| s.bg(theme.sunk2))
                .cursor_pointer()
                .whitespace_nowrap()
                .child("Cancel")
                .child(
                    mono("esc")
                        .text_size(px(Theme::FONT_SMALL))
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
        let bar = bar.child(round_plate(theme).child(spinner(Theme::SPINNER_SIZE, theme)));
        if state.get_stopping() {
            return bar
                .child(
                    message(
                        "Finishing your recording",
                        format!("{} captured · writing to disk", state.get_elapsed()),
                        theme,
                    )
                    .min_w(px(Theme::STOPPING_TEXT_WIDTH)),
                )
                .child(
                    row()
                        .flex_none()
                        .h(px(Theme::RECORD_HEIGHT))
                        .px(px(Theme::RECORDER_PLATE_PADDING))
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

    /// A 60 round control on the bar. `plate` is the `sunk` fill Pause and
    /// Stop stand on; without it the control is bare glass until hovered.
    fn bar_round(
        &self,
        id: &'static str,
        glyph: &'static str,
        label: &'static str,
        color: Hsla,
        plate: bool,
        enabled: bool,
    ) -> Stateful<Div> {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(Theme::RECORD_HEIGHT))
            .rounded_full()
            .when(plate, |s| s.bg(theme.sunk))
            .when(enabled, |s| {
                s.hover(move |s| s.bg(if plate { theme.sunk2 } else { theme.hover }))
                    .cursor_pointer()
            })
            .when(!enabled, |s| s.opacity(Theme::DISABLED_OPACITY))
            .tooltip(move |_, cx| tooltip(label, theme, cx))
            .child(icon_sized(glyph, Theme::ICON_SIZE_LARGE, color))
    }

    /// A bar control that opens its panel, or closes it if it is the open one.
    fn toggle_panel(state: &RecordingLauncher, id: &str) {
        let value = if state.get_panel() == id { "" } else { id };
        state.set_panel(value.into());
        state.defer_panel(value.into());
    }
}

/// The bar's 60 round `sunk` plate: it holds the count, or the spinner.
fn round_plate(theme: Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(Theme::RECORD_HEIGHT))
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
        .h(px(Theme::RECORD_HEIGHT))
        .px(px(Theme::RECORDER_TEXT_INSET))
        .line_height(relative(Theme::MESSAGE_LEADING))
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
                    .text_size(px(Theme::FONT_SMALL))
                    .text_color(theme.muted)
                    .text_ellipsis()
                    .child(detail),
            )
        })
}

/// A `sunk2` ring with an accent arc turning round it once a second.
pub(super) fn spinner(size: f32, theme: Theme) -> Div {
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded_full()
        .shadow(vec![hairline(theme.sunk2, Theme::SPINNER_WIDTH)])
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
