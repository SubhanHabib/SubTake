//! The component catalogue: every primitive in `subtake_ui`, in every state
//! it can take, down one scrolling page in a window of its own beside the
//! gallery's. The gallery shows the controls where the product puts them;
//! this shows each one on its own, so a variation can be found, looked at in
//! both themes and tried without first finding the screen that holds it.
//!
//! Every control is live: toggles flip, pickers pick, sliders drag, fields
//! take typing. The header's Light / Dark switch retints the whole page,
//! the retained controls included. The frosted planes sit over a picture
//! the header's Backdrop picker swaps, since a blur over a flat ground
//! shows nothing.
//!
//! `SUBTAKE_GALLERY_COMPONENTS=off` leaves the window closed;
//! `SUBTAKE_GALLERY_COMPONENTS=frost` (or any section's name, lowercased)
//! opens it scrolled to that section.

use gpui::{prelude::*, *};
use std::path::PathBuf;
use subtake_theme::Theme;
use subtake_ui::{
    self as ui, Dropdown, Slider, Surface, TextInput, TimecodeField,
    unused::{self, RowState},
};

const WINDOW_WIDTH: f32 = 900.;
const WINDOW_HEIGHT: f32 = 1000.;
/// The column naming each specimen row.
const LABEL_WIDTH: f32 = 168.;
/// What a dropdown, slider or field is given, about an inspector row's width.
const CONTROL_WIDTH: f32 = 300.;
/// The stage the frosted planes float over.
const STAGE_HEIGHT: f32 = 340.;
const SAMPLE_WIDTH: f32 = 188.;
const SAMPLE_HEIGHT: f32 = 112.;
/// A menu tall enough to scroll, for its edge fades.
const SCROLL_MENU_HEIGHT: f32 = 168.;
const MENU_WIDTH: f32 = 240.;
const PROGRESS_WIDTH: f32 = 160.;
const EMPTY_HEIGHT: f32 = 260.;
const ICON_CELL: f32 = 124.;
/// Frames a section jump waits before it measures where to scroll.
const JUMP_SETTLE_FRAMES: u32 = 3;

const SECTIONS: [&str; 10] = [
    "Buttons",
    "Pickers",
    "Sliders and fields",
    "Menus",
    "Tiles",
    "Status",
    "Rows and cards",
    "Frost",
    "Type and icons",
    "Parked",
];

const BACKDROPS: [&str; 4] = ["Gradient", "Cherry pop", "Cityscape", "Blue rays"];
const WALLPAPERS: [&str; 3] = ["cherrypop.jpg", "cityscape.jpg", "bluerays.jpeg"];

/// A tile or stage picture from the bundled wallpaper thumbnails.
fn wallpaper(name: &str) -> Img {
    img(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/wallpaper-thumbnails")
        .join(name))
}

/// Opens the catalogue beside the gallery unless the environment says not to.
pub fn open() {
    if std::env::var("SUBTAKE_GALLERY_COMPONENTS").as_deref() == Ok("off") {
        return;
    }
    let opened = subtake_native::ui_runtime::open_view_window(
        "SubTake components",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        Catalogue::new,
    );
    if let Err(error) = opened {
        eprintln!("Open the component catalogue: {error:#}");
    }
}

pub struct Catalogue {
    dark: bool,
    theme: Theme,
    scroll: ScrollHandle,
    menu_scroll: ScrollHandle,
    fade_scroll: ScrollHandle,
    /// Which section to bring into view on the first frame.
    jump: Option<usize>,
    /// Frames drawn while a jump waits for the layout to settle.
    settled: u32,
    backdrop: usize,
    // What the live controls hold.
    tool: usize,
    toggled: bool,
    icon_toggled: bool,
    segment_two: usize,
    segment_three: usize,
    segment_four: usize,
    switch_on: bool,
    toggle_on: bool,
    swatch: usize,
    tile: usize,
    choice: usize,
    zoom: f32,
    highlighted: usize,
    checked: bool,
    radio: usize,
    icons: Vec<String>,
    dropdowns: Vec<(&'static str, Entity<Dropdown>)>,
    sliders: Vec<(&'static str, Entity<Slider>)>,
    inputs: Vec<(&'static str, Entity<TextInput>)>,
    timecodes: Vec<(&'static str, Entity<TimecodeField>)>,
}

impl Catalogue {
    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        ui::init(cx);
        // The gallery's own choice: `SUBTAKE_GALLERY=light` starts light.
        let dark = std::env::var("SUBTAKE_GALLERY").as_deref() != Ok("light");
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let choices = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        let mut dropdown = |label: &'static str, items: &[&str], set: fn(&mut Dropdown)| {
            let items = choices(items);
            let control = cx.new(|cx| {
                let mut d = Dropdown::new(cx, items, 0, theme, |_, _, _| {});
                set(&mut d);
                d
            });
            (label, control)
        };
        let dropdowns = vec![
            dropdown("Plain", &["Native", "16:9", "4:3", "1:1", "9:16"], |_| {}),
            dropdown("Compact", &["Native", "16:9", "4:3"], |d| d.compact = true),
            dropdown(
                "Glyph (select row)",
                &["MacBook Pro Microphone", "AirPods Pro", "No microphone"],
                |d| d.glyph = Some("Microphone-regular".into()),
            ),
            dropdown("Caption (select row)", &["MP4", "GIF", "PNG frame"], |d| {
                d.caption = Some("Format".into())
            }),
            dropdown("Opens up", &["Low", "Medium", "High"], |d| d.opens_up = true),
            dropdown("Disabled", &["Unavailable"], |d| d.enabled = false),
        ];

        let mut slider = |label: &'static str, value: f32, set: fn(&mut Slider)| {
            let control = cx.new(|_| {
                let mut s = Slider::new(0., 1., value, theme, |_, _, _, _| {});
                set(&mut s);
                s
            });
            (label, control)
        };
        let sliders = vec![
            slider("Plain", 0.4, |_| {}),
            slider("Caption and glyph", 0.65, |s| {
                s.set_caption("Volume", "SpeakerHigh-regular")
            }),
            slider("Unit", 0.3, |s| {
                s.set_caption("Padding", "");
                s.set_unit(100., "%");
            }),
            slider("Steps", 1., |s| {
                s.set_caption("Smoothing", "");
                s.maximum = 2.;
                s.steps = vec!["Off".into(), "Low".into(), "High".into()];
            }),
            slider("Disabled", 0.5, |s| {
                s.set_caption("Blur", "Drop-regular");
                s.enabled = false;
            }),
        ];

        let mut input = |label: &'static str, content: &str, placeholder: &str| {
            let placeholder = placeholder.to_owned();
            let content = content.to_owned();
            let control = cx.new(|cx| {
                let mut i = TextInput::new(cx, content, theme, |_, _, _| {});
                i.set_placeholder(placeholder);
                i
            });
            (label, control)
        };
        let inputs = vec![
            input("Empty", "", "Type to filter commands"),
            input("Filled", "Onboarding walkthrough", ""),
        ];

        let mut timecode = |label: &'static str, value: f64, set: fn(&mut TimecodeField)| {
            let control = cx.new(|cx| {
                let mut t = TimecodeField::new(cx, value, theme, |_, _, _| {});
                t.maximum = 148_000.;
                set(&mut t);
                t
            });
            (label, control)
        };
        let timecodes = vec![
            timecode("Plain", 37_500., |_| {}),
            timecode("Labelled", 12_250., |t| t.label = "Start".into()),
            timecode("Disabled", 0., |t| t.enabled = false),
        ];

        let mut icons: Vec<String> =
            std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons"))
                .map(|dir| {
                    dir.filter_map(|entry| {
                        let name = entry.ok()?.file_name().into_string().ok()?;
                        name.strip_suffix(".svg").map(str::to_owned)
                    })
                    .collect()
                })
                .unwrap_or_default();
        icons.sort();

        let jump = std::env::var("SUBTAKE_GALLERY_COMPONENTS").ok().and_then(|want| {
            SECTIONS
                .iter()
                .position(|s| s.to_lowercase().starts_with(&want.to_lowercase()))
        });

        Self {
            dark,
            theme,
            scroll: ScrollHandle::new(),
            menu_scroll: ScrollHandle::new(),
            fade_scroll: ScrollHandle::new(),
            jump,
            settled: 0,
            backdrop: 1,
            tool: 0,
            toggled: true,
            icon_toggled: true,
            segment_two: 0,
            segment_three: 1,
            segment_four: 2,
            switch_on: true,
            toggle_on: false,
            swatch: 2,
            tile: 0,
            choice: 1,
            zoom: 1.,
            highlighted: 1,
            checked: true,
            radio: 0,
            icons,
            dropdowns,
            sliders,
            inputs,
            timecodes,
        }
    }

    /// Retint the page and every retained control on it.
    fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        self.theme = if dark { Theme::dark() } else { Theme::light() };
        let theme = self.theme;
        for (_, d) in &self.dropdowns {
            d.update(cx, |d, cx| {
                d.theme = theme;
                cx.notify();
            });
        }
        for (_, s) in &self.sliders {
            s.update(cx, |s, cx| {
                s.theme = theme;
                cx.notify();
            });
        }
        for (_, i) in &self.inputs {
            i.update(cx, |i, cx| {
                i.theme = theme;
                cx.notify();
            });
        }
        for (_, t) in &self.timecodes {
            t.update(cx, |t, cx| {
                t.theme = theme;
                cx.notify();
            });
        }
        cx.notify();
    }

    /// A control callback that edits the catalogue and repaints it.
    fn with<A: 'static>(
        cx: &mut Context<Self>,
        edit: impl Fn(&mut Self, A) + 'static,
    ) -> impl Fn(A, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |value, _, cx| {
            this.update(cx, |s, cx| {
                edit(s, value);
                cx.notify();
            })
            .ok();
        }
    }

    /// A click callback that edits the catalogue and repaints it.
    fn click(
        cx: &mut Context<Self>,
        edit: impl Fn(&mut Self) + 'static,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |s, _, _, cx| {
            edit(s);
            cx.notify();
        })
    }

    fn header(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(Theme::GAP_LARGE))
            .px(px(Theme::INSET))
            .py(px(Theme::GAP_LARGE))
            .border_b_1()
            .border_color(theme.line)
            .child(ui::panel_title("Components").text_color(theme.text))
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(Theme::FONT_SECONDARY))
                    .text_color(theme.muted)
                    .child("Backdrop"),
            )
            .child(div().w(px(CONTROL_WIDTH)).child(ui::segmented_control(
                "catalogue-backdrop",
                &BACKDROPS,
                self.backdrop,
                theme,
                Self::with(cx, |s, i| s.backdrop = i),
            )))
            .child(div().w(px(SAMPLE_WIDTH)).child(ui::segmented_control(
                "catalogue-theme",
                &["Light", "Dark"],
                usize::from(self.dark),
                theme,
                {
                    let this = cx.entity().downgrade();
                    move |i, _, cx| {
                        this.update(cx, |s, cx| s.set_dark(i == 1, cx)).ok();
                    }
                },
            )))
    }

    /// The picture a frosted plane blurs: a gradient, or a wallpaper.
    fn backdrop(&self) -> Div {
        let stage = div()
            .relative()
            .w_full()
            .h(px(STAGE_HEIGHT))
            .rounded(px(Theme::RADIUS_FRAME))
            .overflow_hidden();
        match self.backdrop.checked_sub(1).and_then(|i| WALLPAPERS.get(i)) {
            Some(name) => stage.child(
                wallpaper(name)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            ),
            // Hard-edged stripes under a gradient, so a blur has edges to
            // soften and a plane with none shows the stripes sharp.
            None => stage
                .bg(linear_gradient(
                    135.,
                    linear_color_stop(rgb(0xff7a59), 0.),
                    linear_color_stop(rgb(0x5b5bf0), 1.),
                ))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .gap(px(Theme::INSET))
                        .children((0..24).map(|i| {
                            div().w(px(Theme::GAP)).h_full().bg(if i % 2 == 0 {
                                hsla(0., 0., 1., 0.55)
                            } else {
                                hsla(0., 0., 0., 0.35)
                            })
                        })),
                ),
        }
    }

    fn buttons(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        type Variant = fn(ui::Button) -> ui::Button;
        let variants: [(&str, Variant); 7] = [
            ("Secondary", |b| b),
            ("Primary", ui::Button::primary),
            ("Raised", ui::Button::raised),
            ("Ghost", ui::Button::ghost),
            ("Danger", ui::Button::danger),
            ("Record", ui::Button::record),
            ("Transport", ui::Button::transport),
        ];
        let mut rows: Vec<AnyElement> = variants
            .iter()
            .map(|(name, variant)| {
                let id = |state: &str| SharedString::from(format!("btn-{name}-{state}"));
                let glyph = match *name {
                    "Record" => "Record-fill",
                    "Transport" => "Play-fill",
                    "Danger" => "Trash-regular",
                    _ => "Scissors-regular",
                };
                let make = |state: &str| variant(ui::button(id(state), *name, theme));
                specimen(
                    theme,
                    name,
                    vec![
                        make("rest").into_any_element(),
                        make("glyph").glyph(glyph).into_any_element(),
                        make("icon")
                            .glyph(glyph)
                            .icon_only()
                            .into_any_element(),
                        make("selected").selected(true).into_any_element(),
                        make("disabled").enabled(false).into_any_element(),
                        make("disabled-glyph")
                            .glyph(glyph)
                            .enabled(false)
                            .into_any_element(),
                    ],
                )
                .into_any_element()
            })
            .collect();

        let sizes: [(&str, Variant); 7] = [
            ("Small 34", ui::Button::small),
            ("Compact 34", ui::Button::compact),
            ("Standard 40", ui::Button::standard),
            ("Large 44", ui::Button::large),
            ("Dialog 48", ui::Button::dialog),
            ("Bar", ui::Button::bar),
            ("Hero", ui::Button::hero),
        ];
        rows.extend(sizes.iter().map(|(name, size)| {
            let id = |state: &str| SharedString::from(format!("size-{name}-{state}"));
            specimen(
                theme,
                name,
                vec![
                    size(ui::button(id("secondary"), "Secondary", theme))
                        .glyph("Plus-regular")
                        .into_any_element(),
                    size(ui::button(id("primary"), "Primary", theme).primary()).into_any_element(),
                    size(ui::icon_button(id("icon"), "Gear-regular", "Settings", theme))
                        .into_any_element(),
                    size(ui::icon_button(id("ghost"), "Gear-regular", "Settings", theme).ghost())
                        .into_any_element(),
                ],
            )
            .into_any_element()
        }));

        rows.push(
            specimen(
                theme,
                "Modifiers",
                vec![
                    ui::button("mod-caret", "Native", theme)
                        .caret()
                        .into_any_element(),
                    ui::button("mod-detail", "Export", theme)
                        .detail("4K")
                        .into_any_element(),
                    ui::button("mod-mono", "00:37.50", theme)
                        .mono()
                        .tabular()
                        .into_any_element(),
                    ui::button("mod-glyph-size", "Big glyph", theme)
                        .glyph("Sparkle-regular")
                        .glyph_size(Theme::ICON_SIZE_MEDIUM)
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        rows.push(
            specimen(
                theme,
                "Stretch",
                vec![
                    div()
                        .flex()
                        .gap(px(Theme::GAP))
                        .w(px(CONTROL_WIDTH))
                        .child(ui::button("stretch-a", "Cancel", theme).stretch())
                        .child(ui::button("stretch-b", "Apply", theme).primary().stretch())
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        rows.push(
            specimen(
                theme,
                "Toggled (click)",
                vec![
                    ui::button("toggled-text", "Snap", theme)
                        .glyph("Magnet-regular")
                        .toggled(self.toggled)
                        .on_click(Self::click(cx, |s| s.toggled = !s.toggled))
                        .into_any_element(),
                    ui::icon_button("toggled-icon", "Magnet-regular", "Snap", theme)
                        .toggled(self.icon_toggled)
                        .on_click(Self::click(cx, |s| s.icon_toggled = !s.icon_toggled))
                        .into_any_element(),
                    ui::icon_button("toggled-on", "Magnet-regular", "Snap", theme)
                        .toggled(true)
                        .into_any_element(),
                    ui::icon_button("toggled-off", "Magnet-regular", "Snap", theme)
                        .toggled(false)
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        let tools = [
            ("Sparkle-regular", "Scene"),
            ("Cursor-regular", "Cursor"),
            ("Camera-regular", "Camera"),
            ("ClosedCaptioning-regular", "Captions"),
        ];
        let pod = ui::pod(theme).flex_col().gap(px(Theme::GAP_SMALL)).children(
            tools.iter().enumerate().map(|(i, (glyph, label))| {
                ui::tool_button(
                    SharedString::from(format!("tool-{label}")),
                    glyph,
                    *label,
                    self.tool == i,
                    theme,
                    Self::click(cx, move |s| s.tool = i),
                )
            }),
        );
        let row_pod = ui::pod_small(theme).gap(px(Theme::GAP_SMALL)).children(
            tools.iter().enumerate().map(|(i, (glyph, label))| {
                ui::tool_button(
                    SharedString::from(format!("tool-row-{label}")),
                    glyph,
                    *label,
                    self.tool == i,
                    theme,
                    Self::click(cx, move |s| s.tool = i),
                )
            }),
        );
        rows.push(
            specimen(
                theme,
                "Tool pod (click)",
                vec![
                    ui::frosted(Surface::Pod.radius(), Surface::Pod.blur(), pod).into_any_element(),
                    ui::frosted(Surface::Pod.radius(), Surface::Pod.blur(), row_pod)
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        section(theme, 0, "Every variant at rest, with a glyph, icon-only, selected and disabled; every size; the modifiers.", rows)
    }

    fn pickers(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let mut rows = vec![
            specimen(
                theme,
                "Segmented, two",
                vec![wide(ui::segmented_control(
                    "seg-two",
                    &["Light", "Dark"],
                    self.segment_two,
                    theme,
                    Self::with(cx, |s, i| s.segment_two = i),
                ))],
            ),
            specimen(
                theme,
                "Segmented, three",
                vec![wide(ui::segmented_control(
                    "seg-three",
                    &["Wallpaper", "Image", "Colour"],
                    self.segment_three,
                    theme,
                    Self::with(cx, |s, i| s.segment_three = i),
                ))],
            ),
            specimen(
                theme,
                "Segmented, four",
                vec![wide(ui::segmented_control(
                    "seg-four",
                    &["None", "Ripple", "Ring", "Pulse"],
                    self.segment_four,
                    theme,
                    Self::with(cx, |s, i| s.segment_four = i),
                ))],
            ),
            specimen(
                theme,
                "Switch",
                vec![
                    ui::switch(
                        "switch-live",
                        self.switch_on,
                        true,
                        theme,
                        Self::with(cx, |s, on| s.switch_on = on),
                    )
                    .into_any_element(),
                    ui::switch("switch-on", true, true, theme, |_, _, _| {}).into_any_element(),
                    ui::switch("switch-off", false, true, theme, |_, _, _| {}).into_any_element(),
                    ui::switch("switch-on-disabled", true, false, theme, |_, _, _| {})
                        .into_any_element(),
                    ui::switch("switch-off-disabled", false, false, theme, |_, _, _| {})
                        .into_any_element(),
                ],
            ),
            specimen(
                theme,
                "Toggle row",
                vec![
                    wide(ui::toggle(
                        "toggle-live",
                        "Show cursor",
                        self.toggle_on,
                        true,
                        theme,
                        Self::with(cx, |s, on| s.toggle_on = on),
                    )),
                    wide(ui::toggle(
                        "toggle-disabled",
                        "Disabled",
                        true,
                        false,
                        theme,
                        |_, _, _| {},
                    )),
                ],
            ),
        ];
        rows.extend(
            self.dropdowns
                .iter()
                .map(|(label, d)| specimen(theme, &format!("Dropdown, {}", label.to_lowercase()), vec![wide(d.clone())])),
        );
        section(theme, 1, "Segments, switches and dropdowns. Every one is live.", rows.into_iter().map(IntoElement::into_any_element).collect())
    }

    fn sliders_and_fields(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let mut rows: Vec<AnyElement> = self
            .sliders
            .iter()
            .map(|(label, s)| {
                specimen(theme, &format!("Slider, {}", label.to_lowercase()), vec![wide(s.clone())])
                    .into_any_element()
            })
            .collect();
        rows.extend(self.inputs.iter().map(|(label, i)| {
            specimen(theme, &format!("Text input, {}", label.to_lowercase()), vec![wide(i.clone())])
                .into_any_element()
        }));
        rows.extend(self.timecodes.iter().map(|(label, t)| {
            specimen(theme, &format!("Timecode, {}", label.to_lowercase()), vec![wide(t.clone())])
                .into_any_element()
        }));
        let zoom = self.zoom;
        rows.push(
            specimen(
                theme,
                "Zoom (click)",
                vec![
                    ui::zoom_control("zoom-live", zoom, theme)
                        .can_zoom_out(zoom > 0.25)
                        .can_zoom_in(zoom < 8.)
                        .can_fit(zoom != 1.)
                        .on_zoom_out(Self::click(cx, |s| s.zoom = (s.zoom / 2.).max(0.25)))
                        .on_zoom_in(Self::click(cx, |s| s.zoom = (s.zoom * 2.).min(8.)))
                        .on_fit(Self::click(cx, |s| s.zoom = 1.))
                        .into_any_element(),
                    ui::zoom_control("zoom-min", 0.25, theme)
                        .can_zoom_out(false)
                        .can_fit(true)
                        .into_any_element(),
                    ui::zoom_control("zoom-max", 8., theme)
                        .can_zoom_in(false)
                        .can_fit(true)
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        section(theme, 2, "Retained controls: drag the sliders and scrub or type in the fields.", rows)
    }

    fn menus(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let highlighted = self.highlighted;
        let rows = ["Undo", "Redo", "Split clip at playhead"];
        let menu = ui::menu_surface(theme)
            .w(px(MENU_WIDTH))
            .child(unused::menu_section_header("Edit", theme))
            .children(rows.iter().enumerate().map(|(i, label)| {
                ui::menu_row(
                    SharedString::from(format!("menu-{label}")),
                    *label,
                    i == 0,
                    i == highlighted,
                    theme,
                    Self::click(cx, move |s| s.highlighted = i),
                )
                .child(unused::shortcut_hint(["⌘Z", "⇧⌘Z", "S"][i], theme))
            }))
            .child(ui::menu_separator(theme))
            .child(
                ui::menu_row("menu-more", "More", false, false, theme, |_, _, _| {})
                    .child(unused::submenu_chevron(theme)),
            )
            .child(unused::destructive_menu_row(
                "menu-delete",
                "Delete region",
                theme,
                |_, _, _| {},
            ));
        let commands = ui::menu_surface(theme).w(px(MENU_WIDTH)).child(
            ui::fade_edges(
                ui::menu_list("menu-scroll", SCROLL_MENU_HEIGHT)
                    .track_scroll(&self.menu_scroll)
                    .children(
                        ["Zoom", "Text", "Arrow", "Blur", "Audio", "Caption", "Trim", "Speed", "Marker"]
                            .iter()
                            .enumerate()
                            .map(|(i, label)| {
                                ui::command_row(
                                    SharedString::from(format!("command-{label}")),
                                    *label,
                                    i == 0,
                                    theme,
                                    |_, _, _| {},
                                )
                            }),
                    ),
            )
            .tracking(&self.menu_scroll),
        );
        let stage = self
            .backdrop()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_start()
                    .justify_center()
                    .gap(px(Theme::INSET))
                    .p(px(Theme::INSET))
                    .child(ui::frosted(Theme::RADIUS_MENU, ui::MENU_BLUR, menu))
                    .child(ui::frosted(Theme::RADIUS_MENU, ui::MENU_BLUR, commands)),
            );
        section(
            theme,
            3,
            "Left: a checked row, the highlighted row (click one to move it), a shortcut, a submenu, a destructive row. Right: the palette's rows, which have no tick gutter, scrolling under the edge fades.",
            vec![stage.into_any_element()],
        )
    }

    fn tiles(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let colours = [0xff5f57, 0xfebc2e, 0x28c840, 0x5b8def, 0xa468e9, 0x1d1d1f];
        let swatches = colours
            .iter()
            .enumerate()
            .map(|(i, c)| {
                ui::swatch(
                    SharedString::from(format!("swatch-{i}")),
                    rgb(*c).into(),
                    self.swatch == i,
                    theme,
                )
                .on_click(Self::click(cx, move |s| s.swatch = i))
                .into_any_element()
            })
            .collect();
        let mut media: Vec<AnyElement> = WALLPAPERS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                ui::media_tile(
                    SharedString::from(format!("media-{i}")),
                    BACKDROPS[i + 1],
                    Some(wallpaper(name)),
                    self.tile == i,
                    theme,
                )
                .on_click(Self::click(cx, move |s| s.tile = i))
                .into_any_element()
            })
            .collect();
        media.push(
            ui::media_tile("media-none", "No picture", None, false, theme).into_any_element(),
        );
        let choices: Vec<AnyElement> = ["Default", "Pointer", "Hand"]
            .iter()
            .enumerate()
            .map(|(i, label)| {
                ui::choice_tile(
                    SharedString::from(format!("choice-{i}")),
                    self.choice == i,
                    true,
                    theme,
                )
                .on_click(Self::click(cx, move |s| s.choice = i))
                .child(ui::icon("Cursor-regular", theme.text))
                .child(div().text_size(px(Theme::FONT_SMALL)).child(*label))
                .into_any_element()
            })
            .chain([ui::choice_tile("choice-disabled", false, false, theme)
                .child(ui::icon("Cursor-regular", theme.text))
                .child(div().text_size(px(Theme::FONT_SMALL)).child("Disabled"))
                .into_any_element()])
            .collect();
        section(
            theme,
            4,
            "Click to pick. The empty state is the editor's with nothing open.",
            vec![
                specimen(theme, "Swatch", swatches).into_any_element(),
                specimen(theme, "Media tile", media).into_any_element(),
                specimen(theme, "Choice tile", vec![ui::tile_grid(4).w_full().children(choices).into_any_element()])
                    .into_any_element(),
                specimen(
                    theme,
                    "Empty state",
                    vec![div()
                        .flex()
                        .w_full()
                        .h(px(EMPTY_HEIGHT))
                        .child(ui::empty_state(theme, "Nothing open yet", "Record something new, or open a recording to edit it."))
                        .into_any_element()],
                )
                .into_any_element(),
            ],
        )
    }

    fn status(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let footer = ui::composer_footer(theme)
            .child(ui::icon_button("footer-a", "FolderOpen-regular", "File", theme).ghost().small().toggled(false))
            .child(ui::icon_button("footer-b", "Plus-regular", "Add", theme).ghost().small().toggled(true))
            .child("A quiet hint");
        section(
            theme,
            5,
            "Chips, progress and the tooltips a hover brings up, drawn in place.",
            vec![
                specimen(
                    theme,
                    "Status",
                    vec![
                        ui::status_dot(theme).into_any_element(),
                        ui::status_chip(theme).child("Gallery mode").into_any_element(),
                        ui::status_chip(theme)
                            .child(ui::status_dot(theme))
                            .child("Transcribing · 42%")
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Context chip",
                    vec![
                        ui::context_chip(theme, &["Add", "Commands"]).into_any_element(),
                        ui::context_chip(theme, &["Project", "Scene", "Frame"]).into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Progress",
                    [0., 0.35, 1.]
                        .iter()
                        .map(|f| ui::progress_bar(*f, theme).w(px(PROGRESS_WIDTH)).into_any_element())
                        .collect(),
                )
                .into_any_element(),
                specimen(theme, "Composer footer", vec![footer.into_any_element()]).into_any_element(),
                specimen(
                    theme,
                    "Tooltip",
                    vec![
                        ui::tooltip("Split clip at playhead", theme, cx).into_any_element(),
                        ui::tooltip_detail("Snap", "Regions snap to the playhead and each other", theme, cx)
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            ],
        )
    }

    fn rows_and_cards(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        section(
            theme,
            6,
            "The inspector's building blocks.",
            vec![
                specimen(
                    theme,
                    "Field row",
                    vec![wide(ui::field_row(theme, "Show cursor").child(ui::switch(
                        "field-switch",
                        self.switch_on,
                        true,
                        theme,
                        Self::with(cx, |s, on| s.switch_on = on),
                    )))],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Setting card",
                    vec![wide(ui::setting_card(theme, "Suggest zooms", "Finds the clicks and typing in a take and zooms into them."))],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Group card",
                    vec![wide(
                        ui::group_card(theme, "Shadow")
                            .child(ui::field_row(theme, "Enabled").child(ui::switch("group-switch", true, true, theme, |_, _, _| {}))),
                    )],
                )
                .into_any_element(),
                specimen(theme, "Panel header", vec![wide(ui::panel_header(theme, "Background"))]).into_any_element(),
                specimen(
                    theme,
                    "Caps label, divider",
                    vec![div()
                        .flex()
                        .flex_col()
                        .gap(px(Theme::GAP))
                        .w(px(CONTROL_WIDTH))
                        .child(ui::caps_label("Appearance", theme))
                        .child(ui::divider(theme))
                        .into_any_element()],
                )
                .into_any_element(),
            ],
        )
    }

    fn frost(&mut self) -> Div {
        let theme = self.theme;
        let surfaces = [
            ("Panel", Surface::Panel),
            ("Content", Surface::Content),
            ("Pod", Surface::Pod),
            ("Card", Surface::Card),
            ("Popup", Surface::Popup),
            ("Overlay", Surface::Overlay),
        ];
        let sample = |name: &str, plane: Div, radius: f32, blur: Option<f32>| {
            let plane = plane
                .w(px(SAMPLE_WIDTH))
                .h(px(SAMPLE_HEIGHT))
                .p(px(Theme::GAP_LARGE))
                .flex()
                .flex_col()
                .gap(px(Theme::GAP_SMALL))
                .child(div().text_size(px(Theme::FONT_BODY)).text_color(theme.text).child(name.to_owned()))
                .child(ui::mono_small(
                    match blur {
                        Some(blur) => format!("radius {radius} · blur {blur}"),
                        None => format!("radius {radius} · no blur"),
                    },
                    theme,
                ));
            match blur {
                Some(blur) => ui::frosted(radius, blur, plane).into_any_element(),
                None => plane.into_any_element(),
            }
        };
        let planes = self.backdrop().child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_wrap()
                .content_start()
                .gap(px(Theme::INSET))
                .p(px(Theme::INSET))
                .children(surfaces.iter().map(|(name, s)| {
                    sample(name, ui::panel_variant(theme, *s), s.radius(), Some(s.blur()))
                })),
        );
        let blurs = [
            ("No frost", None),
            ("Pod", Some(ui::POD_BLUR)),
            ("Panel, menu", Some(ui::PANEL_BLUR)),
            ("Bar", Some(ui::BAR_BLUR)),
        ];
        let strengths = self.backdrop().child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_wrap()
                .content_start()
                .gap(px(Theme::INSET))
                .p(px(Theme::INSET))
                .children(blurs.iter().map(|(name, blur)| {
                    sample(name, ui::panel_variant(theme, Surface::Panel), Surface::Panel.radius(), *blur)
                }))
                .child(ui::frosted(
                    Surface::Pod.radius(),
                    Surface::Pod.blur(),
                    ui::pod(theme)
                        .flex_col()
                        .gap(px(Theme::GAP_SMALL))
                        .child(ui::icon_button("frost-pod-a", "Sparkle-regular", "Scene", theme).ghost().selected(true))
                        .child(ui::icon_button("frost-pod-b", "Cursor-regular", "Cursor", theme).ghost()),
                ))
                .child(ui::frosted(
                    Surface::Pod.radius(),
                    Surface::Pod.blur(),
                    ui::pod_small(theme)
                        .gap(px(Theme::GAP_SMALL))
                        .child(ui::button("frost-thin-a", "Native", theme).compact().caret())
                        .child(ui::button("frost-thin-b", "Crop", theme).compact()),
                )),
        );
        let fading = ui::content_panel(theme).w(px(CONTROL_WIDTH)).p(px(Theme::GAP)).child(
            ui::fade_edges(
                div()
                    .id("fade-list")
                    .flex()
                    .flex_col()
                    .gap(px(Theme::GAP_SMALL))
                    .h(px(SCROLL_MENU_HEIGHT))
                    .overflow_y_scroll()
                    .track_scroll(&self.fade_scroll)
                    .children((1..=14).map(|i| {
                        ui::field_row(theme, format!("Row {i}")).child(ui::mono_small("00:00", theme))
                    })),
            )
            .tracking(&self.fade_scroll)
            .thumb(theme.muted, Theme::GAP_SMALL),
        );
        section(
            theme,
            7,
            "Each plane frosted at its own radius and blur over the backdrop (the header picks it), the four blur strengths side by side with one left unfrosted, the two pods, and a scroll's edge fades and thumb.",
            vec![
                planes.into_any_element(),
                strengths.into_any_element(),
                specimen(theme, "Edge fades, thumb", vec![fading.into_any_element()]).into_any_element(),
            ],
        )
    }

    fn type_and_icons(&mut self) -> Div {
        let theme = self.theme;
        let scale = [
            ("Display 40", ui::title("Nothing open yet", Theme::FONT_DISPLAY)),
            ("Panel 22", ui::panel_title("Background")),
            ("Heading 17", ui::heading("Shadow")),
            ("Action 15", div().text_size(px(Theme::FONT_ACTION)).child("Start recording")),
            ("Body 13", div().text_size(px(Theme::FONT_BODY)).child("Regions snap to the playhead.")),
            ("Secondary 12", div().text_size(px(Theme::FONT_SECONDARY)).child("Finds the clicks in a take.")),
            ("Small 11", div().text_size(px(Theme::FONT_SMALL)).child("Gallery mode")),
            ("Mono", ui::mono("00:37.50 / 02:28")),
            ("Mono small", ui::mono_small("3240 × 1820", theme)),
        ];
        let mut rows: Vec<AnyElement> = scale
            .into_iter()
            .map(|(label, specimen_text)| {
                specimen(theme, label, vec![specimen_text.text_color(theme.text).into_any_element()])
                    .into_any_element()
            })
            .collect();
        let muted = specimen(
            theme,
            "Text colours",
            vec![
                div().text_color(theme.text).child("text").into_any_element(),
                div().text_color(theme.muted).child("muted").into_any_element(),
                div().text_color(theme.accent).child("accent").into_any_element(),
                div().text_color(theme.danger).child("danger").into_any_element(),
            ],
        );
        rows.push(muted.into_any_element());
        rows.push(
            div()
                .flex()
                .flex_wrap()
                .gap(px(Theme::GAP))
                .children(self.icons.iter().map(|name| {
                    div()
                        .w(px(ICON_CELL))
                        .flex()
                        .items_center()
                        .gap(px(Theme::GAP))
                        .child(ui::icon(name, theme.text))
                        .child(
                            div()
                                .min_w_0()
                                .text_ellipsis()
                                .text_size(px(Theme::FONT_SMALL))
                                .text_color(theme.muted)
                                .child(name.clone()),
                        )
                }))
                .into_any_element(),
        );
        section(theme, 8, "The type scale and every bundled glyph.", rows)
    }

    fn parked(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let thumb = |c: u32| div().size_full().rounded(px(Theme::RADIUS_LANE)).bg(rgb(c));
        let list = [
            ("Normal", false, RowState::Normal),
            ("Selected", true, RowState::Normal),
            ("Cut", false, RowState::Cut),
            ("Drag source", false, RowState::DragSource),
        ];
        let dialog = div()
            .relative()
            .w_full()
            .h(px(EMPTY_HEIGHT))
            .rounded(px(Theme::RADIUS_FRAME))
            .overflow_hidden()
            .child(unused::scrim(theme).absolute().inset_0())
            .child(
                div().absolute().inset_0().flex().items_center().justify_center().child(
                    unused::dialog(theme)
                        .child(ui::heading("Discard changes?").text_color(theme.text))
                        .child(
                            div()
                                .text_size(px(Theme::FONT_BODY))
                                .text_color(theme.muted)
                                .child("The take keeps its last saved edit."),
                        )
                        .child(
                            unused::dialog_actions()
                                .child(ui::button("dialog-cancel", "Cancel", theme).dialog())
                                .child(ui::button("dialog-discard", "Discard", theme).danger().dialog()),
                        ),
                ),
            );
        section(
            theme,
            9,
            "Built to the handoff and parked in `subtake_ui::unused`: nothing in the app draws them yet.",
            vec![
                specimen(
                    theme,
                    "Checkbox, radio",
                    vec![
                        unused::checkbox("check-live", "Include audio", self.checked, theme)
                            .on_click(Self::click(cx, |s| s.checked = !s.checked))
                            .into_any_element(),
                        unused::radio("radio-a", "MP4", self.radio == 0, theme)
                            .on_click(Self::click(cx, |s| s.radio = 0))
                            .into_any_element(),
                        unused::radio("radio-b", "GIF", self.radio == 1, theme)
                            .on_click(Self::click(cx, |s| s.radio = 1))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Field, stepper",
                    vec![
                        wide(unused::field("park-field", "MagnifyingGlassPlus-regular", "Search", "", theme)),
                        unused::stepper("park-stepper", "24", theme).into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Key cap",
                    ["⌘", "⇧", "E", "Space"]
                        .iter()
                        .map(|k| unused::key_cap(*k, theme).into_any_element())
                        .collect(),
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Toast",
                    vec![
                        unused::toast("Export finished", theme.accent, None, theme).into_any_element(),
                        unused::toast(
                            "Region deleted",
                            theme.danger,
                            Some(("Undo".into(), Box::new(|_: &ClickEvent, _: &mut Window, _: &mut App| {}))),
                            theme,
                        )
                        .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "List row",
                    vec![div()
                        .flex()
                        .flex_col()
                        .gap(px(Theme::GAP_SMALL))
                        .w_full()
                        .children(list.iter().enumerate().map(|(i, (title, selected, state))| {
                            unused::list_row(
                                SharedString::from(format!("list-{i}")),
                                *title,
                                "Zoom · 2.0×",
                                "00:12.40",
                                thumb([0x5b8def, 0xa468e9, 0x28c840, 0xfebc2e][i]),
                                *selected,
                                *state,
                                theme,
                            )
                        }))
                        .into_any_element()],
                )
                .into_any_element(),
                specimen(theme, "Scrim, dialog", vec![dialog.into_any_element()]).into_any_element(),
            ],
        )
    }
}

/// A section: its caps label, a line on what it shows, and its specimens on
/// the inspector's own plane, which is what the controls are drawn to sit on.
fn section(theme: Theme, index: usize, note: &str, rows: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(Theme::GAP))
        .child(ui::heading(SECTIONS[index]).text_color(theme.text))
        .child(
            div()
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted)
                .child(note.to_owned()),
        )
        .child(
            ui::content_panel(theme)
                .flex()
                .flex_col()
                .gap(px(Theme::GAP_LARGE))
                .p(px(Theme::INSET))
                .children(rows),
        )
}

/// One row of a section: what it is on the left, the specimens on the right.
fn specimen(theme: Theme, label: &str, items: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Theme::GAP_LARGE))
        .child(
            div()
                .w(px(LABEL_WIDTH))
                .flex_none()
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap(px(Theme::GAP))
                .children(items),
        )
}

/// A specimen given an inspector row's width.
fn wide(el: impl IntoElement) -> AnyElement {
    div().w(px(CONTROL_WIDTH)).child(el).into_any_element()
}

impl Render for Catalogue {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ui::motion::enter_window(window.window_handle().window_id().as_u64());
        let theme = self.theme;
        // The jump waits for a laid-out frame, then puts the section where
        // the first one sits: `scroll_to_top_of_item` on the opening frame
        // landed past the heading, measured before the page had settled.
        if let Some(index) = self.jump {
            match (self.scroll.bounds_for_item(0), self.scroll.bounds_for_item(index)) {
                (Some(first), Some(target)) if self.settled >= JUMP_SETTLE_FRAMES => {
                    self.scroll.set_offset(point(px(0.), first.top() - target.top()));
                    self.jump = None;
                }
                _ => {
                    self.settled += 1;
                    window.request_animation_frame();
                }
            }
        }
        let header = self.header(cx);
        let sections = [
            self.buttons(cx),
            self.pickers(cx),
            self.sliders_and_fields(cx),
            self.menus(cx),
            self.tiles(cx),
            self.status(cx),
            self.rows_and_cards(cx),
            self.frost(),
            self.type_and_icons(),
            self.parked(cx),
        ];
        if ui::tick_hover_fades() {
            window.request_animation_frame();
        }
        // An opaque ground the product would get from the window's material:
        // `bg` is a tint laid over the vibrancy, not a colour of its own.
        let ground = if self.dark {
            hsla(240. / 360., 0.06, 0.11, 1.)
        } else {
            hsla(60. / 360., 0.05, 0.9, 1.)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(ground)
            .font_family(subtake_theme::FONT_SANS)
            .text_color(theme.text)
            .text_size(px(Theme::FONT_BODY))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .bg(theme.bg)
                    .child(header)
                    .child(
                        div()
                            .id("catalogue")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            // The sections are the scroll's own children, so
                            // `scroll_to_item` can bring one to the top.
                            .flex()
                            .flex_col()
                            .gap(px(Theme::INSET * 2.))
                            .p(px(Theme::INSET))
                            .children(sections),
                    ),
            )
    }
}
