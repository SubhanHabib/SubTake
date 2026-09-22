//! The command table shared with the native menu bar, the in-window menu
//! buttons and the command palette overlay.

use super::*;

/// The command palette card: wide enough for the longest Edit command
/// without wrapping, and capped so a long menu scrolls rather than filling
/// the window.
pub(super) const PALETTE_WIDTH: f32 = 300.0;
pub(super) const PALETTE_VISIBLE_ROWS: usize = 8;
/// Chip, input, footer and the card's own padding — everything in the card
/// that is not a command row.
pub(super) const PALETTE_CHROME_HEIGHT: f32 =
    Theme::CHIP_HEIGHT + Theme::CONTROL_HEIGHT + Theme::FOOTER_HEIGHT + 4.0 * Theme::GAP_SMALL;

/// The command sets behind both the in-window palette and the native menu
/// bar, grouped so the menu bar can keep its separators. One table, because
/// the two used to carry verbatim copies of these lists that could drift.
///
/// A command starting with `@` opens that inspector panel instead of firing
/// an action; every consumer strips the prefix the same way.
pub fn menu_commands(name: &str) -> &'static [&'static [(&'static str, &'static str)]] {
    match name {
        "File" => &[&[
            ("Open…", "open"),
            ("Save", "save"),
            ("Save As…", "save-as"),
            ("Export…", "@Export"),
        ]],
        "Edit" => &[
            &[("Undo", "undo"), ("Redo", "redo")],
            &[
                ("Add marker", "add-marker"),
                ("Previous marker", "previous-marker"),
                ("Next marker", "next-marker"),
                ("Split clip at playhead", "split-clip"),
                ("Select all regions", "select-all"),
                ("Next overlapping annotation", "next-annotation"),
                ("Previous overlapping annotation", "previous-annotation"),
            ],
            &[
                ("Copy region", "copy"),
                ("Cut region", "cut"),
                ("Paste region", "paste"),
                ("Duplicate region", "duplicate"),
                ("Delete region", "delete"),
            ],
        ],
        "Add" => &[&[
            ("Zoom", "add-zoom"),
            ("Text", "add-text"),
            ("Arrow", "add-figure"),
            ("Blur", "add-blur"),
            ("Audio", "add-audio"),
            ("Caption", "add-caption"),
            ("Trim", "add-trim"),
            ("Speed", "add-speed"),
            ("Marker", "add-marker"),
        ]],
        _ => &[&[
            ("Keyboard shortcuts", "shortcut-reference"),
            ("Feedback and issues", "feedback"),
        ]],
    }
}

impl RootView {
    pub(super) fn menu_button(
        &self,
        name: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let control = button(name, name, self.theme)
            .raised()
            .selected(self.menu.as_deref() == Some(name))
            .glyph("Plus-regular")
            .on_click(cx.listener(move |s, _, _, cx| {
                s.menu = if s.menu.as_deref() == Some(name) {
                    None
                } else {
                    s.menu_filter.clear();
                    s.menu_focus = true;
                    Some(name.into())
                };
                cx.notify();
            }));
        // The palette opens against these bounds. Measuring costs a wrapper,
        // but the alternative is the fixed coordinate this replaced, which
        // put the card at the top of the window while its trigger sat in the
        // timeline strip at the bottom.
        div()
            .relative()
            .flex_none()
            .child(measure(self.menu_anchor.clone()))
            .child(control)
    }

    pub(super) fn menu_commands(name: &str) -> &'static [&'static [(&'static str, &'static str)]] {
        menu_commands(name)
    }

    /// Run `command` and close the palette.
    pub(super) fn run_command(&mut self, command: &str, cx: &mut Context<Self>) {
        if let Some(panel) = command.strip_prefix('@') {
            if let Surface::Editor(e) = &self.surface {
                e.set_panel(panel.into());
                e.defer_panel(panel.into());
            }
        } else {
            self.surface.action(command);
        }
        self.menu = None;
        self.menu_filter.clear();
        cx.notify();
    }

    /// The command palette: a context chip, a filter input, the command
    /// list, and a quiet row of the other menus beneath it.
    ///
    /// It anchors to the trigger's measured bounds and flips above it when
    /// the trigger sits in the lower half of the window — the "Add" button
    /// lives in the timeline strip, so a card that always dropped downward
    /// would open off the bottom edge.
    pub(super) fn menu_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(name) = self.menu.clone() else {
            return div().into_any_element();
        };
        let theme = self.theme;
        let filter = self.menu_filter.to_lowercase();
        let matches: Vec<_> = Self::menu_commands(&name)
            .iter()
            .flat_map(|group| group.iter())
            .filter(|(label, _)| filter.is_empty() || label.to_lowercase().contains(&filter))
            .collect();
        let first = matches.first().map(|(_, command)| command.to_string());

        let anchor = self.menu_anchor.get();
        let viewport = window.viewport_size();
        let left = f32::from(anchor.origin.x)
            .min(f32::from(viewport.width) - PALETTE_WIDTH - Theme::GAP)
            .max(Theme::GAP);
        // Card height is content-driven, so cap it and reserve that much when
        // deciding which way to open.
        let rows = matches.len().clamp(1, PALETTE_VISIBLE_ROWS) as f32;
        let height = PALETTE_CHROME_HEIGHT + rows * Theme::CONTROL_HEIGHT;
        let below = f32::from(anchor.origin.y + anchor.size.height) + Theme::GAP;
        let top = if below + height <= f32::from(viewport.height) - Theme::GAP {
            below
        } else {
            (f32::from(anchor.origin.y) - Theme::GAP - height).max(Theme::GAP)
        };

        let search = self.input("command-palette", "", window, cx, {
            let first = first.clone();
            move |_, _, _| {
                let _ = &first;
            }
        });
        // `cx.listener` hands the callback its event by reference; the input's
        // callbacks take the text by value, so they go through a weak handle.
        let view = cx.entity().downgrade();
        search.update(cx, |input, _| {
            input.set_placeholder("Type to filter commands");
            let filtering = view.clone();
            input.set_on_change(move |value, _, cx| {
                filtering
                    .update(cx, |s: &mut Self, cx| {
                        s.menu_filter = value.clone();
                        cx.notify();
                    })
                    .ok();
            });
            // Enter runs whatever is at the top of the filtered list, which
            // is the only reason the field commits at all.
            let running = view.clone();
            let run = first.clone();
            input.set_handler(move |_, _, cx| {
                let Some(command) = run.clone() else { return };
                running
                    .update(cx, |s: &mut Self, cx| s.run_command(&command, cx))
                    .ok();
            });
            // The field swallows escape, so it has to close the card itself.
            let dismissing = view.clone();
            input.set_on_cancel(move |_, cx| {
                dismissing
                    .update(cx, |s: &mut Self, cx| {
                        s.menu = None;
                        s.menu_filter.clear();
                        cx.notify();
                    })
                    .ok();
            });
        });
        if std::mem::take(&mut self.menu_focus) {
            search.update(cx, |input, cx| {
                input.reset("");
                input.focus(window, cx);
            });
        }

        let mut list = menu_list(
            "palette-list",
            PALETTE_VISIBLE_ROWS as f32 * Theme::CONTROL_HEIGHT,
        )
        .py(px(FADE_BAND));
        if matches.is_empty() {
            list = list.child(
                div()
                    .h(px(Theme::CONTROL_HEIGHT))
                    .flex()
                    .items_center()
                    .px(px(Theme::CONTROL_PADDING))
                    .text_color(theme.muted)
                    .child("No matching command"),
            );
        }
        // Rows carry no glyph: a third of these commands have no icon in the
        // set, and inventing one per row reads worse than a clean list.
        for (label, command) in matches {
            let command = command.to_string();
            list = list.child(menu_row(
                SharedString::from(format!("palette-{command}")),
                *label,
                false,
                false,
                theme,
                cx.listener(move |s, _, _, cx| s.run_command(&command, cx)),
            ));
        }

        // The footer doubles as the menu switcher, which is what finally
        // gives File, Edit and Help an entry point in the window itself.
        let mut footer = composer_footer(theme);
        for (menu, glyph) in [
            ("File", "FolderOpen-regular"),
            ("Edit", "SlidersHorizontal-regular"),
            ("Add", "Plus-regular"),
            ("Help", "Question-regular"),
        ] {
            footer = footer.child(
                icon_button(
                    SharedString::from(format!("palette-menu-{menu}")),
                    glyph,
                    menu,
                    theme,
                )
                .ghost()
                .selected(name == menu)
                .on_click(cx.listener(move |s, _, _, cx| {
                    s.menu = Some(menu.into());
                    s.menu_filter.clear();
                    s.menu_focus = true;
                    cx.notify();
                })),
            );
        }
        footer = footer.child(
            div()
                .flex_1()
                .text_ellipsis()
                .min_w_0()
                .child(SharedString::from(name.clone())),
        );

        deferred(frosted(
            Theme::RADIUS_MENU,
            MENU_BLUR,
            menu_in(
                "command-menu-in",
                top,
                menu_surface(theme)
                    .id("command-menu")
                    .absolute()
                    .left(px(left))
                    .w(px(PALETTE_WIDTH))
                    .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                        s.menu = None;
                        cx.notify();
                    }))
                    .child(context_chip(theme, &[&name, "Commands"]))
                    .child(search)
                    .child(fade_edges(list))
                    .child(footer),
            ),
        ))
        .with_priority(30)
        .into_any_element()
    }
}
