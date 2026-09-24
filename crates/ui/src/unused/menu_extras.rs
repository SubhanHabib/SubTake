//! The four menu pieces the handoff draws that no menu in this build uses
//! yet: a shortcut hint, a submenu chevron, a section header and a
//! destructive item.
//!
//! The rows here are shaped by hand rather than by wrapping `menu_row`: the
//! hint and the chevron share the trailing slot, and a destructive row swaps
//! its text colour, neither of which `menu_row` takes a parameter for. When
//! one of these earns a call site, the right move is to widen `menu_row`, not
//! to keep a second row in this module.

use gpui::{prelude::*, *};
use subtake_theme::{FONT_MONO, Theme};

use crate::{caps_label, icon_sized, motion, row};

/// "Submenus show a 12px chevron in the same slot."
const CHEVRON_SIZE: f32 = 12.0;
/// "Section header — 28 tall."
const SECTION_HEIGHT: f32 = 28.0;

/// "Geist Mono 11, `--muted`, right-aligned." Mono because a shortcut is a
/// set of glyphs read as a unit, and a proportional face sets `⌘⇧K` at three
/// different widths depending on what is in it.
pub fn shortcut_hint(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .flex_none()
        .font_family(FONT_MONO)
        .text_size(px(Theme::font_small()))
        .text_color(theme.muted)
        .child(text.into())
}

/// The mark on a row that opens another menu. It takes the same trailing slot
/// a shortcut would, because a row never has both.
pub fn submenu_chevron(theme: Theme) -> Svg {
    icon_sized("CaretRight-regular", CHEVRON_SIZE, theme.muted)
}

/// A heading inside a menu — what a context menu names its target with.
/// The same caps label every panel heading uses; a menu does not get a
/// heading style of its own.
pub fn menu_section_header(text: impl Into<SharedString>, theme: Theme) -> Div {
    row()
        .flex_none()
        .h(px(SECTION_HEIGHT))
        .px(px(Theme::menu_item_padding()))
        .child(caps_label(text, theme))
}

/// A destructive item: "label in `--danger`, no fill until hover."
///
/// `danger`, not `rec`. The handoff separates the two by role rather than by
/// shade — "red as a fill is `--rec`; red as text is always `--danger`" — so
/// the one red that appears as a label is never the one Record is painted in.
pub fn destructive_menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = id.into();
    let hover_key = motion::tween_key(&id, "menu-row");
    row()
        .id(id)
        .flex_none()
        .h(px(Theme::menu_item_height()))
        .px(px(Theme::menu_item_padding()))
        .gap(px(Theme::icon_gap_row()))
        .rounded(px(Theme::menu_item_radius()))
        .bg(motion::hover_blend(
            &hover_key,
            theme.danger.opacity(0.),
            theme.danger.opacity(0.12),
        ))
        .text_size(px(Theme::font_body()))
        .text_color(theme.danger)
        .cursor_pointer()
        .on_hover(motion::hover_listener(hover_key))
        // The same fixed gutter every other row holds, left empty: a
        // destructive item is never the checked one, and without the gutter
        // its label would sit a tick's width left of the list it is in.
        .child(div().flex_none().w(px(Theme::check_gutter())))
        .child(div().flex_1().min_w_0().text_ellipsis().child(label.into()))
        .on_click(on_click)
}
