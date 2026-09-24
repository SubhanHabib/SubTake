//! The Camera panel: where the webcam overlay sits, its shape, its size and
//! whether it is mirrored.
//!
//! The model calls it the webcam (`webcam.*`, the `Webcam` panel); the
//! handoff calls it the camera, so the panel does.

use super::cursor::{inert, panel_heading};
use super::*;
use subtake_ui::{focus_ring, icon_sized, layered};

/// The three corners the handoff offers, in its order, with where each puts
/// the overlay as a custom position would (`src/render.rs`).
const CORNERS: [(&str, &str, f32, f32); 3] = [
    ("bottom-right", "Bottom right", 1., 1.),
    ("bottom-left", "Bottom left", 0., 1.),
    ("top-right", "Top right", 1., 0.),
];

/// Circle, Rounded and Square as `webcam.roundness`. The renderer's corner
/// radius is the root of it, so a quarter is a corner a quarter of the
/// overlay's side.
const SHAPES: [(&str, f32); 3] = [("Circle", 100.), ("Rounded", 25.), ("Square", 0.)];

impl RootView {
    pub(super) fn camera_panel(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let theme = self.theme;
        let heading = panel_heading("Camera", e, theme);
        let fields: Vec<Field> = e.get_fields().iter().collect();
        let find = |key: &str| fields.iter().find(|f| f.key == key).cloned();
        let value = |key: &str| find(key).map(|f| f.value.to_string()).unwrap_or_default();
        let number = |key: &str, default: f32| value(key).parse::<f32>().unwrap_or(default);

        // Not drawn by the design: the footage and the switch that shows it.
        // The handoff's panel assumes a camera is there; the model has one
        // only once a clip is chosen and turned on, so those come first, and
        // the rest is dimmed until it is.
        let shown = value("webcam.enabled") == "true";
        let mut content = column().gap(px(Theme::GAP_LARGE));
        for key in ["choose-webcam", "webcam.enabled"] {
            if let Some(field) = find(key) {
                content = content.child(self.field(e, field, window, cx));
            }
        }

        let preset = value("webcam.positionPreset");
        let mut grid = div().grid().grid_cols(2).gap(px(Theme::CAMERA_TILE_GAP));
        for (index, (corner, label, x, y)) in CORNERS.into_iter().enumerate() {
            let chosen = preset == corner;
            let editor = e.clone();
            grid = grid.child(
                position_tile(index, label, chosen, theme)
                    .child(
                        div()
                            .absolute()
                            .size(px(Theme::CAMERA_DOT_SIZE))
                            .rounded_full()
                            .bg(if chosen { theme.accent } else { theme.muted })
                            .map(|el| {
                                let inset = px(Theme::CAMERA_DOT_INSET);
                                let el = if x > 0. {
                                    el.right(inset)
                                } else {
                                    el.left(inset)
                                };
                                if y > 0. {
                                    el.bottom(inset)
                                } else {
                                    el.top(inset)
                                }
                            }),
                    )
                    .on_click(move |_, _, _| {
                        editor.defer_field("webcam.positionPreset".into(), corner.into())
                    }),
            );
        }
        // Custom starts where the overlay already is, so choosing it does not
        // move anything.
        //
        // Not wired: the handoff hands a custom position to dragging the
        // overlay on the stage, and the stage has no such drag yet; it is
        // set by the two position rows under More.
        let custom = preset == "custom";
        let editor = e.clone();
        let current = CORNERS.iter().find(|(c, ..)| *c == preset).copied();
        grid = grid.child(
            position_tile(CORNERS.len(), "Custom", custom, theme)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(Theme::CAMERA_CUSTOM_GAP))
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted)
                .child(icon_sized(
                    "ArrowsOutSimple-regular",
                    Theme::CAMERA_CUSTOM_ICON,
                    theme.muted,
                ))
                .child("Custom")
                .on_click(move |_, _, _| {
                    if let Some((_, _, x, y)) = current {
                        editor.defer_field("webcam.positionX".into(), x.to_string());
                        editor.defer_field("webcam.positionY".into(), y.to_string());
                    }
                    editor.defer_field("webcam.positionPreset".into(), "custom".into())
                }),
        );
        // Not drawn by the design: the other six presets a project can hold
        // (top left, the centres). They still place the overlay, and no tile
        // is ringed for them; picking one of these four replaces it.
        let mut rows = column()
            .gap(px(Theme::GAP_LARGE))
            .child(caps_label("Position", theme))
            .child(grid);

        let roundness = number("webcam.roundness", 100.);
        let shape = if roundness >= 100. {
            0
        } else if roundness <= 0. {
            2
        } else {
            1
        };
        let editor = e.clone();
        rows = rows
            .child(caps_label("Shape", theme))
            .child(segmented_control(
                "camera-shape",
                &SHAPES.map(|(label, _)| label),
                shape,
                theme,
                move |index, _, _| {
                    editor.defer_field(
                        "webcam.roundness".into(),
                        SHAPES[index.min(2)].1.to_string(),
                    )
                },
            ));

        // Palette churn: the handoff's width reads in points. The model's is
        // a share of the frame's shorter side, so it reads as a percentage.
        let editor = e.clone();
        let width = find("webcam.width");
        rows = rows.child(self.slider(
            "Webcam:webcam.width",
            width.as_ref().map_or(5., |f| f.minimum),
            width.as_ref().map_or(100., |f| f.maximum),
            number("webcam.width", 40.),
            ("Width", ""),
            (1., "%"),
            cx,
            move |v, commit, _, _| {
                if commit {
                    editor.defer_field("webcam.width".into(), v.to_string());
                }
            },
        ));
        let editor = e.clone();
        rows = rows.child(toggle(
            "Webcam:webcam.mirror",
            "Mirror",
            value("webcam.mirror") == "true",
            true,
            theme,
            move |v, _, _| editor.defer_field("webcam.mirror".into(), v.to_string()),
        ));

        // Not wired: the handoff's helper line says the camera is its own
        // track, hidden on a stretch by trimming it. It is not a track yet —
        // one switch shows it for the whole video — so the line would be
        // untrue and is left out.
        //
        // Not drawn by the design: the rest of the overlay — height, the
        // custom position, margin, shadow, reacting to zoom, the crop and
        // the time offset.
        let drawn = [
            "choose-webcam",
            "webcam.enabled",
            "webcam.positionPreset",
            "webcam.roundness",
            "webcam.width",
            "webcam.mirror",
        ];
        let rest: Vec<Field> = fields
            .iter()
            .filter(|f| !drawn.contains(&f.key.as_str()) && f.kind != 5)
            .cloned()
            .collect();
        if !rest.is_empty() {
            rows = rows.child(caps_label("More", theme));
            for field in rest {
                rows = rows.child(self.field(e, field, window, cx));
            }
        }

        content = content.child(inert("camera-rows", rows, shown));
        (heading, content)
    }
}

/// A 60-tall `sunk` tile, ringed in accent when it is the chosen position.
fn position_tile(index: usize, label: &str, chosen: bool, theme: Theme) -> Stateful<Div> {
    let radius = px(Theme::CAMERA_TILE_RADIUS);
    let id = ElementId::from(SharedString::from(format!("camera-position-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    // Inside a frosted card an inset edge set on the tile would draw under
    // its own fill, so the ring and the hover wash are layers over it.
    let wash = div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .bg(subtake_ui::motion::hover_blend(
            &hover_key,
            theme.sunk2.opacity(0.),
            theme.sunk2,
        ));
    let ring = div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .when(chosen, |s| {
            s.shadow(vec![hairline(theme.accent, Theme::SELECTED_WIDTH)])
        });
    // Palette churn: no `scale .98` under the press, since gpui has no
    // transform on an element; the wash goes to `press` instead.
    div()
        .id(id)
        .on_hover(subtake_ui::motion::hover_listener(hover_key))
        .relative()
        .h(px(Theme::CAMERA_TILE_HEIGHT))
        .rounded(radius)
        .bg(theme.sunk)
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(move |s| s.shadow(vec![focus_ring(theme)]))
        .active(|s| s.bg(theme.press))
        .tooltip({
            let label = label.to_owned();
            move |_, cx| tooltip(label.clone(), theme, cx)
        })
        .child(wash)
        .child(layered(ring))
}
