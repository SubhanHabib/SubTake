//! The Presets dialog: screen 4 of the "Stage" handoff.
//!
//! A choice here is a draft until Apply — picking a row only moves the
//! selection, so browsing the three looks never edits the project. Apply
//! runs the same `look-*` and `motion-*` commands the app has always had.

use super::*;
use crate::presets::{LOOKS, Look, motion_durations};
use subtake_ui::layered;

/// What the dialog has selected but not yet applied.
#[derive(Clone)]
pub(super) struct PresetsDraft {
    look: String,
    motion: String,
}

impl RootView {
    /// Whether the dialog is open is the editor's state (`dialog`), so the
    /// titlebar, a menu or the gallery can open it; the draft inside it is
    /// the view's, started from the project's choices each time it opens.
    pub(super) fn presets_dialog(
        &mut self,
        e: &EditorWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // Closing, the dialog keeps its draft on screen while it fades out.
        let open = e.get_dialog() == "presets";
        if !open && self.presets.is_none() {
            return None;
        }
        let ms = if open { DIALOG_IN_MS } else { DIALOG_OUT_MS };
        let shown = super::slide_toward(&mut self.presets_slide, open, ms, window);
        if !open && shown == 0. {
            self.presets = None;
            return None;
        }
        let draft = self
            .presets
            .get_or_insert_with(|| PresetsDraft {
                look: e.get_look_choice(),
                motion: e.get_motion_choice(),
            })
            .clone();
        let theme = self.theme;
        let editor = e.clone();
        let close =
            move |_: &ClickEvent, _: &mut Window, _: &mut App| editor.set_dialog(String::new());

        let mut looks = column().gap(px(Theme::GAP_SMALL));
        for look in LOOKS {
            let name = look.name;
            looks = looks.child(
                preset_row(look, draft.look == name, theme).on_click(cx.listener(
                    move |s, _, _, cx| {
                        if let Some(d) = s.presets.as_mut() {
                            d.look = name.into();
                        }
                        cx.notify();
                    },
                )),
            );
        }

        let mut motions = row().gap(px(Theme::GAP_LARGE)).items_stretch();
        for (name, title, smooth) in [("focused", "Focused", false), ("smooth", "Smooth", true)] {
            let (zoom_in, zoom_out) = motion_durations(smooth);
            motions = motions.child(
                motion_tile(
                    name,
                    title,
                    format!("in {zoom_in:.0} / out {zoom_out:.0} ms"),
                    draft.motion == name,
                    theme,
                )
                .on_click(cx.listener(move |s, _, _, cx| {
                    if let Some(d) = s.presets.as_mut() {
                        d.motion = name.into();
                    }
                    cx.notify();
                })),
            );
        }

        let surface = self.surface.clone();
        let editor = e.clone();
        let apply = cx.listener(move |s, _, _, _| {
            if let Some(d) = s.presets.take() {
                if !d.look.is_empty() {
                    surface.action(&format!("look-{}", d.look));
                }
                if !d.motion.is_empty() {
                    surface.action(&format!("motion-{}", d.motion));
                }
            }
            editor.set_dialog(String::new());
        });
        let enabled = e.get_has_video() && !e.get_busy();

        let mut card = column()
            .gap(px(Theme::GAP_BLOCK))
            .child(
                row()
                    .child(title("Presets", Theme::FONT_HEADING).flex_1())
                    .child(
                        icon_button("presets-close", "X-regular", "Close", theme)
                            .small()
                            .on_click(close),
                    ),
            )
            // The guarantee stated at the top of `src/presets.rs`. Keep the
            // two in step if what a preset carries ever changes.
            .child(div().text_color(theme.muted).child(
                "A preset stores appearance, motion, cursor and export settings. \
                 It never replaces source media, regions or document identity.",
            ))
            .child(caps_label("Appearance", theme))
            .child(looks)
            .child(caps_label("Motion", theme))
            .child(motions);

        // Not drawn by the design: presets saved to disk, and loading one
        // from a file. The handoff shows only the built-ins, but a saved
        // preset is one the user made on purpose, so it is listed here
        // rather than stranded in the inspector panel this dialog replaced.
        // Each sits on a plate, as the tiles above do.
        let mut saved = column().gap(px(Theme::GAP_SMALL));
        for (index, name) in e.get_saved_presets().iter().enumerate() {
            saved = saved.child(
                row()
                    .child(
                        button(
                            SharedString::from(format!("saved-preset-{index}")),
                            name,
                            theme,
                        )
                        .stretch()
                        .enabled(enabled)
                        .on_click(self.command(&format!("apply-preset-{index}"))),
                    )
                    .child(
                        icon_button(
                            SharedString::from(format!("remove-preset-{index}")),
                            "Trash-regular",
                            "Delete preset",
                            theme,
                        )
                        .ghost()
                        .on_click(self.command(&format!("remove-preset-{index}"))),
                    ),
            );
        }
        saved = saved.child(
            button("presets-load", "Load preset file…", theme)
                .glyph("FolderOpen-regular")
                .stretch()
                .enabled(enabled)
                .on_click(self.command("load-preset")),
        );
        card = card.child(caps_label("Saved", theme)).child(saved);

        card = card.child(
            row()
                .gap(px(Theme::GAP_LARGE))
                .child(
                    button("presets-save", "Save current", theme)
                        .glyph("Plus-regular")
                        .dialog()
                        .stretch()
                        .enabled(enabled)
                        .on_click(self.command("save-preset")),
                )
                .child(
                    button("presets-apply", "Apply", theme)
                        .primary()
                        .dialog()
                        .stretch()
                        .enabled(enabled)
                        .on_click(apply),
                ),
        );

        // The content sits on a layer over the card. Inside a frosted card
        // everything shares one draw order, and at one order gpui draws every
        // shadow before every fill — so Apply's glow and the selection rings
        // went under the card's own `card` fill and all but vanished.
        let card = panel_variant(theme, UiSurface::Content)
            .id("presets-dialog")
            .relative()
            .top(px(DIALOG_RISE * (1. - shown)))
            .opacity(shown)
            .w(px(Theme::DIALOG_WIDTH))
            .p(px(Theme::DIALOG_PADDING))
            .occlude()
            .on_mouse_down_out({
                let editor = e.clone();
                move |_, _, _| editor.set_dialog(String::new())
            })
            .child(layered(card));

        // The scrim takes the pointer from everything under the dialog; it
        // draws nothing, because the handoff sets the dialog straight over
        // the stage. A closing dialog lets the pointer through at once. Its
        // frost is a backdrop blur, which opacity does not reach, so the
        // blur itself eases with the fade.
        Some(
            deferred(
                div()
                    .id("presets-scrim")
                    .absolute()
                    .inset_0()
                    .when(open, |s| s.occlude())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(frosted(
                        UiSurface::Content.radius(),
                        subtake_ui::BAR_BLUR * shown,
                        card,
                    )),
            )
            .with_priority(20)
            .into_any_element(),
        )
    }
}

/// One built-in look: a preview of its background and frame, its name, and
/// the values it writes.
fn preset_row(look: Look, selected: bool, theme: Theme) -> Stateful<Div> {
    let scale = Theme::PRESET_PREVIEW_SCALE;
    let background = parse_hex(look.wallpaper);
    let frame = div()
        .size_full()
        .rounded(px(look.radius as f32 * scale))
        // A stand-in for the captured screen, the same light plate in both
        // themes: it is a picture of a recording, not a piece of chrome.
        .bg(hsla(0., 0., 0.9, 1.))
        .shadow(vec![BoxShadow {
            color: hsla(0., 0., 0., look.shadow as f32),
            offset: point(px(0.), px(2.)),
            blur_radius: px(6.),
            spread_radius: px(0.),
            inset: false,
        }]);
    let preview = div()
        .flex_none()
        .w(px(Theme::PRESET_PREVIEW_WIDTH))
        .h(px(Theme::PRESET_PREVIEW_HEIGHT))
        .rounded(px(Theme::RADIUS_LANE))
        .bg(background)
        .p(px(look.padding as f32 * scale))
        .child(frame);
    let values = format!(
        "{} · pad {} · r {} · shadow {:.0}%",
        look.wallpaper,
        look.padding,
        look.radius,
        look.shadow * 100.
    );
    let id = ElementId::from(SharedString::from(format!("look-{}", look.name)));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let mut row = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(Theme::GAP_BLOCK))
        .p(px(Theme::CONTROL_PADDING_SMALL))
        .rounded(px(Theme::RADIUS_ROW))
        .map(|row| pressable(row, selected, theme))
        .child(preview)
        .child(
            column()
                .flex_1()
                .min_w_0()
                .gap(px(Theme::GAP_SMALL))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(look.title),
                )
                .child(
                    mono(values)
                        .text_size(px(Theme::FONT_SECONDARY))
                        .text_color(theme.muted)
                        .text_ellipsis(),
                ),
        );
    if selected {
        row = row
            .bg(theme.sunk)
            .child(icon("Check-regular", theme.accent))
            .child(selection_ring(Theme::RADIUS_ROW, theme));
    } else {
        // The hover wash fades, as every other row's does.
        row = row
            .bg(subtake_ui::motion::hover_blend(
                &hover_key,
                theme.hover.opacity(0.),
                theme.hover,
            ))
            .on_hover(subtake_ui::motion::hover_listener(hover_key));
    }
    row
}

/// One motion preset: its name and its two zoom durations.
fn motion_tile(
    name: &str,
    title: &'static str,
    values: String,
    selected: bool,
    theme: Theme,
) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("motion-{name}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let mut tile = div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(Theme::GAP_SMALL))
        .py(px(Theme::CONTROL_PADDING_SMALL))
        .px(px(Theme::CONTROL_PADDING))
        .rounded(px(Theme::RADIUS_MENU))
        .bg(subtake_ui::motion::hover_blend(
            &hover_key,
            theme.sunk,
            theme.sunk2,
        ))
        .on_hover(subtake_ui::motion::hover_listener(hover_key))
        .map(|tile| pressable(tile, selected, theme))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(title),
        )
        .child(
            mono(values)
                .text_size(px(Theme::FONT_SECONDARY))
                .text_color(theme.muted),
        );
    if selected {
        tile = tile.child(selection_ring(Theme::RADIUS_MENU, theme));
    }
    tile
}

/// A look row's or motion tile's pointer, press and keyboard focus, as a
/// `Button` carries them. The one in use only dims when pressed: its fill is
/// what says it is picked, and pressing it again picks nothing new.
fn pressable(el: Stateful<Div>, selected: bool, theme: Theme) -> Stateful<Div> {
    let ring = subtake_ui::focus_ring(theme);
    let press = theme.press;
    el.cursor_pointer()
        .tab_index(0)
        .focus_visible(move |s| s.shadow(vec![ring]))
        .active(move |s| if selected { s } else { s.bg(press) }.opacity(Theme::PRESSED_OPACITY))
}

/// The 1.5 accent inset a selected row or tile carries, on a layer of its
/// own over the fill: set on the row itself, it would draw under the row's
/// own `sunk` for the reason the dialog's content is layered.
fn selection_ring(radius: f32, theme: Theme) -> impl IntoElement {
    layered(
        div()
            .absolute()
            .inset_0()
            .rounded(px(radius))
            .shadow(vec![hairline(theme.accent, Theme::SELECTED_WIDTH)]),
    )
}

fn parse_hex(value: &str) -> Hsla {
    u32::from_str_radix(value.trim_start_matches('#'), 16)
        .map(|v| rgb(v).into())
        .unwrap_or(hsla(0., 0., 0.5, 1.))
}
