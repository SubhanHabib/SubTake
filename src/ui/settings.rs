//! The Settings dialog: the app's own preferences, apart from any project,
//! in sections down a sidebar. ⌘, the app menu and the rail's gear open it.
//!
//! Not drawn by the design: all of it. The handoff has Settings as an
//! inspector panel of two rows, so this sets every preference on the
//! Presets dialog's plate, in the inspector's cards.

use super::presets::dialog_frame;
use super::*;
use subtake_ui::icon_sized;

/// How much of the open section's fill the hover pill takes.
const HOVER_WASH: f32 = 0.5;

/// The sections, each with its sidebar glyph.
const SECTIONS: [(&str, &str); 5] = [
    ("General", "Gear-regular"),
    ("Recording", "Record-regular"),
    ("Shortcuts", "Keyboard-regular"),
    ("Library", "FolderOpen-regular"),
    ("Captions", "ClosedCaptioning-regular"),
];

impl RootView {
    pub(super) fn settings_dialog(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = e.get_dialog() == "settings";
        let ms = if open { DIALOG_IN_MS } else { DIALOG_OUT_MS };
        let shown = super::slide_toward(&mut self.settings_slide, open, ms, window);
        if !open && shown == 0. {
            return None;
        }
        let theme = self.theme;
        let section = e.get_settings_section();
        let section = SECTIONS
            .iter()
            .find(|(name, _)| *name == section)
            .map_or("General", |(name, _)| *name);

        // Two marks under the rows, each gliding up and down rather than
        // switching on in place: the open section's, and a fainter one for
        // the row under the pointer.
        let opened = subtake_ui::glide(SECTIONS.iter().enumerate().map(|(index, (name, _))| {
            let id = ElementId::from(("settings-section", index));
            subtake_ui::state_fade(
                &subtake_ui::motion::tween_key(&id, "open"),
                *name == section,
            )
        }));
        let hovered = subtake_ui::glide(
            SECTIONS
                .iter()
                .map(|(name, _)| subtake_ui::hover_progress(&section_hover_key(name))),
        );
        let pitch = Theme::control_height() + Theme::gap_small();
        let mark = |glide, fill| {
            subtake_ui::glide_mark(glide, pitch, Theme::control_height())
                .rounded(px(Theme::radius_row()))
                .bg(fill)
        };
        let mut rows = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(Theme::gap_small()))
            .children(hovered.map(|g| mark(g, theme.sunk2.opacity(HOVER_WASH))))
            .children(opened.map(|g| mark(g, theme.sunk2)));
        for (name, glyph) in SECTIONS {
            let editor = e.clone();
            rows = rows.child(
                section_row(name, glyph, name == section, theme)
                    .on_click(move |_, _, _| editor.set_settings_section(name.into())),
            );
        }
        let sidebar = column()
            .flex_none()
            .w(px(Theme::settings_sidebar_width()))
            .p(px(Theme::gap_large()))
            // The plate's overflow clip is square, so the sidebar rounds its
            // own outer corners to the plate's, or its fill pokes past them.
            .rounded_l(px(UiSurface::Content.radius()))
            .bg(theme.sunk)
            .child(rows);

        let editor = e.clone();
        let heading = row()
            .child(title(section, Theme::font_heading()).flex_1())
            .child(
                icon_button("settings-close", "X-regular", "Close", theme)
                    .small()
                    .on_click(move |_, _, _| editor.set_dialog(String::new())),
            );
        let body = self.settings_section(e, section, window, cx);
        let content = div()
            .id("settings-content")
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .p(px(Theme::dialog_padding()))
            .child(
                column()
                    .gap(px(Theme::gap_block()))
                    .child(heading)
                    .child(body),
            );

        let plate = panel_variant(theme, UiSurface::Content)
            .id("settings-dialog")
            .flex()
            .flex_row()
            .overflow_hidden()
            .w(px(Theme::settings_width()))
            .h(px(Theme::settings_height()))
            .child(sidebar)
            .child(content);
        Some(dialog_frame("settings-scrim", e, open, shown, plate))
    }

    fn settings_section(
        &mut self,
        e: &EditorWindow,
        section: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let fields: Vec<Field> = e.get_settings_fields().iter().collect();
        let field = |key: &str| fields.iter().find(|f| f.key == key).cloned();
        let mut body = column().gap(px(Theme::gap_large()));
        match section {
            "General" => {
                let editor = e.clone();
                let appearances = ["light", "dark", "system"];
                let chosen = appearances
                    .iter()
                    .position(|v| *v == e.get_appearance())
                    .unwrap_or(2);
                body = body.child(group_card(theme, "Appearance").child(segmented_control(
                    "settings-appearance",
                    &["Light", "Dark", "System"],
                    chosen,
                    theme,
                    move |index, _, _| {
                        editor.defer_field("prefs.appearance".into(), appearances[index].into())
                    },
                )));
                if let Some(f) = field("prefs.language") {
                    body = body.child(self.field(e, f, window, cx));
                }
            }
            "Recording" => {
                if let Some(f) = field("prefs.countdown_seconds") {
                    body = body.child(self.field(e, f, window, cx));
                }
                let editor = e.clone();
                body = body.child(
                    setting_card(
                        theme,
                        "Automatic recording zooms",
                        "Suggest zooms from the cursor when a new recording opens.",
                    )
                    .child(switch(
                        "settings-auto-zooms",
                        e.get_auto_apply_zooms(),
                        true,
                        theme,
                        move |v, _, _| {
                            editor.defer_field("prefs.auto_apply_zooms".into(), v.to_string())
                        },
                    )),
                );
                if let Some(f) = field("recording-folder") {
                    body = body.child(self.path_card(f, "Where new recordings are saved."));
                }
            }
            "Shortcuts" => {
                body = body.child(caps_label("Anywhere", theme)).child(
                    div()
                        .text_size(px(Theme::font_secondary()))
                        .text_color(theme.muted)
                        .child("These work in any app while SubTake is running."),
                );
                for key in ["prefs.record_shortcut", "prefs.pause_shortcut"] {
                    if let Some(f) = field(key) {
                        body = body.child(self.field(e, f, window, cx));
                    }
                }
                body = body.child(caps_label("Editor", theme));
                for f in fields.iter().filter(|f| f.key.starts_with("shortcut.")) {
                    body = body.child(self.field(e, f.clone(), window, cx));
                }
            }
            "Library" => {
                if let Some(f) = field("choose-library") {
                    body = body.child(self.path_card(
                        f,
                        "Projects and recordings in this folder are listed when nothing is open.",
                    ));
                }
            }
            "Captions" => {
                if let Some(f) = field("choose-model") {
                    let missing = f.value.is_empty();
                    body = body.child(self.path_card(
                        f,
                        "The Whisper model that transcribes captions, on this Mac.",
                    ));
                    if missing {
                        body = body.child(
                            button("settings-download-model", "Download Whisper Small", theme)
                                .glyph("ArrowClockwise-regular")
                                .enabled(!e.get_busy())
                                .on_click(self.command("download-model")),
                        );
                    }
                }
            }
            _ => {}
        }
        body
    }

    /// A folder or file the app keeps: what it is for, where it is, and a
    /// button that picks another through the field's own action.
    fn path_card(&self, f: Field, detail: &str) -> Div {
        let theme = self.theme;
        // A path reads in mono; nothing chosen yet reads as plain words.
        let path = if f.value.is_empty() {
            div().text_color(theme.muted).child("Not chosen")
        } else {
            mono(f.value.clone()).text_color(theme.text)
        };
        group_card(theme, f.label.clone())
            .child(
                div()
                    .text_size(px(Theme::font_secondary()))
                    .text_color(theme.muted)
                    .child(detail.to_owned()),
            )
            .child(
                row()
                    .gap(px(Theme::gap()))
                    .child(
                        path.flex_1()
                            .min_w_0()
                            .text_size(px(Theme::font_secondary()))
                            .text_ellipsis(),
                    )
                    .child(
                        button(
                            SharedString::from(format!("settings-{}", f.key)),
                            "Choose…",
                            theme,
                        )
                        .small()
                        .on_click(self.command(&f.key)),
                    ),
            )
    }
}

/// A section in the sidebar: its glyph and name, inked up when it is open
/// or hovered. Its fill is one of the pills sliding under the rows.
fn section_row(name: &'static str, glyph: &str, selected: bool, theme: Theme) -> Stateful<Div> {
    let hover_key = section_hover_key(name);
    let ink = if selected {
        theme.text
    } else {
        subtake_ui::hover_blend(&hover_key, theme.muted, theme.text)
    };
    div()
        .id(ElementId::from(SharedString::from(format!(
            "settings-{name}"
        ))))
        .flex()
        .items_center()
        .gap(px(Theme::gap()))
        .h(px(Theme::control_height()))
        .px(px(Theme::control_padding_small()))
        .cursor_pointer()
        .text_color(ink)
        .when(selected, |row| row.font_weight(FontWeight::MEDIUM))
        .on_hover(subtake_ui::hover_listener(hover_key))
        .child(icon_sized(glyph, Theme::icon_size(), ink))
        .child(name)
}

/// The hover tween a section's row drives and the hover pill follows.
fn section_hover_key(name: &str) -> String {
    let id = ElementId::from(SharedString::from(format!("settings-{name}")));
    subtake_ui::motion::tween_key(&id, "hover")
}
