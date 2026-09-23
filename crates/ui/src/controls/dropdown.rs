//! A retained dropdown with keyboard navigation and a frosted popover.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

use crate::{
    fade_edges, frost, icon_sized, measure, menu_in, menu_list, menu_row, menu_surface, motion,
};

/// A retained dropdown: keyboard navigation, selected state, and a native GPUI popover.
pub struct Dropdown {
    focus: FocusHandle,
    pub items: Vec<String>,
    pub selected: usize,
    pub enabled: bool,
    pub theme: Theme,
    /// A leading glyph naming what is being chosen — a microphone, a camera.
    /// A dropdown with one takes the select-row shape: the 44 plate a
    /// `toggle` sits on, so the two stack as one list of settings.
    pub glyph: Option<SharedString>,
    /// The setting's name, set inside the plate with the value after it in
    /// Geist Mono `muted` — the Export panel's selects. Also the select-row
    /// shape.
    pub caption: Option<SharedString>,
    /// The dense trigger: 34 tall at the dense padding, for a thin pod.
    pub compact: bool,
    open: bool,
    leave: motion::Leave,
    highlighted: usize,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    change: Box<dyn Fn(usize, &mut Window, &mut App)>,
}

impl Dropdown {
    pub fn set_handler(&mut self, change: impl Fn(usize, &mut Window, &mut App) + 'static) {
        self.change = Box::new(change);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    pub fn new(
        cx: &mut Context<Self>,
        items: Vec<String>,
        selected: usize,
        theme: Theme,
        change: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            focus: cx.focus_handle(),
            items,
            selected,
            enabled: true,
            theme,
            glyph: None,
            caption: None,
            compact: false,
            open: false,
            leave: motion::Leave::default(),
            highlighted: selected,
            bounds: Rc::new(Cell::new(Bounds::default())),
            change: Box::new(change),
        }
    }

    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.enabled && index < self.items.len() {
            self.selected = index;
            (self.change)(index, window, cx);
        }
        self.open = false;
        cx.notify();
    }
}

impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A dropdown carries no caller-supplied id, and every one of them
        // names its trigger "trigger"; the entity id is the thing that is
        // actually unique per instance, so the tween keys hang off that.
        let menu_key = format!("dropdown-{:?}", cx.entity_id());
        let trigger_key = format!("{menu_key}-trigger");
        let theme = self.theme;
        let open = self.open;
        let row = self.glyph.is_some() || self.caption.is_some();
        // Focus lives on the wrapper, which also holds the menu, so the ring
        // is drawn on the trigger by hand: gpui's `focus_visible` only styles
        // the element that owns the focus handle.
        let ring = self.focus.is_focused(window) && window.last_input_was_keyboard();
        let label = self
            .items
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("—")
            .to_owned();

        let mut root = div()
            .flex()
            .flex_col()
            .id("dropdown")
            .relative()
            .w_full()
            .min_w_0()
            .track_focus(&self.focus)
            .tab_index(0)
            .tab_stop(self.enabled)
            .child(measure(self.bounds.clone()))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if !this.enabled {
                    return;
                }
                match e.keystroke.key.as_str() {
                    "escape" => this.open = false,
                    "down" => {
                        this.open = true;
                        this.highlighted =
                            (this.highlighted + 1).min(this.items.len().saturating_sub(1));
                    }
                    "up" => {
                        this.open = true;
                        this.highlighted = this.highlighted.saturating_sub(1);
                    }
                    "enter" | "space" => {
                        if this.open {
                            this.choose(this.highlighted, w, cx);
                        } else {
                            this.open = true;
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .id("trigger")
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(if row { Theme::ICON_GAP_ROW } else { Theme::GAP }))
                    .h(px(if row {
                        Theme::CONTROL_HEIGHT_LARGE
                    } else if self.compact {
                        Theme::CONTROL_HEIGHT_SMALL
                    } else {
                        Theme::CONTROL_HEIGHT
                    }))
                    .px(px(if row {
                        Theme::CONTROL_PADDING_LARGE
                    } else if self.compact {
                        Theme::CONTROL_PADDING_SMALL
                    } else {
                        Theme::CONTROL_PADDING
                    }))
                    .rounded_full()
                    .bg(motion::hover_blend(
                        &trigger_key,
                        if open { theme.hover } else { theme.sunk },
                        theme.hover,
                    ))
                    .text_size(px(Theme::FONT_CONTROL))
                    .font_weight(if row {
                        FontWeight::NORMAL
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(theme.text)
                    .when(ring, |s| s.shadow(vec![crate::focus_ring(theme)]))
                    .opacity(if self.enabled {
                        1.
                    } else {
                        Theme::DISABLED_OPACITY
                    })
                    .when(self.enabled, |s| {
                        s.cursor_pointer()
                            .active(|s| s.opacity(Theme::PRESSED_OPACITY))
                            .on_hover(motion::hover_listener(trigger_key))
                    })
                    .children(
                        self.glyph
                            .as_ref()
                            .map(|g| icon_sized(g, Theme::ICON_SIZE_MEDIUM, theme.text)),
                    )
                    .map(|el| match &self.caption {
                        Some(caption) => el
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .child(caption.clone()),
                            )
                            .child(crate::mono(label).flex_none().text_color(theme.muted)),
                        None => el.child(div().flex_1().min_w_0().text_ellipsis().child(label)),
                    })
                    // A caret at the caret size, not at the control's. It
                    // says "this opens"; it is not the trigger's own icon,
                    // and at 14 it read as a second glyph competing with the
                    // label.
                    .child(icon_sized(
                        "CaretDown-regular",
                        Theme::ICON_SIZE_CARET,
                        theme.muted,
                    ))
                    .on_click(cx.listener(|this, _, w, cx| {
                        if !this.enabled {
                            return;
                        }
                        w.focus(&this.focus, cx);
                        this.open = !this.open;
                        this.highlighted = this.selected;
                        cx.notify();
                    })),
            );

        // A dismissed menu fades out where it was rather than vanishing.
        let leave = self.leave.shown(open);
        if leave.is_some_and(|leave| leave < 1.) {
            window.request_animation_frame();
        }
        if let Some(leave) = leave {
            // The menu is `anchored`, not absolutely placed. An absolute menu
            // is positioned against its trigger and then clipped by whatever
            // window it happens to be in — which is fine in the editor and
            // wrong in the recorder's options window, a window sized to its
            // sheet with no room below the control. There the menu ran past
            // the frame and the window cut it into a square-cornered box
            // through the middle of a row. `anchored` flips it above the
            // trigger when below will not fit and snaps it inside the frame
            // when neither side will, and the height is capped by the window
            // rather than by the constant alone, so a menu can always be
            // drawn whole.
            let viewport = window.viewport_size();
            let trigger = self.bounds.get();
            let width = trigger.size.width;
            let max_height = Theme::MENU_MAX_HEIGHT
                .min(f32::from(viewport.height) - Theme::GAP * 2.0 - Theme::CONTROL_HEIGHT)
                .max(Theme::CONTROL_HEIGHT);
            root = root.child(
                deferred(
                    anchored()
                        // Anchored in window coordinates off the trigger's own
                        // measured bounds. An `anchored` element lays out
                        // absolutely at its container's origin, so without this
                        // the menu would open on top of the control it belongs
                        // to instead of under it.
                        .position(trigger.bottom_left() + point(px(0.), px(Theme::GAP_SMALL)))
                        .snap_to_window_with_margin(px(Theme::GAP))
                        .child(frost::frosted(
                            Theme::RADIUS_MENU,
                            frost::MENU_BLUR * leave,
                            menu_in(
                                ("dropdown-menu", self.leave.opens),
                                0.0,
                                leave,
                                menu_surface(theme)
                                    .id("choices")
                                    // The menu tracks its trigger rather than
                                    // sizing itself to its longest row: a
                                    // source list holds whole window titles,
                                    // and a menu that grows to fit one runs
                                    // off the edge of the window.
                                    .w(width)
                                    .on_mouse_down_out(cx.listener(
                                        |this, event: &MouseDownEvent, _, cx| {
                                            if !this.bounds.get().contains(&event.position) {
                                                this.open = false;
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .child(fade_edges(
                                        menu_list("dropdown-choices", max_height)
                                            .py(px(crate::FADE_BAND))
                                            .children(self.items.iter().enumerate().map(
                                                |(i, label)| {
                                                    menu_row(
                                                        ("choice", i),
                                                        label.clone(),
                                                        i == self.selected,
                                                        i == self.highlighted,
                                                        theme,
                                                        cx.listener(move |this, _, w, cx| {
                                                            this.choose(i, w, cx)
                                                        }),
                                                    )
                                                },
                                            )),
                                    )),
                            ),
                        )),
                )
                .with_priority(20),
            );
        }
        root
    }
}
