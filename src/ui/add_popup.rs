//! The Add popup, which the timeline's Add button opens in place of the Add
//! command palette: everything that can be added, grouped by where it lands,
//! beside a card previewing the highlighted kind. The card shows how the
//! kind looks on the picture, where it lands on its lane, and what it is for.
//!
//! It stands beside the Add panel (`add.rs`) for now so the two can be
//! compared; one of them goes. The File, Edit and Help menus stay palettes.
//!
//! Not drawn by the design:
//! - The picture is the take's frame at the playhead, where the design drew a
//!   stand-in window.
//! - The lane strip draws the lane's real regions, eight seconds either side
//!   of the playhead.
//! - A trim is the accent at half strength rather than hatched, which gpui
//!   cannot draw.
//! - Adding closes the popup, since Selection opens on the new region, so the
//!   footer never says "Added …".

use super::*;
use subtake_theme::FONT_MONO;
use subtake_ui::{icon_sized, layered, unused::key_cap};

/// How much of the lane the strip shows, centred on the playhead.
const STRIP_SECONDS: f32 = 16.;
/// How long a new region runs, as `App::add_region` makes it.
const NEW_SECONDS: f32 = 2.;
/// The tallest the picture stands, as a share of its width: a 16:9 frame.
/// A taller take is fitted inside that, not stretched down the card.
const PICTURE_TALLEST: f32 = 9. / 16.;
/// The other regions on the strip: the lane's ink, faint beside the accent.
const OLD_BLOCK_ALPHA: f32 = 0.18;
/// A trim, which the design hatches.
const TRIM_ALPHA: f32 = 0.5;
/// The dark behind a lower third, a caption and a speed badge.
const PLATE_ALPHA: f32 = 0.72;
/// The wash a title lays over the whole picture.
const TITLE_WASH_ALPHA: f32 = 0.42;
/// Everything outside a spotlight.
const SPOTLIGHT_ALPHA: f32 = 0.55;
/// A blur's frosted patch, and the accent under a zoom's frame.
const BLUR_WASH_ALPHA: f32 = 0.18;
const ZOOM_WASH_ALPHA: f32 = 0.08;
/// The second step, faint: the one after this.
const NEXT_STEP_ALPHA: f32 = 0.45;

/// Where a mark sits on the picture, as shares of its width and height:
/// left, top, width, height.
#[derive(Clone, Copy)]
struct Spot(f32, f32, f32, f32);

const ZOOM_SPOT: Spot = Spot(0.33, 0.26, 0.41, 0.41);
const SPEED_SPOT: Spot = Spot(0.76, 0.74, 0., 0.);
const LOWER_THIRD_SPOT: Spot = Spot(0.06, 0.64, 0., 0.);
const LABEL_SPOT: Spot = Spot(0.54, 0.44, 0., 0.);
const TEXT_SPOT: Spot = Spot(0.24, 0.74, 0., 0.);
const ARROW_SPOT: Spot = Spot(0.44, 0.48, 0., 0.);
const HIGHLIGHT_SPOT: Spot = Spot(0.56, 0.58, 0.27, 0.24);
const STEP_SPOT: Spot = Spot(0.15, 0.26, 0., 0.);
const NEXT_STEP_SPOT: Spot = Spot(0.55, 0.58, 0., 0.);
const SPOTLIGHT_SPOT: Spot = Spot(0.52, 0.52, 0.34, 0.34);
const BLUR_SPOT: Spot = Spot(0.16, 0.28, 0.41, 0.28);
const IMAGE_SPOT: Spot = Spot(0.69, 0.1, 0.25, 0.34);
const CAPTION_SPOT: Spot = Spot(0.5, 0.8, 0., 0.);

#[derive(Clone, Copy, PartialEq)]
enum Lane {
    Zoom,
    Clip,
    Annotation,
    Caption,
    Audio,
    Ruler,
}

impl Lane {
    fn name(self) -> &'static str {
        match self {
            Lane::Zoom => "Zoom lane",
            Lane::Clip => "Clip lane",
            Lane::Annotation => "Annotation lane",
            Lane::Caption => "Caption lane",
            Lane::Audio => "Audio lane",
            Lane::Ruler => "Ruler",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Lane::Zoom => "MagnifyingGlassPlus-regular",
            Lane::Clip => "FilmStrip-regular",
            Lane::Annotation => "TextT-regular",
            Lane::Caption => "ClosedCaptioning-regular",
            Lane::Audio => "MusicNotes-regular",
            Lane::Ruler => "Flag-regular",
        }
    }

    /// Whether `region` is drawn on this lane.
    fn holds(self, region: &Region) -> bool {
        let kind = region.kind.as_str();
        match self {
            Lane::Zoom => kind == "zoomRegions",
            Lane::Clip => matches!(
                kind,
                "clipRegions" | "trimRegions" | "speedRegions" | Region::TAKE_CLIP
            ),
            Lane::Annotation => kind == "annotationRegions",
            Lane::Caption => kind == "autoCaptions",
            Lane::Audio => matches!(kind, "audioRegions" | Region::TAKE_AUDIO),
            Lane::Ruler => kind == "nativeMarkers",
        }
    }
}

struct Kind {
    action: &'static str,
    label: &'static str,
    /// Empty for an annotation, which takes its glyph from
    /// `crate::annotations::KINDS`.
    glyph: &'static str,
    lane: Lane,
    hint: &'static str,
}

impl Kind {
    fn glyph(&self) -> &'static str {
        crate::annotations::KINDS
            .iter()
            .find(|(action, _, _)| *action == self.action)
            .map_or(self.glyph, |(_, _, glyph)| glyph)
    }
}

const fn kind(
    action: &'static str,
    label: &'static str,
    glyph: &'static str,
    lane: Lane,
    hint: &'static str,
) -> Kind {
    Kind {
        action,
        label,
        glyph,
        lane,
        hint,
    }
}

/// The kinds by where they land: the timeline, the picture, the sound and
/// the words.
const GROUPS: [(&str, &[Kind]); 3] = [
    (
        "Timeline",
        &[
            kind(
                "add-zoom",
                "Zoom",
                "MagnifyingGlassPlus-regular",
                Lane::Zoom,
                "Pushes in on one spot, then eases back out.",
            ),
            kind(
                "add-trim",
                "Trim",
                "Scissors-regular",
                Lane::Clip,
                "Cuts a span out of the take.",
            ),
            kind(
                "add-speed",
                "Speed",
                "Timer-regular",
                Lane::Clip,
                "Plays a span faster or slower.",
            ),
            kind(
                "add-marker",
                "Marker",
                "Flag-regular",
                Lane::Ruler,
                "Flags a moment to jump back to.",
            ),
        ],
    ),
    (
        "Picture",
        &[
            kind(
                "add-title",
                "Title",
                "",
                Lane::Annotation,
                "Big centred words over the picture.",
            ),
            kind(
                "add-lower-third",
                "Lower third",
                "",
                Lane::Annotation,
                "A name and a line in the bottom corner.",
            ),
            kind(
                "add-label",
                "Label",
                "",
                Lane::Annotation,
                "A small tag pinned to something on screen.",
            ),
            kind(
                "add-text",
                "Text",
                "",
                Lane::Annotation,
                "Free text you place anywhere.",
            ),
            kind(
                "add-figure",
                "Arrow",
                "",
                Lane::Annotation,
                "Points at something.",
            ),
            kind(
                "add-highlight",
                "Highlight",
                "",
                Lane::Annotation,
                "Draws a box around something.",
            ),
            kind(
                "add-step",
                "Step",
                "",
                Lane::Annotation,
                "A numbered dot for a walkthrough.",
            ),
            kind(
                "add-spotlight",
                "Spotlight",
                "",
                Lane::Annotation,
                "Dims everything but one spot.",
            ),
            kind(
                "add-blur",
                "Blur",
                "",
                Lane::Annotation,
                "Hides something private.",
            ),
            kind(
                "add-image",
                "Image",
                "",
                Lane::Annotation,
                "Lays a picture of your own on top.",
            ),
        ],
    ),
    (
        "Sound and words",
        &[
            kind(
                "add-audio",
                "Audio",
                "MusicNotes-regular",
                Lane::Audio,
                "Adds music or a voice-over from a file.",
            ),
            kind(
                "add-caption",
                "Caption",
                "ClosedCaptioning-regular",
                Lane::Caption,
                "A line of words along the bottom.",
            ),
        ],
    ),
];

/// The popup's height, for deciding which way it opens.
fn popup_height() -> f32 {
    2. * Theme::add_popup_padding()
        + Theme::control_height_large()
        + Theme::add_popup_body_height()
        + Theme::footer_height()
        + 2. * Theme::add_popup_gap()
}

/// The preview card's inner width: what the list leaves of the popup.
fn card_inner_width() -> f32 {
    Theme::add_popup_width()
        - 2. * Theme::add_popup_padding()
        - Theme::add_popup_list_width()
        - Theme::gap()
        - 2. * Theme::add_popup_card_padding()
}

/// A mark placed at `spot`, sized by it when it gives a size.
fn place(spot: Spot, picture_width: f32, picture_height: f32) -> Div {
    let Spot(left, top, width, height) = spot;
    // In points, not shares: a share of the picture's height does not
    // reach an absolute child.
    let mark = div()
        .absolute()
        .left(px(left * picture_width))
        .top(px(top * picture_height));
    if width > 0. {
        mark.w(px(width * picture_width))
            .h(px(height * picture_height))
    } else {
        mark
    }
}

/// A line of words on a dark plate, as a lower third, a caption or a speed
/// badge draws.
fn plate(text: &'static str) -> Div {
    div()
        .px(px(Theme::gap_small()))
        .rounded(px(Theme::add_popup_mark_radius()))
        .bg(black().opacity(PLATE_ALPHA))
        .text_color(white())
        .text_size(px(Theme::font_small()))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(text)
}

/// What the kind `action` looks like on the picture, in the words and at the
/// size `App::add_region` gives a new one.
fn marks(action: &str, width: f32, height: f32, theme: Theme) -> Vec<AnyElement> {
    let at = |spot: Spot| place(spot, width, height);
    let accent = theme.accent;
    let ring = px(Theme::add_popup_mark_ring());
    let step = |number: &'static str| {
        div()
            .size(px(Theme::add_popup_step_size()))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(accent)
            .text_color(theme.on_accent)
            .text_size(px(Theme::font_small()))
            .font_weight(FontWeight::SEMIBOLD)
            .shadow(vec![hairline(white(), Theme::add_popup_mark_ring())])
            .child(number)
    };
    let mark = match action {
        "add-zoom" => at(ZOOM_SPOT)
            .rounded(px(Theme::add_popup_mark_radius()))
            .border(ring)
            .border_dashed()
            .border_color(accent)
            .bg(accent.opacity(ZOOM_WASH_ALPHA)),
        "add-speed" => at(SPEED_SPOT).child(plate("1.5×")),
        "add-title" => div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().opacity(TITLE_WASH_ALPHA))
            .text_color(white())
            .text_size(px(Theme::font_action()))
            .font_weight(FontWeight::SEMIBOLD)
            .child("Title"),
        "add-lower-third" => at(LOWER_THIRD_SPOT).child(plate("Name · Role")),
        "add-label" => at(LABEL_SPOT).child(
            div()
                .px(px(Theme::gap_small()))
                .rounded_full()
                .bg(accent)
                .text_color(theme.on_accent)
                .text_size(px(Theme::font_small()))
                .font_weight(FontWeight::MEDIUM)
                .child("Label"),
        ),
        "add-text" => at(TEXT_SPOT)
            .text_color(white())
            .text_size(px(Theme::font_small()))
            .font_weight(FontWeight::BOLD)
            .child("Your text"),
        "add-figure" => at(ARROW_SPOT).child(icon_sized(
            "ArrowUpRight-regular",
            Theme::icon_size_transport(),
            accent,
        )),
        "add-highlight" => at(HIGHLIGHT_SPOT)
            .rounded(px(Theme::add_popup_mark_radius()))
            .border(ring)
            .border_color(accent),
        "add-step" => div()
            .absolute()
            .inset_0()
            .child(at(STEP_SPOT).child(step("1")))
            .child(at(NEXT_STEP_SPOT).opacity(NEXT_STEP_ALPHA).child(step("2"))),
        "add-spotlight" => at(SPOTLIGHT_SPOT)
            .rounded(px(Theme::add_popup_mark_radius()))
            .shadow(vec![BoxShadow {
                color: black().opacity(SPOTLIGHT_ALPHA),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                // Far enough to cover the picture from anywhere on it.
                spread_radius: px(Theme::add_popup_width()),
                inset: false,
            }]),
        "add-blur" => at(BLUR_SPOT)
            .rounded(px(Theme::add_popup_mark_radius()))
            .border(ring)
            .border_dashed()
            .border_color(accent)
            .bg(white().opacity(BLUR_WASH_ALPHA)),
        "add-image" => at(IMAGE_SPOT)
            .rounded(px(Theme::add_popup_mark_radius()))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.plate)
            .child(icon_sized("Image-regular", Theme::icon_size_pod(), accent)),
        // A caption is centred on its spot, so its words set the plate.
        "add-caption" => div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(CAPTION_SPOT.1 * height))
            .flex()
            .justify_center()
            .child(plate("Your caption")),
        _ => return Vec::new(),
    };
    vec![mark.into_any_element()]
}

impl RootView {
    /// The Add popup, `leave` into its way out; see the module comment.
    pub(super) fn add_popup(
        &mut self,
        leave: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Surface::Editor(e) = &self.surface else {
            return div().into_any_element();
        };
        let e = e.clone();
        let has_video = e.get_has_video();
        let filter = self.menu_filter.trim().to_lowercase();
        let shown: Vec<(&str, &Kind)> = GROUPS
            .iter()
            .flat_map(|(group, kinds)| kinds.iter().map(move |kind| (*group, kind)))
            .filter(|(_, kind)| filter.is_empty() || kind.label.to_lowercase().contains(&filter))
            .collect();
        let highlight = self.menu_highlight.min(shown.len().saturating_sub(1));
        let chosen = has_video
            .then(|| shown.get(highlight).map(|(_, kind)| *kind))
            .flatten();
        let count = shown.len();
        let elapsed = e
            .get_time_label()
            .split_once(" / ")
            .map_or_else(|| e.get_time_label(), |(elapsed, _)| elapsed.to_owned());
        let shortcuts: Vec<Field> = e.get_settings_fields().iter().collect();
        let shortcut = |action: &str| {
            shortcuts
                .iter()
                .find(|field| field.key.strip_prefix("shortcut.") == Some(action))
                .map(|field| field.value.clone())
                .filter(|key| key.chars().count() == 1)
        };

        let anchor = self.menu_anchor.get();
        let viewport = window.viewport_size();
        let height = popup_height();
        let width = Theme::add_popup_width();
        let (left, top) = if anchor.size.width > px(0.) {
            let left = f32::from(anchor.origin.x)
                .min(f32::from(viewport.width) - width - Theme::gap())
                .max(Theme::gap());
            let below = f32::from(anchor.origin.y + anchor.size.height) + Theme::gap();
            let top = if below + height <= f32::from(viewport.height) - Theme::gap() {
                below
            } else {
                (f32::from(anchor.origin.y) - Theme::gap() - height).max(Theme::gap())
            };
            (left, top)
        } else {
            // Never measured: the take closed before the timeline drew its
            // trigger. Centred over where the console would be.
            (
                ((f32::from(viewport.width) - width) / 2.).max(Theme::gap()),
                (f32::from(viewport.height) - height - Theme::inset()).max(Theme::gap()),
            )
        };

        let search = self.input("add-popup", "", window, cx, |_, _, _| {});
        let view = cx.entity().downgrade();
        // The list's children include the group captions, so the arrow keys'
        // row is not the child to scroll to.
        let mut children_of_rows = Vec::with_capacity(count);
        search.update(cx, |input, _| {
            input.set_bare(true);
            // With nothing open there is no playhead to add at.
            input.set_placeholder(if has_video {
                format!("Add at {elapsed}")
            } else {
                "Add".to_owned()
            });
            let filtering = view.clone();
            input.set_on_change(move |value, _, cx| {
                filtering
                    .update(cx, |s: &mut Self, cx| {
                        s.menu_filter = value.clone();
                        s.menu_highlight = 0;
                        cx.notify();
                    })
                    .ok();
            });
            let running = view.clone();
            let run = chosen.map(|kind| kind.action);
            input.set_handler(move |_, _, cx| {
                let Some(action) = run else { return };
                running
                    .update(cx, |s: &mut Self, cx| s.run_command(action, cx))
                    .ok();
            });
            let dismissing = view.clone();
            input.set_on_cancel(move |_, cx| {
                dismissing
                    .update(cx, |s: &mut Self, cx| {
                        s.menu = None;
                        s.menu_filter.clear();
                        s.menu_highlight = 0;
                        cx.notify();
                    })
                    .ok();
            });
        });
        if std::mem::take(&mut self.menu_focus) {
            let seed = self.menu_filter.clone();
            search.update(cx, |input, cx| {
                input.reset(&seed);
                input.focus(window, cx);
            });
        }
        let focused = search.read(cx).is_focused(window);

        let field = row()
            .flex_none()
            .h(px(Theme::control_height_large()))
            .pl(px(Theme::control_padding()))
            .pr(px(Theme::gap()))
            .gap(px(Theme::icon_gap_row()))
            .rounded_full()
            .bg(theme.sunk)
            .when(focused, |el| el.shadow(vec![subtake_ui::focus_ring(theme)]))
            .child(icon_sized("Plus-regular", Theme::icon_size(), theme.muted))
            .child(search.clone())
            .child(key_cap("esc", theme));

        let mut list = menu_list("add-popup-list", Theme::add_popup_body_height())
            .w(px(Theme::add_popup_list_width()))
            .flex_none()
            .track_scroll(&self.menu_scroll)
            .when(!has_video, |list| list.opacity(Theme::disabled_opacity()));
        let mut child = 0;
        let mut last_group = "";
        for (i, (group, kind)) in shown.iter().enumerate() {
            if *group != last_group {
                last_group = group;
                list = list.child(
                    caps_label(*group, theme)
                        .pt(px(Theme::add_popup_group_top()))
                        .pb(px(Theme::add_popup_group_bottom()))
                        .px(px(Theme::menu_item_padding())),
                );
                child += 1;
            }
            children_of_rows.push(child);
            child += 1;
            list = list.child(self.add_popup_row(
                i,
                kind,
                i == highlight && has_video,
                &filter,
                shortcut(kind.action).filter(|_| filter.is_empty()),
                has_video,
                cx,
            ));
        }
        if shown.is_empty() {
            list = list.child(
                div()
                    .px(px(Theme::menu_item_padding()))
                    .py(px(Theme::gap_block()))
                    .text_color(theme.muted)
                    .child(format!(
                        "Nothing to add called “{}”",
                        self.menu_filter.trim()
                    )),
            );
        }
        // Up and down walk the highlight through the filtered rows, and the
        // list scrolls to keep it in sight.
        let stepping = view.clone();
        search.update(cx, |input, _| {
            input.set_on_step(move |delta, _, cx| {
                let last = count.saturating_sub(1) as isize;
                let next = (highlight as isize + delta).clamp(0, last) as usize;
                let child = children_of_rows.get(next).copied().unwrap_or(0);
                stepping
                    .update(cx, |s: &mut Self, cx| {
                        s.menu_highlight = next;
                        s.menu_scroll.scroll_to_item(child);
                        cx.notify();
                    })
                    .ok();
            });
        });

        let card = column()
            .flex_1()
            .min_w_0()
            .p(px(Theme::add_popup_card_padding()))
            .gap(px(Theme::add_popup_card_gap()))
            .rounded(px(Theme::add_popup_card_radius()))
            .bg(theme.sunk);
        let card = if !has_video {
            card.justify_center().child(
                column()
                    .gap(px(Theme::gap()))
                    .px(px(Theme::gap_small()))
                    .child(
                        div()
                            .text_size(px(Theme::font_action()))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Nothing to add to yet"),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_secondary()))
                            .text_color(theme.muted)
                            .child(
                                "Open or record a video, and everything here adds at the playhead.",
                            ),
                    )
                    .child(
                        div().pt(px(Theme::gap_small())).child(
                            button("add-popup-open", "Open…", theme)
                                .raised()
                                .small()
                                .on_click(cx.listener(|s, _, _, cx| s.run_command("open", cx))),
                        ),
                    ),
            )
        } else if let Some(kind) = chosen {
            self.add_popup_preview(card, kind, &e, &elapsed, cx)
        } else {
            card
        };

        // The same switcher as the palette's footer, which opens the other
        // menus' palettes from here.
        let mut footer = composer_footer(theme);
        for (menu, glyph) in [
            ("File", "FolderOpen-regular"),
            ("Edit", "SlidersHorizontal-regular"),
            ("Add", "Plus-regular"),
            ("Help", "Question-regular"),
        ] {
            footer = footer.child(
                icon_button(
                    SharedString::from(format!("add-popup-menu-{menu}")),
                    glyph,
                    menu,
                    theme,
                )
                .ghost()
                .small()
                .toggled(menu == "Add")
                .on_click(cx.listener(move |s, _, _, cx| {
                    s.menu = Some(menu.into());
                    s.menu_filter.clear();
                    s.menu_highlight = 0;
                    s.menu_focus = true;
                    cx.notify();
                })),
            );
        }
        let footer = footer.child(
            div()
                .flex_1()
                .flex()
                .justify_end()
                .whitespace_nowrap()
                .child(if has_video {
                    "↑↓ move   ↵ add   esc close"
                } else {
                    "Open a video to add to it"
                }),
        );

        deferred(frosted(
            Theme::radius_menu(),
            MENU_BLUR * leave,
            menu_in(
                ("add-popup-in", self.menu_leave.opens),
                top,
                leave,
                menu_surface(theme)
                    .id("add-popup")
                    .absolute()
                    .left(px(left))
                    .w(px(width))
                    .p(px(Theme::add_popup_padding()))
                    .gap(px(Theme::add_popup_gap()))
                    .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                        s.menu = None;
                        cx.notify();
                    }))
                    .child(field)
                    .child(
                        row()
                            .items_start()
                            .gap(px(Theme::gap()))
                            .h(px(Theme::add_popup_body_height()))
                            .child(fade_edges(list).tracking(&self.menu_scroll))
                            .child(card),
                    )
                    .child(
                        footer
                            .flex_none()
                            .border_t(px(Theme::border_width()))
                            .border_color(theme.line),
                    ),
            ),
        ))
        .with_priority(30)
        .into_any_element()
    }

    /// One kind in the list: its glyph, its name with the filter's match
    /// picked out, and its shortcut while nothing is typed.
    #[allow(clippy::too_many_arguments)]
    fn add_popup_row(
        &self,
        index: usize,
        kind: &'static Kind,
        highlighted: bool,
        filter: &str,
        shortcut: Option<String>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let label = kind.label;
        let at = (!filter.is_empty())
            .then(|| label.to_lowercase().find(filter))
            .flatten();
        // One run of text, so the matched part sits flush against the rest.
        let name = StyledText::new(label).with_highlights(at.map(|at| {
            (
                at..at + filter.len(),
                HighlightStyle {
                    color: Some(theme.accent),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            )
        }));
        let action = kind.action;
        row()
            .id(SharedString::from(format!("add-popup-{action}")))
            .flex_none()
            .h(px(Theme::add_popup_row_height()))
            .pl(px(Theme::menu_item_padding()))
            .pr(px(Theme::gap()))
            .gap(px(Theme::icon_gap_row()))
            .rounded(px(Theme::add_popup_row_radius()))
            .when(highlighted, |el| el.bg(theme.sunk))
            .child(icon_sized(
                kind.glyph(),
                Theme::icon_size(),
                if highlighted {
                    theme.accent
                } else {
                    theme.text
                },
            ))
            .child(div().flex_1().min_w_0().overflow_hidden().child(name))
            .children(shortcut.map(|key| key_cap(key, theme)))
            .when(enabled, |el| {
                el.on_click(cx.listener(move |s, _, _, cx| s.run_command(action, cx)))
                    .on_mouse_move(cx.listener(move |s, _: &MouseMoveEvent, _, cx| {
                        if s.menu_highlight != index {
                            s.menu_highlight = index;
                            cx.notify();
                        }
                    }))
            })
    }

    /// The card for `kind`: the picture with it on, the lane it lands on,
    /// what it is for, and the button that adds it.
    fn add_popup_preview(
        &self,
        card: Div,
        kind: &'static Kind,
        e: &EditorWindow,
        elapsed: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let aspect = e.get_preview_aspect().max(f32::EPSILON);
        let inner = card_inner_width();
        let picture_width = inner.min(inner * PICTURE_TALLEST * aspect);
        let picture_height = picture_width / aspect;
        let duration = e.get_duration().max(f32::EPSILON);
        let playhead = e.get_playhead();
        let frame = self.clip_frames.as_ref().and_then(|(_, frames)| {
            let last = frames.len().checked_sub(1)?;
            let index = ((playhead / duration) * frames.len() as f32).floor() as usize;
            frames.get(index.min(last)).cloned()
        });
        let picture = div()
            .relative()
            .flex_none()
            .self_center()
            .w(px(picture_width))
            .h(px(picture_height))
            .rounded(px(Theme::add_popup_picture_radius()))
            .overflow_hidden()
            .bg(theme.sunk2)
            .children(frame.map(|frame| {
                img(frame)
                    .absolute()
                    .inset_0()
                    .size_full()
                    // The picture's own rounding does not clip an image.
                    .rounded(px(Theme::add_popup_picture_radius()))
                    .object_fit(ObjectFit::Cover)
            }))
            // Its own layer: in the frosted popup's one layer the marks' quads
            // would draw under the frame.
            .child(layered(div().absolute().inset_0().children(marks(
                kind.action,
                picture_width,
                picture_height,
                theme,
            ))));

        // The strip: the lane from STRIP_SECONDS / 2 before the playhead to
        // as far after, the playhead at its middle.
        let from = playhead - STRIP_SECONDS / 2.;
        let share = |seconds: f32| ((seconds - from) / STRIP_SECONDS).clamp(0., 1.);
        let block = |start: f32, end: f32| {
            div()
                .absolute()
                .top(px(Theme::add_popup_strip_inset()))
                .bottom(px(Theme::add_popup_strip_inset()))
                .left(relative(share(start)))
                .w(relative(share(end) - share(start)))
                .rounded(px(Theme::add_popup_strip_block_radius()))
        };
        let tick = |at: f32| {
            div()
                .absolute()
                .top(px(Theme::add_popup_strip_inset()))
                .bottom(px(Theme::add_popup_strip_inset()))
                .left(relative(share(at)))
                .w(px(Theme::add_popup_marker_width()))
        };
        let old = theme.text.opacity(OLD_BLOCK_ALPHA);
        let mut strip = div()
            .relative()
            .flex_none()
            .h(px(Theme::add_popup_strip_height()))
            .rounded_full()
            .bg(theme.sunk);
        for region in e.get_regions().iter().filter(|r| kind.lane.holds(r)) {
            let visible = region.end >= from && region.start <= from + STRIP_SECONDS;
            if !visible {
                continue;
            }
            strip = strip.child(if kind.lane == Lane::Ruler {
                tick(region.start).bg(old)
            } else {
                block(region.start, region.end).bg(old)
            });
        }
        let end = (playhead + NEW_SECONDS).min(duration);
        strip = strip.child(match kind.action {
            "add-marker" => tick(playhead).bg(theme.accent),
            // A file's length is not known until it is picked.
            "add-audio" => block(playhead, duration).bg(theme.accent),
            "add-trim" => block(playhead, end).bg(theme.accent.opacity(TRIM_ALPHA)),
            _ => block(playhead, end).bg(theme.accent),
        });
        let strip = strip.child(
            div()
                .absolute()
                .left(relative(share(playhead)))
                .top(px(-Theme::add_popup_playhead_overhang()))
                .bottom(px(-Theme::add_popup_playhead_overhang()))
                .w(px(Theme::add_popup_playhead_width()))
                .rounded_full()
                .bg(theme.accent),
        );

        let action = kind.action;
        card.child(picture)
            .child(
                column()
                    .gap(px(Theme::gap_small()))
                    .child(
                        row()
                            .gap(px(Theme::gap_small()))
                            .text_size(px(Theme::font_small()))
                            .text_color(theme.muted)
                            .child(icon_sized(
                                kind.lane.glyph(),
                                Theme::icon_size_caret(),
                                theme.muted,
                            ))
                            .child(kind.lane.name()),
                    )
                    .child(strip),
            )
            .child(
                column()
                    .gap(px(Theme::gap_small()))
                    .child(
                        div()
                            .text_size(px(Theme::font_action()))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(kind.label),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_secondary()))
                            .text_color(theme.muted)
                            .child(kind.hint),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(Theme::font_small()))
                    .text_color(theme.muted)
                    .child(format!("{elapsed} · {}", kind.lane.name().to_lowercase())),
            )
            .child(
                button(
                    "add-popup-add",
                    format!("Add {}  ↵", kind.label.to_lowercase()),
                    theme,
                )
                .primary()
                .small()
                .stretch()
                .on_click(cx.listener(move |s, _, _, cx| s.run_command(action, cx))),
            )
    }
}
