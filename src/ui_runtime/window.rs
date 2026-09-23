//! The window wrapper each surface owns, and the sync that pushes its
//! requested geometry and visibility into gpui.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowKind {
    Editor,
    Launcher,
    Options,
    /// The count before a capture, full screen over the display it will
    /// record: nothing to press, so it takes no clicks and no focus.
    Countdown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloseRequestResponse {
    HideWindow,
    KeepWindowShown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

impl LogicalSize {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalPosition {
    pub x: i32,
    pub y: i32,
}

impl PhysicalPosition {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

pub(super) struct WindowState {
    pub(super) id: u64,
    kind: WindowKind,
    visible: Cell<bool>,
    /// Times the window has gone from hidden to shown.
    opens: Cell<u32>,
    dirty: Cell<bool>,
    native: Cell<*mut std::ffi::c_void>,
    handle: Cell<Option<gpui::WindowHandle<RootView>>>,
    size: Cell<LogicalSize>,
    resize: Cell<bool>,
    scale: Cell<f32>,
    close: RefCell<Option<Rc<dyn Fn() -> CloseRequestResponse>>>,
    drop_file: RefCell<Option<Rc<dyn Fn(std::path::PathBuf)>>>,
}

#[derive(Clone)]
pub struct Window(pub(super) Rc<WindowState>);
impl Window {
    pub fn new(kind: WindowKind) -> Self {
        Self(Rc::new(WindowState {
            id: next_id(),
            kind,
            visible: Cell::new(false),
            opens: Cell::new(0),
            dirty: Cell::new(true),
            native: Cell::new(std::ptr::null_mut()),
            handle: Cell::new(None),
            size: Cell::new(match kind {
                WindowKind::Editor => LogicalSize::new(1360., 880.),
                // The recorder window IS the bar, so its height is the
                // bar's — the two cannot be allowed to drift apart.
                WindowKind::Launcher => {
                    LogicalSize::new(724., subtake_theme::Theme::RECORDER_HEIGHT)
                }
                // Only the width is fixed: the card measures itself and
                // resizes the window to its own height (`options_fit`).
                WindowKind::Options => {
                    LogicalSize::new(subtake_theme::Theme::RECORDER_CARD_WIDTH, 264.)
                }
                // The native side gives it the display's frame once open.
                WindowKind::Countdown => LogicalSize::new(640., 400.),
            }),
            resize: Cell::new(false),
            scale: Cell::new(1.),
            close: RefCell::new(None),
            drop_file: RefCell::new(None),
        }))
    }

    pub fn native_view(&self) -> Option<*mut std::ffi::c_void> {
        let ptr = self.0.native.get();
        (!ptr.is_null()).then_some(ptr)
    }

    pub fn invalidate(&self) {
        self.0.dirty.set(true);
        wake();
    }

    pub fn request_redraw(&self) {
        self.invalidate();
    }

    pub fn is_visible(&self) -> bool {
        self.0.visible.get()
    }

    /// Bumps each time the window goes from hidden to shown, so a view can
    /// tell a fresh opening from staying open.
    pub fn opens(&self) -> u32 {
        self.0.opens.get()
    }

    pub fn show(&self) {
        if !self.0.visible.get() {
            self.0.opens.set(self.0.opens.get() + 1);
        }
        self.0.visible.set(true);
        self.invalidate();
        if let Some(view) = self.native_view() {
            unsafe {
                crate::platform::ui_window_show(view);
            }
        }
    }

    pub fn hide(&self) {
        self.0.visible.set(false);
        if let Some(view) = self.native_view() {
            unsafe {
                crate::platform::ui_window_hide(view);
            }
        }
    }

    pub fn set_size(&self, size: LogicalSize) {
        self.0.size.set(size);
        self.0.resize.set(true);
        self.invalidate();
    }

    pub fn size(&self) -> PhysicalSize {
        let s = self.0.size.get();
        let scale = self.scale_factor();
        PhysicalSize {
            width: (s.width * scale) as u32,
            height: (s.height * scale) as u32,
        }
    }

    pub fn scale_factor(&self) -> f32 {
        self.0.scale.get()
    }

    pub fn position(&self) -> PhysicalPosition {
        self.native_view()
            .map(|v| {
                let (x, y) = unsafe { crate::platform::ui_window_position(v) };
                PhysicalPosition { x, y }
            })
            .unwrap_or_default()
    }

    pub fn set_position(&self, position: PhysicalPosition) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_set_position(v, position.x, position.y);
            }
        }
    }

    pub fn focus_window(&self) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_focus(v);
            }
        }
    }

    /// Takes keyboard focus without bringing the app or its other windows
    /// forward.
    pub fn make_key(&self) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_make_key(v);
            }
        }
    }

    pub fn set_minimized(&self, value: bool) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_minimize(v, value);
            }
        }
    }

    pub fn set_transparent(&self, value: bool) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_set_transparent(v, value);
            }
        }
    }

    pub fn set_blur(&self, value: bool) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_set_blur(v, value);
            }
        }
    }

    pub fn drag_window(&self) -> Result<()> {
        unsafe {
            crate::platform::ui_window_drag(self.native_view().context("Window is not open yet")?)
        }
    }

    pub fn on_close_requested(&self, f: impl Fn() -> CloseRequestResponse + 'static) {
        *self.0.close.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_drop_file(&self, f: impl Fn(std::path::PathBuf) + 'static) {
        *self.0.drop_file.borrow_mut() = Some(Rc::new(f));
    }

    pub fn dispatch_drop(&self, path: std::path::PathBuf) {
        let f = self.0.drop_file.borrow().clone();
        if let Some(f) = f {
            f(path);
        }
    }

    pub fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>> {
        let view = self.native_view().context("Window is not open yet")?;
        let number = unsafe { crate::platform::ui_window_number(view) };
        anyhow::ensure!(number != 0, "Window has no native capture ID yet");
        // screencapture rejects hidden destination basenames, including the
        // default .tmp*.png produced by NamedTempFile, sometimes with exit 0.
        let directory = tempfile::Builder::new()
            .prefix("subtake-snapshot-")
            .tempdir()?;
        let path = directory.path().join("window.png");
        let capture = std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", "-t", "png", "-l", &number.to_string()])
            .arg(&path)
            .output()
            .with_context(|| format!("Run native capture for window {number}"))?;
        let diagnostic = String::from_utf8_lossy(&capture.stderr);
        anyhow::ensure!(
            capture.status.success() && path.metadata().is_ok_and(|metadata| metadata.len() > 0),
            "Native capture of window {number} failed (status {}): {}",
            capture.status,
            if diagnostic.trim().is_empty() {
                "No image was produced; check window visibility and Screen Recording permission"
            } else {
                diagnostic.trim()
            },
        );
        let image = image::open(&path)
            .with_context(|| format!("Decode native capture of window {number}"))?
            .into_rgba8();
        Ok(SharedPixelBuffer::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        ))
    }
}

pub fn register(surface: Surface) {
    SURFACES.with(|s| s.borrow_mut().push(surface));
}

pub(super) fn surface_window(surface: &Surface) -> &Window {
    match surface {
        Surface::Editor(s) => s.window(),
        Surface::Launcher(s) => s.window(),
        Surface::Options(s) => s.window(),
        Surface::Countdown(s) => s.window(),
    }
}

pub(super) fn sync_windows(cx: &mut gpui::App) -> Result<()> {
    let surfaces = SURFACES.with(|s| s.borrow().clone());
    for surface in surfaces {
        let runtime = surface_window(&surface).clone();
        let title = match &surface {
            Surface::Editor(ui) => format!(
                "{}{} — SubTake",
                ui.get_document_title(),
                if ui.get_dirty() { " •" } else { "" }
            ),
            Surface::Launcher(_) => "SubTake recorder".into(),
            Surface::Options(_) => "SubTake recorder options".into(),
            Surface::Countdown(_) => "SubTake countdown".into(),
        };
        if let Surface::Options(ui) = &surface {
            let wanted = LogicalSize::new(
                ui.get_options_width(),
                ui.get_options_height().max(ui.get_options_room()),
            );
            if runtime.0.size.get() != wanted {
                runtime.set_size(wanted);
            }
        }
        if runtime.0.handle.get().is_none() && runtime.is_visible() {
            let dimensions = runtime.0.size.get();
            let is_editor = runtime.0.kind == WindowKind::Editor;
            // `SUBTAKE_DISPLAY=<id>` opens every window on that display
            // instead of the main one, to check motion at other refresh rates.
            let display = std::env::var("SUBTAKE_DISPLAY")
                .ok()
                .and_then(|id| id.parse::<u64>().ok())
                .and_then(|id| cx.displays().into_iter().find(|d| u64::from(d.id()) == id))
                .map(|d| d.id());
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    display,
                    size(px(dimensions.width), px(dimensions.height)),
                    cx,
                ))),
                titlebar: if is_editor {
                    Some(gpui::TitlebarOptions {
                        title: Some("SubTake".into()),
                        appears_transparent: true,
                        ..Default::default()
                    })
                } else {
                    None
                },
                kind: if is_editor {
                    gpui::WindowKind::Normal
                } else {
                    gpui::WindowKind::Floating
                },
                // `Blurred` gives the editor the reference's glass: the
                // fork's macOS backend installs its blurred view on
                // `UnderWindowBackground` (`8a8954c`) — the material macOS 26
                // still vends — and composites translucent paint Porter-Duff
                // OVER (`f596cde`), so the chrome tint lands instead of
                // rendering see-through. See `Theme::WINDOW_GLASS_SUPPORTED`.
                //
                // The recorder windows cannot take it. That view fills the
                // content view rectangularly and a borderless window gets no
                // system corner mask, so the material paints a grey square
                // around the rounded recorder plate. They stay transparent and
                // get their material from `update_recorder_glass`, which masks
                // it to the plate.
                window_background: if is_editor {
                    gpui::WindowBackgroundAppearance::Blurred
                } else {
                    gpui::WindowBackgroundAppearance::Transparent
                },
                window_min_size: is_editor.then_some(size(px(980.), px(680.))),
                is_resizable: is_editor,
                focus: runtime.0.kind != WindowKind::Countdown,
                // The card's window opens hidden and is shown once it sits
                // above the bar; shown at once, its first frames land wherever
                // AppKit put it — a card still folded to a dark line, or under
                // Reduce motion the whole card jumping into place.
                show: runtime.0.kind != WindowKind::Options,
                display_id: display,
                ..Default::default()
            };
            let rt = runtime.clone();
            let handle = cx.open_window(options, move |window, cx| {
                if let Ok(handle) = HasWindowHandle::window_handle(window)
                    && let RawWindowHandle::AppKit(handle) = handle.as_raw()
                {
                    rt.0.native.set(handle.ns_view.as_ptr());
                }
                rt.0.scale.set(window.scale_factor());
                let closed = rt.clone();
                window.on_window_should_close(cx, move |_, _| {
                    let closed = closed.clone();
                    Timer::single_shot(Duration::ZERO, move || {
                        let callback = closed.0.close.borrow().clone();
                        if callback
                            .map(|f| f())
                            .unwrap_or(CloseRequestResponse::HideWindow)
                            == CloseRequestResponse::HideWindow
                        {
                            closed.hide();
                        }
                    });
                    false
                });
                cx.new(|cx| RootView::new(surface.clone(), window, cx))
            })?;
            runtime.0.handle.set(Some(handle));
            if is_editor && let Some(view) = runtime.native_view() {
                unsafe {
                    crate::platform::ui_window_install_magnify(view, native_magnify)?;
                }
            }
            if !is_editor {
                // AppKit style/position changes synchronously emit resize and activation
                // events. Apply them after releasing GPUI's mutable application borrow.
                let runtime = runtime.clone();
                Timer::single_shot(Duration::ZERO, move || {
                    if !runtime.is_visible() {
                        return;
                    }
                    if runtime.0.kind == WindowKind::Countdown {
                        let _ = crate::platform::configure_countdown_overlay(&runtime);
                        return;
                    }
                    let _ = crate::platform::configure_recording_hud(
                        &runtime,
                        runtime.0.kind == WindowKind::Launcher,
                    );
                    // Both recorder plates fill their windows, so the
                    // material takes the window's shape at the plate's
                    // radius and the two are one surface: the bar's, or the
                    // card's.
                    crate::platform::update_recorder_glass(
                        &runtime,
                        if runtime.0.kind == WindowKind::Launcher {
                            subtake_theme::Theme::RADIUS_BAR
                        } else {
                            subtake_theme::Theme::RADIUS_PANEL
                        },
                    );
                    if runtime.0.kind == WindowKind::Launcher {
                        let _ = crate::platform::position_launcher(&runtime);
                    } else {
                        let launcher = SURFACES.with(|surfaces| {
                            surfaces.borrow().iter().find_map(|surface| match surface {
                                Surface::Launcher(ui) => Some(ui.window().clone()),
                                _ => None,
                            })
                        });
                        if let Some(launcher) = launcher {
                            let _ = crate::platform::position_launcher_options(&runtime, &launcher);
                        }
                        if let Some(view) = runtime.native_view() {
                            unsafe {
                                crate::platform::ui_window_show(view);
                                crate::platform::ui_window_make_key(view);
                            }
                        }
                    }
                });
            }
        }
        if let Some(handle) = runtime.0.handle.get()
            && (runtime.0.dirty.replace(false) || runtime.0.resize.get())
        {
            handle.update(cx, |_, window, _| {
                window.set_window_title(&title);
                let resizing = runtime.0.resize.replace(false);
                if resizing {
                    let s = runtime.0.size.get();
                    window.resize(size(px(s.width), px(s.height)));
                }
                if !resizing {
                    let dimensions = window.viewport_size();
                    runtime.0.size.set(LogicalSize::new(
                        dimensions.width.into(),
                        dimensions.height.into(),
                    ));
                }
                runtime.0.scale.set(window.scale_factor());
                window.refresh();
            })?;
        }
    }
    Ok(())
}

extern "C" fn native_magnify(
    view: *mut std::ffi::c_void,
    x: f32,
    y: f32,
    delta: f32,
    native_phase: u8,
) {
    // Resolve the identity during the native callback; never dereference a queued raw pointer.
    let handle = SURFACES.with(|surfaces| {
        surfaces.borrow().iter().find_map(|surface| {
            let window = surface_window(surface);
            if window.native_view() == Some(view) {
                window.0.handle.get()
            } else {
                None
            }
        })
    });
    if let Some(handle) = handle {
        let phase = if native_phase & crate::platform::UI_MAGNIFY_PHASE_CANCELLED != 0 {
            3
        } else if native_phase & crate::platform::UI_MAGNIFY_PHASE_ENDED != 0 {
            2
        } else if native_phase & crate::platform::UI_MAGNIFY_PHASE_BEGAN != 0 {
            0
        } else {
            1
        };
        Timer::single_shot(Duration::ZERO, move || {
            let context = CONTEXT.with(|c| c.borrow().clone());
            if let Some(mut context) = context {
                let _ = handle.update(&mut context, |root, window, cx| {
                    root.magnify(x, y, delta, phase, window, cx)
                });
            }
        });
    }
}
