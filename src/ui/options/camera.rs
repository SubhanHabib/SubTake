//! The Camera card: the webcam overlay.

use super::*;

impl RootView {
    pub(super) fn camera_card(
        &mut self,
        state: &RecordingOptions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        let busy = state.get_busy();
        let on = state.get_camera();
        let toggle_state = state.clone();
        let mut body = vec![
            toggle(
                "camera-toggle",
                "Webcam overlay",
                on,
                !busy,
                theme,
                move |v, _, _| toggle_state.defer_option("camera".into(), v.to_string()),
            )
            .into_any_element(),
        ];
        let names: Vec<String> = state.get_camera_names().iter().collect();
        if names.is_empty() {
            // Not drawn by the design: a Mac with no camera. The plate keeps
            // the preview's size so the card does not jump when one appears.
            body.push(
                row()
                    .h(px(Theme::camera_preview_height()))
                    .justify_center()
                    .gap(px(Theme::icon_gap_row()))
                    .rounded(px(Theme::radius_menu()))
                    .bg(theme.sunk)
                    .text_color(theme.muted)
                    .child(icon("VideoCameraSlash-regular", theme.muted))
                    .child("No camera found")
                    .into_any_element(),
            );
        } else {
            let options = state.clone();
            let camera = self.dropdown(
                "camera",
                names,
                state.get_camera_index(),
                !busy && on,
                cx,
                move |i, _, _| options.defer_option("camera-device".into(), i.to_string()),
            );
            camera.update(cx, |d, _| d.glyph = Some("VideoCamera-regular".into()));
            body.push(camera.into_any_element());
            body.push(camera_preview(state.get_camera_preview(), on, theme).into_any_element());
        }
        body.push(
            helper(
                "Your camera is recorded separately and added to the project.",
                theme,
            )
            .into_any_element(),
        );
        body
    }
}

/// The camera's live picture, with the shape it will take set in its
/// corner. Off, it dims.
fn camera_preview(image: crate::ui_runtime::Image, on: bool, theme: Theme) -> Div {
    let plate = div()
        .relative()
        .h(px(Theme::camera_preview_height()))
        .rounded(px(Theme::radius_menu()))
        .overflow_hidden()
        .bg(theme.sunk)
        .opacity(if on { 1. } else { Theme::disabled_opacity() });
    let plate = match image.0 {
        Some(image) => plate.child(
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(Theme::radius_menu()))
                .object_fit(ObjectFit::Cover),
        ),
        // Not wired: the app does not stream the camera into this card yet,
        // so outside the gallery the plate shows the camera glyph.
        None => plate
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                "VideoCamera-regular",
                Theme::icon_size_large(),
                theme.muted,
            )),
    };
    // The swatch and the chip sit on the picture, not on chrome, so they
    // keep one fill in both appearances as the handoff draws them.
    plate
        .child(
            div()
                .absolute()
                .left(px(Theme::gap_large()))
                .bottom(px(Theme::gap_large()))
                .size(px(Theme::camera_swatch()))
                .rounded_full()
                .bg(rgb(0x1b2434))
                .shadow(theme.camera_swatch_shadow()),
        )
        // Palette churn: the handoff blurs what is under this chip by 18.
        // A backdrop blur inside the card would be a second frosted layer
        // over a live picture for one word, so the chip keeps the tint only.
        .child(
            div()
                .absolute()
                .right(px(Theme::gap_large()))
                .top(px(Theme::gap_large()))
                .flex()
                .items_center()
                .h(px(Theme::chip_height()))
                .px(px(Theme::gap_large()))
                .rounded_full()
                .bg(theme.scrim_chip())
                .text_size(px(Theme::font_small()))
                .text_color(gpui::white())
                .child("Preview"),
        )
}
