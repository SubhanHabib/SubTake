//! The Audio card: the microphone and the system's sound.

use super::*;

impl RootView {
    pub(super) fn audio_card(
        &mut self,
        state: &RecordingOptions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        let busy = state.get_busy();
        let on = state.get_microphone();
        let options = state.clone();
        let microphone = self.dropdown(
            "microphone",
            state.get_microphone_names().iter().collect(),
            state.get_microphone_index(),
            !busy && on,
            cx,
            move |i, _, _| options.defer_option("microphone-device".into(), i.to_string()),
        );
        microphone.update(cx, |d, _| d.glyph = Some("Microphone-regular".into()));
        let mic = state.clone();
        let system = state.clone();
        vec![
            caps_label("Microphone", theme).into_any_element(),
            microphone.into_any_element(),
            toggle(
                "mic-toggle",
                "Record microphone",
                on,
                !busy,
                theme,
                move |v, _, _| mic.defer_option("microphone".into(), v.to_string()),
            )
            .into_any_element(),
            self.level_meter(state.get_mic_level(), on)
                .into_any_element(),
            caps_label("System", theme)
                .mt(px(Theme::list_gap()))
                .into_any_element(),
            toggle(
                "system-toggle",
                "Record system audio",
                state.get_system_audio(),
                !busy,
                theme,
                move |v, _, _| system.defer_option("system-audio".into(), v.to_string()),
            )
            .into_any_element(),
            helper(
                "Microphone and system audio land on separate tracks, so you can mix them later.",
                theme,
            )
            .into_any_element(),
        ]
    }

    /// Twelve bars lit from the left as the level rises, in `ink` — the
    /// meter says "sound is arriving", not "this is selected" — and the
    /// peak beside them. Clipping turns the top two `rec` and holds them
    /// for a second. With the microphone off the meter dims and stops.
    fn level_meter(&mut self, level: f32, on: bool) -> Div {
        let theme = self.theme;
        let floor = Theme::meter_floor_db();
        let bars = Theme::METER_BARS.len();
        let lit = if on && level > floor {
            (((level - floor) / -floor) * bars as f32)
                .round()
                .clamp(0., bars as f32) as usize
        } else {
            0
        };
        let now = Instant::now();
        if on && level >= Theme::meter_clip_db() {
            self.mic_clipped = Some(now);
        }
        let clipped = on
            && self
                .mic_clipped
                .is_some_and(|at| now.duration_since(at) < std::time::Duration::from_secs(1));
        let mut meter = row()
            .h(px(Theme::meter_height()))
            .gap(px(Theme::meter_gap()))
            .opacity(if on { 1. } else { Theme::disabled_opacity() });
        for (i, height) in Theme::METER_BARS.into_iter().enumerate() {
            let fill = if clipped && i >= bars - 2 {
                theme.rec
            } else if i < lit {
                theme.ink
            } else {
                theme.sunk2
            };
            meter = meter.child(
                div()
                    .flex_none()
                    .w(px(Theme::meter_bar_width()))
                    .h(px(height))
                    .rounded(px(Theme::meter_bar_radius()))
                    .bg(fill),
            );
        }
        // Without microphone access, which the card never asks for, the
        // level stays at negative infinity and the peak reads as a dash
        // rather than as a made-up number.
        let peak = if level > floor {
            format!("−{:.0} dB", -level.min(0.))
        } else {
            "— dB".into()
        };
        meter.child(div().flex_1()).child(
            mono(peak)
                .text_size(px(Theme::font_small()))
                .text_color(theme.muted),
        )
    }
}
