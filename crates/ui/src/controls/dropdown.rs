//! A retained dropdown with keyboard navigation and a frosted popover.

use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
use subtake_theme::Theme;

use crate::{frost, icon_sized, measure, menu_in, menu_list, menu_row, menu_surface, motion};

/// A retained dropdown: keyboard navigation, selected state, and a native GPUI popover.
pub struct Dropdown {
    focus: FocusHandle,
    pub items: Vec<String>,
    pub selected: usize,
    pub enabled: bool,
    pub theme: Theme,
    open: bool,
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
            open: false,
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A dropdown carries no caller-supplied id, and every one of them
        // names its trigger "trigger"; the entity id is the thing that is
        // actually unique per instance, so the tween keys hang off that.
        let menu_key = format!("dropdown-{:?}", cx.entity_id());
        let trigger_key = format!("{menu_key}-trigger");
        let theme = self.theme;
        let open = self.open;
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
                    .gap(px(Theme::GAP))
                    .h(px(Theme::CONTROL_HEIGHT))
                    .px(px(Theme::CONTROL_PADDING))
                    .rounded(px(Theme::RADIUS_CONTROL))
                    .bg(motion::hover_blend(
                        &trigger_key,
                        if open { theme.hover } else { theme.sunk },
                        theme.hover,
                    ))
                    .text_size(px(Theme::FONT_CONTROL))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
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
                    .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                    .child(icon_sized(
                        "CaretDown-regular",
                        Theme::ICON_SIZE_SMALL,
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

        if open {
            root = root.child(
                deferred(frost::frosted(
                    Theme::RADIUS_CARD,
                    frost::MENU_BLUR,
                    menu_in(
                        "dropdown-menu",
                        Theme::CONTROL_HEIGHT + Theme::GAP_SMALL,
                        menu_surface(theme)
                            .id("choices")
                            .absolute()
                            .left_0()
                            // The menu tracks its trigger rather than sizing
                            // itself to its longest row: a source list holds
                            // whole window titles, and a menu that grows to
                            // fit one runs off the edge of the window.
                            .w_full()
                            .min_w(px(Theme::MENU_MIN_WIDTH))
                            .on_mouse_down_out(cx.listener(
                                |this, event: &MouseDownEvent, _, cx| {
                                    if !this.bounds.get().contains(&event.position) {
                                        this.open = false;
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(
                                menu_list("dropdown-choices", Theme::MENU_MAX_HEIGHT).children(
                                    self.items.iter().enumerate().map(|(i, label)| {
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
                                    }),
                                ),
                            ),
                    ),
                ))
                .with_priority(20),
            );
        }
        root
    }
}
