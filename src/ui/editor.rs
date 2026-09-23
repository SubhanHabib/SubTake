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
        // The unified window titlebar: 64 tall, carrying no fill of its own —
        // the redesign has the shell's `bg` run unbroken behind it rather
        // than a second tone on top.
        //
        // Open, Save, Undo and Redo used to sit here as an icon cluster. The
        // handoff's titlebar holds the document and the three things you do
        // to a finished one, and nothing else; all four commands are in the
        // File and Edit menus, so the cluster was a second copy of a menu in
        // the app's most valuable strip of chrome.
        let mut header = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(Theme::GAP))
            .h(px(Theme::TITLEBAR_HEIGHT))
            .pl(px(Theme::GAP_LARGE))
            .pr(px(Theme::TITLEBAR_PADDING))
            .flex_shrink_0();
        if e.get_mac_titlebar() {
            header = header.pl(px(Theme::TITLEBAR_TRAFFIC_LIGHTS));
        }
        // With nothing open the titlebar is empty, as the handoff draws it:
        // Record and Open are the empty state's own two buttons, and there is
        // no document to name, preset or export.
        if e.get_has_video() || e.get_recording() {
            // Not drawn by the design: where the export pill sits. The
            // handoff draws it filling a titlebar of its own, so it stands in
            // for the document pill while it shows, centred on the window as
            // the document is and narrowed where the buttons would meet it.
            let export = self.export_pill(e, window, cx);
            header = header.child(div().flex_1());
            let mut cluster = row()
                .relative()
                .gap(px(Theme::GAP))
                .child(measure(self.titlebar_cluster.clone()));
            cluster = cluster.child(
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
                .enabled(!e.get_busy())
                .on_click(self.command(if e.get_recording() {
                    "stop-recording"
                } else {
                    "record"
                })),
            );
            if e.get_recording() {
                cluster = cluster.child(
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
                    .on_click(self.command("pause-recording")),
                );
            }
            cluster = cluster
                .child(
                    button("presets", "Presets", theme)
                        .glyph("Stack-regular")
                        .icon_only()
                        .selected(e.get_dialog() == "presets")
                        .enabled(!e.get_busy())
                        .on_click({
                            let e = e.clone();
                            move |_, _, _| {
                                let open = e.get_dialog() == "presets";
                                e.set_dialog(if open { "" } else { "presets" }.into());
                            }
                        }),
                )
                .child(
                    self.panel_button(e, "Export", "Export")
                        .glyph("Export-regular")
                        .primary()
                        // Not wired: a second export queued behind the first.
                        // The handoff moves progress out of the panel so one
                        // can be; until there is a queue, Export waits.
                        .enabled(
                            e.get_has_video()
                                && !e.get_busy()
                                && e.get_export_state() != "exporting",
                        ),
                );
            // The document pill, centred on the window rather than on the gap
            // between the two clusters: a `flex_1` between them centres it in
            // whatever they leave, which moves every time a button appears.
            // gpui at the pinned revision has no transform, so it is a
            // full-width absolute strip with the pill centred inside it.
            //
            // The strip itself takes no pointer events — only the pill has a
            // listener — so the buttons underneath it stay clickable.
            header = header.child(cluster);
            // The document pill, centred on the window rather than on the gap
            // between the two clusters: a `flex_1` between them centres it in
            // whatever they leave, which moves every time a button appears.
            // gpui at the pinned revision has no transform, so it is a
            // full-width absolute strip with the pill centred inside it.
            //
            // The strip itself takes no pointer events — only the pill has a
            // listener — so the buttons underneath it stay clickable.
            let title_pill = row()
                .id("title-drag")
                .relative()
                .max_w(relative(0.4))
                .h(px(Theme::TITLE_PILL_HEIGHT))
                .px(px(Theme::CONTROL_PADDING_SMALL))
                .gap(px(Theme::GAP_SMALL))
                .rounded_full()
                .bg(theme.sunk)
                // The handoff's dot is decoration. This one says
                // the document has unsaved work, which is the
                // only thing the titlebar has left to say it
                // with, so it appears rather than always burning.
                .when(e.get_dirty(), |el| el.child(status_dot(theme)))
                .child(
                    div()
                        .min_w_0()
                        .text_ellipsis()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(e.get_document_title()),
                )
                .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move())
                .child(measure(self.title_pill.clone()));
            // While an export runs or has just ended its pill takes the
            // document pill's place, the one growing into the other: the
            // width and height ease between the two and the old contents fade
            // out before the new fade in.
            let shown = slide_toward(
                &mut self.pill_morph,
                export.is_some(),
                PILL_MORPH_MS,
                window,
            );
            let centre = f32::from(window.viewport_size().width) / 2.;
            let cluster = f32::from(self.titlebar_cluster.get().left());
            let room = (cluster - centre - Theme::GAP) * 2.;
            let export_width = if cluster > centre {
                Theme::EXPORT_PILL_WIDTH.min(room)
            } else {
                Theme::EXPORT_PILL_WIDTH
            };
            let piece = match export {
                Some(pill) if shown >= 1. => pill.w(px(export_width)).into_any_element(),
                None if shown <= 0. => title_pill.into_any_element(),
                export => {
                    let title_width = f32::from(self.title_pill.get().size.width);
                    let (content, fade) = match export {
                        Some(pill) => {
                            (pill.w(px(export_width)).into_any_element(), shown * 2. - 1.)
                        }
                        None => (title_pill.into_any_element(), 1. - shown * 2.),
                    };
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(subtake_ui::motion::lerp(
                            title_width,
                            export_width,
                            shown,
                        )))
                        .h(px(subtake_ui::motion::lerp(
                            Theme::TITLE_PILL_HEIGHT,
                            Theme::CONTROL_HEIGHT_LARGE,
                            shown,
                        )))
                        .rounded_full()
                        .bg(theme.sunk)
                        .overflow_hidden()
                        .child(div().flex_none().opacity(fade.clamp(0., 1.)).child(content))
                        .into_any_element()
                }
            };
            header = header.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(piece),
            );
        }
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
        // Icons only; the active panel takes the accent fill. The pod offers
        // tools — modes that change what the stage does — and stays at six:
        // Selection is not one (it follows the timeline's selection), and
        // Presets and Export are the titlebar's.
        //
        // Not drawn by the design: the Zoom tool. The round-2 handoff puts a
        // Sparkle "Zoom" first and moves Scene onto the aspect pod, but draws
        // neither the Zoom inspector nor the aspect pod's way into Scene, so
        // the Sparkle keeps opening Scene until both are.
        for (label, name, glyph) in [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Camera", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ] {
            panels = panels.child(self.rail_panel_button(e, label, name, glyph));
        }
        // A hairline before the two that are not tools, as the pod is drawn.
        //
        // Not drawn by the design: Help. The round-2 pod stops at six with
        // Settings, but Help is the only way to the shortcut reference until
        // the menus and the reference (3e, 3f) are drawn, so it stays.
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
        let aspect_pod = self.aspect_pod(e, cx);
        // The empty state has no tools to pod and no scene to inspect. The
        // inspector still opens over it for the panels that stand on their
        // own — Settings from the menu, Projects when a recovery is waiting.
        let inspector =
            (e.get_has_video() || e.get_panel() != "Frame").then(|| self.inspector(e, window, cx));
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
                    .children(aspect_pod)
                    .children(e.get_has_video().then_some(rail))
                    .children(inspector),
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
        root.children(self.presets_dialog(e, window, cx))
            .child(self.menu_overlay(window, cx))
            .into_any_element()
    }

    /// The transcription and background-job line. `None` when there is
    /// nothing to say, so the console keeps the shell's own bottom margin
    /// instead of reserving a strip under it.
    ///
    /// Not drawn by the design: the handoff has no status strip at all.
    /// Export has left it for the titlebar pill; transcription, captions and
    /// the other jobs still need somewhere to speak until the round that
    /// draws toasts gives them one, so it stays, inside the console and only
    /// while it has something to report.
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
}
