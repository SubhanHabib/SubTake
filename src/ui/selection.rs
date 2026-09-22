//! The Selection panel: whatever region the timeline has selected, named by
//! its kind and edited in place.
//!
//! It is not a tool and has no pod button. Clicking a region shows it over
//! the open panel; deselecting goes back (`App::show_selection`).

use super::*;
use subtake_ui::{TimecodeField, focus_ring, icon_sized};

/// What each region kind is called at the top of the panel.
fn kind_title(kind: &str) -> &'static str {
    match kind {
        "zoomRegions" => "Zoom region",
        "trimRegions" => "Trim region",
        "speedRegions" => "Speed region",
        "clipRegions" => "Clip",
        "annotationRegions" => "Annotation",
        "audioRegions" => "Audio region",
        "autoCaptions" => "Caption",
        "nativeMarkers" => "Marker",
        _ => "Region",
    }
}

/// The zoom sizes a zoom region's depth steps through, as the renderer
/// has them (`src/timeline.rs`).
const ZOOM_LEVELS: [&str; 6] = ["125%", "150%", "180%", "220%", "350%", "500%"];

impl RootView {
    fn timecode(
        &mut self,
        id: &str,
        label: &str,
        value: f64,
        range: (f64, f64),
        cx: &mut Context<Self>,
        change: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Entity<TimecodeField> {
        let theme = self.theme;
        let field = self
            .timecodes
            .entry(id.to_owned())
            .or_insert_with(|| cx.new(|cx| TimecodeField::new(cx, value, theme, |_, _, _| {})))
            .clone();
        field.update(cx, |f, _| {
            f.set_handler(change);
            f.label = label.to_owned().into();
            f.theme = theme;
            (f.minimum, f.maximum) = range;
            f.sync(value);
        });
        field
    }

    pub(super) fn selection_panel(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let theme = self.theme;
        let id = e.get_selected_id();
        let region = (!id.is_empty())
            .then(|| e.get_regions().iter().find(|r| r.selected && r.id == id))
            .flatten();
        let Some(region) = region else {
            return (
                row()
                    .h(px(Theme::CONTROL_HEIGHT_SMALL))
                    .flex_none()
                    .child(title("Selection", Theme::FONT_HEADING)),
                nothing_selected(theme),
            );
        };

        let heading = row()
            .h(px(Theme::CONTROL_HEIGHT_SMALL))
            .flex_none()
            .gap(px(Theme::ICON_GAP_ROW))
            .child(
                div()
                    .flex_none()
                    .size(px(Theme::SELECTION_SWATCH))
                    .rounded(px(Theme::SELECTION_SWATCH_RADIUS))
                    .bg(region.tint.to_gpui()),
            )
            .child(title(kind_title(&region.kind), Theme::FONT_HEADING).flex_1())
            .child(
                icon_button("selection-close", "X-regular", "Deselect", theme)
                    .small()
                    .on_click(self.command("deselect")),
            );

        let fields: Vec<Field> = e.get_fields().iter().collect();
        let value = |key: &str| {
            fields
                .iter()
                .find(|f| f.key == key)
                .map(|f| f.value.to_string())
                .unwrap_or_default()
        };
        let start = value("region.startMs").parse::<f64>().unwrap_or(0.);
        let end = value("region.endMs").parse::<f64>().unwrap_or(start);
        let duration = e.get_duration() as f64 * 1000.;

        let editor = e.clone();
        let start_field = self.timecode(
            "selection-start",
            "Start",
            start,
            (0., end),
            cx,
            move |v, _, _| editor.defer_field("region.startMs".into(), v.round().to_string()),
        );
        let editor = e.clone();
        let end_field = self.timecode(
            "selection-end",
            "End",
            end,
            (start, duration.max(start)),
            cx,
            move |v, _, _| editor.defer_field("region.endMs".into(), v.round().to_string()),
        );
        let mut content = column()
            .gap(px(Theme::GAP_LARGE))
            .child(caps_label("Timing", theme))
            .child(
                row()
                    .gap(px(Theme::GAP))
                    .child(start_field)
                    .child(end_field),
            )
            .child(
                row()
                    .justify_between()
                    .px(px(Theme::GAP_SMALL))
                    .text_size(px(Theme::FONT_SECONDARY))
                    .text_color(theme.muted)
                    .child("Duration")
                    .child(mono(format!("{:.1} s", (end - start).max(0.) / 1000.))),
            );

        // The keys this panel draws itself; any other field the kind has is
        // listed after them.
        let mut drawn = vec!["region.startMs", "region.endMs"];
        if region.kind == "zoomRegions" {
            drawn.extend(["region.depth", "region.mode"]);
            let depth = value("region.depth").parse::<f32>().unwrap_or(3.);
            let editor = e.clone();
            let level = self.slider(
                "Selection:region.depth",
                0.,
                (ZOOM_LEVELS.len() - 1) as f32,
                (depth.round() - 1.).clamp(0., 5.),
                ("Zoom level", ""),
                (1., ""),
                cx,
                move |v, commit, _, _| {
                    if commit {
                        editor.defer_field("region.depth".into(), (v.round() + 1.).to_string());
                    }
                },
            );
            level.update(cx, |s, _| {
                s.steps = ZOOM_LEVELS.iter().map(|l| SharedString::from(*l)).collect()
            });
            let editor = e.clone();
            // Not wired: the handoff's Ease / Linear / Spring. A zoom has one
            // curve, the motion preset's, so there is nothing per region for
            // the three to choose between and the row is left out.
            content = content
                .child(caps_label("Motion", theme))
                .child(level)
                .child(toggle(
                    "Selection:region.mode",
                    "Follow cursor",
                    value("region.mode") != "manual",
                    true,
                    theme,
                    move |v, _, _| {
                        editor.defer_field(
                            "region.mode".into(),
                            if v { "auto" } else { "manual" }.into(),
                        )
                    },
                ));
        }
        // Not drawn by the design: every row but timing for the other kinds,
        // and a zoom's focus point. The handoff draws a zoom region only;
        // the rest keep the rows they had, on the new tokens.
        for field in fields {
            if drawn.contains(&field.key.as_str()) {
                continue;
            }
            content = content.child(self.field(e, field, window, cx));
        }

        content = content.child(divider(theme)).child(
            div()
                .id("delete-region")
                .tab_index(0)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(Theme::ICON_GAP_ROW))
                .h(px(Theme::CONTROL_HEIGHT_LARGE))
                .rounded_full()
                .cursor_pointer()
                .text_color(theme.danger)
                .hover(|s| s.bg(theme.hover))
                .active(|s| s.bg(theme.press))
                .focus_visible(move |s| s.shadow(vec![focus_ring(theme)]))
                .child(icon_sized("X-regular", Theme::ICON_SIZE, theme.danger))
                .child("Delete region")
                .child(
                    mono("⌫")
                        .text_size(px(Theme::FONT_SMALL))
                        .text_color(theme.muted),
                )
                .on_click(self.command("delete")),
        );
        (heading, content)
    }
}

/// Nothing is selected: a plate, a line, and where to click.
fn nothing_selected(theme: Theme) -> Div {
    column()
        .items_center()
        .justify_center()
        .gap(px(Theme::GAP))
        .min_h(px(Theme::SELECTION_EMPTY_HEIGHT))
        .py(px(Theme::SELECTION_EMPTY_PADDING_Y))
        .px(px(Theme::SELECTION_EMPTY_PADDING_X))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(Theme::CONTROL_HEIGHT_HERO))
                .rounded_full()
                .bg(theme.sunk)
                .child(icon_sized(
                    "Selection-regular",
                    Theme::SELECTION_EMPTY_ICON,
                    theme.muted,
                )),
        )
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child("Nothing selected"),
        )
        .child(
            div()
                .max_w(px(Theme::SELECTION_EMPTY_TEXT_WIDTH))
                .text_center()
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted)
                .child("Click a region in the timeline to edit it."),
        )
}
