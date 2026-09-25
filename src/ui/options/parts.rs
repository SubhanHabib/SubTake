//! The parts the recorder cards are built from, as the popovers handoff
//! names them: section labels, groups of rows, a row and its plate, key
//! caps. Helper text is `helper`, next door in `options.rs`.

use super::*;
use subtake_theme::FONT_MONO;
use subtake_ui::edge;

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A block's name above it: 18 tall, 6 in, small caps in `muted`.
///
/// Not carried: the label's 0.06em tracking. gpui at the pinned revision
/// has no letter-spacing.
pub(super) fn section_label(text: &str, theme: Theme) -> Div {
    row()
        .flex_none()
        .h(px(Theme::card_label_height()))
        .px(px(Theme::card_text_inset()))
        .text_size(px(Theme::font_small()))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.muted)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .child(text.to_uppercase()),
        )
}

/// One row of a [`group`]: a title, perhaps a subtitle under it, perhaps a
/// glyph on a plate before it, and whatever the row keeps at its right.
pub(super) struct CardRow {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    subtitle_mono: bool,
    glyph: Option<&'static str>,
    trailing: Option<AnyElement>,
    selected: bool,
    click: Option<Click>,
    height: Option<f32>,
}

impl CardRow {
    pub(super) fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            subtitle_mono: false,
            glyph: None,
            trailing: None,
            selected: false,
            click: None,
            height: None,
        }
    }

    pub(super) fn subtitle(mut self, text: impl Into<SharedString>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// The subtitle in Geist Mono at the small size: a path.
    pub(super) fn subtitle_mono(mut self) -> Self {
        self.subtitle_mono = true;
        self
    }

    pub(super) fn plate(mut self, glyph: &'static str) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub(super) fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// A height other than the one its lines give it: 48 alone, 52 with
    /// a subtitle.
    pub(super) fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub(super) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The whole row answers a click, with the `hover` wash under it. A row
    /// whose only control is at its right leaves this off: the control
    /// answers for itself.
    pub(super) fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Box::new(click));
        self
    }

    fn build(self, theme: Theme) -> Stateful<Div> {
        let height = self.height.unwrap_or(if self.subtitle.is_some() {
            Theme::card_row_height_tall()
        } else {
            Theme::card_row_height()
        });
        let hover_key = subtake_ui::motion::tween_key(&self.id, "hover");
        let subtitle_mono = self.subtitle_mono;
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(Theme::card_row_gap()))
            .h(px(height))
            .pl(px(if self.glyph.is_some() {
                Theme::card_row_inset_plate()
            } else {
                Theme::card_row_inset()
            }))
            .pr(px(Theme::card_row_inset_end()));
        el = if self.selected {
            el.bg(theme.sunk2)
        } else if self.click.is_some() {
            el.bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.hover.opacity(0.),
                theme.hover,
            ))
        } else {
            el
        };
        if let Some(click) = self.click {
            el = subtake_ui::pressable(el, theme, Some(theme.press), hover_key).on_click(click);
        }
        el.children(self.glyph.map(|glyph| row_plate(glyph, theme)))
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(Theme::card_row_subtitle_gap()))
                    .child(
                        div()
                            .text_size(px(Theme::font_body()))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .text_ellipsis()
                            .child(self.title),
                    )
                    .children(self.subtitle.map(|text| {
                        div()
                            .map(|s| {
                                if subtitle_mono {
                                    s.font_family(FONT_MONO).text_size(px(Theme::font_small()))
                                } else {
                                    s.text_size(px(Theme::font_secondary()))
                                }
                            })
                            .text_color(theme.muted)
                            .text_ellipsis()
                            .child(text)
                    })),
            )
            .children(self.trailing)
    }
}

/// A row's glyph on its 36 plate.
pub(super) fn row_plate(glyph: &str, theme: Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(Theme::card_row_plate()))
        .rounded_full()
        .bg(theme.plate)
        .child(icon_sized(glyph, Theme::card_row_icon(), theme.text))
}

/// Rows on one recess, with a hairline between each two: 56 in, past the
/// plates, when the rows have them, and 16 in when they don't.
///
/// The group draws its hairline edge over its rows rather than clipping
/// them, since gpui at the pinned revision clips to a rectangle; its first
/// and last rows take its corners instead, so a hover or a selected fill
/// stays inside them.
pub(super) fn group(rows: Vec<CardRow>, theme: Theme) -> Div {
    let plated = rows.iter().any(|r| r.glyph.is_some());
    let divider_inset = if plated {
        Theme::card_divider_inset_plate()
    } else {
        Theme::card_divider_inset()
    };
    let radius = px(Theme::card_group_radius());
    let last = rows.len().saturating_sub(1);
    let mut el = div()
        .flex()
        .flex_col()
        .relative()
        .flex_none()
        .rounded(radius)
        .bg(theme.sunk);
    for (index, card_row) in rows.into_iter().enumerate() {
        if index > 0 {
            el = el.child(
                div()
                    .flex_none()
                    .h(px(Theme::hairline_width()))
                    .ml(px(divider_inset))
                    .bg(theme.line),
            );
        }
        let mut built = card_row.build(theme);
        if index == 0 {
            built = built.rounded_t(radius);
        }
        if index == last {
            built = built.rounded_b(radius);
        }
        el = el.child(built);
    }
    el.child(edge(
        Theme::card_group_radius(),
        vec![hairline(theme.line, Theme::hairline_width())],
    ))
}

/// Keys as caps, 3 apart: `⌘` `O`.
pub(super) fn key_caps(keys: &[&str], theme: Theme) -> Div {
    row()
        .flex_none()
        .gap(px(Theme::key_cap_gap()))
        .children(keys.iter().map(|key| key_cap(key, theme)))
}

/// One key: at least 22 square, `sunk2` inside a hairline, in mono.
fn key_cap(key: &str, theme: Theme) -> Div {
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(Theme::key_cap_size()))
        .min_w(px(Theme::key_cap_size()))
        .px(px(Theme::key_cap_padding()))
        .rounded(px(Theme::key_cap_radius()))
        .bg(theme.sunk2)
        .font_family(FONT_MONO)
        .text_size(px(Theme::font_small()))
        .text_color(theme.text)
        .child(key.to_string())
        .child(edge(
            Theme::key_cap_radius(),
            vec![hairline(theme.line, Theme::hairline_width())],
        ))
}
