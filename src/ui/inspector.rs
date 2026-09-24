//! The inspector panel: every field kind and the panels built from them.

use super::preview::{inspector_collapsed, macos_cursor_image};
use super::*;

/// The glyph a numeric field wears in its scrub plate. Keyed on the field so
/// the inspector reads as a set of labelled dials rather than a list of rows.
/// How a slider's number should read. The model carries the raw value, so the
/// unit is presentation: a 0-1 factor reads as a percentage, a multiplier as
/// a cross, a length in points. Returned as (scale, suffix); anything not
/// listed keeps the bare number it has today.
pub(super) fn field_unit(key: &str) -> (f32, &'static str) {
    // An annotation is placed and sized in percent of the picture.
    if key.starts_with("region.position.") || key.starts_with("region.size.") {
        return (1.0, "%");
    }
    if key == "region.style.padding" {
        return (1.0, " px");
    }
    match key.rsplit('.').next().unwrap_or(key) {
        "fontSize" | "strokeWidth" => (1.0, " px"),
        "dimOpacity" => (1.0, "%"),
        "borderRadius" | "backgroundBlur" | "margin" => (1.0, " px"),
        "cursorSize" | "cursorSmoothing" | "cursorSway" | "cursorMotionBlur"
        | "cursorClickBounce" | "depth" | "speed" | "volume" => (1.0, "\u{00d7}"),
        // Roundness is already carried as 0-100, so it takes the suffix
        // without the rescale the other factors need.
        "roundness" => (1.0, "%"),
        "zoomSmoothness" | "shadowIntensity" | "shadow" | "cx" | "cy" => (100.0, "%"),
        _ => (1.0, ""),
    }
}

/// A cursor style as its tile draws it.
struct CursorTile {
    value: &'static str,
    label: &'static str,
    asset: &'static str,
    /// The file's canvas, and the box inside it the cursor is actually
    /// drawn in. The files pad their cursors very differently — the Windows
    /// arrow fills a third of its canvas, the Tahoe one nearly all of it —
    /// so a tile sizes and centres the drawn box, not the canvas.
    canvas: (f32, f32),
    ink: (f32, f32, f32, f32),
    /// How tall the drawn cursor is in its tile. Close together, so the row
    /// reads as one set, but the styles that are smaller on screen stay a
    /// little smaller here.
    height: f32,
}

/// The lightness above which a colour swatch's check is drawn dark.
const LIGHT_SWATCH: f32 = 0.6;

const CURSOR_STYLES: [CursorTile; 5] = [
    CursorTile {
        value: "tahoe",
        label: "Tahoe",
        asset: "legacy-electron/src/assets/cursors/tahoe/pointer-1__14-6.svg",
        canvas: (618., 958.),
        ink: (35., 24., 544., 895.),
        height: 30.,
    },
    CursorTile {
        value: "macos",
        label: "macOS",
        asset: "",
        canvas: (768., 746.),
        ink: (252., 179., 259., 413.),
        height: 26.,
    },
    CursorTile {
        value: "windows11",
        label: "Windows",
        asset: "legacy-electron/src/assets/cursors/windows11/arrow__31-22.svg",
        canvas: (32., 32.),
        ink: (0.76, 0.21, 13.18, 19.04),
        height: 26.,
    },
    CursorTile {
        value: "dot",
        label: "Dot",
        asset: "assets/icons/Record-fill.svg",
        canvas: (24., 24.),
        ink: (2.25, 2.25, 19.5, 19.5),
        height: 20.,
    },
    CursorTile {
        value: "figma",
        label: "Minimal",
        asset: "legacy-electron/src/assets/cursors/custom/minimal-cursor.svg",
        canvas: (396., 433.),
        ink: (35., 10., 335., 378.),
        height: 28.,
    },
];

impl CursorTile {
    /// The drawn cursor at its tile height: the whole canvas scaled, offset
    /// so the drawn box sits at the origin, and clipped to that box.
    fn glyph(&self, theme: Theme) -> Div {
        let scale = self.height / self.ink.3;
        let (w, h) = (self.canvas.0 * scale, self.canvas.1 * scale);
        let picture = if self.value == "macos" {
            img(macos_cursor_image())
                .w(px(w))
                .h(px(h))
                .object_fit(ObjectFit::Fill)
                .into_any_element()
        } else {
            svg()
                .path(self.asset)
                .w(px(w))
                .h(px(h))
                .text_color(theme.text)
                .into_any_element()
        };
        div()
            .relative()
            .flex_none()
            .overflow_hidden()
            .w(px(self.ink.2 * scale))
            .h(px(self.height))
            .child(
                div()
                    .absolute()
                    .left(px(-self.ink.0 * scale))
                    .top(px(-self.ink.1 * scale))
                    .child(picture),
            )
    }
}

impl RootView {
    /// A tool-pod entry: one round button, accent-filled while its panel is
    /// the open one.
    pub(super) fn rail_panel_button(
        &self,
        editor: &EditorWindow,
        label: &str,
        name: &str,
        glyph: &str,
    ) -> AnyElement {
        let e = editor.clone();
        let target = name.to_owned();
        // A command name (it has a dash) runs rather than opening a panel.
        let is_panel = !target.contains('-');
        let active = is_panel && editor.get_panel() == target;
        let surface = self.surface.clone();
        let caption = editor.invoke_translate(label.into(), editor.get_language());
        let caption = if caption.is_empty() {
            label.to_owned()
        } else {
            caption
        };
        tool_button(
            SharedString::from(format!("rail-{target}")),
            glyph,
            caption,
            active,
            self.theme,
            move |_, _, _| {
                if is_panel {
                    // While the inspector is folded, a tool slides it in and
                    // the tool already showing slides it back out.
                    let open = !(e.get_panel() == target && e.get_inspector_open());
                    e.set_inspector_open(open);
                    e.set_panel(target.clone());
                    e.defer_panel(target.clone());
                } else {
                    surface.action(&target);
                }
            },
        )
        .into_any_element()
    }

    pub(super) fn panel_button(&self, editor: &EditorWindow, label: &str, name: &str) -> Button {
        let e = editor.clone();
        let name = name.to_owned();
        button(
            SharedString::from(format!("panel-{name}-{label}")),
            self.translate(editor, label),
            self.theme,
        )
        .selected(editor.get_panel() == name)
        .on_click(move |_, _, _| {
            e.set_panel(name.clone());
            e.defer_panel(name.clone());
        })
    }

    pub(super) fn input(
        &mut self,
        id: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        accept: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Entity<TextInput> {
        let theme = self.theme;
        let accept = Rc::new(accept);
        let initial = accept.clone();
        let input = self
            .inputs
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    TextInput::new(cx, value.to_owned(), theme, move |v, w, cx| {
                        initial(v, w, cx)
                    })
                })
            })
            .clone();
        input.update(cx, |s, _| {
            s.set_handler(move |v, w, cx| accept(v, w, cx));
            s.theme = theme;
            s.sync(value, window);
        });
        input
    }

    pub(super) fn dropdown(
        &mut self,
        id: &str,
        items: Vec<String>,
        selected: i32,
        enabled: bool,
        cx: &mut Context<Self>,
        change: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Entity<Dropdown> {
        let theme = self.theme;
        let change = Rc::new(change);
        let initial = change.clone();
        let control = self
            .dropdowns
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    let mut control = Dropdown::new(
                        cx,
                        items.clone(),
                        selected.max(0) as usize,
                        theme,
                        move |v, w, cx| initial(v, w, cx),
                    );
                    // The gallery can open one by id, as the pointer would.
                    if std::env::var("SUBTAKE_GALLERY_OPEN").is_ok_and(|open| open == id) {
                        control.show();
                    }
                    control
                })
            })
            .clone();
        control.update(cx, |s, _| {
            s.set_handler(move |v, w, cx| change(v, w, cx));
            s.items = items;
            s.selected = selected.max(0) as usize;
            s.enabled = enabled;
            s.theme = theme;
        });
        control
    }

    pub(super) fn slider(
        &mut self,
        id: &str,
        minimum: f32,
        maximum: f32,
        value: f32,
        caption: (&str, &str),
        unit: (f32, &str),
        cx: &mut Context<Self>,
        change: impl Fn(f32, bool, &mut Window, &mut App) + 'static,
    ) -> Entity<Slider> {
        let theme = self.theme;
        let change = Rc::new(change);
        let initial = change.clone();
        let control = self
            .sliders
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|_| {
                    Slider::new(minimum, maximum, value, theme, move |v, commit, w, cx| {
                        initial(v, commit, w, cx)
                    })
                })
            })
            .clone();
        control.update(cx, |s, _| {
            s.set_handler(move |v, commit, w, cx| change(v, commit, w, cx));
            s.minimum = minimum;
            s.maximum = maximum;
            s.sync(value);
            s.theme = theme;
            s.set_caption(caption.0.to_owned(), caption.1.to_owned());
            s.set_unit(unit.0, unit.1.to_owned());
            // Cached across renders, so a row disabled last frame is live
            // again unless its caller says otherwise this one.
            s.enabled = true;
        });
        control
    }

    pub(super) fn field(
        &mut self,
        e: &EditorWindow,
        field: Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let key = field.key.clone();
        let id = format!("{}:{}", e.get_panel(), key);
        let label = e.invoke_translate(field.label.clone(), e.get_language());
        let label = if label.is_empty() {
            field.label.clone()
        } else {
            label
        };
        let mut body = column().gap(px(Theme::gap_small()));
        if key == "cursorStyle" {
            // Three across, the last row centred: five in a grid of four left
            // a lone tile hanging under an even row. Each tile spans two of
            // six columns, so the last row can start half a tile in.
            let columns = Theme::cursor_tile_columns() as u16;
            let full_rows = CURSOR_STYLES.len() / columns as usize * columns as usize;
            let short = (CURSOR_STYLES.len() - full_rows) as i16;
            let mut choices = tile_grid(columns * 2);
            for (index, style) in CURSOR_STYLES.iter().enumerate() {
                let editor = e.clone();
                let value = style.value;
                let label = style.label;
                let mut tile = choice_tile(value, field.value == value, true, theme)
                    .items_center()
                    .justify_center()
                    .col_span(2)
                    .h(px(Theme::cursor_tile_height()))
                    .child(style.glyph(theme))
                    .tooltip(move |_, cx| tooltip(label, theme, cx))
                    .on_click(move |_, _, _| {
                        editor.defer_field("cursorStyle".into(), value.into())
                    });
                if index == full_rows {
                    tile = tile.col_start(columns as i16 - short + 1);
                }
                choices = choices.child(tile);
            }
            body = body.child(caps_label(label, theme)).child(choices);
            // Not drawn by the design: a style the project names that has no
            // tile — one from a newer build, or written by hand — so it says
            // which, rather than showing no tile picked and nothing else.
            if !CURSOR_STYLES.iter().any(|style| style.value == field.value) {
                body = body.child(
                    div()
                        .px(px(Theme::gap_small()))
                        .text_size(px(Theme::font_secondary()))
                        .text_color(theme.muted)
                        .child(format!("Using “{}”, which has no tile here", field.value)),
                );
            }
            return body.into_any_element();
        }
        if key == "webcam.positionPreset" {
            // Nine cells, three across — the grid IS the picture of where the
            // webcam lands, which is why the marker stays a dot rather than an
            // arrow glyph the icon set does not carry.
            let mut choices = tile_grid(3);
            for (i, (value, name)) in [
                ("top-left", "Top left"),
                ("top-center", "Top"),
                ("top-right", "Top right"),
                ("center-left", "Left"),
                ("center", "Center"),
                ("center-right", "Right"),
                ("bottom-left", "Bottom left"),
                ("bottom-center", "Bottom"),
                ("bottom-right", "Bottom right"),
            ]
            .into_iter()
            .enumerate()
            {
                let editor = e.clone();
                choices = choices.child(
                    choice_tile(value, field.value == value, true, theme)
                        .h(px(Theme::control_height()))
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .h_full()
                                .map(|el| match i % 3 {
                                    0 => el.justify_start(),
                                    1 => el.justify_center(),
                                    _ => el.justify_end(),
                                })
                                .map(|el| match i / 3 {
                                    0 => el.items_start(),
                                    1 => el.items_center(),
                                    _ => el.items_end(),
                                })
                                .child(
                                    div()
                                        .size(px(Theme::position_dot_size()))
                                        .rounded_full()
                                        .bg(if field.value == value {
                                            theme.accent
                                        } else {
                                            theme.muted
                                        }),
                                ),
                        )
                        .tooltip(move |_, cx| tooltip(name, theme, cx))
                        .on_click(move |_, _, _| {
                            editor.defer_field("webcam.positionPreset".into(), value.into())
                        }),
                );
            }
            let editor = e.clone();
            return body
                .child(caps_label(label, theme))
                .child(choices)
                .child(
                    button("custom-position", "Custom position", theme)
                        .selected(field.value == "custom")
                        .on_click(move |_, _, _| {
                            editor.defer_field("webcam.positionPreset".into(), "custom".into())
                        }),
                )
                .into_any_element();
        }
        match field.kind {
            6 => {
                // A colour: the common ones as swatches, the first "none",
                // then a field that takes any other as hex.
                let mut swatches = row().gap(px(Theme::gap_small()));
                for value in crate::annotations::COLOURS {
                    let [r, g, b, a] = crate::project::parse_color(value);
                    let colour: Hsla = rgba(u32::from_be_bytes([r, g, b, a])).into();
                    let editor = e.clone();
                    let key = key.clone();
                    let picked = field.value.eq_ignore_ascii_case(value);
                    let mut tile = swatch(format!("{id}:{value}"), colour, picked, theme)
                        .on_click(move |_, _, _| editor.defer_field(key.clone(), value.into()));
                    // The accent ring is lost on a blue swatch, so the picked
                    // one also carries a check, dark on light colours.
                    let mark = if picked {
                        let ink = if colour.l > LIGHT_SWATCH {
                            black()
                        } else {
                            white()
                        };
                        Some(("Check-regular", if a == 0 { theme.accent } else { ink }))
                    } else if a == 0 {
                        Some(("X-regular", theme.muted))
                    } else {
                        None
                    };
                    if let Some((glyph, ink)) = mark {
                        tile =
                            tile.flex().items_center().justify_center().child(
                                subtake_ui::icon_sized(glyph, Theme::icon_size_small(), ink),
                            );
                    }
                    swatches = swatches.child(tile);
                }
                let editor = e.clone();
                let input = self.input(&id, &field.value, window, cx, move |v, _, _| {
                    editor.defer_field(key.clone(), v)
                });
                // One setting, so one plate under its own quiet label, as a
                // group of related controls is drawn, not a caps caption
                // that reads as a new section.
                return group_card(theme, label)
                    .child(swatches)
                    .child(input)
                    .into_any_element();
            }
            5 => {
                // A small muted caption. The handoff draws it bare, with no
                // rule running out to the edge.
                return caps_label(label, theme)
                    .mt(px(Theme::gap_small()))
                    .into_any_element();
            }
            3 => {
                // An inspector action is a row in a stacked picker, not a
                // toolbar control, so it carries the recessed field plate.
                return self
                    .action(SharedString::from(id), field.value, &key, true)
                    .into_any_element();
            }
            2 => {
                let e = e.clone();
                return toggle(
                    SharedString::from(id),
                    label,
                    field.value == "true",
                    true,
                    theme,
                    move |v, _, _| e.defer_field(key.clone(), v.to_string()),
                )
                .into_any_element();
            }
            4 => {
                // The model remains authoritative, including custom cursor/position choices.
                let e = e.clone();
                let values: Vec<_> = field.values.iter().collect();
                let control = self.dropdown(
                    &id,
                    field.choices.iter().collect(),
                    field.choice,
                    true,
                    cx,
                    move |i, _, _| {
                        if let Some(v) = values.get(i) {
                            e.defer_field(key.clone(), v.clone());
                        }
                    },
                );
                // One row, not two: a label stacked over its control reads
                // as a heading plus a thing, when it is one setting. The
                // label takes a fixed third, so a stack of these starts its
                // controls in one column rather than wherever each label
                // happens to end.
                body = body.child(
                    row()
                        .h(px(Theme::control_height()))
                        .child(
                            div()
                                .flex_none()
                                .w(px(PANEL_WIDTH / 3.0))
                                .text_ellipsis()
                                .text_color(theme.text)
                                .child(label),
                        )
                        .child(div().flex_1().min_w_0().child(control)),
                );
            }
            1 => {
                let e1 = e.clone();
                let k1 = key.clone();
                let min = field.minimum;
                let max = field.maximum;
                let _ = (&e1, &k1);
                let e2 = e.clone();
                // One control, not three: the plate carries the caption, the
                // level and the value together (the product's "unified
                // control geometry"). No glyph -- a stack of these is a list
                // of settings, and a pictogram on every row reads as
                // decoration rather than as information.
                let scrub = self.slider(
                    &id,
                    min,
                    max,
                    field.value.parse().unwrap_or(min),
                    (&label, ""),
                    field_unit(&key),
                    cx,
                    move |v, commit, _, _| {
                        if commit {
                            e2.defer_field(key.clone(), v.to_string());
                        }
                    },
                );
                body = body.child(scrub);
            }
            _ => {
                let e = e.clone();
                let input = self.input(&id, &field.value, window, cx, move |v, _, _| {
                    e.defer_field(key.clone(), v)
                });
                body = body.child(
                    row()
                        .h(px(Theme::control_height()))
                        .child(
                            div()
                                .flex_none()
                                .w(px(PANEL_WIDTH / 3.0))
                                .text_ellipsis()
                                .text_color(theme.text)
                                .child(label),
                        )
                        .child(div().flex_1().min_w_0().child(input)),
                );
            }
        }
        body.into_any_element()
    }

    pub(super) fn inspector(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let name = e.get_panel();
        let shown = match name.as_str() {
            "Frame" => "Scene",
            "Preferences" => "Settings",
            "Recent" => "Projects",
            "Wallpapers" => "Background",
            _ => &name,
        };
        // Every panel names itself as round 2 draws Scene: the name in Space
        // Grotesk 19 and a close control. A sub-panel keeps its way back, a
        // caret before the name.
        let mut heading = row()
            .h(px(Theme::control_height_small()))
            .flex_none()
            .gap(px(Theme::icon_gap_row()));
        if let Some(back) = sub_panel_of(&name) {
            let editor = e.clone();
            heading = heading.child(
                icon_button("inspector-back", "CaretLeft-regular", "Back", theme)
                    .ghost()
                    .small()
                    .on_click(move |_, _, _| {
                        editor.set_panel(back.into());
                        editor.defer_panel(back.into());
                    }),
            );
        }
        heading = heading.child(title(shown.to_owned(), Theme::font_heading()).flex_1());
        // Folded, every panel's close slides the inspector back out and
        // leaves the panel as it was, for the toggle to bring back.
        //
        // Not wired: the handoff's close on Scene while the inspector sits
        // beside the stage. It always shows a panel there and Scene is where
        // closing lands, so with a video open Scene and Background have no
        // close. Over the empty state, with no rail to pick another panel
        // from, every panel keeps one.
        let collapsed = inspector_collapsed(window);
        let root = matches!(name.as_str(), "Frame" | "Wallpapers");
        if collapsed || !root || !e.get_has_video() {
            let editor = e.clone();
            heading = heading.child(
                icon_button("inspector-close", "X-regular", "Close", theme)
                    .small()
                    .on_click(move |_, _, _| {
                        if collapsed {
                            editor.set_inspector_open(false);
                        } else {
                            editor.set_panel("Frame".into());
                            editor.defer_panel("Frame".into());
                        }
                    }),
            );
        }
        let mut content = column().gap(px(Theme::gap()));
        if name == "Frame" || name == "Wallpapers" {
            // Scene / Background is one segmented control, not two buttons.
            let editor = e.clone();
            let surface = self.surface.clone();
            content = content.child(segmented_control(
                "scene-background",
                &["Scene", "Background"],
                if name == "Wallpapers" { 1 } else { 0 },
                theme,
                move |index, _, _| {
                    if index == 0 {
                        editor.set_panel("Frame".into());
                        editor.defer_panel("Frame".into());
                    } else {
                        surface.action("wallpapers");
                    }
                },
            ));
        }
        if name == "Recent" {
            content = content
                .child(
                    self.action(
                        "storyboard",
                        "Create video · spike",
                        "storyboard-spike",
                        true,
                    )
                    .primary(),
                )
                .child(self.action("import", "Import video or project", "open", true));
        }
        if name == "Wallpapers" {
            let mut wallpapers = row().flex_wrap();
            for tile in e.get_wallpapers().iter() {
                let editor = e.clone();
                let key = tile.key.clone();
                let item = media_tile(
                    SharedString::from(format!("wallpaper-{key}")),
                    tile.title,
                    tile.source.0.map(img),
                    tile.value == e.get_background_value(),
                    theme,
                );
                wallpapers = wallpapers
                    .child(item.on_click(move |_, _, _| editor.defer_action(key.clone())));
            }
            content = content
                .child(caps_label("Choose a background", theme))
                .child(wallpapers)
                .child(self.action(
                    "upload-background",
                    "Upload image or video",
                    "choose-background",
                    true,
                ));
            let mut swatches = row();
            for value in [
                "#17171c", "#22364a", "#253c32", "#54324a", "#784832", "#ededed",
            ] {
                let editor = e.clone();
                swatches = swatches.child(
                    swatch(
                        value,
                        rgb(u32::from_str_radix(&value[1..], 16).unwrap()).into(),
                        e.get_background_value() == value,
                        theme,
                    )
                    .on_click(move |_, _, _| editor.defer_field("wallpaper".into(), value.into())),
                );
            }
            let editor = e.clone();
            let input = self.input(
                "wallpaper-color",
                &e.get_background_value(),
                window,
                cx,
                move |v, _, _| editor.defer_field("wallpaper".into(), v),
            );
            content = content.child(
                group_card(theme, "Background color or gradient")
                    .child(swatches)
                    .child(input),
            );
        }
        if name == "Preferences" {
            // Appearance is one choice of three, so it is one segmented
            // control rather than three buttons that happen to sit together.
            let editor = e.clone();
            let appearances = ["light", "dark", "system"];
            let chosen = appearances
                .iter()
                .position(|v| *v == e.get_appearance())
                .unwrap_or(2);
            let e1 = e.clone();
            let e2 = e.clone();
            content = content
                .child(caps_label("Appearance", theme))
                .child(segmented_control(
                    "appearance",
                    &["Light", "Dark", "System"],
                    chosen,
                    theme,
                    move |index, _, _| {
                        editor.defer_field("prefs.appearance".into(), appearances[index].to_owned())
                    },
                ))
                .child(caps_label("Zooms", theme))
                // A setting and the sentence that explains it are one thing,
                // so they share one plate. Loose muted lines under a control
                // read as unattached commentary.
                .child(
                    setting_card(
                        theme,
                        "Automatic recording zooms",
                        "Suggest zooms when a new recording opens.",
                    )
                    .child(switch(
                        "auto-zooms",
                        e.get_auto_apply_zooms(),
                        true,
                        theme,
                        move |v, _, _| {
                            e1.defer_field("prefs.auto_apply_zooms".into(), v.to_string())
                        },
                    )),
                )
                .child(
                    setting_card(
                        theme,
                        "Connect zooms",
                        "Join nearby zooms into a continuous camera move.",
                    )
                    .child(switch(
                        "connect-zooms",
                        e.get_connect_zooms(),
                        e.get_has_video(),
                        theme,
                        move |v, _, _| e2.defer_field("connectZooms".into(), v.to_string()),
                    )),
                );
        }
        if name == "Preferences" {
            // A preset is a choice you make once and live with, so it states
            // what it does rather than making the name carry it alone.
            let mut presets = tile_grid(2);
            for (value, title, glyph, detail) in [
                (
                    "focused",
                    "Focused",
                    "Cursor-regular",
                    "Snappier motion for demos, walkthroughs and everyday recordings.",
                ),
                (
                    "smooth",
                    "Smooth",
                    "FilmStrip-regular",
                    "Gentler motion for presentations, keynote-style videos and polished reveals.",
                ),
            ] {
                let surface = self.surface.clone();
                let command = format!("motion-{value}");
                presets = presets.child(
                    choice_tile(
                        value,
                        e.get_motion_choice() == value,
                        e.get_has_video(),
                        theme,
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_none()
                            .size(px(Theme::control_height()))
                            .rounded(px(Theme::radius_lane()))
                            .bg(theme.sunk)
                            .child(icon(glyph, theme.text)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_small()))
                            .text_color(theme.muted)
                            .child(detail),
                    )
                    .on_click(move |_, _, _| surface.action(&command)),
                );
            }
            content = content
                .child(caps_label("Motion presets", theme))
                .child(presets);
            if !e.get_has_video() {
                content = content.child(
                    div()
                        .text_color(theme.muted)
                        .child("Open a project to adjust its motion."),
                );
            }
        }
        if name == "Recording" {
            let editor = e.clone();
            let source = self.dropdown(
                "editor-source",
                e.get_source_names().iter().collect(),
                e.get_source_index(),
                !e.get_busy(),
                cx,
                move |i, _, _| editor.set_source_index(i as i32),
            );
            let editor = e.clone();
            let camera = self.dropdown(
                "editor-camera",
                e.get_camera_names().iter().collect(),
                e.get_camera_index(),
                e.get_capture_camera(),
                cx,
                move |i, _, _| editor.set_camera_index(i as i32),
            );
            let editor = e.clone();
            let microphone = self.dropdown(
                "editor-microphone",
                e.get_microphone_names().iter().collect(),
                e.get_microphone_index(),
                e.get_capture_mic(),
                cx,
                move |i, _, _| editor.set_microphone_index(i as i32),
            );
            let e1 = e.clone();
            let e2 = e.clone();
            let e3 = e.clone();
            content = content
                .child(caps_label("Capture source", theme))
                .child(source)
                .child(self.action(
                    "sources",
                    if e.get_sources_loading() {
                        "Finding sources…"
                    } else {
                        "Refresh displays and windows"
                    },
                    "sources",
                    !e.get_busy(),
                ))
                // Eight controls in one flat stack gave no clue which
                // dropdown belonged to which switch. They are groups now.
                .child(
                    group_card(theme, "Camera")
                        .child(toggle(
                            "capture-camera",
                            "Record camera",
                            e.get_capture_camera(),
                            true,
                            theme,
                            move |v, _, _| e1.set_capture_camera(v),
                        ))
                        .child(camera),
                )
                .child(
                    group_card(theme, "Audio")
                        .child(toggle(
                            "capture-mic",
                            "Microphone",
                            e.get_capture_mic(),
                            true,
                            theme,
                            move |v, _, _| e2.set_capture_mic(v),
                        ))
                        .child(microphone)
                        .child(toggle(
                            "capture-system",
                            "System audio",
                            e.get_capture_system(),
                            true,
                            theme,
                            move |v, _, _| e3.set_capture_system(v),
                        )),
                );
        }
        if !matches!(
            name.as_str(),
            "Export" | "Selection" | "Cursor" | "Webcam" | "Add"
        ) {
            for field in e.get_fields().iter() {
                // A section named after the panel only repeats its title.
                if field.kind == 5 && field.label.eq_ignore_ascii_case(shown) {
                    continue;
                }
                content = content.child(self.field(e, field, window, cx));
            }
        }
        // Only these panels pin an action strip under the scroll region. An
        // always-present empty column still cost the panel's gap plus its
        // bottom padding, which is what left the dead band under the last row.
        let has_footer = matches!(
            name.as_str(),
            "Preferences" | "Recording" | "Export" | "Captions"
        );
        let mut footer = column();
        // TODO(redesign): every arm below belongs to a panel the handoff does
        // not draw (see the panel list in `src/app/playback.rs`). The action
        // strip is the one piece of per-panel layout in the inspector, so
        // this match is where an answer about those panels will land — the
        // rest of a panel is whatever fields the model hands over. Until
        // then each arm keeps the strip it has, on the new tokens.
        match name.as_str() {
            "Preferences" => {
                footer =
                    footer.child(self.panel_button(e, "Customize keyboard shortcuts…", "Shortcuts"))
            }
            "Recording" => {
                footer = footer.child(e.get_recording_hint()).child(
                    self.action(
                        "start-recording",
                        if e.get_recording() {
                            "Stop recording"
                        } else {
                            "Start recording"
                        },
                        if e.get_recording() {
                            "stop-recording"
                        } else {
                            "start-recording"
                        },
                        !e.get_busy()
                            && (e.get_recording() || e.get_source_names().row_count() > 0),
                    )
                    .primary(),
                )
            }
            "Selection" => (heading, content) = self.selection_panel(e, window, cx),
            "Cursor" => (heading, content) = self.cursor_panel(e, window, cx),
            "Webcam" => (heading, content) = self.camera_panel(e, window, cx),
            "Add" => (heading, content) = self.add_panel(e),
            "Export" => {
                let (h, c, f) = self.export_panel(e, cx);
                heading = h;
                content = c;
                footer = f;
            }
            "Captions" => {
                footer = footer.child(
                    row()
                        .child(self.action("import-srt", "Import SRT", "import-captions", true))
                        .child(
                            self.action("transcribe", "Transcribe", "transcribe", true)
                                .primary(),
                        ),
                )
            }
            _ => {}
        }
        // The scroll region runs to the panel's inner top and bottom edges so
        // the fade ramp sits exactly on the clip line; the panel's vertical
        // padding moves onto the heading and footer instead of stacking with
        // the band and pushing the first row down.
        //
        // The band is the handoff's 12 between a title and the first row, and
        // between the last row and the footer, so the panel's own block gap
        // is taken out rather than added to it.
        let scroll = self
            .inspector_scroll
            .entry(name.to_string())
            .or_default()
            .clone();
        let mut body = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            // Padding on the fixed-height title row would come out of its
            // height and push the close control up past the panel's edge; on
            // a wrapper it sits 18 down, as far as it sits from the side.
            .child(
                div()
                    .flex_none()
                    .pt(px(Theme::panel_padding()))
                    .child(heading),
            )
            .child(
                // Each edge fades only by what is scrolled out past it: a
                // panel that fits is never dimmed, and a row the edge cuts
                // dissolves over the longer band instead of hanging there as
                // a half-faded copy of itself.
                fade_edges(
                    div()
                        .id(SharedString::from(format!("inspector-{name}")))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .pt(px(Theme::gap_large()))
                        .pb(px(if has_footer {
                            Theme::gap_large()
                        } else {
                            Theme::panel_padding()
                        }))
                        .child(content),
                )
                .band(Theme::scroll_fade_band())
                .tracking(&scroll)
                .bottom(has_footer)
                .thumb(theme.muted.opacity(0.5), Theme::panel_padding()),
            );
        // Without a footer the scroll region runs to the glass's bottom edge
        // and cuts there, unfaded: the edge of the card is the clip line, and
        // the content's own padding keeps the last row the panel's padding
        // clear of it at rest. Above a footer it still fades into the footer.
        if has_footer {
            body = body.child(footer.pb(px(Theme::panel_padding())));
        }
        // A newly picked panel fades in and rises into place; the card
        // around it stays put. Keyed by the panel, so each pick starts over.
        // Into a sub-panel it slides in from the right instead, and back out
        // from the left, so the caret's way back reads as a way back.
        if self.panel_drill.0 != name {
            let from = std::mem::replace(&mut self.panel_drill.0, name.clone());
            self.panel_drill.1 = if sub_panel_of(&name) == Some(&*from) {
                1.
            } else if sub_panel_of(&from) == Some(&*name) {
                -1.
            } else {
                0.
            };
        }
        let drill = self.panel_drill.1;
        let ms = if drill == 0. {
            PANEL_ENTER_MS
        } else {
            PANEL_DRILL_MS
        };
        // The old panel is gone the frame the new one arrives, and gpui
        // draws an animation's first frame at its start, so a fade from
        // nothing left the card empty for a frame — 33ms while the editor is
        // in the background. It starts a 60Hz frame in instead.
        let lead = 1000. / 60. / ms as f32;
        let enter = Animation::new(std::time::Duration::from_millis(ms))
            .with_easing(move |t| subtake_ui::motion::EASE_OUT.eval(lead + (1. - lead) * t));
        let el = panel(theme)
            .py_0()
            .gap_0()
            .size_full()
            .min_h_0()
            .child(body.with_animation(
                SharedString::from(format!("panel-enter-{name}")),
                enter,
                move |body, t| {
                    let body = body.opacity(t);
                    if drill == 0. {
                        body.top(px(PANEL_ENTER_RISE * (1. - t)))
                    } else {
                        body.left(px(drill * PANEL_DRILL_SHIFT * (1. - t)))
                    }
                },
            ));
        // The panel takes its backdrop blur here rather than inside
        // `panel_variant`: the blur is painted by an element that wraps the
        // whole subtree in one scene layer, and `panel_variant` returns a
        // builder the caller is still adding children to.
        //
        // It floats over the stage rather than occupying a column of it: 24
        // from the window's right edge, 20 down, and as tall as the stage it
        // sits on. The picture runs underneath, and the stage reserves the
        // width back so the two never overlap.
        //
        // The card takes that full height itself — `frosted` hands its child
        // the layout it was given, so a card that only asked for its content
        // would leave the float's spare height empty and scroll rows away
        // that had room to be drawn.
        let width = self.inspector_width;
        let handle = self.resize_edge(ResizeEdge::Inspector, cx);
        let float = |right: f32| {
            div()
                .absolute()
                .right(px(right))
                .top(px(Theme::inset_top()))
                .bottom(px(Theme::inset()))
                .w(px(width))
                .flex()
                .occlude()
                .child(frosted(
                    UiSurface::Panel.radius(),
                    UiSurface::Panel.blur(),
                    el,
                ))
                // Its left edge takes a drag, trading stage for panel.
                .child(handle)
        };
        if !collapsed {
            self.inspector_slide = None;
            e.set_inspector_open(false);
            return float(Theme::inset()).into_any_element();
        }
        // Folded: a 44 round toggle where the float's corner would be, and
        // the float sliding in over the stage from past the window's edge.
        // The toggle sits under it, so an open inspector covers it.
        let shown = slide_toward(
            &mut self.inspector_slide,
            e.get_inspector_open(),
            INSPECTOR_SLIDE_MS,
            window,
        );
        let editor = e.clone();
        let toggle = div()
            .absolute()
            .right(px(Theme::inset()))
            .top(px(Theme::inset_top()))
            .child(
                button("inspector-toggle", "Show inspector", theme)
                    .glyph("SlidersHorizontal-regular")
                    .icon_only()
                    .on_click(move |_, _, _| editor.set_inspector_open(true)),
            );
        let away = (width + Theme::inset() * 2.) * (1. - shown);
        div()
            .absolute()
            .inset_0()
            .child(toggle)
            .when(shown > 0., |el| el.child(float(Theme::inset() - away)))
            .into_any_element()
    }
}

/// The panel a sub-panel's caret goes back to, or `None` for a top panel.
fn sub_panel_of(name: &str) -> Option<&'static str> {
    match name {
        "Crop" | "Wallpapers" => Some("Frame"),
        "Shortcuts" => Some("Preferences"),
        _ => None,
    }
}
