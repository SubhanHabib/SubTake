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
        // The unified window titlebar: a translucent strip tall enough for
        // the 40px icon cluster, on the `header` tone so the vibrancy
        // material reads through it.
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .h(px(TITLEBAR_HEIGHT))
            .px(px(Theme::GAP_LARGE))
            .flex_shrink_0()
            .bg(theme.header);
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
        // Only the panel list scrolls. Settings and Help used to sit after a
        // `flex_1` spacer INSIDE the scroll region, which pins them to the
        // bottom only while the content fits — the moment it overflows the
        // spacer collapses and they scroll away with everything else, so at
        // 980x680 they were unreachable. They now live outside the scroller.
        let mut panels = div()
            .id("rail")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w_full()
            .flex_1()
            .min_h_0()
            .py(px(FADE_BAND))
            .overflow_y_scroll();
        // Icons only; the active panel keeps the accent plate and the marker.
        for (label, name, glyph) in [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Webcam", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ] {
            panels = panels.child(self.rail_panel_button(e, label, name, glyph));
        }
        let rail = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::GAP_SMALL))
            .w(px(RAIL_WIDTH))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .child(fade_edges(panels))
            .child(self.rail_panel_button(e, "Settings", "Preferences", "Gear-regular"))
            .child(self.rail_panel_button(e, "Help", "shortcut-reference", "Question-regular"))
            .pb(px(Theme::GAP_SMALL));
        let preview = self.preview(e, window, cx);
        let inspector = self.inspector(e, window, cx);
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .gap_0()
            .child(header)
            .child(
                div()
                    .flex()
                    .items_start()
                    .flex_1()
                    .min_h_0()
                    .p(px(Theme::GAP))
                    .gap(px(Theme::GAP))
                    .child(rail)
                    .child(preview)
                    .child(inspector),
            );
        if e.get_has_video() {
            root = root.child(self.timeline(e, cx));
        }
        // Reserved status strip under the content outlet (reference
        // `Theme::CONTROL_HEIGHT`): reserving it keeps the timeline from
        // shifting when a status line appears.
        let mut status = div()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(Theme::GAP_SMALL))
            .min_h(px(Theme::CONTROL_HEIGHT))
            .px(px(Theme::GAP_LARGE))
            .text_size(px(Theme::FONT_SMALL));
        let mut status_line = row()
            .text_color(theme.muted)
            .child(div().flex_1().text_ellipsis().child(e.get_status()));
        if e.get_busy() {
            status_line = status_line.child(self.action("cancel", "Cancel", "cancel", true));
        }
        status = status.child(status_line);
        if e.get_busy() {
            status = status.child(progress_bar(e.get_progress(), theme));
        }
        root.child(status)
            .child(self.menu_overlay(window, cx))
            .into_any_element()
    }

    pub(super) fn brand(&self) -> impl IntoElement {
        svg()
            .path("assets/branding/menu-bar.svg")
            .size(px(Theme::ICON_SIZE_LARGE))
            .flex_none()
            .text_color(self.theme.text)
    }
}
