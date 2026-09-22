//! The editor window chrome: title bar, rail, canvas, inspector and timeline
//! composed into one surface.

use super::*;

impl RootView {
    pub(super) fn editor(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        // The unified window titlebar: a strip tall enough for the 40px icon
        // cluster, carrying no fill of its own — the redesign has the shell's
        // `bg` run unbroken behind it rather than a second tone on top.
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .h(px(TITLEBAR_HEIGHT))
            .px(px(Theme::GAP_LARGE))
            .flex_shrink_0();
        if e.get_mac_titlebar() {
            // Traffic lights sit at {14,15}; the cluster starts at 88px.
            header = header.pl(px(88.));
        }
        header = header
            .child(self.brand())
            .child(self.icon_action(
                "open",
                "FolderOpen-regular",
                "Open projects",
                "Recent",
                true,
            ))
            .child(self.icon_action(
                "save",
                "FloppyDisk-regular",
                "Save",
                "save",
                e.get_has_video(),
            ))
            .child(self.icon_action(
                "undo",
                "ArrowCounterClockwise-regular",
                "Undo",
                "undo",
                e.get_can_undo(),
            ))
            .child(self.icon_action(
                "redo",
                "ArrowClockwise-regular",
                "Redo",
                "redo",
                e.get_can_redo(),
            ))
            // Document title, with the unsaved dot as a coloured mark rather
            // than a bullet in the string.
            .child(
                div()
                    .id("title-drag")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(Theme::GAP_SMALL))
                    .when(e.get_dirty(), |el| el.child(status_dot(theme)))
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(Theme::FONT_BODY))
                            .text_color(theme.text)
                            .child(e.get_document_title()),
                    )
                    .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move()),
            )
            .child(
                button(
                    "record",
                    self.translate(e, if e.get_recording() { "Stop" } else { "Record" }),
                    theme,
                )
                .glyph(if e.get_recording() {
                    "Stop-fill"
                } else {
                    "Record-regular"
                })
                .ghost()
                .enabled(!e.get_busy())
                .on_click(self.command(if e.get_recording() {
                    "stop-recording"
                } else {
                    "record"
                })),
            );
        if e.get_recording() {
            header = header.child(
                button(
                    "pause-recording",
                    if e.get_recording_paused() {
                        "Resume"
                    } else {
                        "Pause"
                    },
                    theme,
                )
                .glyph("Pause-regular")
                .ghost()
                .on_click(self.command("pause-recording")),
            );
        }
        header = header
            .child(
                self.panel_button(e, "Presets", "Presets")
                    .glyph("Stack-regular")
                    .ghost(),
            )
            .child(
                self.panel_button(e, "Export", "Export")
                    .glyph("Export-regular")
                    .primary()
                    .enabled(e.get_has_video() && !e.get_busy()),
            );
        // The tool pod. The handoff docks nothing: the rail is a 60-wide
        // float 24 from the window's left edge, vertically centred, and the
        // picture runs under it. It was a full-height column in the content
        // flex, which is the layout the redesign exists to replace.
        //
        // Only the panel list scrolls. Settings and Help used to sit after a
        // `flex_1` spacer INSIDE the scroll region, which pins them to the
        // bottom only while the content fits — the moment it overflows the
        // spacer collapses and they scroll away with everything else, so at
        // 980x680 they were unreachable. They live outside the scroller.
        let mut panels = div()
            .id("rail")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w_full()
            .min_h_0()
            .overflow_y_scroll();
        // Icons only; the active panel takes the accent fill.
        for (label, name, glyph) in [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Webcam", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ] {
            panels = panels.child(self.rail_panel_button(e, label, name, glyph));
        }
        // A hairline before the two that are not tools, as the pod is drawn.
        let rail = pod(theme)
            .flex_col()
            .w(px(Theme::POD_WIDTH))
            .max_h_full()
            .child(panels)
            .child(divider(theme).mx(px(Theme::GAP_SMALL)))
            .child(self.rail_panel_button(e, "Settings", "Preferences", "Gear-regular"))
            .child(self.rail_panel_button(e, "Help", "shortcut-reference", "Question-regular"));
        // Centred on the stage's own height. gpui at the pinned revision has
        // no transform, so a float is centred by a full-height strip around
        // it rather than by a half-height offset.
        let rail = div()
            .absolute()
            .left(px(Theme::INSET))
            .top_0()
            .bottom_0()
            .py(px(Theme::INSET))
            .flex()
            .items_center()
            .child(frosted(
                UiSurface::Pod.radius(),
                UiSurface::Pod.blur(),
                rail,
            ));
        let preview = self.preview(e, window, cx);
        let inspector = self.inspector(e, window, cx);
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .gap_0()
            .child(header)
            .child(
                // The stage. Everything over it is absolute, so the preview
                // measures the picture area and nothing else — the zoom and
                // the pan both read that measurement.
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(preview)
                    .child(rail)
                    .child(inspector),
            );
        let status = self.status_strip(e);
        if e.get_has_video() {
            root = root.child(self.timeline(e, status, cx));
        } else if let Some(status) = status {
            root = root.child(
                div()
                    .px(px(Theme::GAP_LARGE))
                    .pb(px(Theme::GAP))
                    .child(status),
            );
        }
        root.child(self.menu_overlay(window, cx)).into_any_element()
    }

    /// The export and transcription line. `None` when there is nothing to
    /// say, so the console keeps the shell's own bottom margin instead of
    /// reserving a strip under it — the console's inset from the bottom edge
    /// now matches its inset from the left and right.
    ///
    /// TODO(redesign): the handoff has no status strip at all. Its four
    /// screens put progress on the thing that is progressing — a bar inside
    /// the export dialog, a spinner on the region being rendered — and leave
    /// the shell's bottom edge to the console. This is carried because export
    /// and transcription still need somewhere to speak, but it now lives
    /// inside the console rather than under it, and only while it has
    /// something to report.
    fn status_strip(&mut self, e: &EditorWindow) -> Option<AnyElement> {
        let theme = self.theme;
        let busy = e.get_busy();
        let text = e.get_status();
        if !busy && text.is_empty() {
            return None;
        }
        let mut line = row()
            .text_size(px(Theme::FONT_SMALL))
            .text_color(theme.muted)
            .child(div().flex_1().text_ellipsis().child(text));
        if busy {
            line = line.child(self.action("cancel", "Cancel", "cancel", true));
        }
        let mut status = div()
            .flex()
            .flex_col()
            .gap(px(Theme::GAP_SMALL))
            .child(line);
        if busy {
            status = status.child(progress_bar(e.get_progress(), theme));
        }
        Some(status.into_any_element())
    }

    pub(super) fn brand(&self) -> impl IntoElement {
        svg()
            .path("assets/branding/menu-bar.svg")
            .size(px(Theme::ICON_SIZE_LARGE))
            .flex_none()
            .text_color(self.theme.text)
    }
}
