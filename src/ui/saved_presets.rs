//! The Presets dialog's Saved tab: the presets saved to disk, and the one
//! picked open with its name, the parts it carries, whether new videos
//! start from it, and what can be done with it.
//!
//! Not drawn by the design: all of it. The handoff shows only the built-in
//! looks, so this follows their rows and the inspector's setting cards.

use super::presets::selection_ring;
use super::*;
use crate::presets::GROUPS;

impl RootView {
    pub(super) fn saved_presets(
        &mut self,
        e: &EditorWindow,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        let names: Vec<String> = e.get_saved_presets().iter().collect();
        let parts: Vec<String> = e.get_saved_preset_parts().iter().collect();
        let default = e.get_default_preset();
        // The one the app last pointed at, or the first.
        let chosen = e.get_selected_preset();
        let selected = names
            .iter()
            .position(|n| *n == chosen)
            .or((!names.is_empty()).then_some(0));
        let mut out = vec![];

        if names.is_empty() {
            out.push(
                setting_card(
                    theme,
                    "No saved presets yet",
                    "Save this project's settings as a preset, or import a preset file.",
                )
                .into_any_element(),
            );
        } else {
            let mut list = column().gap(px(Theme::gap_small()));
            for (index, name) in names.iter().enumerate() {
                let editor = e.clone();
                let pick = name.clone();
                list = list.child(
                    saved_row(
                        index,
                        name,
                        parts.get(index).map(String::as_str).unwrap_or(""),
                        *name == default,
                        Some(index) == selected,
                        theme,
                    )
                    .on_click(move |_, _, _| editor.set_selected_preset(pick.clone())),
                );
            }
            out.push(
                div()
                    .id("saved-presets")
                    .flex_none()
                    .max_h(px(Theme::preset_list_height()))
                    .overflow_y_scroll()
                    .child(list)
                    .into_any_element(),
            );
        }

        if let Some(index) = selected {
            out.push(self.saved_preset(
                e,
                index,
                &names[index],
                &parts[index],
                &default,
                enabled,
                window,
                cx,
            ));
        }

        let surface = self.surface.clone();
        let editor = e.clone();
        let apply = move |_: &ClickEvent, _: &mut Window, _: &mut App| {
            if let Some(index) = selected {
                surface.action(&format!("apply-preset-{index}"));
            }
            editor.set_dialog(String::new());
        };
        out.push(
            row()
                .gap(px(Theme::gap_large()))
                .child(
                    icon_button(
                        "presets-import",
                        "FolderOpen-regular",
                        "Import preset…",
                        theme,
                    )
                    .dialog()
                    .on_click(self.command("import-preset")),
                )
                .child(
                    button("presets-new", "Save current", theme)
                        .glyph("Plus-regular")
                        .dialog()
                        .stretch()
                        .enabled(enabled)
                        .on_click(self.command("new-preset")),
                )
                .child(
                    button("presets-apply", "Apply", theme)
                        .primary()
                        .dialog()
                        .stretch()
                        .enabled(enabled && selected.is_some())
                        .on_click(apply),
                )
                .into_any_element(),
        );
        out
    }

    /// The picked preset laid open: its name, its parts, the default
    /// switch, and its commands.
    #[allow(clippy::too_many_arguments)]
    fn saved_preset(
        &mut self,
        e: &EditorWindow,
        index: usize,
        name: &str,
        parts: &str,
        default: &str,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let editor = e.clone();
        let input = self.input("preset-name", name, window, cx, move |v, _, _| {
            editor.defer_field(format!("preset.{index}.name"), v)
        });

        let mut grid = tile_grid(Theme::preset_part_columns() as u16);
        for (group, title, _) in GROUPS {
            let on = parts.split(',').any(|p| p == group);
            grid = grid.child(
                choice_tile(
                    SharedString::from(format!("preset-part-{group}")),
                    on,
                    true,
                    theme,
                )
                .items_center()
                .text_size(px(Theme::font_secondary()))
                .text_color(if on { theme.text } else { theme.muted })
                .child(title)
                .on_click(self.command(&format!("preset-part-{index}-{group}"))),
            );
        }

        let surface = self.surface.clone();
        let is_default = name == default;
        column()
            .gap(px(Theme::gap_large()))
            .child(group_card(theme, "Name").child(input))
            .child(group_card(theme, "Carries").child(grid))
            .child(
                setting_card(
                    theme,
                    "Start new videos from this preset",
                    "Applied when a recording or a video is opened for the first time.",
                )
                .child(switch(
                    "preset-default",
                    is_default,
                    true,
                    theme,
                    move |_, _, _| surface.action(&format!("default-preset-{index}")),
                )),
            )
            .child(
                row()
                    .gap(px(Theme::gap_small()))
                    .child(
                        button("preset-update", "Update from current", theme)
                            .glyph("ArrowClockwise-regular")
                            .stretch()
                            .enabled(enabled)
                            .on_click(self.command(&format!("update-preset-{index}"))),
                    )
                    .child(
                        icon_button("preset-duplicate", "Stack-regular", "Duplicate", theme)
                            .ghost()
                            .on_click(self.command(&format!("duplicate-preset-{index}"))),
                    )
                    .child(
                        icon_button("preset-share", "Export-regular", "Export…", theme)
                            .ghost()
                            .on_click(self.command(&format!("share-preset-{index}"))),
                    )
                    .child(
                        icon_button("preset-remove", "Trash-regular", "Delete preset", theme)
                            .ghost()
                            .on_click(self.command(&format!("remove-preset-{index}"))),
                    ),
            )
            .into_any_element()
    }
}

/// One saved preset: its name, the parts it carries, and whether new
/// videos start from it.
fn saved_row(
    index: usize,
    name: &str,
    parts: &str,
    is_default: bool,
    selected: bool,
    theme: Theme,
) -> Stateful<Div> {
    let on: Vec<&str> = GROUPS
        .iter()
        .filter(|(group, _, _)| parts.split(',').any(|p| p == *group))
        .map(|(_, title, _)| *title)
        .collect();
    let summary = if on.len() == GROUPS.len() {
        "Everything".to_owned()
    } else {
        on.join(" · ")
    };
    let id = ElementId::from(SharedString::from(format!("saved-preset-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let mut row = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(Theme::gap_block()))
        .p(px(Theme::control_padding_small()))
        .rounded(px(Theme::radius_row()))
        .map(|row| {
            subtake_ui::pressable(
                row,
                theme,
                (!selected).then_some(theme.press),
                hover_key.clone(),
            )
        })
        .child(
            column()
                .flex_1()
                .min_w_0()
                .gap(px(Theme::gap_small()))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .text_ellipsis()
                        .child(name.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(Theme::font_secondary()))
                        .text_color(theme.muted)
                        .text_ellipsis()
                        .child(summary),
                ),
        )
        .when(is_default, |row| {
            row.child(
                div()
                    .flex_none()
                    .px(px(Theme::gap()))
                    .rounded(px(Theme::radius_lane()))
                    .bg(theme.accent_soft)
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.accent)
                    .child("Default"),
            )
        });
    if selected {
        row = row
            .bg(theme.sunk)
            .child(icon("Check-regular", theme.accent))
            .child(selection_ring(Theme::radius_row(), theme));
    } else {
        row = row.bg(subtake_ui::motion::hover_blend(
            &hover_key,
            theme.sunk2.opacity(0.),
            theme.sunk2,
        ));
    }
    row
}
