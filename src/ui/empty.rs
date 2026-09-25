//! The empty state: screen 2 of the "Stage" handoff, shown while no video is
//! open.

use super::*;

impl RootView {
    pub(super) fn empty_stage(&mut self, e: &EditorWindow) -> AnyElement {
        let theme = self.theme;
        let idle = !e.get_busy();
        let mut column = empty_state(
            theme,
            "Nothing open yet",
            "Record your screen, or drop a video anywhere in this window.",
        )
        .child(
            row()
                .gap(px(Theme::gap_large()))
                .child(
                    self.action(
                        "new-recording",
                        "New recording",
                        "record",
                        idle && !e.get_recording(),
                    )
                    .glyph("Record-regular")
                    .primary()
                    .hero(),
                )
                .child(
                    self.action("open-video", "Open video", "open", idle)
                        .glyph("FolderOpen-regular")
                        .hero(),
                ),
        );

        let recents = e.get_recents();
        if recents.row_count() > 0 {
            let mut cards = row().w_full().gap(px(Theme::gap_large())).items_start();
            for recent in recents.iter().take(3) {
                cards = cards.child(self.recent_card(recent, idle));
            }
            // An empty slot keeps one or two recents at a third of the row
            // each, the size the handoff draws them, rather than stretching
            // a single card across all 760.
            for _ in recents.row_count().min(3)..3 {
                cards = cards.child(div().flex_1());
            }
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .max_w(px(Theme::recent_width()))
                    .gap(px(Theme::gap_large()))
                    .child(
                        row()
                            .w_full()
                            .justify_between()
                            .child(caps_label("Recent", theme))
                            .child(self.all_projects(e)),
                    )
                    .child(cards),
            );
        }
        column.into_any_element()
    }

    /// The way from the Recent row to the Projects view: the whole library,
    /// on the stage.
    fn all_projects(&self, e: &EditorWindow) -> Button {
        let editor = e.clone();
        button("all-projects", "All projects", self.theme)
            .ghost()
            .small()
            .on_click(move |_, _, _| {
                editor.set_panel("Recent".into());
                editor.defer_panel("Recent".into());
            })
    }

    /// One recent project: its thumbnail, title, and length and age.
    pub(super) fn recent_card(&self, recent: Recent, enabled: bool) -> AnyElement {
        let theme = self.theme;
        // Not drawn by the design: a project without a thumbnail. The card
        // keeps its shape with the film glyph on `sunk`, so a row of mixed
        // cards stays one row.
        let thumbnail = match recent.thumbnail.0 {
            Some(image) => img(image)
                .w_full()
                .h(px(Theme::recent_thumb_height()))
                .rounded(px(Theme::radius_inner()))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h(px(Theme::recent_thumb_height()))
                .rounded(px(Theme::radius_inner()))
                .bg(theme.sunk)
                .child(icon("FilmStrip-regular", theme.muted))
                .into_any_element(),
        };
        let mut shadows = theme.panel_shadow();
        shadows.push(hairline(theme.line, Theme::border_width()));
        let id = ElementId::from(SharedString::from(format!("recent-{}", recent.key)));
        let hover_key = subtake_ui::motion::tween_key(&id, "hover");
        // Not drawn by the design: the card's hover, a `hover` wash laid
        // over the glass rather than mixed into it, so the glass keeps its
        // own translucency.
        let wash = div()
            .absolute()
            .inset_0()
            .rounded(px(Theme::radius_row()))
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.sunk2.opacity(0.),
                theme.sunk2,
            ));
        let mut card = div()
            .id(id)
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(Theme::recent_card_padding()))
            .p(px(Theme::recent_card_padding()))
            .rounded(px(Theme::radius_row()))
            .bg(theme.glass)
            .shadow(shadows.clone())
            .child(wash)
            .child(thumbnail)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Theme::gap_small()))
                    .px(px(Theme::gap_small()))
                    .pb(px(Theme::gap_small()))
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(recent.title),
                    )
                    .child(
                        mono(recent.meta)
                            .text_size(px(Theme::font_small()))
                            .text_color(theme.muted)
                            .text_ellipsis(),
                    ),
            );
        if enabled {
            // `pressable`'s ring would replace the card's lift rather than
            // join it, since a shadow list is set whole, so the card states
            // its own.
            let mut focused = shadows;
            focused.push(subtake_ui::focus_ring(theme));
            card = card
                .cursor_pointer()
                .tab_index(0)
                .focus_visible(move |s| s.shadow(focused))
                .active(|s| s.opacity(Theme::pressed_opacity()))
                .on_hover(subtake_ui::motion::hover_listener(hover_key))
                .on_click(self.command(&recent.key));
        }
        card.into_any_element()
    }
}
