//! The Countdown card: the delay before capture starts, as four tiles, and
//! how the count shows while it runs.

use super::parts::{CardRow, group, key_caps};
use super::*;

impl RootView {
    pub(super) fn countdown_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let tiles = div()
            .grid()
            .grid_cols(4)
            .gap(px(Theme::countdown_tile_gap()))
            .children(
                [0, 3, 5, 10]
                    .into_iter()
                    .map(|value| countdown_tile(value, state, theme)),
            );

        let setting = |key: &'static str| {
            let options = state.clone();
            switch(
                SharedString::from(format!("{key}-toggle")),
                state.get_recorder_flag(key),
                true,
                theme,
                move |on, _, _| options.defer_option(key.into(), on.to_string()),
            )
        };
        let settings = group(
            vec![
                // The drawing's copy: the handoff's longer "Large numbers on
                // the captured display" does not fit the row.
                CardRow::new("count-on-screen", "Count on screen")
                    .subtitle("Big numbers on the display")
                    .trailing(setting("count-on-screen")),
                // Not wired: the setting is kept, but nothing plays a tick
                // yet.
                CardRow::new("tick-sound", "Tick sound")
                    .subtitle("A soft click each second")
                    .trailing(setting("tick-sound")),
            ],
            theme,
        );

        let hint = row()
            .flex_none()
            .gap(px(Theme::key_hint_gap()))
            .px(px(Theme::card_text_inset()))
            .text_size(px(Theme::font_secondary()))
            .text_color(theme.muted)
            .child("Press")
            .child(key_caps(&["Esc"], theme))
            .child("during the countdown to cancel.");

        vec![
            tiles.into_any_element(),
            settings.into_any_element(),
            hint.into_any_element(),
        ]
    }
}

/// One delay: the numeral and "sec" under it, or just Off. The current one
/// is raised on `seg_active` inside the accent ring; applying another keeps
/// the card open.
fn countdown_tile(value: i32, state: &RecordingOptions, theme: Theme) -> impl IntoElement {
    let chosen = state.get_countdown() == value;
    let options = state.clone();
    let id = ElementId::from(SharedString::from(format!("countdown-{value}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let radius = Theme::countdown_tile_radius();
    let ink = if chosen { theme.text } else { theme.muted };
    let face = if value == 0 {
        div().child(title("Off", Theme::font_card_title()).text_color(ink))
    } else {
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(
                title(value.to_string(), Theme::font_countdown_tile())
                    .line_height(relative(Theme::countdown_tile_leading()))
                    .text_color(ink),
            )
            .child(
                div()
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
                    .child("sec"),
            )
    };
    let tile = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .h(px(Theme::countdown_tile_height()))
        .rounded(px(radius))
        .map(|s| {
            subtake_ui::pressable(s, theme, (!chosen).then_some(theme.press), hover_key.clone())
        })
        .on_click(move |_, _, _| options.defer_option("countdown".into(), value.to_string()))
        .child(face);
    if chosen {
        tile.bg(theme.seg_active)
            .shadow(vec![theme.segment_shadow()])
            .child(selection_ring(Some(radius), theme))
    } else {
        tile.bg(subtake_ui::motion::hover_blend(&hover_key, theme.sunk, theme.sunk2))
            .child(subtake_ui::edge(
                radius,
                vec![hairline(theme.line, Theme::hairline_width())],
            ))
    }
}
