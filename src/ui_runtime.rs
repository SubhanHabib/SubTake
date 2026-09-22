//! GPUI application runtime. Keeps the media controller independent of render borrows.
//! Commands and timers execute on the main thread; workers only enqueue completions.
use crate::{
    gpui_views::{RootView, Surface},
    ui_state::UiData,
};
use anyhow::{Context, Result};
use gpui::{px, size, AppContext, AsyncApp, Bounds, WindowBounds, WindowOptions};
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

pub type SharedString = String;
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRc<T>(Rc<Vec<T>>);
impl<T> Default for ModelRc<T> {
    fn default() -> Self {
        Self(Rc::new(Vec::new()))
    }
}
impl<T: Clone> ModelRc<T> {
    pub fn new(model: VecModel<T>) -> Self {
        Self(Rc::new(model.0))
    }
    pub fn row_count(&self) -> usize {
        self.0.len()
    }
    pub fn row_data(&self, row: usize) -> Option<T> {
        self.0.get(row).cloned()
    }
    pub fn iter(&self) -> std::vec::IntoIter<T> {
        self.0.as_ref().clone().into_iter()
    }
}
pub struct VecModel<T>(Vec<T>);
impl<T> From<Vec<T>> for VecModel<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color(u32);
impl Color {
    pub fn from_rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }
    pub fn to_gpui(self) -> gpui::Hsla {
        gpui::rgb(self.0).into()
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image(pub Option<Arc<gpui::RenderImage>>);
impl Image {
    pub fn from_rgba8(buffer: SharedPixelBuffer<Rgba8Pixel>) -> Self {
        let mut bytes = buffer.bytes;
        // GPUI's RenderImage stores BGRA, while the shared compositor emits RGBA.
        for pixel in bytes.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let image = image::RgbaImage::from_raw(buffer.width, buffer.height, bytes)
            .expect("validated pixel buffer");
        Self(Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(image)
        ]))))
    }
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let image = image::open(path)?.into_rgba8();
        Ok(Self::from_rgba8(SharedPixelBuffer::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        )))
    }
}
#[derive(Clone, Copy, Default)]
pub struct Rgba8Pixel;
#[derive(Clone)]
pub struct SharedPixelBuffer<T> {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    marker: PhantomData<T>,
}
impl<T> SharedPixelBuffer<T> {
    pub fn clone_from_slice(bytes: &[u8], width: u32, height: u32) -> Self {
        assert_eq!(bytes.len(), width as usize * height as usize * 4);
        Self {
            bytes: bytes.to_vec(),
            width,
            height,
            marker: PhantomData,
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}
pub trait FromUiData {
    fn from_data(data: Rc<UiData>) -> Self;
}
pub struct Weak<T> {
    id: u64,
    thread: std::thread::ThreadId,
    marker: PhantomData<fn() -> T>,
}
thread_local! {static WEAK_DATA:RefCell<BTreeMap<u64,std::rc::Weak<UiData>>>=RefCell::new(BTreeMap::new());}
impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            thread: self.thread,
            marker: PhantomData,
        }
    }
}
impl<T: FromUiData> Weak<T> {
    pub fn new(data: &Rc<UiData>) -> Self {
        let id = data.window.0.id;
        WEAK_DATA.with(|registry| {
            registry.borrow_mut().insert(id, Rc::downgrade(data));
        });
        Self {
            id,
            thread: std::thread::current().id(),
            marker: PhantomData,
        }
    }
    pub fn upgrade(&self) -> Option<T> {
        // Worker threads may carry this identity, but UI ownership never crosses threads.
        if std::thread::current().id() != self.thread {
            return None;
        }
        WEAK_DATA
            .with(|registry| {
                registry
                    .borrow()
                    .get(&self.id)
                    .and_then(std::rc::Weak::upgrade)
            })
            .map(T::from_data)
    }
}

type Completion = Box<dyn FnOnce() + Send>;
static COMPLETIONS: OnceLock<(
    crossbeam_channel::Sender<Completion>,
    crossbeam_channel::Receiver<Completion>,
)> = OnceLock::new();
static WAKE: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> = OnceLock::new();
fn wake() {
    let _ = WAKE
        .get_or_init(|| async_channel::bounded(1))
        .0
        .try_send(());
}
pub fn invoke_from_event_loop(f: impl FnOnce() + Send + 'static) -> Result<()> {
    COMPLETIONS
        .get_or_init(crossbeam_channel::unbounded)
        .0
        .send(Box::new(f))
        .map_err(|_| anyhow::anyhow!("UI event queue disconnected"))?;
    wake();
    Ok(())
}
type TimerCallback = Rc<RefCell<Box<dyn FnMut()>>>;
struct Scheduled {
    deadline: Instant,
    interval: Option<Duration>,
    callback: TimerCallback,
}
thread_local! {
    static TIMERS:RefCell<BTreeMap<u64,Scheduled>>=RefCell::new(BTreeMap::new());
    static NEXT_ID:Cell<u64>=const{Cell::new(1)};
    static SURFACES:RefCell<Vec<Surface>>=const{RefCell::new(Vec::new())};
    static QUIT:Cell<bool>=const{Cell::new(false)};
    static CONTEXT:RefCell<Option<AsyncApp>>=const{RefCell::new(None)};
}
fn next_id() -> u64 {
    NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}
#[derive(Clone, Copy)]
pub enum TimerMode {
    SingleShot,
    Repeated,
}
#[derive(Default)]
pub struct Timer {
    id: Cell<Option<u64>>,
}
impl Timer {
    pub fn start(&self, mode: TimerMode, duration: Duration, f: impl FnMut() + 'static) {
        self.stop();
        let id = next_id();
        self.id.set(Some(id));
        schedule(id, mode, duration, f);
    }
    pub fn stop(&self) {
        if let Some(id) = self.id.take() {
            TIMERS.with(|t| {
                t.borrow_mut().remove(&id);
            });
        }
    }
    pub fn single_shot(duration: Duration, f: impl FnOnce() + 'static) {
        let mut f = Some(f);
        schedule(next_id(), TimerMode::SingleShot, duration, move || {
            if let Some(f) = f.take() {
                f();
            }
        });
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.stop();
    }
}
fn schedule(id: u64, mode: TimerMode, duration: Duration, f: impl FnMut() + 'static) {
    TIMERS.with(|t| {
        t.borrow_mut().insert(
            id,
            Scheduled {
                deadline: Instant::now() + duration,
                interval: match mode {
                    TimerMode::SingleShot => None,
                    TimerMode::Repeated => Some(duration),
                },
                callback: Rc::new(RefCell::new(Box::new(f))),
            },
        );
    });
    wake();
}
fn drain_completions(queue: &crossbeam_channel::Receiver<Completion>, rearm: impl FnOnce()) {
    // Bound each turn so a producer cannot starve native input.
    for _ in 0..128 {
        match queue.try_recv() {
            Ok(f) => f(),
            Err(_) => break,
        }
    }
    if !queue.is_empty() {
        rearm();
    }
}
fn drain_commands() {
    let queue = &COMPLETIONS.get_or_init(crossbeam_channel::unbounded).1;
    drain_completions(queue, wake);
    let now = Instant::now();
    let due = TIMERS.with(|t| {
        t.borrow()
            .iter()
            .filter(|(_, s)| s.deadline <= now)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>()
    });
    for id in due {
        let callback = TIMERS.with(|t| {
            let mut timers = t.borrow_mut();
            let task = timers.get_mut(&id)?;
            let callback = task.callback.clone();
            if let Some(interval) = task.interval {
                task.deadline = now + interval;
            } else {
                timers.remove(&id);
            }
            Some(callback)
        });
        if let Some(callback) = callback {
            if let Ok(mut f) = callback.try_borrow_mut() {
                f();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowKind {
    Editor,
    Launcher,
    Options,
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
struct WindowState {
    id: u64,
    kind: WindowKind,
    visible: Cell<bool>,
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
pub struct Window(Rc<WindowState>);
impl Window {
    pub fn new(kind: WindowKind) -> Self {
        Self(Rc::new(WindowState {
            id: next_id(),
            kind,
            visible: Cell::new(false),
            dirty: Cell::new(true),
            native: Cell::new(std::ptr::null_mut()),
            handle: Cell::new(None),
            size: Cell::new(match kind {
                WindowKind::Editor => LogicalSize::new(1360., 880.),
                WindowKind::Launcher => LogicalSize::new(724., 80.),
                WindowKind::Options => LogicalSize::new(430., 264.),
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
    pub fn show(&self) {
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
    pub fn set_position(&self, p: PhysicalPosition) {
        if let Some(v) = self.native_view() {
            unsafe {
                crate::platform::ui_window_set_position(v, p.x, p.y);
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
fn surface_window(surface: &Surface) -> &Window {
    match surface {
        Surface::Editor(s) => s.window(),
        Surface::Launcher(s) => s.window(),
        Surface::Options(s) => s.window(),
    }
}
fn sync_windows(cx: &mut gpui::App) -> Result<()> {
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
        };
        if let Surface::Options(ui) = &surface {
            let wanted = LogicalSize::new(ui.get_options_width(), ui.get_options_height());
            if runtime.0.size.get() != wanted {
                runtime.set_size(wanted);
            }
        }
        if runtime.0.handle.get().is_none() && runtime.is_visible() {
            let dimensions = runtime.0.size.get();
            let is_editor = runtime.0.kind == WindowKind::Editor;
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
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
                // their plates carry the near-opaque `overlay` tone instead.
                window_background: if is_editor {
                    gpui::WindowBackgroundAppearance::Blurred
                } else {
                    gpui::WindowBackgroundAppearance::Transparent
                },
                window_min_size: is_editor.then_some(size(px(980.), px(680.))),
                is_resizable: is_editor,
                focus: true,
                show: true,
                ..Default::default()
            };
            let rt = runtime.clone();
            let handle = cx.open_window(options, move |window, cx| {
                if let Ok(handle) = HasWindowHandle::window_handle(window) {
                    if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                        rt.0.native.set(handle.ns_view.as_ptr());
                    }
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
            if is_editor {
                if let Some(view) = runtime.native_view() {
                    unsafe {
                        crate::platform::ui_window_install_magnify(view, native_magnify)?;
                    }
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
                    let _ = crate::platform::configure_recording_hud(
                        &runtime,
                        runtime.0.kind == WindowKind::Launcher,
                    );
                    if runtime.0.kind == WindowKind::Launcher {
                        let _ = crate::platform::position_launcher(&runtime);
                    } else {
                        crate::platform::update_options_glass(&runtime);
                        let launcher = SURFACES.with(|surfaces| {
                            surfaces.borrow().iter().find_map(|surface| match surface {
                                Surface::Launcher(ui) => Some(ui.window().clone()),
                                _ => None,
                            })
                        });
                        if let Some(launcher) = launcher {
                            let _ = crate::platform::position_launcher_options(&runtime, &launcher);
                        }
                    }
                });
            }
        }
        if let Some(handle) = runtime.0.handle.get() {
            if runtime.0.dirty.replace(false) || runtime.0.resize.get() {
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
    }
    Ok(())
}
pub fn quit_event_loop() -> Result<()> {
    QUIT.with(|q| q.set(true));
    wake();
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
pub fn run_event_loop_until_quit() -> Result<()> {
    gpui_platform::application().with_assets(Assets).run(|cx| {
        install_menus(cx);
        CONTEXT.with(|c| *c.borrow_mut() = Some(cx.to_async()));
        if let Err(error) = sync_windows(cx) {
            eprintln!("Open GPUI window: {error:#}");
            cx.quit();
            return;
        }
        cx.spawn(async move |cx| {
            loop {
                let delay = TIMERS
                    .with(|t| {
                        t.borrow()
                            .values()
                            .map(|t| t.deadline.saturating_duration_since(Instant::now()))
                            .min()
                    })
                    .unwrap_or(Duration::from_secs(3600));
                let timer = cx.background_executor().timer(delay);
                let receiver = &WAKE.get_or_init(|| async_channel::bounded(1)).1;
                futures_lite::future::race(
                    async {
                        timer.await;
                    },
                    async {
                        let _ = receiver.recv().await;
                    },
                )
                .await;
                // Run application callbacks outside GPUI's app borrow, allowing native modal dialogs.
                subtake_ui::perf::log("pump wake");
                let began = Instant::now();
                drain_commands();
                subtake_ui::perf::log_took("pump drain_commands", began, 1.0);
                let quit = QUIT.with(Cell::get);
                let began = Instant::now();
                cx.update(|cx| {
                    if quit {
                        cx.quit();
                    } else if let Err(error) = sync_windows(cx) {
                        eprintln!("GPUI update: {error:#}");
                    }
                });
                subtake_ui::perf::log_took("pump sync_windows", began, 1.0);
                if quit {
                    break;
                }
            }
        })
        .detach();
    });
    CONTEXT.with(|c| c.borrow_mut().take());
    Ok(())
}
pub fn run_event_loop() -> Result<()> {
    run_event_loop_until_quit()
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, gpui::Action)]
#[action(no_json)]
struct EditorCommand {
    action: String,
}
fn install_menus(cx: &mut gpui::App) {
    cx.on_action(|command: &EditorCommand, _| {
        let command = command.action.clone();
        Timer::single_shot(Duration::ZERO, move || {
            let editor = SURFACES.with(|s| {
                s.borrow().iter().find_map(|s| {
                    if let Surface::Editor(ui) = s {
                        Some(ui.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(ui) = editor {
                // `@Panel` opens an inspector; everything else is an action.
                // The palette in `gpui_views` strips the prefix the same way.
                if let Some(panel) = command.strip_prefix('@') {
                    ui.set_panel(panel.into());
                    ui.invoke_panel_change(panel.into());
                } else {
                    ui.invoke_action(command);
                }
            }
        });
    });
    // Both menu bars are built from `gpui_views::menu_commands`, so the
    // palette and the OS menu can no longer drift apart. Groups in that table
    // become the separators here.
    let menu = |name: &'static str| gpui::Menu {
        name: name.into(),
        items: crate::gpui_views::menu_commands(name)
            .iter()
            .enumerate()
            .flat_map(|(group, commands)| {
                (group > 0)
                    .then(gpui::MenuItem::separator)
                    .into_iter()
                    .chain(commands.iter().map(|(label, command)| {
                        gpui::MenuItem::action(
                            (*label).to_owned(),
                            EditorCommand {
                                action: (*command).into(),
                            },
                        )
                    }))
            })
            .collect(),
        disabled: false,
    };
    let action = |label: &str, command: &str| {
        gpui::MenuItem::action(
            label.to_owned(),
            EditorCommand {
                action: command.into(),
            },
        )
    };
    cx.set_menus(vec![
        gpui::Menu {
            name: "SubTake".into(),
            items: vec![
                gpui::MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
                gpui::MenuItem::separator(),
                action("Quit SubTake", "quit"),
            ],
            disabled: false,
        },
        menu("File"),
        menu("Edit"),
        menu("Help"),
    ]);
    // The keymap is also what gpui reads to print a menu item's shortcut
    // column, so anything listed in the menus above has to be bound here or
    // it shows blank even though the keystroke works.
    //
    // Only the keys with no text-field meaning are bound. A key equivalent on
    // an NSMenuItem is consumed by Cocoa in `performKeyEquivalent:`, BEFORE
    // the window sees a key-down — so binding Copy/Cut/Paste/Select All here
    // would take them away from a focused `TextInput`, which has its own
    // scoped bindings for them. Those four stay on the key-down path in
    // `RootView`, which already steps aside when an input holds focus.
    let key = |keystroke: &str, command: &str| {
        gpui::KeyBinding::new(
            keystroke,
            EditorCommand {
                action: command.into(),
            },
            None,
        )
    };
    cx.bind_keys([
        key("cmd-q", "quit"),
        key("cmd-o", "open"),
        key("cmd-s", "save"),
        key("cmd-shift-s", "save-as"),
        key("cmd-z", "undo"),
        key("cmd-shift-z", "redo"),
    ]);
}

struct Assets;
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let relative = Path::new(path);
        // App assets are trusted paths supplied by views, never project contents.
        let path = if relative.is_absolute() {
            relative.to_path_buf()
        } else {
            crate::media::resources()
                .join(relative.strip_prefix("legacy-electron").unwrap_or(relative))
        };
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(std::borrow::Cow::Owned(bytes))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let local = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
                Ok(std::fs::read(local).ok().map(std::borrow::Cow::Owned))
            }
            Err(e) => Err(e.into()),
        }
    }
    fn list(&self, path: &str) -> Result<Vec<gpui::SharedString>> {
        let directory = crate::media::resources().join(path);
        if !directory.is_dir() {
            return Ok(vec![]);
        }
        Ok(std::fs::read_dir(directory)?
            .filter_map(|entry| {
                entry
                    .ok()
                    .map(|e| e.path().to_string_lossy().to_string().into())
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn completion_batches_rearm_until_all_work_is_delivered() {
        let (sender, receiver) = crossbeam_channel::unbounded::<Completion>();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..257 {
            let count = count.clone();
            sender
                .send(Box::new(move || {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }))
                .unwrap();
        }
        let wakes = std::cell::Cell::new(0);
        for expected in [128, 256, 257] {
            super::drain_completions(&receiver, || wakes.set(wakes.get() + 1));
            assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), expected);
        }
        assert_eq!(wakes.get(), 2);
        assert!(receiver.is_empty());
    }
    use super::*;
    #[test]
    fn preview_upload_swizzles_red_and_blue_without_changing_alpha() {
        let image = Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
            &[255, 20, 10, 128, 1, 2, 3, 255],
            2,
            1,
        ));
        assert_eq!(
            image.0.unwrap().as_bytes(0).unwrap(),
            &[10, 20, 255, 128, 3, 2, 1, 255]
        );
    }
    #[test]
    fn worker_can_return_ui_identity_but_cannot_access_main_thread_state() {
        let ui = crate::ui_state::EditorWindow::new().unwrap();
        let weak = ui.as_weak();
        let returned = std::thread::spawn(move || {
            assert!(weak.upgrade().is_none());
            weak
        })
        .join()
        .unwrap();
        returned.upgrade().unwrap().set_status("completion".into());
        assert_eq!(ui.get_status(), "completion");
    }
    #[test]
    fn restarting_and_stopping_timers_discards_obsolete_callbacks() {
        let count = Rc::new(Cell::new(0));
        let timer = Timer::default();
        let c = count.clone();
        timer.start(TimerMode::SingleShot, Duration::ZERO, move || {
            c.set(c.get() + 100)
        });
        let c = count.clone();
        timer.start(TimerMode::SingleShot, Duration::ZERO, move || {
            c.set(c.get() + 1)
        });
        drain_commands();
        assert_eq!(count.get(), 1);
        let c = count.clone();
        timer.start(TimerMode::Repeated, Duration::ZERO, move || {
            c.set(c.get() + 1)
        });
        drain_commands();
        assert_eq!(count.get(), 2);
        timer.stop();
        drain_commands();
        assert_eq!(count.get(), 2);
    }
    #[test]
    fn callback_can_schedule_another_timer_without_borrowing_the_scheduler() {
        let count = Rc::new(Cell::new(0));
        let c = count.clone();
        Timer::single_shot(Duration::ZERO, move || {
            c.set(1);
            Timer::single_shot(Duration::ZERO, move || c.set(2));
        });
        drain_commands();
        assert_eq!(count.get(), 1);
        drain_commands();
        assert_eq!(count.get(), 2);
    }
}
