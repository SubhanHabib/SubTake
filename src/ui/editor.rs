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
            .gap(px(Theme::gap()))
            .h(px(Theme::titlebar_height()))
            .pl(px(Theme::gap_large()))
            .pr(px(Theme::titlebar_padding()))
            .flex_shrink_0();
        if e.get_mac_titlebar() {
            header = header.pl(px(Theme::titlebar_traffic_lights()));
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
                .gap(px(Theme::gap()))
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
                .icon_only()
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
                    .glyph(if e.get_recording_paused() {
                        "Play-regular"
                    } else {
                        "Pause-regular"
                    })
                    .icon_only()
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
                        .icon_only()
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
            header = header.child(cluster);
            // The document pill, centred on the window rather than on the gap
            // between the two clusters: a `flex_1` between them centres it in
            // whatever they leave, which moves every time a button appears.
            // gpui at the pinned revision has no transform, so it is a
            // full-width absolute strip with the pill centred inside it.
            //
            // The strip itself takes no pointer events — only the pill has a
            // listener — so the buttons underneath it stay clickable.
            // Not drawn by the design: the pill's hover, which says it can
            // be grabbed. It takes no press and no focus: it only moves the
            // window, which the system takes over mid-press, and there is
            // nothing for the keyboard to do with it.
            let title_hover = subtake_ui::motion::tween_key(&"title-drag".into(), "hover");
            let status = self.status_chip(e);
            let title_pill = row()
                .id("title-drag")
                .on_hover(subtake_ui::motion::hover_listener(title_hover.clone()))
                .relative()
                .max_w(relative(Theme::title_pill_share()))
                .h(px(Theme::title_pill_height()))
                .px(px(Theme::control_padding_small()))
                .when(status.is_some(), |el| {
                    el.pr(px(Theme::title_pill_chip_padding()))
                })
                .gap(px(Theme::gap_small()))
                .rounded_full()
                // The handoff's dot is decoration. This one says
                // the document has unsaved work, which is the
                // only thing the titlebar has left to say it
                // with, so it appears rather than always burning.
                .when(e.get_dirty(), |el| el.child(status_dot(theme)))
                .child({
                    // At its cap the title is cut short, and the tooltip is
                    // the only place left to read the rest of it.
                    let title = e.get_document_title();
                    let cap = f32::from(window.viewport_size().width) * Theme::title_pill_share();
                    let cut = f32::from(self.title_pill.get().size.width) + 0.5 >= cap;
                    div()
                        .id("document-title")
                        .min_w_0()
                        .text_ellipsis()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .when(cut, |el| {
                            let title = title.clone();
                            el.tooltip(move |_, cx| tooltip(title.clone(), theme, cx))
                        })
                        .child(title)
                })
                .children(status)
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
            let room = (cluster - centre - Theme::gap()) * 2.;
            let export_width = if cluster > centre {
                Theme::export_pill_width().min(room)
            } else {
                Theme::export_pill_width()
            };
            // Neither pill paints its own plate: mid-morph the one growing
            // around them does, and a plate of their own, clipped square by
            // it, would show corners. The plate is a pod's frosted `glass`
            // with its hairline and shadow, since the stage and the desktop
            // run under the titlebar and a `sunk` tint over them left the
            // title unreadable. Not drawn by the design, whose pill is
            // `sunk` on the window's own ground.
            let edge = || {
                let mut edge = vec![subtake_ui::hairline(theme.line, Theme::hairline_width())];
                edge.extend(theme.panel_shadow());
                edge
            };
            let (piece, height) = match export {
                Some(pill) if shown >= 1. => (
                    pill.w(px(export_width))
                        .bg(theme.glass)
                        .shadow(edge())
                        .into_any_element(),
                    Theme::control_height_large(),
                ),
                None if shown <= 0. => (
                    title_pill
                        .bg(subtake_ui::motion::hover_blend(
                            &title_hover,
                            theme.glass,
                            theme.card,
                        ))
                        .shadow(edge())
                        .into_any_element(),
                    Theme::title_pill_height(),
                ),
                export => {
                    let title_width = f32::from(self.title_pill.get().size.width);
                    // The title holds the first half whichever way it runs;
                    // the export pill, while there is one, the second. An
                    // export that has already gone leaves that half empty.
                    let (content, fade) = match export {
                        Some(pill) if shown >= 0.5 => {
                            (pill.w(px(export_width)).into_any_element(), shown * 2. - 1.)
                        }
                        // At the width it had in the titlebar: its cap is a
                        // share of its parent, which is now the morph.
                        _ if shown < 0.5 => (
                            title_pill.max_w(px(title_width)).into_any_element(),
                            1. - shown * 2.,
                        ),
                        _ => (div().into_any_element(), 0.),
                    };
                    let height = subtake_ui::motion::lerp(
                        Theme::title_pill_height(),
                        Theme::control_height_large(),
                        shown,
                    );
                    (
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(subtake_ui::motion::lerp(
                                title_width,
                                export_width,
                                shown,
                            )))
                            .h(px(height))
                            .rounded_full()
                            .bg(theme.glass)
                            .shadow(edge())
                            .overflow_hidden()
                            .child(div().flex_none().opacity(fade.clamp(0., 1.)).child(content))
                            .into_any_element(),
                        height,
                    )
                }
            };
            let piece = frosted(height / 2., UiSurface::Pod.blur(), piece);
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
        // The whole pod is one scroll region, Settings and Help included, so
        // a short window scrolls the pod rather than cutting one of its
        // halves short. The region runs to the glass's edge and cuts there,
        // with the pod's padding inside it: nothing stands after a spacer,
        // so nothing is pinned to an end that overflowing would lose.
        let mut panels = div()
            .id("rail")
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::gap_small()))
            .p(px(Theme::pod_padding()))
            .w_full()
            .min_h_0()
            .overflow_y_scroll();
        // Icons only; the active panel takes the accent fill. The pod offers
        // tools — modes that change what the stage does: Selection is not
        // one (it follows the timeline's selection), and Presets and Export
        // are the titlebar's.
        //
        // Not drawn by the design: Add. The round-2 pod stops at six, and
        // the timeline's Add menu is its only way to put things on the
        // picture; the tool gives each annotation kind a tile.
        //
        // Not drawn by the design: the Zoom tool. The round-2 handoff puts a
        // Sparkle "Zoom" first and moves Scene onto the aspect pod, but draws
        // neither the Zoom inspector nor the aspect pod's way into Scene, so
        // the Sparkle keeps opening Scene until both are.
        let tools = [
            ("Scene", "Frame", "Sparkle-regular"),
            ("Cursor", "Cursor", "Cursor-regular"),
            ("Camera", "Webcam", "Camera-regular"),
            ("Captions", "Captions", "ClosedCaptioning-regular"),
            ("Add", "Add", "PlusSquare-regular"),
            ("Audio", "Audio", "SpeakerHigh-regular"),
        ];
        // The active tool's accent and the hover wash glide between tools
        // rather than switching on in place, as the Settings sidebar's do.
        let panel = e.get_panel();
        let hover = |name: &str| {
            let id = ElementId::from(SharedString::from(format!("rail-{name}")));
            subtake_ui::hover_progress(&subtake_ui::motion::tween_key(&id, "hover"))
        };
        let active = subtake_ui::glide(tools.iter().enumerate().map(|(index, (_, name, _))| {
            let id = ElementId::from(("rail-tool", index));
            subtake_ui::state_fade(
                &subtake_ui::motion::tween_key(&id, "active"),
                panel == *name,
            )
        }));
        let hovered = subtake_ui::glide(tools.iter().map(|(_, name, _)| hover(name)));
        // Over the active tool the accent deepens, as a filled button's does.
        let deepen = tools
            .iter()
            .find(|(_, name, _)| panel == *name)
            .map_or(0., |(_, name, _)| hover(name));
        let size = Theme::control_height();
        let disc = |fill: Hsla| div().size(px(size)).rounded_full().bg(fill);
        let mark = |glide, disc: Div| {
            subtake_ui::glide_mark(glide, size + Theme::gap_small(), size)
                .flex()
                .justify_center()
                .child(disc)
        };
        let mut tool_column = div()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::gap_small()))
            .children(hovered.map(|g| mark(g, disc(theme.sunk2))))
            .children(active.map(|g| {
                // It carries the glow a filled button casts, being that
                // button's fill now.
                let fill = subtake_ui::blend(theme.accent, theme.accent_hover, deepen);
                mark(g, disc(fill).shadow(vec![theme.action_glow(false)]))
            }));
        for (label, name, glyph) in tools {
            tool_column = tool_column.child(self.rail_panel_button(e, label, name, glyph).glided());
        }
        panels = panels.child(tool_column);
        // A hairline before Settings, which is not a tool, as the pod is
        // drawn. The shortcut reference is the Help menu's.
        let panels = panels
            .child(
                div()
                    .w_full()
                    .px(px(Theme::gap_small()))
                    .child(divider(theme)),
            )
            .child(self.rail_panel_button(e, "Settings", "open-settings", "Gear-regular"));
        let rail = pod(theme)
            .p_0()
            .flex_col()
            .w(px(Theme::pod_width()))
            .max_h_full()
            .child(panels);
        // Centred on the stage's own height. gpui at the pinned revision has
        // no transform, so a float is centred by a full-height strip around
        // it rather than by a half-height offset.
        let rail = div()
            .absolute()
            .left(px(Theme::inset()))
            .top_0()
            .bottom_0()
            .py(px(Theme::inset()))
            .flex()
            .items_center()
            .child(frosted(
                UiSurface::Pod.radius(),
                UiSurface::Pod.blur(),
                rail,
            ));
        let (preview, picture) = self.preview(e, window, cx);
        let aspect_pod = self.aspect_pod(e, window, cx);
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
            // First, so the titlebar, the pods and the console all draw over
            // a picture zoomed out past the stage.
            .children(picture)
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
        if e.get_has_video() {
            root = root.child(self.timeline(e, window, cx));
        } else if let Some(status) = self.status_strip(e, window) {
            root = root.child(div().px(px(Theme::gap_large())).child(status));
        }
        root.children(self.presets_dialog(e, window, cx))
            .children(self.settings_dialog(e, window, cx))
            .child(self.menu_overlay(window, cx))
            .into_any_element()
    }

    /// The document's status as a chip at the end of the title pill — a
    /// running transcription, an error, "Gallery mode" — or `None` when
    /// there is nothing to say. It replaced a status row at the foot of the
    /// console and the hairline over it.
    ///
    /// Not drawn by the design: the job's Cancel and its progress. The
    /// review's chip is a label; a running job still has to be stoppable,
    /// so it carries a small X, and the percentage is already in the words.
    /// The full status is its tooltip, since the pill's cap can cut it short.
    fn status_chip(&self, e: &EditorWindow) -> Option<AnyElement> {
        let theme = self.theme;
        let text: SharedString = e.get_status().into();
        if text.is_empty() && !e.get_busy() {
            return None;
        }
        let tip = text.clone();
        let surface = self.surface.clone();
        Some(
            status_chip(theme)
                // It stands off the title by the pill's own side padding,
                // so the title has the same air on both sides.
                .ml(px(Theme::title_pill_chip_gap()))
                .id("document-status")
                .tooltip(move |_, cx| tooltip(tip.clone(), theme, cx))
                .child(div().min_w_0().text_ellipsis().child(text))
                .when(e.get_busy(), |el| {
                    el.child(
                        div()
                            .id("cancel-status")
                            .flex_none()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_, _, _| surface.action("cancel"))
                            .child(subtake_ui::icon_sized(
                                "X-regular",
                                Theme::icon_size_small(),
                                theme.muted,
                            )),
                    )
                })
                .into_any_element(),
        )
    }

    /// The empty state's status line, with nothing open and so no title
    /// pill to carry it, and how far it has grown in. `None` when there is
    /// nothing to say.
    ///
    /// Not drawn by the design: the handoff has no status line on the empty
    /// state. Opening a file and failing to still need somewhere to speak.
    /// It grows in and folds away rather than popping, and on the way out
    /// keeps showing what it last said.
    fn status_strip(&mut self, e: &EditorWindow, window: &mut Window) -> Option<AnyElement> {
        let theme = self.theme;
        let busy = e.get_busy();
        let text = e.get_status();
        let live = busy || !text.is_empty();
        if live {
            self.status_kept = (text.into(), busy, e.get_progress());
        }
        let shown = super::slide_toward(&mut self.status_slide, live, STATUS_SLIDE_MS, window);
        if shown <= 0. {
            return None;
        }
        let (text, busy, progress) = self.status_kept.clone();
        let mut line = row()
            .text_size(px(Theme::font_small()))
            .text_color(theme.muted)
            .child(div().flex_1().text_ellipsis().child(text));
        if busy && live {
            line = line.child(self.action("cancel", "Cancel", "cancel", true));
        }
        let mut status = div()
            .flex()
            .flex_col()
            .gap(px(Theme::gap_small()))
            .child(line);
        if busy {
            status = status.child(progress_bar(progress, theme));
        }
        let body = div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .pb(px(Theme::gap()))
            .child(status)
            .child(measure(self.status_bounds.clone()));
        let full = f32::from(self.status_bounds.get().size.height);
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .overflow_hidden()
                .when(shown < 1., |el| el.h(px(full * shown)).opacity(shown))
                .child(body)
                .into_any_element(),
        )
    }
}
