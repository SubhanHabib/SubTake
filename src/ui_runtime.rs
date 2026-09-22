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
