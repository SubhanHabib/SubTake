//! The Camera card: the live picture with the device on it, and where the
//! camera sits in the recording, its shape and its size.

use super::parts::{access_off, section_label};
use super::*;
use subtake_ui::{edge, segmented};

/// The corners Position offers, in the order they are drawn, and what each
/// is kept as.
const CORNERS: [(&str, bool, bool); 4] = [
    ("top-left", true, true),
    ("top-right", true, false),
    ("bottom-left", false, true),
    ("bottom-right", false, false),
];
const SHAPES: [&str; 3] = ["circle", "rounded", "square"];
const SIZES: [(&str, &str); 3] = [("s", "S"), ("m", "M"), ("l", "L")];

impl RootView {
    pub(super) fn camera_card(
        &mut self,
        state: &RecordingOptions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        if !state.get_camera_access() {
            return vec![
                access_off("Camera", self.command("access-camera"), theme).into_any_element(),
            ];
        }
        let busy = state.get_busy();
        let on = state.get_camera();
        let names: Vec<String> = state.get_camera_names().iter().collect();
        let mut body = Vec::new();
        if !camera_usable(state) {
            // A Mac with no camera, as round 2 draws it: the plate keeps the
            // picture's shape so the card does not jump when one appears.
            body.push(
                row()
                    .w_full()
                    .aspect_ratio(Theme::camera_preview_aspect())
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
            camera.update(cx, |d, _| {
                d.glyph = Some("VideoCamera-regular".into());
                d.chip = true;
            });
            body.push(
                camera_preview(state.get_camera_preview(), on, theme)
                    .child(
                        div()
                            .absolute()
                            .left(px(Theme::camera_chip_inset()))
                            .right(px(Theme::camera_chip_inset()))
                            .bottom(px(Theme::camera_chip_inset()))
                            .child(camera),
                    )
                    .into_any_element(),
            );
            body.push(camera_controls(state, on && !busy, theme).into_any_element());
        }
        body.push(
            helper(
                "Recorded as its own track, so you can move or restyle it in the editor.",
                theme,
            )
            .into_any_element(),
        );
        body
    }
}

/// The camera's picture at 16:10, mirrored as a mirror would show it, with
/// Live on it once it comes in; before then, a spinner while the camera
/// starts. Off, the picture gives way to the camera struck through.
fn camera_preview(image: crate::ui_runtime::Image, on: bool, theme: Theme) -> Div {
    let radius = Theme::radius_menu();
    let plate = div()
        .relative()
        .w_full()
        .aspect_ratio(Theme::camera_preview_aspect())
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    let live = on && image.0.is_some();
    let plate = match (on, image.0) {
        (false, _) => plate
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                "VideoCameraSlash-regular",
                Theme::camera_off_icon(),
                theme.muted,
            )),
        (true, Some(image)) => plate.child(
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        (true, None) => {
            plate
                .flex()
                .items_center()
                .justify_center()
                .child(super::super::recorder::spinner(
                    Theme::camera_spinner_size(),
                    theme,
                ))
        }
    };
    let plate = plate.child(edge(
        radius,
        vec![hairline(theme.line, Theme::hairline_width())],
    ));
    if !live {
        return plate;
    }
    // Palette churn: the handoff blurs what is under the chips by 16. A
    // backdrop blur inside the card would be a second frosted layer over a
    // live picture, so the chips keep the `frost` tint only.
    plate.child(
        row()
            .absolute()
            .left(px(Theme::camera_chip_inset()))
            .top(px(Theme::camera_chip_inset()))
            .gap(px(Theme::live_chip_gap()))
            .h(px(Theme::live_chip_height()))
            .px(px(Theme::live_chip_padding()))
            .rounded_full()
            .bg(theme.frost)
            .text_size(px(Theme::font_small()))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text)
            .child(
                div()
                    .size(px(Theme::live_dot()))
                    .rounded_full()
                    .bg(theme.rec),
            )
            .child("Live"),
    )
}

/// Position beside Shape and Size. Off, they dim where they are and take
/// no clicks.
fn camera_controls(state: &RecordingOptions, enabled: bool, theme: Theme) -> Div {
    let corner = state.get_recorder_setting("camera-corner");
    let position = div()
        .relative()
        .flex_1()
        .min_h(px(Theme::camera_position_min_height()))
        .rounded(px(Theme::camera_position_radius()))
        .bg(theme.sunk)
        .children(CORNERS.iter().map(|&(key, top, left)| {
            corner_target(key, top, left, corner == key, enabled, state, theme)
        }))
        .child(edge(
            Theme::camera_position_radius(),
            vec![hairline(theme.line, Theme::hairline_width())],
        ));

    let choice = |key: &'static str, values: &'static [&'static str]| {
        let current = state.get_recorder_setting(key);
        let options = state.clone();
        (
            values.iter().position(|v| *v == current).unwrap_or(0),
            move |index: usize, _: &mut Window, _: &mut App| {
                if enabled {
                    options.defer_option(key.into(), values[index].into())
                }
            },
        )
    };
    let (shape, pick_shape) = choice("camera-shape", &SHAPES);
    const SIZE_KEYS: [&str; 3] = [SIZES[0].0, SIZES[1].0, SIZES[2].0];
    let (size, pick_size) = choice("camera-size", &SIZE_KEYS);

    row()
        .items_stretch()
        .gap(px(Theme::camera_controls_gap()))
        .opacity(if enabled {
            1.
        } else {
            Theme::disabled_opacity()
        })
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(Theme::camera_position_width()))
                .gap(px(Theme::gap()))
                .child(section_label("Position", theme))
                .child(position),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(Theme::gap()))
                .child(section_label("Shape", theme))
                .child(segmented(
                    "camera-shape",
                    SHAPES.len(),
                    shape,
                    Theme::control_height(),
                    theme,
                    pick_shape,
                    |index, color| shape_glyph(index, color).into_any_element(),
                ))
                .child(section_label("Size", theme))
                .child(segmented(
                    "camera-size",
                    SIZES.len(),
                    size,
                    Theme::control_height(),
                    theme,
                    pick_size,
                    |index, _| SIZES[index].1.into_any_element(),
                )),
        )
}

/// One corner of Position: a ring, or the chosen corner raised on
/// `seg_active` inside the accent. None is chosen once the camera has been
/// dragged on the stage in the editor, which the summary reads as Custom
/// position; a corner pressed takes it back there.
fn corner_target(
    key: &'static str,
    top: bool,
    left: bool,
    chosen: bool,
    enabled: bool,
    state: &RecordingOptions,
    theme: Theme,
) -> impl IntoElement {
    let (size, inset) = if chosen {
        (Theme::corner_selected(), Theme::corner_selected_inset())
    } else {
        (Theme::corner_target(), Theme::corner_target_inset())
    };
    let id = ElementId::from(SharedString::from(format!("camera-corner-{key}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let target = div()
        .id(id)
        .absolute()
        .size(px(size))
        .rounded_full()
        .border(px(Theme::selected_width()))
        .map(|s| {
            if top {
                s.top(px(inset))
            } else {
                s.bottom(px(inset))
            }
        })
        .map(|s| {
            if left {
                s.left(px(inset))
            } else {
                s.right(px(inset))
            }
        });
    let target = if chosen {
        target
            .border_color(theme.accent)
            .bg(theme.seg_active)
            .shadow(vec![theme.segment_shadow()])
    } else {
        target
            .border_color(theme.switch_off)
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.hover.opacity(0.),
                theme.hover,
            ))
    };
    if !enabled || chosen {
        return target;
    }
    let options = state.clone();
    subtake_ui::pressable(target, theme, Some(theme.press), hover_key)
        .on_click(move |_, _, _| options.defer_option("camera-corner".into(), key.into()))
}

/// Shape's glyphs: a circle, a rounded square and a square, outlined in
/// the segment's colour.
fn shape_glyph(index: usize, color: Hsla) -> Div {
    let glyph = div()
        .size(px(Theme::shape_glyph()))
        .border(px(Theme::selected_width()))
        .border_color(color);
    match index {
        0 => glyph.rounded_full(),
        1 => glyph.rounded(px(Theme::shape_glyph_rounded())),
        _ => glyph.rounded(px(Theme::shape_glyph_square())),
    }
}
