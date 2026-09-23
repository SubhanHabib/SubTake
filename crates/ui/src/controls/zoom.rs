//! The zoom control, shared by everything that zooms.

use gpui::{prelude::*, *};
use subtake_theme::Theme;

use crate::{icon_button, row};

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Zoom out, the zoom as a percentage and zoom in on one `sunk` plate, so the
/// three read as one control that is set rather than three that are pressed,
/// then Fit as an icon beside it. The stage's picture and the console's
/// timeline both zoom through it, so the two read and behave alike.
///
/// The steps dim where they can go no further, and Fit dims at Fit. The
/// glyphs are at full strength, as the figure beside them is, so a dimmed
/// one reads as disabled rather than as quiet. The figure is in tabular
/// figures in a fixed-width slot, so the control holds its shape while a
/// pinch runs.
///
/// Not drawn by the design: the handoff has a single "Fit · 100%" pill on
/// the stage and three icons on the console.
#[derive(IntoElement)]
pub struct ZoomControl {
    id: SharedString,
    zoom: f32,
    theme: Theme,
    can_zoom_out: bool,
    can_zoom_in: bool,
    can_fit: bool,
    zoom_out: Option<Handler>,
    zoom_in: Option<Handler>,
    fit: Option<Handler>,
}

/// A zoom control reading `zoom`, where 1 is Fit. `id` keys its three
/// buttons, so two controls in one window need two ids.
pub fn zoom_control(id: impl Into<SharedString>, zoom: f32, theme: Theme) -> ZoomControl {
    ZoomControl {
        id: id.into(),
        zoom,
        theme,
        can_zoom_out: true,
        can_zoom_in: true,
        can_fit: true,
        zoom_out: None,
        zoom_in: None,
        fit: None,
    }
}

impl ZoomControl {
    pub fn can_zoom_out(mut self, can: bool) -> Self {
        self.can_zoom_out = can;
        self
    }

    pub fn can_zoom_in(mut self, can: bool) -> Self {
        self.can_zoom_in = can;
        self
    }

    pub fn can_fit(mut self, can: bool) -> Self {
        self.can_fit = can;
        self
    }

    pub fn on_zoom_out(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.zoom_out = Some(Box::new(handler));
        self
    }

    pub fn on_zoom_in(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.zoom_in = Some(Box::new(handler));
        self
    }

    pub fn on_fit(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.fit = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ZoomControl {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = self.theme;
        let id = self.id;
        let step =
            |name: &str, glyph: &str, label: &str, enabled: bool, handler: Option<Handler>| {
                let button = icon_button(
                    SharedString::from(format!("{id}-{name}")),
                    glyph,
                    label.to_owned(),
                    theme,
                )
                .ghost()
                .small()
                .strong()
                .enabled(enabled);
                match handler {
                    Some(handler) => button.on_click(handler),
                    None => button,
                }
            };
        row()
            .gap(px(Theme::GAP_SMALL))
            .flex_none()
            .child(
                row()
                    .gap_0()
                    .flex_none()
                    .rounded_full()
                    .bg(theme.sunk)
                    .child(step(
                        "out",
                        "MagnifyingGlassMinus-regular",
                        "Zoom out",
                        self.can_zoom_out,
                        self.zoom_out,
                    ))
                    .child(
                        div()
                            .w(px(Theme::ZOOM_READOUT_WIDTH))
                            .flex_none()
                            .text_center()
                            .text_size(px(Theme::FONT_CONTROL))
                            .text_color(theme.text)
                            .font_features(FontFeatures(std::sync::Arc::new(vec![(
                                "tnum".into(),
                                1,
                            )])))
                            .child(format!("{}%", (self.zoom * 100.).round())),
                    )
                    .child(step(
                        "in",
                        "MagnifyingGlassPlus-regular",
                        "Zoom in",
                        self.can_zoom_in,
                        self.zoom_in,
                    )),
            )
            .child(step(
                "fit",
                "ArrowsOutSimple-regular",
                "Fit",
                self.can_fit,
                self.fit,
            ))
    }
}
