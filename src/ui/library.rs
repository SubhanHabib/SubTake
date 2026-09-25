//! The Projects view: the whole library on the stage, as Recent cards, with
//! its search, folder and any unsaved recoveries. It takes the stage while
//! the panel is "Recent", over the empty state or an open take alike, and
//! its back arrow returns the panel to Scene.
//!
//! Not drawn by the design: the handoff has no library beyond the empty
//! state's three Recent cards. This view extends that row, and the cards are
//! the same cards.

use super::*;

impl RootView {
    pub(super) fn library_stage(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let idle = !e.get_busy();
        let fields: Vec<Field> = e.get_fields().iter().collect();
        let value = |key: &str| {
            fields
                .iter()
                .find(|f| f.key == key)
                .map(|f| f.value.to_string())
                .unwrap_or_default()
        };
        let folder = value("choose-library");
        let folder_name = std::path::Path::new(&folder)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(folder.clone());

        let editor = e.clone();
        let back = icon_button("library-back", "CaretLeft-regular", "Back", theme)
            .ghost()
            .small()
            .on_click(move |_, _, _| {
                editor.set_panel("Frame".into());
                editor.defer_panel("Frame".into());
            });
        let editor = e.clone();
        let search = self.input(
            "library.query",
            &value("library.query"),
            window,
            cx,
            move |v, _, _| editor.defer_field("library.query".into(), v),
        );
        search.update(cx, |s, _| {
            s.set_placeholder("Search projects and recordings")
        });
        let heading = row()
            .w_full()
            .gap(px(Theme::gap()))
            .child(back)
            .child(title("Projects", Theme::font_heading()).flex_1())
            .child(div().w(px(Theme::library_search_width())).child(search))
            .child(
                self.action("library-folder", folder_name, "choose-library", idle)
                    .glyph("FolderOpen-regular")
                    .small(),
            )
            .child(
                icon_button(
                    "library-refresh",
                    "ArrowClockwise-regular",
                    "Refresh",
                    theme,
                )
                .small()
                .on_click(self.command("refresh-library")),
            )
            .child(
                self.action("library-open", "Open…", "open", idle)
                    .glyph("Plus-regular")
                    .small(),
            );

        let mut content = column().w_full().gap(px(Theme::empty_gap()));

        // A recovery waiting is the reason this view opened at launch, so
        // it comes first, one row of its own.
        let recoveries: Vec<_> = fields
            .iter()
            .filter(|f| f.key.starts_with("recovery-"))
            .collect();
        if !recoveries.is_empty() {
            let mut buttons = row().flex_wrap().gap(px(Theme::gap()));
            for field in recoveries {
                buttons = buttons.child(
                    self.action(
                        SharedString::from(format!("library-{}", field.key)),
                        field.value.clone(),
                        &field.key,
                        idle,
                    )
                    .glyph("ArrowCounterClockwise-regular")
                    .small(),
                );
            }
            content = content.child(
                column()
                    .gap(px(Theme::gap_large()))
                    .child(caps_label("Unsaved recovery", theme))
                    .child(buttons),
            );
        }

        let recents = e.get_recents();
        if recents.row_count() == 0 {
            let detail = if value("library.query").is_empty() {
                "Recordings and projects you open, and those in the project folder, show here."
            } else {
                "Nothing in the library matches the search."
            };
            content = content.child(empty_state(theme, "No projects", detail).flex_none());
        } else {
            let mut grid = div()
                .grid()
                .grid_cols(Theme::library_columns() as u16)
                .gap(px(Theme::gap_large()));
            for recent in recents.iter() {
                grid = grid.child(self.recent_card(recent, idle));
            }
            content = content.child(
                column()
                    .gap(px(Theme::gap_large()))
                    .child(caps_label(
                        format!("{} in the library", recents.row_count()),
                        theme,
                    ))
                    .child(grid),
            );
        }

        // The scroll region spans the stage and only its content is held to
        // the library's width, so the region's clip, which would cut the
        // cards' shadows, falls at the stage's edges instead.
        let centred = |child: Div| {
            div()
                .w_full()
                .flex()
                .justify_center()
                .px(px(Theme::inset()))
                .child(child.w_full().max_w(px(Theme::library_width())))
        };
        column()
            .flex_1()
            .min_h_0()
            .gap(px(Theme::gap_large()))
            .child(centred(div().pt(px(Theme::inset_top())).child(heading)))
            .child(
                fade_edges(
                    div()
                        .id("library-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.library_scroll)
                        .pt(px(Theme::gap_large()))
                        .pb(px(Theme::inset()))
                        .child(centred(content)),
                )
                .band(Theme::scroll_fade_band())
                .tracking(&self.library_scroll),
            )
            .into_any_element()
    }
}
