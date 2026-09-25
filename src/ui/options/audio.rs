//! The Microphone card: the meter, the input to record from, and the
//! system's sound.

use super::parts::{CardRow, access_off, group, none_found, section_label};
use super::*;
use std::time::Duration;
use subtake_theme::{METER_CLIP_HOLD_MS, METER_SAMPLE_MS};
use subtake_ui::edge;

impl RootView {
    pub(super) fn audio_card(
        &mut self,
        state: &RecordingOptions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        if !state.get_microphone_access() {
            return vec![
                access_off("Microphone", self.command("access-microphone"), theme)
                    .into_any_element(),
            ];
        }
        let busy = state.get_busy();
        // Off, the meter and the inputs stay where they are, dimmed and
        // still, so turning the microphone back on changes nothing else.
        let on = state.get_microphone() && microphone_usable(state);
        let dim = if on { 1. } else { Theme::disabled_opacity() };

        let meter = self.meter_block(state, on, cx).opacity(dim);

        let kinds: Vec<String> = state.get_microphone_kinds().iter().collect();
        let chosen = usize::try_from(state.get_microphone_index()).unwrap_or(0);
        let inputs = state
            .get_microphone_names()
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let kind = kinds.get(index).cloned().unwrap_or_default();
                let selected = index == chosen;
                let mut input = CardRow::new(
                    ElementId::from(SharedString::from(format!("microphone-{index}"))),
                    name,
                )
                .height(Theme::card_row_height())
                .selected(selected);
                if !kind.is_empty() {
                    input = input.subtitle(kind);
                }
                if selected {
                    input = input.trailing(icon_sized(
                        "Check-regular",
                        Theme::icon_size(),
                        theme.accent,
                    ));
                }
                if on && !busy {
                    let options = state.clone();
                    input = input.on_click(move |_, _, _| {
                        options.defer_option("microphone-device".into(), index.to_string())
                    });
                }
                input
            })
            .collect();

        let system = state.clone();
        let system_audio = group(
            vec![
                CardRow::new("system-audio", "System audio")
                    .plate("SpeakerHigh-regular")
                    .subtitle("Apps, alerts and calls")
                    .trailing(switch(
                        "system-toggle",
                        state.get_system_audio(),
                        !busy,
                        theme,
                        move |v, _, _| system.defer_option("system-audio".into(), v.to_string()),
                    )),
            ],
            theme,
        );

        vec![
            meter.into_any_element(),
            column()
                .gap(px(Theme::gap()))
                // "No microphones found" is the message, so it keeps its ink.
                .opacity(if microphone_usable(state) { dim } else { 1. })
                .child(section_label("Input", theme))
                .child(if microphone_usable(state) {
                    group(inputs, theme)
                } else {
                    none_found("microphones", theme)
                })
                .into_any_element(),
            system_audio.into_any_element(),
            helper("Mic and system audio record to separate tracks.", theme).into_any_element(),
        ]
    }

    /// The meter on its recess, with the input level and Test under it.
    fn meter_block(&mut self, state: &RecordingOptions, on: bool, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let radius = Theme::card_group_radius();
        let options = state.clone();
        let level = self.slider(
            "input-level",
            0.,
            100.,
            state
                .get_recorder_setting("input-level")
                .parse()
                .unwrap_or(100.),
            ("Input level", ""),
            (1., "%"),
            cx,
            move |v, commit, _, _| {
                if commit {
                    options.defer_option("input-level".into(), v.round().to_string())
                }
            },
        );
        level.update(cx, |s, _| {
            s.height = Theme::control_height();
            s.recessed = true;
            s.value_width = Theme::mic_level_value_width();
            s.padding = Theme::control_padding();
            s.enabled = on;
        });
        column()
            .relative()
            .flex_none()
            .gap(px(Theme::meter_block_gap()))
            .pt(px(Theme::meter_block_padding()))
            .px(px(Theme::meter_block_padding()))
            .pb(px(Theme::meter_block_padding_bottom()))
            .rounded(px(radius))
            .bg(theme.sunk)
            .child(self.level_meter(state.get_mic_level(), on))
            .child(
                row()
                    .gap(px(Theme::meter_controls_gap()))
                    .child(div().flex_1().min_w_0().child(level))
                    .child(test_button(
                        state.get_mic_test(),
                        on && !state.get_busy(),
                        self.command("mic-test"),
                        theme,
                    )),
            )
            .child(edge(
                radius,
                vec![hairline(theme.line, Theme::hairline_width())],
            ))
    }

    /// Thirty bars, each as tall as the level was when it came in, the
    /// newest at the left, and lit from the left as far as the level now
    /// reaches; the level in dB beside them. Past `METER_CLIP_DB` the lit
    /// bars turn `rec` and hold it a moment. With the microphone off the
    /// meter stops.
    fn level_meter(&mut self, level: f32, on: bool) -> Div {
        let theme = self.theme;
        let floor = Theme::meter_floor_db();
        let bars = Theme::METER_BAR_COUNT;
        let reach = |db: f32| ((db - floor) / -floor).clamp(0., 1.);
        let now = Instant::now();
        let level = if on { level } else { f32::NEG_INFINITY };
        if self
            .mic_sampled
            .is_none_or(|at| now.duration_since(at) >= Duration::from_millis(METER_SAMPLE_MS))
        {
            self.mic_sampled = Some(now);
            self.mic_history.push_front(level);
            self.mic_history.truncate(bars);
        }
        let lit = (reach(level) * bars as f32).round() as usize;
        if on && level > Theme::meter_clip_db() {
            self.mic_clipped = Some(now);
        }
        let clipped = on
            && self.mic_clipped.is_some_and(|at| {
                now.duration_since(at) < Duration::from_millis(METER_CLIP_HOLD_MS)
            });
        let ink = if clipped { theme.rec } else { theme.text };
        let room = Theme::meter_height() - Theme::meter_bar_min();
        let meter = row()
            .flex_1()
            .min_w_0()
            .h(px(Theme::meter_height()))
            .gap(px(Theme::meter_gap()))
            .children((0..bars).map(|i| {
                let then = self
                    .mic_history
                    .get(i)
                    .copied()
                    .unwrap_or(f32::NEG_INFINITY);
                div()
                    .flex_1()
                    .h(px(Theme::meter_bar_min() + reach(then) * room))
                    .rounded(px(Theme::meter_bar_radius()))
                    .bg(if i < lit {
                        ink.opacity(Theme::meter_lit_alpha())
                    } else {
                        theme.text.opacity(Theme::meter_unlit_alpha())
                    })
            }));
        // Without microphone access, which the card never asks for, the
        // level stays at negative infinity and the readout is a dash rather
        // than a made-up number.
        let readout = if level > floor {
            format!("−{:.0} dB", -level.min(0.))
        } else {
            "— dB".into()
        };
        row().gap(px(Theme::gap_large())).child(meter).child(
            mono(readout)
                .flex_none()
                .w(px(Theme::meter_readout_width()))
                .text_right()
                .text_size(px(Theme::font_secondary()))
                .text_color(theme.text),
        )
    }
}

/// What the Test reads while it runs, each of which it is kept wide enough
/// for, so the slider beside it holds still through the countdown.
const TEST_LABELS: [&str; 4] = ["Listening… 3", "Listening… 2", "Listening… 1", "Playing"];

/// Records three seconds and plays them back, raised on `seg_active` with
/// the `rec` dot. `phase` is the options' `mic_test`: "Listening… 3", "2",
/// "1", then "Playing", then back to "Test"; a press in either stops it.
fn test_button(
    phase: i32,
    enabled: bool,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    theme: Theme,
) -> impl IntoElement {
    let id = ElementId::from("mic-test");
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let label = match phase {
        0 => "Test".to_owned(),
        seconds if seconds > 0 => format!("Listening… {seconds}"),
        _ => "Playing".to_owned(),
    };
    // One line tall, so the label reads level with the dot; the labels it
    // runs through sit clipped under it, widening the button so the slider
    // beside it holds still through the countdown.
    let mut words = column()
        .h(px(Theme::font_body()))
        .line_height(relative(1.))
        .overflow_hidden()
        .child(div().flex_none().child(label));
    if phase != 0 {
        words = words.children(
            TEST_LABELS
                .iter()
                .map(|label| div().flex_none().child(*label)),
        );
    }
    let mut button = div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(Theme::mic_test_gap()))
        .h(px(Theme::control_height()))
        .pl(px(Theme::control_padding_small()))
        .pr(px(Theme::control_padding()))
        .rounded_full()
        .bg(theme.seg_active)
        .shadow(vec![theme.segment_shadow()])
        .text_size(px(Theme::font_body()))
        .font_weight(FontWeight::MEDIUM)
        .child(
            div()
                .flex_none()
                .size(px(Theme::mic_test_dot()))
                .rounded_full()
                .bg(theme.rec),
        )
        .child(words)
        .child(subtake_ui::pill_edge(vec![hairline(
            theme.line,
            Theme::hairline_width(),
        )]));
    if enabled {
        button = subtake_ui::pressable(button, theme, None, hover_key).on_click(press);
    }
    layered(button)
}
