//! The recorder bar and its detached options card.

use super::*;

impl RootView {
    pub(super) fn launcher(&self, s: &RecordingLauncher) -> AnyElement {
        let t = self.theme;
        // The bar IS the window's plate — the window itself is transparent and
        // borderless, so there is nothing behind this to tint. An outer plate
        // around it only drew a second, square box.
        let mut bar = panel_variant(t, UiSurface::Overlay)
            .flex_row()
            .items_center()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .p(px(Theme::GAP))
            .gap(px(Theme::GAP_SMALL))
            .child(
                // Was a bare "⠿" text character: no size token, no colour
                // token and no control geometry, so it sat misaligned beside
                // the 40px controls. Same glyph, on the scale everything
                // else uses.
                div()
                    .id("launcher-drag")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(Theme::CONTROL_HEIGHT / 2.0))
                    .h(px(Theme::CONTROL_HEIGHT))
                    .cursor(CursorStyle::ClosedHand)
                    .child(icon("DotsSixVertical-regular", t.muted))
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            );
        if !s.get_recording() && !s.get_busy() {
            bar = bar.child(self.brand());
            for (id, label, selected) in [
                (
                    "sources",
                    s.get_source_names()
                        .row_data(s.get_source_index().max(0) as usize)
                        .unwrap_or_else(|| "Choose a source".into()),
                    s.get_panel() == "sources",
                ),
                (
                    "audio",
                    "Audio".into(),
                    s.get_microphone() || s.get_system_audio(),
                ),
                ("camera", "Webcam".into(), s.get_camera()),
                (
                    "countdown",
                    format!("{}s", s.get_countdown()),
                    s.get_countdown() > 0,
                ),
                ("more", "More".into(), s.get_panel() == "more"),
            ] {
                let launcher = s.clone();
                let control = button(id, label, t)
                    .selected(selected)
                    .on_click(move |_, _, _| {
                        let value = if launcher.get_panel() == id { "" } else { id };
                        launcher.set_panel(value.into());
                        launcher.defer_panel(value.into());
                    });
                // The source name is the only variable-width control, so it
                // takes the slack and ellipsizes; a fixed width pushed the
                // rest of the bar past the window on long display names.
                bar = bar.child(if id == "sources" {
                    control.stretch()
                } else {
                    control
                });
            }
            let launcher = s.clone();
            bar = bar.child(
                button("record", "Record", t)
                    .primary()
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
            bar = bar.child(div().flex_1().child(if s.get_recording() {
                format!(
                    "{}  {}",
                    if s.get_paused() { "PAUSED" } else { "REC" },
                    s.get_elapsed()
                )
            } else {
                s.get_status()
            }));
            if s.get_recording() {
                bar = bar
                    .child(self.action(
                        "pause",
                        if s.get_paused() { "Resume" } else { "Pause" },
                        "pause-recording",
                        !s.get_busy(),
                    ))
                    .child(self.action("stop", "Stop", "stop-recording", !s.get_busy()));
            }
            if s.get_cancellable() {
                bar = bar.child(self.action("cancel", "Cancel", "cancel", true));
            }
        }
        // The quiet end of the bar. These were a bare "?" with no hit target
        // and a full button plate whose caption was the literal character
        // "×"; both are now icon controls at the shared geometry.
        let hint = s.get_status();
        bar = bar
            .child(
                div()
                    .id("recorder-status")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(Theme::CONTROL_HEIGHT))
                    .child(icon("Question-regular", t.muted))
                    .tooltip(move |_, cx| tooltip(hint.clone(), t, cx)),
            )
            .child(self.icon_action("close", "X-regular", "Hide recorder", "hide-launcher", true));
        bar.into_any_element()
    }

    pub(super) fn options(&mut self, s: &RecordingOptions, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let name = s.get_panel();
        let title = match name.as_str() {
            "sources" => "Screens and windows",
            "audio" => "Microphone & system audio",
            "camera" => "Webcam",
            "countdown" => "Countdown delay",
            _ => "More",
        };
        let options = s.clone();
        // Composer structure: a context chip naming the surface, the controls
        // beneath it, and a quiet footer row. The close control is an icon at
        // the shared geometry rather than a button whose caption was the
        // literal character "×".
        let mut body = panel_variant(t, UiSurface::Overlay)
            .size_full()
            .p(px(Theme::GAP_LARGE))
            .gap(px(Theme::GAP))
            .child(
                row()
                    .child(context_chip(t, &["Recorder", title]).flex_1().min_w_0())
                    .child(
                        icon_button("close", "X-regular", "Close", t)
                            .ghost()
                            .on_click(move |_, _, _| options.defer_panel("".into())),
                    ),
            );
        match name.as_str() {
            "sources" => {
                let options = s.clone();
                let sources = self.dropdown(
                    "sources",
                    s.get_source_names().iter().collect(),
                    s.get_source_index(),
                    !s.get_busy(),
                    cx,
                    move |i, _, _| options.defer_option("source".into(), i.to_string()),
                );
                body = body
                    .child(section_label("Capture source", t))
                    .child(sources)
                    .child(self.action(
                        "refresh",
                        "Refresh displays and windows",
                        "sources",
                        !s.get_busy(),
                    ))
                    .child(div().flex_1())
                    .child(
                        composer_footer(t).child("Choose a display or a visible window to record."),
                    );
            }
            "audio" => {
                let options = s.clone();
                let microphone = self.dropdown(
                    "microphone",
                    s.get_microphone_names().iter().collect(),
                    s.get_microphone_index(),
                    !s.get_busy() && s.get_microphone(),
                    cx,
                    move |i, _, _| options.defer_option("microphone-device".into(), i.to_string()),
                );
                let s1 = s.clone();
                let s2 = s.clone();
                body = body
                    .child(toggle(
                        "mic-toggle",
                        "Microphone",
                        s.get_microphone(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| s1.defer_option("microphone".into(), v.to_string()),
                    ))
                    .child(microphone)
                    .child(toggle(
                        "system-toggle",
                        "System audio",
                        s.get_system_audio(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| s2.defer_option("system-audio".into(), v.to_string()),
                    ));
            }
            "camera" => {
                let options = s.clone();
                let camera = self.dropdown(
                    "camera",
                    s.get_camera_names().iter().collect(),
                    s.get_camera_index(),
                    !s.get_busy() && s.get_camera(),
                    cx,
                    move |i, _, _| options.defer_option("camera-device".into(), i.to_string()),
                );
                let options = s.clone();
                body = body
                    .child(toggle(
                        "camera-toggle",
                        "Webcam overlay",
                        s.get_camera(),
                        !s.get_busy(),
                        t,
                        move |v, _, _| options.defer_option("camera".into(), v.to_string()),
                    ))
                    .child(camera)
                    .child(
                        div()
                            .text_color(t.muted)
                            .child("Your camera is recorded separately and added to the project."),
                    );
            }
            "countdown" => {
                let mut choices = row().flex_wrap();
                for (label, value) in [
                    ("No delay", 0),
                    ("3 seconds", 3),
                    ("5 seconds", 5),
                    ("10 seconds", 10),
                ] {
                    let options = s.clone();
                    choices = choices.child(
                        button(label, label, t)
                            .selected(s.get_countdown() == value)
                            .on_click(move |_, _, _| {
                                options.defer_option("countdown".into(), value.to_string())
                            }),
                    );
                }
                body = body
                    .child("Give yourself a moment before recording starts.")
                    .child(choices);
            }
            _ => {
                body = body
                    .child(self.action(
                        "storyboard",
                        "Create video · spike",
                        "storyboard-spike",
                        true,
                    ))
                    .child(
                        row()
                            .child(self.action("open", "Open video or project", "open", true))
                            .child(self.action("projects", "Projects", "projects", true))
                            .child(self.action(
                                "editor",
                                "Back to editor",
                                "show-editor",
                                s.get_has_project(),
                            )),
                    )
                    .child(div().flex_1())
                    // The path and the control that changes it are one
                    // setting; they were a muted line and a row a gap apart,
                    // with the value drifting away from its own label.
                    .child(
                        group_card(t, "Recordings path").child(
                            row()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_ellipsis()
                                        .child(s.get_directory()),
                                )
                                .child(self.action(
                                    "folder",
                                    "Choose folder…",
                                    "recording-folder",
                                    true,
                                )),
                        ),
                    );
            }
        }
        body.into_any_element()
    }
}
