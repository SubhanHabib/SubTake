//! GPUI application runtime. Keeps the media controller independent of render borrows.
//! Commands and timers execute on the main thread; workers only enqueue completions.
use crate::{
    ui::{RootView, Surface},
    ui_state::UiData,
};
use anyhow::{Context, Result};
use gpui::{AppContext, AsyncApp, Bounds, WindowBounds, WindowOptions, px, size};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

thread_local! {
    /// Every surface registered with the runtime, in creation order.
    static SURFACES: RefCell<Vec<Surface>> = const { RefCell::new(Vec::new()) };
    /// Set by `quit_event_loop`; the loop exits on its next wake.
    static QUIT: Cell<bool> = const { Cell::new(false) };
    /// The async gpui handle, available once the app has started.
    static CONTEXT: RefCell<Option<AsyncApp>> = const { RefCell::new(None) };
}

mod assets;
mod dispatch;
mod event_loop;
mod menus;
mod models;
#[cfg(test)]
mod tests;
mod timer;
mod weak;
mod window;

pub use dispatch::invoke_from_event_loop;
pub use event_loop::{quit_event_loop, run_event_loop, run_event_loop_until_quit};
pub use models::{Color, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
pub use timer::{Timer, TimerMode};
pub use weak::{FromUiData, Weak};
pub use window::{
    CloseRequestResponse, LogicalSize, PhysicalPosition, PhysicalSize, Window, WindowKind, register,
};

use assets::*;
use dispatch::*;
use menus::*;
use timer::*;
use window::*;

/// Open a plain window around a view of the caller's own — the gallery's
/// component catalogue — outside the surface registry, so `sync_windows`
/// never titles, sizes or hides it. It opens opaque, unfocused and flush
/// with the right of the main display. Call it from a timer, not from inside
/// a render: it takes the app borrow.
pub fn open_view_window<V: gpui::Render + 'static>(
    title: &str,
    width: f32,
    height: f32,
    build: impl FnOnce(&mut gpui::Window, &mut gpui::Context<V>) -> V + 'static,
) -> Result<()> {
    let cx = CONTEXT
        .with(|c| c.borrow().clone())
        .context("the gpui app has not started")?;
    let title = gpui::SharedString::from(title.to_owned());
    cx.update(|cx| {
        let screen = cx
            .primary_display()
            .map(|display| display.bounds())
            .unwrap_or_default();
        let height = px(height).min(screen.size.height);
        let origin = gpui::point(screen.right() - px(width), screen.top());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                origin,
                size(px(width), height),
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(title),
                ..Default::default()
            }),
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            focus: false,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| build(window, cx)))
            .map(|_| ())
    })
}
