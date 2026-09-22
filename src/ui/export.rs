//! The Export panel: an inspector panel, not a dialog, so the picture it is
//! about to export stays in view while it is set up.
//!
//! The app hands the settings over as fields (`App::export_fields`); the
//! panel looks each one up by key and draws it where the handoff puts it.

use super::*;
use subtake_ui::icon_sized;

impl RootView {
    /// The panel's heading, rows and hero button, for the inspector shell
    /// to frame like every other panel.
    pub(super) fn export_panel(
        &mut self,
        e: &EditorWindow,
        cx: &mut Context<Self>,
    ) -> (Div, Div, Div) {
        let theme = self.theme;
        let fields: Vec<Field> = e.get_fields().iter().collect();
        let find = |key: &str| fields.iter().find(|f| f.key == key).cloned();
        let value = |key: &str| find(key).map(|f| f.value.to_string()).unwrap_or_default();
        let format = value("export.format");
        let format = if format.is_empty() {
            "video".into()
        } else {
            format
        };

        // Not drawn by the design: where closing lands. The inspector always
        // shows a panel, so the close control goes back to Scene.
        let editor = e.clone();
        let heading = row()
            .h(px(Theme::CONTROL_HEIGHT_SMALL))
            .flex_none()
            .gap(px(Theme::ICON_GAP_ROW))
            .child(title("Export", Theme::FONT_HEADING).flex_1())
            .child(
                icon_button("export-close", "X-regular", "Close", theme)
                    .small()
                    .on_click(move |_, _, _| {
                        editor.set_panel("Frame".into());
                        editor.defer_panel("Frame".into());
                    }),
            );

        let editor = e.clone();
        let mut content = column().gap(px(Theme::GAP_LARGE)).child(segmented_control(
            "export-format",
            &["Video", "GIF", "Frame"],
            match format.as_str() {
                "gif" => 1,
                "frame" => 2,
                _ => 0,
            },
            theme,
            move |index, _, _| {
                let format = ["video", "gif", "frame"][index.min(2)];
                editor.defer_field("export.format".into(), format.into());
            },
        ));

        content = content.child(caps_label("Output", theme));
        for (key, caption) in [
            ("export.resolution", "Resolution"),
            ("export.fps", "Frame rate"),
        ] {
            // A still has no frame rate.
            if format == "frame" && key == "export.fps" {
                continue;
            }
            let Some(field) = find(key) else { continue };
            let editor = e.clone();
            let values: Vec<_> = field.values.iter().collect();
            let select = self.dropdown(
                &format!("Export:{key}"),
                field.choices.iter().collect(),
                field.choice,
                true,
                cx,
                move |i, _, _| {
                    if let Some(v) = values.get(i) {
                        editor.defer_field(key.into(), v.clone());
                    }
                },
            );
            select.update(cx, |d, _| d.caption = Some(caption.into()));
            content = content.child(select);
        }
        if format == "video"
            && let Some(field) = find("export.quality")
        {
            let editor = e.clone();
            let values: Vec<_> = field.values.iter().collect();
            let steps: Vec<SharedString> = field.choices.iter().map(Into::into).collect();
            let top = (steps.len().max(2) - 1) as f32;
            let quality = self.slider(
                "Export:export.quality",
                0.,
                top,
                field.choice as f32,
                ("Quality", ""),
                (1., ""),
                cx,
                move |v, commit, _, _| {
                    if commit && let Some(value) = values.get(v.round().max(0.) as usize) {
                        editor.defer_field("export.quality".into(), value.clone());
                    }
                },
            );
            quality.update(cx, |s, _| s.steps = steps);
            content = content.child(quality);
        }

        content = content.child(caps_label("Destination", theme)).child(
            row()
                .h(px(Theme::CONTROL_HEIGHT_LARGE))
                .gap(px(Theme::ICON_GAP_ROW))
                .pl(px(Theme::CONTROL_PADDING_LARGE))
                .pr(px(Theme::EXPORT_DESTINATION_INSET))
                .rounded_full()
                .bg(theme.sunk)
                .child(
                    mono(value("export.destination"))
                        .flex_1()
                        .min_w_0()
                        .text_ellipsis()
                        .text_size(px(Theme::FONT_SECONDARY))
                        .text_color(theme.muted),
                )
                .child(
                    button("export-change", "Change…", theme)
                        .raised()
                        .small()
                        .on_click(self.command("export-destination")),
                ),
        );

        // Not drawn by the design: the encoder, loop and subtitle switches.
        // The handoff's panel has no row for them, but each changes the file
        // that comes out, so they stay, under the destination, for the
        // formats they apply to.
        let mut switches = vec![];
        match format.as_str() {
            "video" => {
                switches.push(("export.hardware", "Hardware encoding"));
                switches.push(("nativeCaptionSidecars", "Save subtitle files"));
            }
            "gif" => switches.push(("export.loop", "Loop GIF")),
            _ => {}
        }
        for (key, label) in switches {
            let editor = e.clone();
            content = content.child(toggle(
                SharedString::from(format!("Export:{key}")),
                label,
                value(key) == "true",
                true,
                theme,
                move |v, _, _| editor.defer_field(key.into(), v.to_string()),
            ));
        }

        content = content.child(
            row()
                .justify_between()
                .px(px(Theme::GAP_SMALL))
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted)
                .child("Estimated size")
                .child(mono(value("export.estimate"))),
        );

        let label = match format.as_str() {
            "gif" => "Export GIF",
            "frame" => "Export frame",
            _ => "Export video",
        };
        let footer = column().child(
            button("export-video", label, theme)
                .glyph("Export-fill")
                .primary()
                .hero()
                .enabled(e.get_has_video() && !e.get_busy())
                .on_click(self.command("export")),
        );
        (heading, content, footer)
    }
}

/// How long a finished export's pill stays up before it goes by itself.
const DONE_SECONDS: u64 = 8;

impl RootView {
    /// The titlebar pill a running, finished or failed export shows, so the
    /// panel can close and editing carry on while the file is written.
    pub(super) fn export_pill(
        &mut self,
        e: &EditorWindow,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let theme = self.theme;
        let state = e.get_export_state();
        // A finished export leaves by itself unless the pointer has been
        // over it: once looked at, it waits to be dismissed.
        if state != "done" {
            self.export_dismiss = None;
        } else if self.export_dismiss.is_none() {
            let timer = crate::ui_runtime::Timer::default();
            let editor = e.clone();
            timer.start(
                crate::ui_runtime::TimerMode::SingleShot,
                std::time::Duration::from_secs(DONE_SECONDS),
                move || editor.set_export_state(String::new()),
            );
            self.export_dismiss = Some(timer);
        }
        let name = e.get_export_name();
        let dismiss = {
            let editor = e.clone();
            icon_button("export-dismiss", "X-regular", "Dismiss", theme)
                .ghost()
                .small()
                .on_click(move |_, _, _| editor.set_export_state(String::new()))
        };
        let caption = |text: String| {
            div()
                .text_size(px(Theme::FONT_SECONDARY))
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .text_ellipsis()
                .child(text)
        };
        let pill = row()
            .id("export-pill")
            .w(px(Theme::EXPORT_PILL_WIDTH))
            .min_w_0()
            .flex_shrink_1()
            .h(px(Theme::CONTROL_HEIGHT_LARGE))
            .gap(px(Theme::EXPORT_PILL_GAP))
            .pl(px(Theme::EXPORT_PILL_INSET_LEFT))
            .pr(px(Theme::EXPORT_PILL_INSET_RIGHT))
            .rounded_full()
            .bg(theme.sunk)
            .text_color(theme.text);
        let pill = match state.as_str() {
            "exporting" => pill
                .child(super::recorder::spinner(Theme::EXPORT_SPINNER_SIZE, theme))
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(Theme::EXPORT_TRACK_GAP))
                        .child(
                            row()
                                .items_baseline()
                                .line_height(relative(Theme::MESSAGE_LEADING))
                                .gap(px(Theme::EXPORT_TITLE_GAP))
                                .child(caption(format!("Exporting {name}")).min_w_0())
                                .child(
                                    mono(e.get_export_detail())
                                        .flex_none()
                                        .text_size(px(Theme::FONT_SMALL))
                                        .text_color(theme.muted),
                                ),
                        )
                        .child(
                            div()
                                .h(px(Theme::EXPORT_TRACK_HEIGHT))
                                .rounded(px(Theme::EXPORT_TRACK_RADIUS))
                                .overflow_hidden()
                                .bg(theme.sunk2)
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(e.get_export_progress().clamp(0., 1.)))
                                        .rounded(px(Theme::EXPORT_TRACK_RADIUS))
                                        .bg(theme.accent),
                                ),
                        ),
                )
                .child(
                    button("export-cancel", "Cancel", theme)
                        .raised()
                        .small()
                        .on_click(self.command("cancel-export")),
                ),
            "done" => pill
                .on_hover(cx.listener(|s, hovered: &bool, _, _| {
                    if *hovered && let Some(timer) = &s.export_dismiss {
                        timer.stop();
                    }
                }))
                .child(icon_sized(
                    "Check-regular",
                    Theme::EXPORT_CHECK_SIZE,
                    theme.accent,
                ))
                .child(caption(format!("Exported {name}")).flex_1().min_w_0())
                .child(
                    button("export-reveal", "Reveal in Finder", theme)
                        .raised()
                        .small()
                        .on_click(self.command("reveal-export")),
                )
                .child(dismiss),
            "failed" => pill
                .shadow(vec![hairline(theme.danger, 1.)])
                .child(
                    div()
                        .flex_none()
                        .size(px(Theme::EXPORT_DOT_SIZE))
                        .rounded_full()
                        .bg(theme.danger),
                )
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap_0()
                        .line_height(relative(Theme::MESSAGE_LEADING))
                        .child(caption("Export failed".into()).text_color(theme.danger))
                        .child(
                            div()
                                .text_size(px(Theme::FONT_SMALL))
                                .text_color(theme.muted)
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(e.get_export_detail()),
                        ),
                )
                .child(
                    button("export-retry", "Try again", theme)
                        .raised()
                        .small()
                        .on_click(self.command("export")),
                )
                .child(dismiss),
            _ => return None,
        };
        Some(pill)
    }
}
