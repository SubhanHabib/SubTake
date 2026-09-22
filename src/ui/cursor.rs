//! The Cursor panel: whether the recorded cursor is drawn, what it looks
//! like, what a click does and how it moves.
//!
//! The rows the handoff draws come first, in its order; every other cursor
//! setting the model hands over follows under More.

use super::inspector::field_unit;
use super::*;

/// The click effects the renderer draws (`src/render/cursor.rs`).
const CLICK_EFFECTS: [(&str, &str); 4] = [
    ("none", "None"),
    ("ripple", "Ripple"),
    ("spotlight", "Spotlight"),
    ("echo", "Echo"),
];

/// What turning Smooth movement on restores: the model's default.
const DEFAULT_SMOOTHING: f32 = 0.67;

impl RootView {
    pub(super) fn cursor_panel(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let theme = self.theme;
        let heading = panel_heading("Cursor", e, theme);
        let fields: Vec<Field> = e.get_fields().iter().collect();
        let find = |key: &str| fields.iter().find(|f| f.key == key).cloned();
        let value = |key: &str| find(key).map(|f| f.value.to_string()).unwrap_or_default();
        let number = |key: &str, default: f32| value(key).parse::<f32>().unwrap_or(default);

        let shown = value("showCursor") != "false";
        let editor = e.clone();
        let mut content = column().gap(px(Theme::GAP_LARGE)).child(toggle(
            "Cursor:showCursor",
            "Show cursor",
            shown,
            true,
            theme,
            move |v, _, _| editor.defer_field("showCursor".into(), v.to_string()),
        ));

        // Everything under Show cursor is about a cursor that is drawn, so
        // with it off the rows dim and a sheet over them takes the pointer.
        let mut rows = column().gap(px(Theme::GAP_LARGE));

        // Not drawn by the design: the style picker. The handoff's Arrow /
        // Hand / Dot are not styles the renderer has; its five are, each
        // drawn as the cursor it is, so the tiles stay.
        if let Some(mut style) = find("cursorStyle") {
            style.label = "Style".into();
            rows = rows.child(self.field(e, style, window, cx));
        }
        let editor = e.clone();
        let (scale, unit) = field_unit("cursorSize");
        rows = rows.child(self.slider(
            "Cursor:cursorSize",
            find("cursorSize").map_or(0.5, |f| f.minimum),
            find("cursorSize").map_or(8., |f| f.maximum),
            number("cursorSize", 3.),
            ("Size", ""),
            (scale, unit),
            cx,
            move |v, commit, _, _| {
                if commit {
                    editor.defer_field("cursorSize".into(), v.to_string());
                }
            },
        ));

        // Palette churn: the handoff's third effect is Pulse. The renderer
        // has no Pulse; it has Spotlight and Echo, so the control has four.
        let effect = value("cursorClickEffect");
        let editor = e.clone();
        rows = rows
            .child(caps_label("Click effect", theme))
            .child(segmented_control(
                "cursor-click-effect",
                &CLICK_EFFECTS.map(|(_, label)| label),
                CLICK_EFFECTS
                    .iter()
                    .position(|(v, _)| *v == effect)
                    .unwrap_or(1),
                theme,
                move |index, _, _| {
                    editor.defer_field(
                        "cursorClickEffect".into(),
                        CLICK_EFFECTS[index.min(3)].0.into(),
                    )
                },
            ));

        // Smooth movement is not a setting of its own: the renderer smooths
        // by `cursorSmoothing`, and none is 0. Off writes 0, on restores the
        // default, and Smoothness is that same number.
        let smoothing = number("cursorSmoothing", DEFAULT_SMOOTHING);
        let smooth = smoothing > 0.;
        let editor = e.clone();
        let smoothness = {
            let editor = e.clone();
            let (scale, unit) = field_unit("cursorSmoothing");
            self.slider(
                "Cursor:cursorSmoothing",
                0.,
                find("cursorSmoothing").map_or(2., |f| f.maximum),
                smoothing,
                ("Smoothness", ""),
                (scale, unit),
                cx,
                move |v, commit, _, _| {
                    if commit {
                        editor.defer_field("cursorSmoothing".into(), v.to_string());
                    }
                },
            )
        };
        // Dimmed already when the cursor is hidden; dimming it twice would
        // take it further than every other row.
        smoothness.update(cx, |s, _| s.enabled = smooth || !shown);
        rows = rows
            .child(caps_label("Movement", theme))
            .child(toggle(
                "Cursor:smooth",
                "Smooth movement",
                smooth,
                true,
                theme,
                move |v, _, _| {
                    let value = if v { DEFAULT_SMOOTHING } else { 0. };
                    editor.defer_field("cursorSmoothing".into(), value.to_string())
                },
            ))
            .child(smoothness);

        // Not drawn by the design: every other cursor setting — sway, motion
        // blur, the click effect's size, opacity, length and colour, the
        // bounce, looping — and the camera's motion blur. The handoff's
        // panel has six rows; these still change the video, so they follow.
        let drawn = [
            "showCursor",
            "cursorStyle",
            "cursorSize",
            "cursorClickEffect",
            "cursorSmoothing",
        ];
        let rest: Vec<Field> = fields
            .iter()
            .filter(|f| {
                !drawn.contains(&f.key.as_str()) && !f.key.starts_with("motion-") && f.kind != 5
            })
            .cloned()
            .collect();
        if !rest.is_empty() {
            rows = rows.child(caps_label("More", theme));
            for field in rest {
                rows = rows.child(self.field(e, field, window, cx));
            }
        }

        content = content.child(inert(rows, shown));
        (heading, content)
    }
}

/// A panel's title row as round 2 draws it: the name in Space Grotesk 19
/// and a close control.
///
/// Not drawn by the design: where closing lands. The inspector always shows
/// a panel, so it goes back to Scene.
pub(super) fn panel_heading(name: &'static str, e: &EditorWindow, theme: Theme) -> Div {
    let editor = e.clone();
    row()
        .h(px(Theme::CONTROL_HEIGHT_SMALL))
        .flex_none()
        .gap(px(Theme::ICON_GAP_ROW))
        .child(title(name, Theme::FONT_HEADING).flex_1())
        .child(
            icon_button(
                SharedString::from(format!("{name}-close")),
                "X-regular",
                "Close",
                theme,
            )
            .small()
            .on_click(move |_, _, _| {
                editor.set_panel("Frame".into());
                editor.defer_panel("Frame".into());
            }),
        )
}

/// Rows that stand for something switched off: dimmed, and under a sheet
/// that takes the pointer so nothing in them can be changed.
///
/// Not wired: Tab still reaches the controls under the sheet, since gpui
/// has no way to take a whole subtree out of the tab order.
pub(super) fn inert(rows: Div, enabled: bool) -> Div {
    div().relative().child(rows).when(!enabled, |el| {
        el.opacity(Theme::DISABLED_OPACITY)
            .child(div().absolute().inset_0().occlude())
    })
}
