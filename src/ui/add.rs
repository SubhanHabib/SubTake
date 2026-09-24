//! The Add panel: a tile for each kind of annotation. A tile adds one at the
//! playhead, and Selection opens on it with its settings.
//!
//! Not drawn by the design: the whole panel. The handoff has no way to add
//! anything to the picture but the timeline's Add menu, so this follows the
//! Cursor panel's tiles.

use super::cursor::panel_heading;
use super::*;
use subtake_ui::icon_sized;

/// The kinds by what they are for, each an add action in
/// `crate::annotations::KINDS`.
const GROUPS: [(&str, &[&str]); 4] = [
    ("Text", &["add-title", "add-lower-third", "add-label"]),
    ("Point out", &["add-figure", "add-highlight", "add-step"]),
    (
        "Hide or focus",
        &["add-blur", "add-pixelate", "add-spotlight"],
    ),
    ("More", &["add-text", "add-image"]),
];

impl RootView {
    pub(super) fn add_panel(&mut self, e: &EditorWindow) -> (Div, Div) {
        let theme = self.theme;
        let heading = panel_heading("Add", e, theme);
        let mut content = column().gap(px(Theme::gap_large()));
        for (group, actions) in GROUPS {
            let mut tiles = tile_grid(Theme::add_tile_columns() as u16);
            for &action in actions {
                let Some(&(_, label, glyph)) = crate::annotations::KINDS
                    .iter()
                    .find(|(kind, _, _)| *kind == action)
                else {
                    continue;
                };
                let editor = e.clone();
                tiles = tiles.child(
                    choice_tile(action, false, true, theme)
                        .items_center()
                        .justify_center()
                        .h(px(Theme::add_tile_height()))
                        .child(icon_sized(glyph, Theme::icon_size(), theme.text))
                        .child(
                            div()
                                .text_size(px(Theme::font_secondary()))
                                .text_color(theme.muted)
                                .truncate()
                                .child(label),
                        )
                        .on_click(move |_, _, _| editor.defer_action(action.into())),
                );
            }
            content = content.child(caps_label(group, theme)).child(tiles);
        }
        (heading, content)
    }
}
