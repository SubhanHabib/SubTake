//! The command table shared with the native menu bar, the in-window menu
//! buttons and the command palette overlay.

use super::*;

/// The command palette card: wide enough for the longest Edit command
/// without wrapping, and capped so a long menu scrolls rather than filling
/// the window.
pub(super) const PALETTE_WIDTH: f32 = 300.0;
/// Nine, so the Add menu, the one the timeline's button opens, shows whole:
/// at eight its last row sat under the list's fade and read as a gap.
pub(super) const PALETTE_VISIBLE_ROWS: usize = 9;
/// Chip, input (with room for its ring), footer, the gaps between the four
/// and the card's own padding — everything in the card that is not a
/// command row.
pub(super) fn palette_chrome_height() -> f32 {
    Theme::chip_height()
        + Theme::control_height()
        + 2.0 * Theme::focus_width()
        + Theme::footer_height()
        + 3.0 * Theme::menu_item_gap()
        + 2.0 * Theme::menu_padding()
}
/// One command row and the hair after it.
pub(super) fn palette_row_pitch() -> f32 {
    Theme::menu_item_height() + Theme::menu_item_gap()
}

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
        // The annotations run in the Add panel's order
        // (`crate::annotations::KINDS`).
        "Add" => &[
            &[("Zoom", "add-zoom")],
            &[
                ("Title", "add-title"),
                ("Lower third", "add-lower-third"),
                ("Label", "add-label"),
                ("Text", "add-text"),
                ("Arrow", "add-figure"),
                ("Highlight", "add-highlight"),
                ("Step", "add-step"),
                ("Blur", "add-blur"),
                ("Spotlight", "add-spotlight"),
                ("Image", "add-image"),
            ],
            &[
                ("Audio", "add-audio"),
                ("Caption", "add-caption"),
                ("Trim", "add-trim"),
                ("Speed", "add-speed"),
                ("Marker", "add-marker"),
            ],
        ],
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
                    s.menu_highlight = 0;
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
        self.menu_highlight = 0;
        cx.notify();
    }

    /// `SUBTAKE_GALLERY_OPEN=menu-Add` opens that menu's palette once, as a
    /// click on its trigger would: the gallery's unfocused windows take no
    /// clicks. It waits for a frame that has measured the trigger, which the
    /// palette opens against.
    fn gallery_open_menu(&mut self, window: &mut Window) {
        static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let Some(name) = std::env::var("SUBTAKE_GALLERY_OPEN")
            .ok()
            .and_then(|open| open.strip_prefix("menu-").map(str::to_owned))
        else {
            return;
        };
        if OPENED.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if self.menu_anchor.get().size.width <= px(0.) {
            window.request_animation_frame();
            return;
        }
        OPENED.store(true, std::sync::atomic::Ordering::Relaxed);
        self.menu = Some(name.into());
        self.menu_focus = true;
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
        self.gallery_open_menu(window);
        if let Some(name) = &self.menu {
            self.menu_last = name.clone();
        }
        let Some(leave) = self.menu_leave.shown(self.menu.is_some()) else {
            return div().into_any_element();
        };
        if leave < 1. {
            window.request_animation_frame();
        }
        let name = self.menu_last.clone();
        let theme = self.theme;
        let filter = self.menu_filter.to_lowercase();
        let matches: Vec<_> = Self::menu_commands(&name)
            .iter()
            .flat_map(|group| group.iter())
            .filter(|(label, _)| filter.is_empty() || label.to_lowercase().contains(&filter))
            .collect();
        let highlight = self.menu_highlight.min(matches.len().saturating_sub(1));
        let chosen = matches
            .get(highlight)
            .map(|(_, command)| command.to_string());
        let count = matches.len();

        let anchor = self.menu_anchor.get();
        let viewport = window.viewport_size();
        let left = f32::from(anchor.origin.x)
            .min(f32::from(viewport.width) - PALETTE_WIDTH - Theme::gap())
            .max(Theme::gap());
        // Card height is content-driven, so cap it and reserve that much when
        // deciding which way to open.
        let rows = matches.len().clamp(1, PALETTE_VISIBLE_ROWS) as f32;
        let height = palette_chrome_height() + rows * palette_row_pitch();
        let below = f32::from(anchor.origin.y + anchor.size.height) + Theme::gap();
        let top = if below + height <= f32::from(viewport.height) - Theme::gap() {
            below
        } else {
            (f32::from(anchor.origin.y) - Theme::gap() - height).max(Theme::gap())
        };

        let search = self.input("command-palette", "", window, cx, |_, _, _| {});
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
                        s.menu_highlight = 0;
                        cx.notify();
                    })
                    .ok();
            });
            // Up and down walk the highlight through the filtered list, and
            // the list scrolls to keep it in sight.
            let stepping = view.clone();
            input.set_on_step(move |delta, _, cx| {
                stepping
                    .update(cx, |s: &mut Self, cx| {
                        let last = count.saturating_sub(1) as isize;
                        let next = (highlight as isize + delta).clamp(0, last) as usize;
                        s.menu_highlight = next;
                        s.menu_scroll.scroll_to_item(next);
                        cx.notify();
                    })
                    .ok();
            });
            // Enter runs the highlighted command, which is the top of the
            // filtered list until the arrow keys move it.
            let running = view.clone();
            let run = chosen.clone();
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
                        s.menu_highlight = 0;
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
            PALETTE_VISIBLE_ROWS as f32 * palette_row_pitch(),
        )
        .track_scroll(&self.menu_scroll);
        if matches.is_empty() {
            list = list.child(
                div()
                    .h(px(Theme::menu_item_height()))
                    .flex()
                    .items_center()
                    .px(px(Theme::menu_item_padding()))
                    .text_color(theme.muted)
                    .child("No matching command"),
            );
        }
        // Rows carry no glyph: a third of these commands have no icon in the
        // set, and inventing one per row reads worse than a clean list.
        //
        // The pointer moves the highlight, so the list shows one lit row —
        // the one Enter runs — rather than the keyboard's and the pointer's.
        for (i, (label, command)) in matches.into_iter().enumerate() {
            let command = command.to_string();
            list = list.child(
                command_row(
                    SharedString::from(format!("palette-{command}")),
                    *label,
                    i == highlight,
                    theme,
                    cx.listener(move |s, _, _, cx| s.run_command(&command, cx)),
                )
                .on_mouse_move(cx.listener(move |s, _: &MouseMoveEvent, _, cx| {
                    if s.menu_highlight != i {
                        s.menu_highlight = i;
                        cx.notify();
                    }
                })),
            );
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
                // The menu showing is an on/off state, not the active tool:
                // a `sunk` plate at the footer's small size, where the
                // accent disc at 40 stood out of a 28 footer onto the list.
                .ghost()
                .small()
                .toggled(name == menu)
                .on_click(cx.listener(move |s, _, _, cx| {
                    s.menu = Some(menu.into());
                    s.menu_filter.clear();
                    s.menu_highlight = 0;
                    s.menu_focus = true;
                    cx.notify();
                })),
            );
        }

        deferred(frosted(
            Theme::radius_menu(),
            MENU_BLUR * leave,
            menu_in(
                ("command-menu-in", self.menu_leave.opens),
                top,
                leave,
                menu_surface(theme)
                    .id("command-menu")
                    .absolute()
                    .left(px(left))
                    .w(px(PALETTE_WIDTH))
                    .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                        s.menu = None;
                        cx.notify();
                    }))
                    // A chip, not a bar: stretched across the card it read
                    // as a second field over the real one.
                    .child(context_chip(theme, &[&name, "Commands"]).self_start())
                    // The field is always focused here, so its ring always
                    // shows; the ring's own width either side keeps it off
                    // the chip and the first row.
                    .child(div().py(px(Theme::focus_width())).child(search))
                    .child(fade_edges(list).tracking(&self.menu_scroll))
                    .child(footer),
            ),
        ))
        .with_priority(30)
        .into_any_element()
    }
}
