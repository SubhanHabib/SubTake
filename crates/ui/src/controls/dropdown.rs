//! A retained dropdown with keyboard navigation and a frosted popover.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

use crate::{
    fade_edges, frost, icon_sized, measure, menu_in, menu_in_above, menu_list, menu_row,
    menu_surface, motion,
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
    /// Open the menu above the trigger rather than below it, for a control
    /// that sits at the bottom of what it belongs to. It still flips below
    /// when above will not fit.
    pub opens_up: bool,
    /// The chip shape, for a dropdown set on a picture rather than on
    /// chrome: 36 tall on `frost`, its glyph at the card size and its label
    /// in medium — the Camera card's device chip.
    pub chip: bool,
    open: bool,
    leave: motion::Leave,
    highlighted: usize,
    scroll: ScrollHandle,
    /// The menu easing to its height when its choices change while open.
    fit: crate::FitHeight,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    change: Box<dyn Fn(usize, &mut Window, &mut App)>,
}

impl Dropdown {
    pub fn set_handler(&mut self, change: impl Fn(usize, &mut Window, &mut App) + 'static) {
        self.change = Box::new(change);
    }

    /// Open the menu on the current choice, as a click on the trigger does.
    pub fn show(&mut self) {
        self.open = true;
        self.highlighted = self.selected;
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
            opens_up: false,
            chip: false,
            open: false,
            leave: motion::Leave::default(),
            highlighted: selected,
            scroll: ScrollHandle::new(),
            fit: crate::FitHeight::default(),
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
        let chip = self.chip;
        let row = !chip && (self.glyph.is_some() || self.caption.is_some());
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
                        this.scroll.scroll_to_item(this.highlighted);
                    }
                    "up" => {
                        this.open = true;
                        this.highlighted = this.highlighted.saturating_sub(1);
                        this.scroll.scroll_to_item(this.highlighted);
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
                    .gap(px(if chip {
                        Theme::gap()
                    } else if row {
                        Theme::icon_gap_row()
                    } else {
                        Theme::gap()
                    }))
                    .h(px(if chip {
                        Theme::device_chip_height()
                    } else if row {
                        Theme::control_height_large()
                    } else if self.compact {
                        Theme::control_height_small()
                    } else {
                        Theme::control_height()
                    }))
                    .map(|el| {
                        if chip {
                            el.pl(px(Theme::device_chip_padding()))
                                .pr(px(Theme::control_padding_small()))
                        } else {
                            el.px(px(if row {
                                Theme::control_padding_large()
                            } else if self.compact {
                                Theme::control_padding_small()
                            } else {
                                Theme::control_padding()
                            }))
                        }
                    })
                    .rounded_full()
                    // `sunk`, one step up to `sunk2` under the pointer, as
                    // every tinted control's hover goes; open holds it there.
                    // A chip keeps its `frost` under the pointer: it sits on
                    // a live picture, where a step to `sunk2` would read as
                    // the picture changing.
                    .bg(if chip {
                        theme.frost
                    } else {
                        motion::hover_blend(
                            &trigger_key,
                            if open { theme.sunk2 } else { theme.sunk },
                            theme.sunk2,
                        )
                    })
                    .text_size(px(if chip {
                        Theme::font_body()
                    } else {
                        Theme::font_control()
                    }))
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
                        Theme::disabled_opacity()
                    })
                    .when(self.enabled, |s| {
                        s.cursor_pointer()
                            .active(|s| s.opacity(Theme::pressed_opacity()))
                            .on_hover(motion::hover_listener(trigger_key))
                    })
                    .children(
                        self.glyph
                            .as_ref()
                            .map(|g| {
                                icon_sized(
                                    g,
                                    if chip {
                                        Theme::icon_size_card()
                                    } else {
                                        Theme::icon_size_medium()
                                    },
                                    theme.text,
                                )
                            }),
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
                        Theme::icon_size_caret(),
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
            let max_height = Theme::menu_max_height()
                .min(f32::from(viewport.height) - Theme::gap() * 2.0 - Theme::control_height())
                .max(Theme::control_height());
            let list = fade_edges(
                menu_list("dropdown-choices", max_height)
                    .track_scroll(&self.scroll)
                    .children(self.items.iter().enumerate().map(|(i, label)| {
                        menu_row(
                            ("choice", i),
                            label.clone(),
                            i == self.selected,
                            i == self.highlighted,
                            theme,
                            cx.listener(move |this, _, w, cx| this.choose(i, w, cx)),
                        )
                    })),
            )
            .tracking(&self.scroll);
            root = root.child(
                deferred(
                    anchored()
                        // Anchored in window coordinates off the trigger's own
                        // measured bounds. An `anchored` element lays out
                        // absolutely at its container's origin, so without this
                        // the menu would open on top of the control it belongs
                        // to instead of under it.
                        .when(!self.opens_up, |el| {
                            el.position(
                                trigger.bottom_left() + point(px(0.), px(Theme::gap_small())),
                            )
                        })
                        .when(self.opens_up, |el| {
                            el.anchor(Anchor::BottomLeft)
                                .position(trigger.origin - point(px(0.), px(Theme::gap_small())))
                        })
                        .snap_to_window_with_margin(px(Theme::gap()))
                        .child(frost::frosted(
                            Theme::radius_menu(),
                            frost::MENU_BLUR * leave,
                            (if self.opens_up {
                                menu_in_above
                            } else {
                                menu_in
                            })(
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
                                    // Each edge fades only by what is
                                    // scrolled out past it, so a list that
                                    // fits keeps the surface's own padding
                                    // above its first row and below its last,
                                    // the same as at its sides.
                                    .child(self.fit.opening(self.leave.opens).wrap(list, window)),
                            ),
                        )),
                )
                .with_priority(20),
            );
        }
        root
    }
}
