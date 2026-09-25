//! The More card: everything else the recorder offers.

use super::*;

impl RootView {
    pub(super) fn more_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let items = column()
            .gap(px(Theme::list_gap()))
            .child(self.more_item(
                "open",
                "FolderOpen-regular",
                "Open video or project…",
                Some("⌘O"),
                "open",
                true,
            ))
            .child(self.more_item(
                "projects",
                "Stack-regular",
                "Projects",
                None,
                "projects",
                true,
            ))
            .child(self.more_item(
                "editor",
                "ArrowUpRight-regular",
                "Back to editor",
                None,
                "show-editor",
                state.get_has_project(),
            ))
            // Not drawn by the design: the storyboard spike. It is a
            // development entry point, so it stays last and unlabelled by
            // any shortcut rather than being dropped while it is in use.
            .child(self.more_item(
                "storyboard",
                "Sparkle-regular",
                "Create video · spike",
                None,
                "storyboard-spike",
                true,
            ));
        let path = row()
            .h(px(Theme::control_height_large()))
            .gap(px(Theme::icon_gap_row()))
            .pl(px(Theme::control_padding_large()))
            .pr(px(Theme::gap_block()))
            .rounded_full()
            .bg(theme.sunk)
            .child(
                mono(state.get_directory())
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_size(px(Theme::font_secondary()))
                    .text_color(theme.muted),
            )
            .child(
                self.action("folder", "Choose folder…", "recording-folder", true)
                    .raised()
                    .small(),
            );
        vec![
            items.into_any_element(),
            divider(theme).my(px(Theme::list_gap())).into_any_element(),
            caps_label("Recordings path", theme).into_any_element(),
            path.into_any_element(),
            helper(
                "New recordings are written here, then opened in the editor.",
                theme,
            )
            .into_any_element(),
        ]
    }

    /// One row of More: a muted glyph, the caption, and its shortcut.
    fn more_item(
        &self,
        id: &'static str,
        glyph: &str,
        label: &'static str,
        shortcut: Option<&'static str>,
        command: &str,
        enabled: bool,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let hover_key = subtake_ui::motion::tween_key(&id.into(), "hover");
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(Theme::icon_gap_row()))
            .h(px(Theme::control_height_small()))
            .px(px(Theme::control_padding_small()))
            .rounded(px(Theme::radius_lane()))
            .opacity(if enabled {
                1.
            } else {
                Theme::disabled_opacity()
            })
            .child(icon(glyph, theme.muted))
            .child(div().flex_1().child(label))
            .children(shortcut.map(|s| {
                mono(s)
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
            }))
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.sunk2.opacity(0.),
                theme.sunk2,
            ))
            .when(enabled, |s| {
                subtake_ui::pressable(s, theme, Some(theme.press), hover_key)
                    .on_click(self.command(command))
            })
    }
}
