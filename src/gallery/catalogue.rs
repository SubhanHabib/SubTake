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
//! shows nothing. The window itself is frosted as the editor's is, the
//! desktop blurred through it and `bg` over that.
//!
//! Every section has a Tune button. It lists each metric the section's
//! components read, found by recording the reads as they draw, with a slider
//! per metric. A change is live everywhere, the editor's window included,
//! since a metric is one value app-wide; the header copies the changes as
//! `metrics.rs` lines. Nothing is saved (see `subtake_theme::tune`).
//!
//! The dock's Colours view tunes the palette the page is showing: every
//! token, since a colour is a field read and cannot be recorded per section,
//! each as a chip, and the chosen one on H, S, L and A sliders and a CSS
//! field. The copy carries them as `palette.rs` lines.
//!
//! `SUBTAKE_GALLERY_COMPONENTS=off` leaves the window closed;
//! `SUBTAKE_GALLERY_COMPONENTS=frost` (or any section's name, lowercased)
//! opens it scrolled to that section, and `=frost+tune` with its Tune list
//! open (`+colours` for its Colours view).
//! `SUBTAKE_GALLERY_TUNED=GAP=12,RADIUS_MENU=8,dark.accent=#ff6a00` starts
//! with those metrics and colours tuned.
//!
//! Not drawn by the design: a gallery tool. The dock's Themes row keeps
//! whole sets of overrides as text files in the preferences folder's
//! `tuned-themes`, to save, import and switch between.

use gpui::{prelude::*, *};
use std::{collections::HashMap, path::PathBuf};
use subtake_theme::{
    Appearance, Theme,
    tune::{self, Colour, Metric, Reads},
};
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
/// A tuning row: two to a line across the dock.
const TUNE_ROW_WIDTH: f32 = 404.;
/// The tuning dock under the page, which scrolls on its own so the section
/// stays in view above it.
const TUNE_DOCK_HEIGHT: f32 = 400.;
/// A colour token's chip: three to a line beside the colour editor.
const CHIP_WIDTH: f32 = 160.;
const CHIP_SWATCH: f32 = 20.;
/// The chosen colour, large, at the head of the editor.
const PICKED_SWATCH: f32 = 44.;
/// The dock's list of what has been tuned, beside whichever view is open.
const CHANGES_WIDTH: f32 = 272.;
const SWATCH_RADIUS: f32 = 6.;
/// The editor's sliders, in `Hsla` order, with the scale and unit each
/// shows its 0–1 channel in.
const CHANNELS: [(&str, f32, &str); 4] = [
    ("Hue", 360., "°"),
    ("Saturation", 100., "%"),
    ("Lightness", 100., "%"),
    ("Alpha", 100., "%"),
];
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
    /// The metrics each section's components read, as they draw.
    reads: Vec<Reads>,
    /// The section whose metrics the dock is tuning.
    tuning: Option<usize>,
    /// A slider per metric shown in a tuning list, made the first time.
    tuners: HashMap<&'static str, Entity<Slider>>,
    /// Whether the dock tunes the palette rather than the section's sizes.
    tuning_colours: bool,
    /// The colour token the dock's editor holds.
    picked: &'static str,
    /// The editor's [`CHANNELS`] sliders.
    channels: Vec<Entity<Slider>>,
    /// The editor's field, which takes the colour as CSS writes it.
    colour_field: Entity<TextInput>,
    /// [`tune::generation`] when `theme` was last built.
    generation: u64,
    /// The theme files in `tuned-themes`, read when the dock opens and
    /// after each save or import.
    themes: Vec<SavedTheme>,
}

struct SavedTheme {
    name: String,
    path: PathBuf,
    /// Its overrides, to tell the one on show.
    entries: Vec<String>,
}

impl Catalogue {
    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        ui::init(cx);
        tune::enable();
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
            dropdown("Opens up", &["Low", "Medium", "High"], |d| {
                d.opens_up = true
            }),
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

        let this = cx.weak_entity();
        let channels = (0..CHANNELS.len())
            .map(|index| {
                let this = this.clone();
                let (label, scale, unit) = CHANNELS[index];
                cx.new(|_| {
                    let mut s = Slider::new(0., 1., 0., theme, move |value, _, _, cx| {
                        this.update(cx, |s, cx| s.set_channel(index, value, cx))
                            .ok();
                    });
                    s.set_caption(label, "");
                    s.set_unit(scale, unit);
                    s
                })
            })
            .collect();
        let colour_field = cx.new(|cx| {
            TextInput::new(cx, String::new(), theme, move |text, _, cx| {
                this.update(cx, |s, cx| s.type_colour(&text, cx)).ok();
            })
        });

        let wanted = std::env::var("SUBTAKE_GALLERY_COMPONENTS").unwrap_or_default();
        let (wanted, tune_wanted, colours_wanted) = match (
            wanted.strip_suffix("+tune"),
            wanted.strip_suffix("+colours"),
        ) {
            (Some(section), _) => (section.to_owned(), true, false),
            (_, Some(section)) => (section.to_owned(), true, true),
            _ => (wanted, false, false),
        };
        let jump = (!wanted.is_empty())
            .then(|| {
                SECTIONS
                    .iter()
                    .position(|s| s.to_lowercase().starts_with(&wanted.to_lowercase()))
            })
            .flatten();
        let tuning = jump.filter(|_| tune_wanted);
        if let Ok(tuned) = std::env::var("SUBTAKE_GALLERY_TUNED") {
            for entry in tune::load_text(&tuned) {
                eprintln!("SUBTAKE_GALLERY_TUNED: no metric or colour to tune in {entry:?}");
            }
        }
        // The editor drew before tuning was on.
        cx.refresh_windows();

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
            reads: SECTIONS.iter().map(|_| Reads::default()).collect(),
            tuning,
            tuners: HashMap::new(),
            tuning_colours: colours_wanted,
            picked: "accent",
            channels,
            colour_field,
            // `theme` was built before the presets were applied; the first
            // frame rebuilds it.
            generation: u64::MAX,
            themes: saved_themes(),
        }
    }

    fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        self.retint(cx);
        cx.notify();
    }

    /// Rebuild the page's theme, tuned colours and all, and hand it to every
    /// retained control on the page.
    fn retint(&mut self, cx: &mut Context<Self>) {
        self.generation = tune::generation();
        self.theme = if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        };
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
        let dock = legible(theme);
        for s in self.tuners.values().chain(&self.channels) {
            s.update(cx, |s, cx| {
                s.theme = dock;
                cx.notify();
            });
        }
        self.colour_field.update(cx, |i, cx| {
            i.theme = dock;
            cx.notify();
        });
    }

    fn appearance(&self) -> Appearance {
        if self.dark {
            Appearance::Dark
        } else {
            Appearance::Light
        }
    }

    /// Move one channel of the chosen colour, from its slider.
    fn set_channel(&mut self, index: usize, value: f32, cx: &mut Context<Self>) {
        let Some(colour) = tune::colour(self.picked) else {
            return;
        };
        let appearance = self.appearance();
        let mut hsla = colour.value(appearance);
        match index {
            0 => hsla.h = value,
            1 => hsla.s = value,
            2 => hsla.l = value,
            _ => hsla.a = value,
        }
        tune::set_colour(appearance, colour.name, hsla);
        cx.refresh_windows();
    }

    /// Set the chosen colour from what was typed into its field. Anything
    /// that is not a CSS colour is dropped, and the field goes back to the
    /// colour's value on the next frame.
    fn type_colour(&mut self, text: &str, cx: &mut Context<Self>) {
        if let (Some(colour), Some(value)) = (tune::colour(self.picked), tune::parse_colour(text)) {
            tune::set_colour(self.appearance(), colour.name, value);
        }
        cx.refresh_windows();
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
            .gap(px(Theme::gap_large()))
            // The editor's titlebar: its height, clear of the traffic lights,
            // and the drag that moves the window, since the titlebar is
            // see-through and this is all of it there is.
            .h(px(Theme::titlebar_height()))
            .pl(px(Theme::titlebar_traffic_lights()))
            .pr(px(Theme::inset()))
            .on_mouse_down(MouseButton::Left, |_, w, _| w.start_window_move())
            .border_b_1()
            .border_color(theme.line)
            .child(ui::panel_title("Components").text_color(theme.text))
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(Theme::font_secondary()))
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
        tune::unrecorded(|| self.backdrop_stage())
    }

    fn backdrop_stage(&self) -> Div {
        let stage = div()
            .relative()
            .w_full()
            .h(px(STAGE_HEIGHT))
            .rounded(px(Theme::radius_frame()))
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
                        .gap(px(Theme::inset()))
                        .children((0..24).map(|i| {
                            div().w(px(Theme::gap())).h_full().bg(if i % 2 == 0 {
                                hsla(0., 0., 1., 0.55)
                            } else {
                                hsla(0., 0., 0., 0.35)
                            })
                        })),
                ),
        }
    }

    fn buttons(&mut self, cx: &mut Context<Self>) -> Section {
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
                        make("icon").glyph(glyph).icon_only().into_any_element(),
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
                    size(ui::icon_button(
                        id("icon"),
                        "Gear-regular",
                        "Settings",
                        theme,
                    ))
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
                        .glyph_size(Theme::icon_size_medium())
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
                        .gap(px(Theme::gap()))
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
        let pod = ui::pod(theme)
            .flex_col()
            .gap(px(Theme::gap_small()))
            .children(tools.iter().enumerate().map(|(i, (glyph, label))| {
                ui::tool_button(
                    SharedString::from(format!("tool-{label}")),
                    glyph,
                    *label,
                    self.tool == i,
                    theme,
                    Self::click(cx, move |s| s.tool = i),
                )
            }));
        let row_pod = ui::pod_small(theme).gap(px(Theme::gap_small())).children(
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
        section(
            0,
            "Every variant at rest, with a glyph, icon-only, selected and disabled; every size; the modifiers.",
            rows,
        )
    }

    fn pickers(&mut self, cx: &mut Context<Self>) -> Section {
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
        rows.extend(self.dropdowns.iter().map(|(label, d)| {
            specimen(
                theme,
                &format!("Dropdown, {}", label.to_lowercase()),
                vec![wide(d.clone())],
            )
        }));
        section(
            1,
            "Segments, switches and dropdowns. Every one is live.",
            rows.into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
        )
    }

    fn sliders_and_fields(&mut self, cx: &mut Context<Self>) -> Section {
        let theme = self.theme;
        let mut rows: Vec<AnyElement> = self
            .sliders
            .iter()
            .map(|(label, s)| {
                specimen(
                    theme,
                    &format!("Slider, {}", label.to_lowercase()),
                    vec![wide(s.clone())],
                )
                .into_any_element()
            })
            .collect();
        rows.extend(self.inputs.iter().map(|(label, i)| {
            specimen(
                theme,
                &format!("Text input, {}", label.to_lowercase()),
                vec![wide(i.clone())],
            )
            .into_any_element()
        }));
        rows.extend(self.timecodes.iter().map(|(label, t)| {
            specimen(
                theme,
                &format!("Timecode, {}", label.to_lowercase()),
                vec![wide(t.clone())],
            )
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
        section(
            2,
            "Retained controls: drag the sliders and scrub or type in the fields.",
            rows,
        )
    }

    fn menus(&mut self, cx: &mut Context<Self>) -> Section {
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
                        [
                            "Zoom", "Text", "Arrow", "Blur", "Audio", "Caption", "Trim", "Speed",
                            "Marker",
                        ]
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
        let stage = self.backdrop().child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_start()
                .justify_center()
                .gap(px(Theme::inset()))
                .p(px(Theme::inset()))
                .child(ui::frosted(Theme::radius_menu(), ui::MENU_BLUR, menu))
                .child(ui::frosted(Theme::radius_menu(), ui::MENU_BLUR, commands)),
        );
        section(
            3,
            "Left: a checked row, the highlighted row (click one to move it), a shortcut, a submenu, a destructive row. Right: the palette's rows, which have no tick gutter, scrolling under the edge fades.",
            vec![stage.into_any_element()],
        )
    }

    fn tiles(&mut self, cx: &mut Context<Self>) -> Section {
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
                .child(div().text_size(px(Theme::font_small())).child(*label))
                .into_any_element()
            })
            .chain([ui::choice_tile("choice-disabled", false, false, theme)
                .child(ui::icon("Cursor-regular", theme.text))
                .child(div().text_size(px(Theme::font_small())).child("Disabled"))
                .into_any_element()])
            .collect();
        section(
            4,
            "Click to pick. The empty state is the editor's with nothing open.",
            vec![
                specimen(theme, "Swatch", swatches).into_any_element(),
                specimen(theme, "Media tile", media).into_any_element(),
                specimen(
                    theme,
                    "Choice tile",
                    vec![
                        ui::tile_grid(4)
                            .w_full()
                            .children(choices)
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Empty state",
                    vec![
                        div()
                            .flex()
                            .w_full()
                            .h(px(EMPTY_HEIGHT))
                            .child(ui::empty_state(
                                theme,
                                "Nothing open yet",
                                "Record something new, or open a recording to edit it.",
                            ))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            ],
        )
    }

    fn status(&mut self, cx: &mut Context<Self>) -> Section {
        let theme = self.theme;
        let footer = ui::composer_footer(theme)
            .child(
                ui::icon_button("footer-a", "FolderOpen-regular", "File", theme)
                    .ghost()
                    .small()
                    .toggled(false),
            )
            .child(
                ui::icon_button("footer-b", "Plus-regular", "Add", theme)
                    .ghost()
                    .small()
                    .toggled(true),
            )
            .child("A quiet hint");
        section(
            5,
            "Chips, progress and the tooltips a hover brings up, drawn in place.",
            vec![
                specimen(
                    theme,
                    "Status",
                    vec![
                        ui::status_dot(theme).into_any_element(),
                        ui::status_chip(theme)
                            .child("Gallery mode")
                            .into_any_element(),
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
                        .map(|f| {
                            ui::progress_bar(*f, theme)
                                .w(px(PROGRESS_WIDTH))
                                .into_any_element()
                        })
                        .collect(),
                )
                .into_any_element(),
                specimen(theme, "Composer footer", vec![footer.into_any_element()])
                    .into_any_element(),
                specimen(
                    theme,
                    "Tooltip",
                    vec![
                        ui::tooltip("Split clip at playhead", theme, cx).into_any_element(),
                        ui::tooltip_detail(
                            "Snap",
                            "Regions snap to the playhead and each other",
                            theme,
                            cx,
                        )
                        .into_any_element(),
                    ],
                )
                .into_any_element(),
            ],
        )
    }

    fn rows_and_cards(&mut self, cx: &mut Context<Self>) -> Section {
        let theme = self.theme;
        section(
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
                    vec![wide(ui::setting_card(
                        theme,
                        "Suggest zooms",
                        "Finds the clicks and typing in a take and zooms into them.",
                    ))],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Group card",
                    vec![wide(ui::group_card(theme, "Shadow").child(
                        ui::field_row(theme, "Enabled").child(ui::switch(
                            "group-switch",
                            true,
                            true,
                            theme,
                            |_, _, _| {},
                        )),
                    ))],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Panel header",
                    vec![wide(ui::panel_header(theme, "Background"))],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "Caps label, divider",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Theme::gap()))
                            .w(px(CONTROL_WIDTH))
                            .child(ui::caps_label("Appearance", theme))
                            .child(ui::divider(theme))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            ],
        )
    }

    fn frost(&mut self, _: &mut Context<Self>) -> Section {
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
                .p(px(Theme::gap_large()))
                .flex()
                .flex_col()
                .gap(px(Theme::gap_small()))
                .child(
                    div()
                        .text_size(px(Theme::font_body()))
                        .text_color(theme.text)
                        .child(name.to_owned()),
                )
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
                .gap(px(Theme::inset()))
                .p(px(Theme::inset()))
                .children(surfaces.iter().map(|(name, s)| {
                    sample(
                        name,
                        ui::panel_variant(theme, *s),
                        s.radius(),
                        Some(s.blur()),
                    )
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
                .gap(px(Theme::inset()))
                .p(px(Theme::inset()))
                .children(blurs.iter().map(|(name, blur)| {
                    sample(
                        name,
                        ui::panel_variant(theme, Surface::Panel),
                        Surface::Panel.radius(),
                        *blur,
                    )
                }))
                .child(ui::frosted(
                    Surface::Pod.radius(),
                    Surface::Pod.blur(),
                    ui::pod(theme)
                        .flex_col()
                        .gap(px(Theme::gap_small()))
                        .child(
                            ui::icon_button("frost-pod-a", "Sparkle-regular", "Scene", theme)
                                .ghost()
                                .selected(true),
                        )
                        .child(
                            ui::icon_button("frost-pod-b", "Cursor-regular", "Cursor", theme)
                                .ghost(),
                        ),
                ))
                .child(ui::frosted(
                    Surface::Pod.radius(),
                    Surface::Pod.blur(),
                    ui::pod_small(theme)
                        .gap(px(Theme::gap_small()))
                        .child(
                            ui::button("frost-thin-a", "Native", theme)
                                .compact()
                                .caret(),
                        )
                        .child(ui::button("frost-thin-b", "Crop", theme).compact()),
                )),
        );
        let fading = ui::content_panel(theme)
            .w(px(CONTROL_WIDTH))
            .p(px(Theme::gap()))
            .child(
                ui::fade_edges(
                    div()
                        .id("fade-list")
                        .flex()
                        .flex_col()
                        .gap(px(Theme::gap_small()))
                        .h(px(SCROLL_MENU_HEIGHT))
                        .overflow_y_scroll()
                        .track_scroll(&self.fade_scroll)
                        .children((1..=14).map(|i| {
                            ui::field_row(theme, format!("Row {i}"))
                                .child(ui::mono_small("00:00", theme))
                        })),
                )
                .tracking(&self.fade_scroll)
                .thumb(theme.muted, Theme::gap_small()),
            );
        section(
            7,
            "Each plane frosted at its own radius and blur over the backdrop (the header picks it), the four blur strengths side by side with one left unfrosted, the two pods, and a scroll's edge fades and thumb.",
            vec![
                planes.into_any_element(),
                strengths.into_any_element(),
                specimen(theme, "Edge fades, thumb", vec![fading.into_any_element()])
                    .into_any_element(),
            ],
        )
    }

    fn type_and_icons(&mut self, _: &mut Context<Self>) -> Section {
        let theme = self.theme;
        let scale = [
            (
                "Display 40",
                ui::title("Nothing open yet", Theme::font_display()),
            ),
            ("Panel 22", ui::panel_title("Background")),
            ("Heading 17", ui::heading("Shadow")),
            (
                "Action 15",
                div()
                    .text_size(px(Theme::font_action()))
                    .child("Start recording"),
            ),
            (
                "Body 13",
                div()
                    .text_size(px(Theme::font_body()))
                    .child("Regions snap to the playhead."),
            ),
            (
                "Secondary 12",
                div()
                    .text_size(px(Theme::font_secondary()))
                    .child("Finds the clicks in a take."),
            ),
            (
                "Small 11",
                div()
                    .text_size(px(Theme::font_small()))
                    .child("Gallery mode"),
            ),
            ("Mono", ui::mono("00:37.50 / 02:28")),
            ("Mono small", ui::mono_small("3240 × 1820", theme)),
        ];
        let mut rows: Vec<AnyElement> = scale
            .into_iter()
            .map(|(label, specimen_text)| {
                specimen(
                    theme,
                    label,
                    vec![specimen_text.text_color(theme.text).into_any_element()],
                )
                .into_any_element()
            })
            .collect();
        let muted = specimen(
            theme,
            "Text colours",
            vec![
                div()
                    .text_color(theme.text)
                    .child("text")
                    .into_any_element(),
                div()
                    .text_color(theme.muted)
                    .child("muted")
                    .into_any_element(),
                div()
                    .text_color(theme.accent)
                    .child("accent")
                    .into_any_element(),
                div()
                    .text_color(theme.danger)
                    .child("danger")
                    .into_any_element(),
            ],
        );
        rows.push(muted.into_any_element());
        rows.push(
            div()
                .flex()
                .flex_wrap()
                .gap(px(Theme::gap()))
                .children(self.icons.iter().map(|name| {
                    div()
                        .w(px(ICON_CELL))
                        .flex()
                        .items_center()
                        .gap(px(Theme::gap()))
                        .child(ui::icon(name, theme.text))
                        .child(
                            div()
                                .min_w_0()
                                .text_ellipsis()
                                .text_size(px(Theme::font_small()))
                                .text_color(theme.muted)
                                .child(name.clone()),
                        )
                }))
                .into_any_element(),
        );
        section(8, "The type scale and every bundled glyph.", rows)
    }

    fn parked(&mut self, cx: &mut Context<Self>) -> Section {
        let theme = self.theme;
        let thumb = |c: u32| {
            div()
                .size_full()
                .rounded(px(Theme::radius_lane()))
                .bg(rgb(c))
        };
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
            .rounded(px(Theme::radius_frame()))
            .overflow_hidden()
            .child(unused::scrim(theme).absolute().inset_0())
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        unused::dialog(theme)
                            .child(ui::heading("Discard changes?").text_color(theme.text))
                            .child(
                                div()
                                    .text_size(px(Theme::font_body()))
                                    .text_color(theme.muted)
                                    .child("The take keeps its last saved edit."),
                            )
                            .child(
                                unused::dialog_actions()
                                    .child(ui::button("dialog-cancel", "Cancel", theme).dialog())
                                    .child(
                                        ui::button("dialog-discard", "Discard", theme)
                                            .danger()
                                            .dialog(),
                                    ),
                            ),
                    ),
            );
        section(
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
                        wide(unused::field(
                            "park-field",
                            "MagnifyingGlassPlus-regular",
                            "Search",
                            "",
                            theme,
                        )),
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
                        unused::toast("Export finished", theme.accent, None, theme)
                            .into_any_element(),
                        unused::toast(
                            "Region deleted",
                            theme.danger,
                            Some((
                                "Undo".into(),
                                Box::new(|_: &ClickEvent, _: &mut Window, _: &mut App| {}),
                            )),
                            theme,
                        )
                        .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(
                    theme,
                    "List row",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Theme::gap_small()))
                            .w_full()
                            .children(list.iter().enumerate().map(
                                |(i, (title, selected, state))| {
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
                                },
                            ))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
                specimen(theme, "Scrim, dialog", vec![dialog.into_any_element()])
                    .into_any_element(),
            ],
        )
    }
}

impl Catalogue {
    /// A section: its heading, a line on what it shows, its Tune button and
    /// list, and its specimens on the inspector's own plane, which is what
    /// the controls are drawn to sit on. The heading and plane are the
    /// catalogue's, so only the specimens' reads are recorded.
    fn section(&mut self, cx: &mut Context<Self>, section: Section) -> Div {
        let Section { index, note, rows } = section;
        let theme = self.theme;
        tune::unrecorded(|| {
            let open = self.tuning == Some(index);
            let count = self.reads[index].borrow().len();
            div()
                .flex()
                .flex_col()
                .gap(px(Theme::gap()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(Theme::gap_large()))
                        .child(ui::heading(SECTIONS[index]).text_color(theme.text))
                        .child(div().flex_1())
                        .child(
                            ui::button(
                                SharedString::from(format!("tune-{index}")),
                                format!("Tune · {count}"),
                                theme,
                            )
                            .compact()
                            .glyph("SlidersHorizontal-regular")
                            .toggled(open)
                            .on_click(Self::click(cx, move |s| {
                                s.tuning = if s.tuning == Some(index) {
                                    None
                                } else {
                                    Some(index)
                                }
                            })),
                        ),
                )
                .child(
                    div()
                        .text_size(px(Theme::font_secondary()))
                        .text_color(theme.muted)
                        .child(note.to_owned()),
                )
                .child(Recorded {
                    reads: self.reads[index].clone(),
                    child: ui::panel(theme)
                        .flex()
                        .flex_col()
                        .gap(px(Theme::gap_large()))
                        .p(px(Theme::inset()))
                        .children(rows)
                        .into_any_element(),
                })
        })
    }

    /// The tuning dock: every metric a section's specimens read, grouped as
    /// `metrics.rs` groups them, each with a slider, a nudge either way and
    /// a reset.
    fn tuner(&mut self, window: &Window, cx: &mut Context<Self>, index: usize) -> Div {
        let theme = legible(self.theme);
        let reads = self.reads[index].borrow().clone();
        let colours = self.tuning_colours;
        let (title, note) = if colours {
            let palette = if self.dark { "dark" } else { "light" };
            (
                format!("Tuning the {palette} palette"),
                "Every token: a colour is not recorded per section.",
            )
        } else {
            (
                format!("Tuning {}", SECTIONS[index]),
                "Live in every window. A metric is one value app-wide.",
            )
        };
        // `metrics.rs` order keeps a group's metrics together.
        let mut groups: Vec<(&str, Vec<AnyElement>)> = Vec::new();
        for metric in tune::metrics().iter().filter(|m| reads.contains(m.name)) {
            let row = tuner_row(metric, self.tuner_slider(cx, metric), theme).into_any_element();
            match groups.last_mut() {
                Some((group, rows)) if *group == metric.group => rows.push(row),
                _ => groups.push((metric.group, vec![row])),
            }
        }
        let list = div()
            .id("tune-dock")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(Theme::gap_block()))
            .px(px(Theme::inset()))
            .pb(px(Theme::inset()));
        let dock = div()
            .flex()
            .flex_col()
            .flex_none()
            .h(px(TUNE_DOCK_HEIGHT))
            .border_t_1()
            .border_color(theme.line)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::gap_large()))
                    .p(px(Theme::inset()))
                    .child(ui::heading(title).text_color(theme.text))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(Theme::font_secondary()))
                            .text_color(theme.muted)
                            .child(note),
                    )
                    .child(
                        div()
                            .w(px(SAMPLE_WIDTH))
                            .flex_none()
                            .child(ui::segmented_control(
                                "tune-view",
                                &["Sizes", "Colours"],
                                usize::from(colours),
                                theme,
                                Self::with(cx, |s, i| s.tuning_colours = i == 1),
                            )),
                    )
                    .child(
                        ui::icon_button("tune-close", "X-regular", "Close", theme)
                            .small()
                            .ghost()
                            .on_click(Self::click(cx, |s| s.tuning = None)),
                    ),
            );
        let body = if colours {
            self.colour_tuner(window, cx).into_any_element()
        } else if groups.is_empty() {
            list.child(
                div()
                    .text_size(px(Theme::font_secondary()))
                    .text_color(theme.muted)
                    .child("Nothing read yet: scroll the section into view."),
            )
            .into_any_element()
        } else {
            self.metric_groups(list, groups, theme).into_any_element()
        };
        let changes = tune::changes();
        dock.child(self.theme_row(cx, theme)).child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(div().flex_1().min_w_0().flex().flex_col().child(body))
                .when(!changes.is_empty(), |row| {
                    row.child(self.change_stack(cx, changes, theme))
                }),
        )
    }

    /// The Sizes view's metrics, under the group each belongs to.
    fn metric_groups(
        &self,
        list: Stateful<Div>,
        groups: Vec<(&str, Vec<AnyElement>)>,
        theme: Theme,
    ) -> Stateful<Div> {
        list.children(groups.into_iter().map(|(group, rows)| {
            div()
                .flex()
                .flex_col()
                .gap(px(Theme::gap()))
                .child(ui::caps_label(group.to_owned(), theme))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x(px(Theme::gap_large()))
                        .gap_y(px(Theme::gap()))
                        .children(rows),
                )
        }))
    }

    /// The saved themes, the one on show marked, with saving the current
    /// overrides as another and importing one from elsewhere.
    fn theme_row(&self, cx: &mut Context<Self>, theme: Theme) -> Div {
        let current = theme_entries(&tune::as_text());
        let chips = self.themes.iter().enumerate().map(|(index, saved)| {
            let path = saved.path.clone();
            ui::button(
                SharedString::from(format!("tune-theme-{index}")),
                saved.name.clone(),
                theme,
            )
            .compact()
            .selected(tune::changed() > 0 && saved.entries == current)
            .on_click(cx.listener(move |s, _, _, cx| s.load_theme(&path, cx)))
        });
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(Theme::gap_small()))
            .px(px(Theme::inset()))
            .pb(px(Theme::inset()))
            .child(ui::caps_label("Themes", theme))
            .when(self.themes.is_empty(), |row| {
                row.child(
                    div()
                        .text_size(px(Theme::font_secondary()))
                        .text_color(theme.muted)
                        .child("None saved yet."),
                )
            })
            .children(chips)
            .child(div().flex_1())
            .child(
                ui::button("tune-theme-save", "Save as…", theme)
                    .compact()
                    .enabled(tune::changed() > 0)
                    .on_click(cx.listener(|s, _, _, cx| s.save_theme(cx))),
            )
            .child(
                ui::button("tune-theme-import", "Import…", theme)
                    .compact()
                    .on_click(cx.listener(|s, _, _, cx| s.import_theme(cx))),
            )
    }

    fn save_theme(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = themes_folder() else {
            return;
        };
        if let Err(error) = std::fs::create_dir_all(&folder) {
            eprintln!("Tuned themes: could not make {}: {error}", folder.display());
            return;
        }
        let dialog = rfd::FileDialog::new()
            .set_directory(&folder)
            .add_filter("Tuned theme", &[THEME_EXTENSION])
            .set_file_name(format!("Theme {}.{THEME_EXTENSION}", self.themes.len() + 1));
        Self::after_dialog(
            cx,
            move || dialog.save_file(),
            |s, path, cx| {
                let text = format!("# SubTake tuned theme\n{}", tune::as_text());
                if let Err(error) = std::fs::write(&path, text) {
                    eprintln!("Tuned themes: could not write {}: {error}", path.display());
                }
                s.themes = saved_themes();
                cx.notify();
            },
        );
    }

    /// Run a native file dialog, then `then` with the path it chose.
    ///
    /// Not from inside the click: a dialog runs a modal loop, and in the
    /// click's handler the app is already borrowed, so the first frame or
    /// timer to fire under the dialog panicked on borrowing it again. A task
    /// runs the dialog between updates instead, as the editor's own pump
    /// runs its commands.
    fn after_dialog(
        cx: &mut Context<Self>,
        dialog: impl FnOnce() -> Option<PathBuf> + 'static,
        then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            if let Some(path) = dialog() {
                this.update(cx, |s, cx| then(s, path, cx)).ok();
            }
        })
        .detach();
    }

    /// Load a theme from anywhere, keeping a copy among the saved ones.
    fn import_theme(&mut self, cx: &mut Context<Self>) {
        let dialog = rfd::FileDialog::new().add_filter("Tuned theme", &[THEME_EXTENSION, "txt"]);
        Self::after_dialog(cx, move || dialog.pick_file(), Self::import_from);
    }

    fn import_from(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(folder) = themes_folder()
            && path.parent() != Some(folder.as_path())
            && let Some(name) = path.file_stem()
        {
            let copy = folder.join(name).with_extension(THEME_EXTENSION);
            let copied = std::fs::create_dir_all(&folder).and_then(|()| {
                if copy.exists() {
                    Ok(())
                } else {
                    std::fs::copy(&path, &copy).map(drop)
                }
            });
            if let Err(error) = copied {
                eprintln!("Tuned themes: could not keep {}: {error}", copy.display());
            }
        }
        self.load_theme(&path, cx);
        self.themes = saved_themes();
    }

    fn load_theme(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                for entry in tune::load_text(&text) {
                    eprintln!(
                        "{}: no metric or colour to tune in {entry:?}",
                        path.display()
                    );
                }
            }
            Err(error) => eprintln!("Tuned themes: could not read {}: {error}", path.display()),
        }
        cx.refresh_windows();
    }

    /// Everything tuned so far, newest palette and metric alike, each with
    /// what it was, what it is, and its own line to copy — so a colour can
    /// be taken back to `palette.rs` one at a time, not only all at once.
    fn change_stack(
        &self,
        cx: &mut Context<Self>,
        changes: Vec<tune::Change>,
        theme: Theme,
    ) -> Stateful<Div> {
        let rows = changes.into_iter().map(|change| {
            let id = |slot: &str| {
                let palette = match change.value {
                    tune::ChangeValue::Colour { appearance, .. } => {
                        if appearance.is_dark() {
                            "dark"
                        } else {
                            "light"
                        }
                    }
                    tune::ChangeValue::Size { .. } => "size",
                };
                SharedString::from(format!("tune-change-{slot}-{palette}-{}", change.name))
            };
            let (lead, values, reset): (AnyElement, String, Box<dyn Fn(&mut App)>) =
                match change.value {
                    tune::ChangeValue::Colour {
                        appearance,
                        from,
                        to,
                    } => {
                        let name = change.name;
                        (
                            div()
                                .flex()
                                .flex_none()
                                .gap(px(Theme::hairline_width()))
                                .child(colour_swatch(from, CHIP_SWATCH, theme))
                                .child(colour_swatch(to, CHIP_SWATCH, theme))
                                .into_any_element(),
                            // The swatches show what it was; the words, what
                            // to paste.
                            format!(
                                "{} · {}",
                                if appearance.is_dark() {
                                    "dark"
                                } else {
                                    "light"
                                },
                                tune::hex(to)
                            ),
                            Box::new(move |cx: &mut App| {
                                tune::reset_colour(appearance, name);
                                cx.refresh_windows();
                            }),
                        )
                    }
                    tune::ChangeValue::Size { from, to } => {
                        let name = change.name;
                        (
                            div()
                                .w(px(CHIP_SWATCH * 2. + Theme::hairline_width()))
                                .flex_none()
                                .into_any_element(),
                            format!("{from} → {to}"),
                            Box::new(move |cx: &mut App| {
                                tune::reset(name);
                                cx.refresh_windows();
                            }),
                        )
                    }
                };
            let line = change.line.clone();
            let pick = matches!(change.value, tune::ChangeValue::Colour { .. })
                .then_some((change.name, change.value));
            div()
                .id(id("row"))
                .flex()
                .items_center()
                .gap(px(Theme::gap_small()))
                .p(px(Theme::gap_small()))
                .rounded(px(Theme::radius_region()))
                .hover(|row| row.bg(theme.sunk2))
                .when_some(pick, |row, (name, value)| {
                    // A colour opens in the editor, in its own palette.
                    row.on_click(cx.listener(move |s, _, _, cx| {
                        if let tune::ChangeValue::Colour { appearance, .. } = value {
                            s.tuning_colours = true;
                            s.picked = name;
                            s.set_dark(appearance.is_dark(), cx);
                        }
                    }))
                })
                .child(lead)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_ellipsis()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(Theme::font_secondary()))
                                .text_color(theme.text)
                                .child(change.name),
                        )
                        .child(
                            div()
                                .text_ellipsis()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(Theme::font_small()))
                                .text_color(theme.accent)
                                .child(values),
                        ),
                )
                .child(
                    ui::icon_button(id("copy"), "Stack-regular", "Copy line", theme)
                        .small()
                        .ghost()
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(line.clone()))
                        }),
                )
                .child(
                    ui::icon_button(id("reset"), "ArrowCounterClockwise-regular", "Reset", theme)
                        .small()
                        .ghost()
                        .on_click(move |_, _, cx| reset(cx)),
                )
        });
        div()
            .id("tune-changes")
            .w(px(CHANGES_WIDTH))
            .flex_none()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(Theme::gap_small()))
            .pr(px(Theme::inset()))
            .pb(px(Theme::inset()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::gap_small()))
                    .child(div().flex_1().child(ui::caps_label("Changes", theme)))
                    .child(
                        ui::button("tune-copy", "Copy all", theme)
                            .compact()
                            .on_click(|_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(tune::as_rust()))
                            }),
                    )
                    .child(
                        ui::button("tune-reset", "Reset all", theme)
                            .compact()
                            .on_click(|_, _, cx| {
                                tune::reset_all();
                                cx.refresh_windows();
                            }),
                    ),
            )
            // Before and after: the files as written, or with every
            // override on. Tuning anything shows it again.
            .child(ui::segmented_control(
                "tune-compare",
                &["Original", "Tuned"],
                usize::from(!tune::showing_original()),
                theme,
                |index, _, cx| {
                    tune::show_original(index == 0);
                    cx.refresh_windows();
                },
            ))
            .children(rows)
    }

    /// The slider for one metric, made the first time it is shown and
    /// brought to the metric's current value every frame after, so a
    /// derived metric follows the ones it is built from.
    fn tuner_slider(&mut self, cx: &mut Context<Self>, metric: &'static Metric) -> Entity<Slider> {
        let theme = legible(self.theme);
        let slider = self
            .tuners
            .entry(metric.name)
            .or_insert_with(|| {
                let (minimum, maximum) = tune_range(metric.default);
                cx.new(|_| {
                    let mut s = Slider::new(
                        minimum,
                        maximum,
                        metric.value(),
                        theme,
                        move |value, _, _, cx| {
                            tune::set(metric.name, snap(metric.default, value));
                            cx.refresh_windows();
                        },
                    );
                    s.set_caption(metric.name, "");
                    s
                })
            })
            .clone();
        let value = metric.value();
        slider.update(cx, |s, _| s.sync(value));
        slider
    }

    /// The dock's Colours view: the chosen token's editor, and every token
    /// of the palette on show as a chip that chooses it.
    fn colour_tuner(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = legible(self.theme);
        let appearance = self.appearance();
        let picked = tune::colour(self.picked).unwrap_or(&tune::colours()[0]);
        let value = picked.value(appearance);
        for (slider, channel) in self
            .channels
            .iter()
            .zip([value.h, value.s, value.l, value.a])
        {
            slider.update(cx, |s, _| s.sync(channel));
        }
        let written = tune::hex(value);
        self.colour_field
            .update(cx, |i, _| i.sync(&written, window));

        let overridden = picked.overridden(appearance);
        let mut detail = String::new();
        if overridden {
            detail.push_str(&format!(
                "Default {}. ",
                tune::hex(picked.default(appearance))
            ));
        }
        detail.push_str(picked.doc);
        let editor = div()
            .w(px(CONTROL_WIDTH))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(Theme::gap()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::gap()))
                    .child(colour_swatch(value, PICKED_SWATCH, theme))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(Theme::font_heading()))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(picked.name),
                    )
                    .child(
                        ui::icon_button(
                            SharedString::from(format!("tune-reset-colour-{}", picked.name)),
                            "ArrowCounterClockwise-regular",
                            "Reset",
                            theme,
                        )
                        .small()
                        .ghost()
                        .enabled(overridden)
                        .on_click(move |_, _, cx| {
                            tune::reset_colour(appearance, picked.name);
                            cx.refresh_windows();
                        }),
                    ),
            )
            .children(self.channels.iter().cloned())
            .child(self.colour_field.clone())
            .child(
                div()
                    .text_size(px(Theme::font_small()))
                    .text_color(if overridden {
                        theme.accent
                    } else {
                        theme.muted
                    })
                    .child(detail),
            );

        // `struct Theme` order keeps a group's tokens together.
        let mut groups: Vec<(&str, Vec<AnyElement>)> = Vec::new();
        for colour in tune::colours() {
            let chip = self.colour_chip(cx, colour, colour.name == picked.name);
            match groups.last_mut() {
                Some((group, chips)) if *group == colour.group => chips.push(chip),
                _ => groups.push((colour.group, vec![chip])),
            }
        }
        let chips = div()
            .id("tune-colours")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(Theme::gap_block()))
            .children(groups.into_iter().map(|(group, chips)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Theme::gap()))
                    .child(ui::caps_label(group.to_owned(), theme))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(Theme::gap_small()))
                            .children(chips),
                    )
            }));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(Theme::gap_block()))
            .px(px(Theme::inset()))
            .pb(px(Theme::inset()))
            .child(editor)
            .child(chips)
    }

    /// One colour token: its swatch, its name, and its value, in the accent
    /// once it is tuned. Choosing it puts it in the editor.
    fn colour_chip(
        &mut self,
        cx: &mut Context<Self>,
        colour: &'static Colour,
        chosen: bool,
    ) -> AnyElement {
        let theme = legible(self.theme);
        let appearance = self.appearance();
        let value = colour.value(appearance);
        div()
            .id(SharedString::from(format!("tune-colour-{}", colour.name)))
            .w(px(CHIP_WIDTH))
            .flex()
            .items_center()
            .gap(px(Theme::gap_small()))
            .p(px(Theme::gap_small()))
            .rounded(px(Theme::radius_region()))
            .border_1()
            .border_color(if chosen {
                theme.accent
            } else {
                gpui::transparent_black()
            })
            .when(chosen, |chip| chip.bg(theme.sunk))
            .hover(|chip| chip.bg(theme.sunk2))
            .on_click(Self::click(cx, move |s| s.picked = colour.name))
            .child(colour_swatch(value, CHIP_SWATCH, theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_ellipsis()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(Theme::font_secondary()))
                            .text_color(theme.text)
                            .child(colour.name),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_small()))
                            .text_color(if colour.overridden(appearance) {
                                theme.accent
                            } else {
                                theme.muted
                            })
                            .child(tune::hex(value)),
                    ),
            )
            .into_any_element()
    }
}

/// The dock's theme: secondary text at the primary text's colour. The dock
/// sits on the bare window frost, where `muted` fell too near the ground to
/// read, in both palettes.
fn legible(theme: Theme) -> Theme {
    Theme {
        muted: theme.text,
        ..theme
    }
}

/// A colour on a ground half white and half black, so a translucent token
/// shows what it does to either.
fn colour_swatch(colour: Hsla, size: f32, theme: Theme) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .relative()
        .overflow_hidden()
        .rounded(px(SWATCH_RADIUS))
        .border_1()
        .border_color(theme.line)
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .child(div().flex_1().bg(gpui::white()))
                .child(div().flex_1().bg(gpui::black())),
        )
        .child(div().absolute().inset_0().bg(colour))
}

/// What a section method builds: which section, the line on what it
/// shows, and its specimens. [`Catalogue::section`] frames it.
struct Section {
    index: usize,
    note: &'static str,
    rows: Vec<AnyElement>,
}

fn section(index: usize, note: &'static str, rows: Vec<AnyElement>) -> Section {
    Section { index, note, rows }
}

/// Draws its child with every metric read in layout, prepaint and paint
/// added to `reads`: the components' renders run there, not when the
/// section is built.
struct Recorded {
    reads: Reads,
    child: AnyElement,
}

impl IntoElement for Recorded {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Recorded {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = &mut self.child;
        (
            tune::record(&self.reads, || child.request_layout(window, cx)),
            (),
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = &mut self.child;
        tune::record(&self.reads, || child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = &mut self.child;
        tune::record(&self.reads, || child.paint(window, cx));
    }
}

/// How far a metric's slider runs: to double its default, with room to
/// grow a small one, and as far below zero as above for a negative one.
fn tune_range(default: f32) -> (f32, f32) {
    let size = default.abs();
    let maximum = if size < 2. {
        (size * 2.).max(2.)
    } else {
        (size * 2.).max(size + 24.)
    };
    (if default < 0. { -maximum } else { 0. }, maximum)
}

/// The step a metric moves in: whole points for a whole size, halves for
/// a half, and hundredths for a factor.
fn tune_step(default: f32) -> f32 {
    if default.abs() < 2. {
        0.01
    } else if default.fract() == 0. {
        1.
    } else {
        0.5
    }
}

fn snap(default: f32, value: f32) -> f32 {
    let step = tune_step(default);
    (value / step).round() * step
}

/// One metric: its slider, nudges and reset, and the first line of its doc.
fn tuner_row(metric: &'static Metric, slider: Entity<Slider>, theme: Theme) -> Div {
    let step = tune_step(metric.default);
    let nudge = if step < 1. { step * 5. } else { step };
    let overridden = metric.overridden();
    let nudge_button = |id: &str, label: &str, by: f32| {
        ui::button(
            SharedString::from(format!("tune-{id}-{}", metric.name)),
            label.to_owned(),
            theme,
        )
        .small()
        .ghost()
        .on_click(move |_, _, cx| {
            tune::set(metric.name, snap(metric.default, metric.value() + by));
            cx.refresh_windows();
        })
    };
    let mut detail = String::new();
    if metric.derived {
        detail.push_str("Derived. ");
    }
    if overridden {
        detail.push_str(&format!("Default {}. ", metric.default));
    }
    detail.push_str(metric.doc);
    div()
        .w(px(TUNE_ROW_WIDTH))
        .flex()
        .flex_col()
        .gap(px(Theme::gap_small()))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(Theme::gap_small()))
                .child(div().flex_1().min_w_0().child(slider))
                .child(nudge_button("less", "−", -nudge))
                .child(nudge_button("more", "+", nudge))
                .child(
                    ui::icon_button(
                        SharedString::from(format!("tune-reset-{}", metric.name)),
                        "ArrowCounterClockwise-regular",
                        "Reset",
                        theme,
                    )
                    .small()
                    .ghost()
                    .enabled(overridden)
                    .on_click(move |_, _, cx| {
                        tune::reset(metric.name);
                        cx.refresh_windows();
                    }),
                ),
        )
        .child(
            div()
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(Theme::font_small()))
                .text_color(if overridden {
                    theme.accent
                } else {
                    theme.muted
                })
                .child(detail),
        )
}

/// One row of a section: what it is on the left, the specimens on the right.
fn specimen(theme: Theme, label: &str, items: Vec<AnyElement>) -> Div {
    tune::unrecorded(|| specimen_row(theme, label, items))
}

fn specimen_row(theme: Theme, label: &str, items: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Theme::gap_large()))
        .child(
            div()
                .w(px(LABEL_WIDTH))
                .flex_none()
                .text_size(px(Theme::font_secondary()))
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
                .gap(px(Theme::gap()))
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
            match (
                self.scroll.bounds_for_item(0),
                self.scroll.bounds_for_item(index),
            ) {
                (Some(first), Some(target)) if self.settled >= JUMP_SETTLE_FRAMES => {
                    self.scroll
                        .set_offset(point(px(0.), first.top() - target.top()));
                    self.jump = None;
                }
                _ => {
                    self.settled += 1;
                    window.request_animation_frame();
                }
            }
        }
        // A tuned colour changes what `Theme::light()` and `dark()` return;
        // rebuild the page's copy, and its retained controls', to match.
        if self.generation != tune::generation() {
            self.retint(cx);
        }
        let header = self.header(cx);
        // Each section is built with its reads recorded, for its Tune list:
        // a builder that sizes itself as it is called reads here, and the
        // rest read as they draw, inside `Recorded`.
        let builders: [fn(&mut Self, &mut Context<Self>) -> Section; SECTIONS.len()] = [
            Self::buttons,
            Self::pickers,
            Self::sliders_and_fields,
            Self::menus,
            Self::tiles,
            Self::status,
            Self::rows_and_cards,
            Self::frost,
            Self::type_and_icons,
            Self::parked,
        ];
        let sections: Vec<Div> = builders
            .iter()
            .enumerate()
            .map(|(i, build)| {
                let reads = self.reads[i].clone();
                let built = tune::record(&reads, || build(self, cx));
                self.section(cx, built)
            })
            .collect();
        let dock = self
            .tuning
            .map(|index| tune::unrecorded(|| self.tuner(window, cx, index)));
        if ui::tick_hover_fades() {
            window.request_animation_frame();
        }
        // The editor's frost: the same native material at the same blur and
        // saturation, with `bg` tinting it, so every specimen sits on what it
        // sits on in the app.
        let saturation = if self.dark {
            Theme::window_saturation_dark()
        } else {
            Theme::window_saturation()
        };
        subtake_native::platform::set_gpui_window_glass(
            window,
            Theme::window_blur(),
            saturation,
            theme.ground,
        );
        div()
            .size_full()
            .font_family(subtake_theme::FONT_SANS)
            .text_color(theme.text)
            .text_size(px(Theme::font_body()))
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
                            .gap(px(Theme::inset() * 2.))
                            .p(px(Theme::inset()))
                            .children(sections),
                    )
                    .children(dock),
            )
    }
}

const THEME_EXTENSION: &str = "subtaketheme";

fn themes_folder() -> Option<PathBuf> {
    subtake_native::preferences::Preferences::directory()
        .ok()
        .map(|directory| directory.join("tuned-themes"))
}

/// Every theme file in `tuned-themes`, by name.
fn saved_themes() -> Vec<SavedTheme> {
    let Some(entries) = themes_folder().and_then(|folder| std::fs::read_dir(folder).ok()) else {
        return Vec::new();
    };
    let mut themes: Vec<SavedTheme> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == THEME_EXTENSION))
        .filter_map(|path| {
            let entries = theme_entries(&std::fs::read_to_string(&path).ok()?);
            Some(SavedTheme {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                path,
                entries,
            })
        })
        .collect();
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}

/// A theme file's entries in a set order, however the file lists them.
fn theme_entries(text: &str) -> Vec<String> {
    let mut entries: Vec<String> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split(','))
        .map(|entry| entry.split_whitespace().collect())
        .filter(|entry: &String| !entry.is_empty())
        .collect();
    entries.sort();
    entries
}
