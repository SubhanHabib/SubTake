//! The Countdown card: the delay before capture starts.

use super::*;

impl RootView {
    pub(super) fn countdown_card(&self, state: &RecordingOptions) -> Vec<AnyElement> {
        let theme = self.theme;
        let mut choices = column().gap(px(Theme::list_gap()));
        for (label, value) in [
            ("No delay", 0),
            ("3 seconds", 3),
            ("5 seconds", 5),
            ("10 seconds", 10),
        ] {
            let chosen = state.get_countdown() == value;
            let options = state.clone();
            let id = ElementId::from(SharedString::from(format!("countdown-{value}")));
            let hover_key = subtake_ui::motion::tween_key(&id, "hover");
            let mut choice = div()
                .id(id)
                .relative()
                .flex()
                .items_center()
                .gap(px(Theme::icon_gap_row()))
                .h(px(Theme::control_height_large()))
                .px(px(Theme::control_padding()))
                .rounded_full()
                .map(|s| {
                    subtake_ui::pressable(
                        s,
                        theme,
                        (!chosen).then_some(theme.press),
                        hover_key.clone(),
                    )
                })
                .on_click(move |_, _, _| {
                    options.defer_option("countdown".into(), value.to_string())
                })
                .child(div().flex_1().child(label));
            if chosen {
                choice = choice
                    .bg(theme.sunk)
                    .font_weight(FontWeight::MEDIUM)
                    .child(icon_sized(
                        "Check-regular",
                        Theme::icon_size_medium(),
                        theme.accent,
                    ))
                    .child(selection_ring(None, theme));
            } else {
                choice = choice.bg(subtake_ui::motion::hover_blend(
                    &hover_key,
                    theme.sunk2.opacity(0.),
                    theme.sunk2,
                ));
            }
            choices = choices.child(choice);
        }
        vec![
            choices.into_any_element(),
            helper("Give yourself a moment before recording starts.", theme).into_any_element(),
        ]
    }
}
