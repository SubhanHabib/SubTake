//! The recorder bar. Its option cards are `options.rs`.

use super::*;

thread_local! {
    /// The anchor last handed to the options window, so the bar only moves
    /// the card when its control has actually moved.
    static OPTIONS_ANCHOR: Cell<f32> = const { Cell::new(-1.) };
}

impl RootView {
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
        let mut bar = panel_variant(theme, UiSurface::Overlay)
            .flex_row()
            .items_center()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .p(px(Theme::RECORDER_PADDING))
            .gap(px(Theme::GAP))
            .child(
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
        } else {
            // While recording, the word and the clock are two children, not
            // one formatted string: the clock is Geist Mono so its digits do
            // not shove the word beside them every time the seconds tick.
            bar = bar.child(if state.get_recording() {
                row()
                    .flex_1()
                    .gap(px(Theme::ICON_GAP))
                    .child(if state.get_paused() { "PAUSED" } else { "REC" })
                    .child(mono(state.get_elapsed()))
            } else {
                row().flex_1().child(state.get_status())
            });
            if state.get_recording() {
                bar = bar
                    .child(self.action(
                        "pause",
                        if state.get_paused() {
                            "Resume"
                        } else {
                            "Pause"
                        },
                        "pause-recording",
                        !state.get_busy(),
                    ))
                    .child(self.action("stop", "Stop", "stop-recording", !state.get_busy()));
            }
            if state.get_cancellable() {
                bar = bar.child(self.action("cancel", "Cancel", "cancel", true));
            }
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
        bar.into_any_element()
    }

    /// A bar control that opens its panel, or closes it if it is the open one.
    fn toggle_panel(state: &RecordingLauncher, id: &str) {
        let value = if state.get_panel() == id { "" } else { id };
        state.set_panel(value.into());
        state.defer_panel(value.into());
    }
}
