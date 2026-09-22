//! Native window operations on GPUI-owned views: positioning the recorder
//! windows, the magnify gesture bridge, and the recorder glass material.

use super::*;

/// Keep recording controls reachable in fullscreen Spaces. This runs only on
/// the UI thread, while GPUI owns and retains the NSView/NSWindow handles.
pub fn configure_recording_hud(window: &crate::ui_runtime::Window, movable: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            subtake_configure_recorder_overlay(native_view(window)?, movable);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, movable);
    Ok(())
}

pub fn open_feedback() -> Result<()> {
    let url = "https://github.com/SubhanHabib/SubTake/issues";
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = Command::new("xdg-open");
    command.arg(url).spawn()?;
    Ok(())
}

/// The recorder is a menu-bar accessory; the document editor is a regular app window.
pub fn set_editor_active(active: bool) {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_set_editor_active(active);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = active;
}
pub fn activate_launcher() {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_activate_launcher();
    }
}
pub fn install_status_item(callback: extern "C" fn(*const std::ffi::c_char)) {
    #[cfg(target_os = "macos")]
    unsafe {
        let icon = include_bytes!("../../assets/branding/app-icon.png");
        subtake_set_app_icon(icon.as_ptr(), icon.len());
        subtake_install_status_item(callback);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = callback;
}
#[cfg(target_os = "macos")]
pub(super) fn native_view(window: &crate::ui_runtime::Window) -> Result<*mut std::ffi::c_void> {
    window
        .native_view()
        .filter(|view| !view.is_null())
        .context("GPUI window has no native AppKit view yet")
}

pub fn position_launcher(window: &crate::ui_runtime::Window) -> Result<()> {
    configure_recording_hud(window, true)?;
    #[cfg(target_os = "macos")]
    {
        unsafe {
            subtake_position_launcher(native_view(window)?);
        }
    }
    Ok(())
}
/// Position the custom options surface above the fixed recorder bar. The two
/// GPUI render trees remain separate, while AppKit makes the options window a
/// native child of the overlay host for movement and ordering.
pub fn position_launcher_options(
    options: &crate::ui_runtime::Window,
    launcher: &crate::ui_runtime::Window,
) -> Result<()> {
    configure_recording_hud(options, false)?;
    #[cfg(target_os = "macos")]
    {
        unsafe {
            subtake_position_launcher_options(native_view(options)?, native_view(launcher)?);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (options, launcher);
    Ok(())
}
/// Check actual native window geometry rather than the runtime's cached logical
/// coordinates, which can lag behind an AppKit child-window move.
pub fn launcher_options_are_attached(
    options: &crate::ui_runtime::Window,
    launcher: &crate::ui_runtime::Window,
) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(unsafe {
            subtake_launcher_options_are_attached(native_view(options)?, native_view(launcher)?)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let option_position = options.position();
        let option_size = options.size();
        let launcher_position = launcher.position();
        let launcher_size = launcher.size();
        Ok(
            option_position.y + option_size.height as i32 <= launcher_position.y - 12
                && (option_position.x + option_size.width as i32 / 2
                    - (launcher_position.x + launcher_size.width as i32 / 2))
                    .abs()
                    <= 2,
        )
    }
}

/// Called on the UI thread; the generated workspace has no native message bridge.
pub fn open_agent_workspace(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let url = std::ffi::CString::new(url)?;
        unsafe { subtake_open_agent_workspace(url.as_ptr()) };
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    bail!("The embedded agent workspace is currently macOS-only")
}

// Runtime bridge. GPUI retains the view and enclosing window; none of these
// functions takes ownership. Null pointers are harmless (position returns 0,0).
// Non-null pointers MUST remain valid throughout the call, on the UI thread.
macro_rules! ui_window_operation {
    ($name:ident, $native:ident $(, $arg:ident: $ty:ty)*) => {
        /// # Safety
        /// `view` must be null or a live GPUI-owned AppKit NSView on the UI thread.
        pub unsafe fn $name(view: *mut std::ffi::c_void $(, $arg: $ty)*) {
            #[cfg(target_os = "macos")]
            unsafe { $native(view $(, $arg)*); }
            #[cfg(not(target_os = "macos"))]
            let _ = (view $(, $arg)*);
        }
    };
}
ui_window_operation!(ui_window_show, subtake_window_show);
ui_window_operation!(ui_window_hide, subtake_window_hide);
ui_window_operation!(ui_window_focus, subtake_window_focus);
ui_window_operation!(ui_window_minimize, subtake_window_minimize, minimized: bool);
ui_window_operation!(ui_window_set_transparent, subtake_window_set_transparent, transparent: bool);
ui_window_operation!(ui_window_set_blur, subtake_window_set_blur, enabled: bool);

/// Main-thread callback: borrowed NSView identity, logical content top-left x/y,
/// incremental native magnification (positive = expand), raw NSEventPhase bits.
/// A delta of 0.1 means a 10% expansion for this event, not an accumulated scale.
/// Coordinates may fall outside the content bounds as a gesture finishes.
pub type UiMagnifyCallback = extern "C" fn(*mut std::ffi::c_void, f32, f32, f32, u8);
pub const UI_MAGNIFY_PHASE_NONE: u8 = 0;
pub const UI_MAGNIFY_PHASE_BEGAN: u8 = 1;
pub const UI_MAGNIFY_PHASE_STATIONARY: u8 = 2;
pub const UI_MAGNIFY_PHASE_CHANGED: u8 = 4;
pub const UI_MAGNIFY_PHASE_ENDED: u8 = 8;
pub const UI_MAGNIFY_PHASE_CANCELLED: u8 = 16;
pub const UI_MAGNIFY_PHASE_MAY_BEGIN: u8 = 32;

/// Install or replace this view's local magnify monitor. Other windows' events
/// are ignored; native events always continue to GPUI. Window close or view
/// destruction removes the monitor automatically. Hiding suppresses delivery.
///
/// # Safety
/// Call on the UI thread with a live GPUI-owned AppKit NSView. The callback must
/// remain callable until removal and must not unwind across the C ABI. Its view
/// pointer is borrowed only for that call: use it as an identity key, then enqueue
/// work through a weak GPUI entity/context instead of retaining/dereferencing it
/// later. Removal/replacement from inside the callback is supported. A destroyed
/// view's address can be reused, so queued work must target the original weak
/// entity rather than resolving the pointer again. Null returns an error.
pub unsafe fn ui_window_install_magnify(
    view: *mut std::ffi::c_void,
    callback: UiMagnifyCallback,
) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        ensure!(
            unsafe { subtake_window_install_magnify(view, callback) },
            "Magnify monitor requires a live AppKit window"
        );
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (view, callback);
        bail!("Native magnify monitoring is currently macOS-only")
    }
}

/// Remove this view's registration. Repeated removal and null are harmless.
/// An in-progress callback finishes, but no subsequent callbacks are delivered.
/// # Safety
/// Call on the UI thread with null or a live GPUI-owned AppKit NSView. Never
/// pass a stale pointer for cleanup; destruction already removes its monitor.
pub unsafe fn ui_window_remove_magnify(view: *mut std::ffi::c_void) {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_window_remove_magnify(view);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = view;
}

/// AppKit window-server ID for native capture. Returns zero if unavailable.
/// # Safety
/// `view` must be null or a live GPUI-owned AppKit NSView on the UI thread.
pub unsafe fn ui_window_number(view: *mut std::ffi::c_void) -> u32 {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_window_number(view)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = view;
        0
    }
}

/// Outer frame top-left in logical points, measured from the primary screen's
/// top-left, with y increasing downwards. Other displays may use negative values.
/// # Safety
/// `view` must be null or a live GPUI-owned AppKit NSView on the UI thread.
pub unsafe fn ui_window_position(view: *mut std::ffi::c_void) -> (i32, i32) {
    #[cfg(target_os = "macos")]
    {
        let (mut x, mut y) = (0., 0.);
        unsafe {
            subtake_window_get_position(view, &mut x, &mut y);
        }
        (x.round() as i32, y.round() as i32)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = view;
        (0, 0)
    }
}

/// Uses the same logical coordinate system as `ui_window_position`.
/// # Safety
/// `view` must be null or a live GPUI-owned AppKit NSView on the UI thread.
pub unsafe fn ui_window_set_position(view: *mut std::ffi::c_void, x: i32, y: i32) {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_window_set_position(view, x as f64, y as f64);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (view, x, y);
}

/// Begin dragging synchronously during this window's left mouse event.
/// # Safety
/// `view` must be null or a live GPUI-owned AppKit NSView on the UI thread.
pub unsafe fn ui_window_drag(view: *mut std::ffi::c_void) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        ensure!(
            unsafe { subtake_window_drag(view) },
            "Window drag requires a current left mouse event in the GPUI window"
        );
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = view;
        bail!("Native window dragging is currently macOS-only")
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    pub(super) fn subtake_window_install_magnify(
        view: *mut std::ffi::c_void,
        callback: UiMagnifyCallback,
    ) -> bool;
    pub(super) fn subtake_window_remove_magnify(view: *mut std::ffi::c_void);
    pub(super) fn subtake_window_number(view: *mut std::ffi::c_void) -> u32;
    pub(super) fn subtake_window_show(view: *mut std::ffi::c_void);
    pub(super) fn subtake_window_hide(view: *mut std::ffi::c_void);
    pub(super) fn subtake_window_focus(view: *mut std::ffi::c_void);
    pub(super) fn subtake_window_minimize(view: *mut std::ffi::c_void, minimized: bool);
    pub(super) fn subtake_window_set_transparent(view: *mut std::ffi::c_void, transparent: bool);
    pub(super) fn subtake_window_set_blur(view: *mut std::ffi::c_void, enabled: bool);
    pub(super) fn subtake_window_get_position(
        view: *mut std::ffi::c_void,
        x: *mut f64,
        y: *mut f64,
    ) -> bool;
    pub(super) fn subtake_window_set_position(view: *mut std::ffi::c_void, x: f64, y: f64);
    pub(super) fn subtake_window_drag(view: *mut std::ffi::c_void) -> bool;
    pub(super) fn subtake_open_agent_workspace(url: *const std::ffi::c_char);
    pub(super) fn subtake_configure_recorder_overlay(view: *mut std::ffi::c_void, movable: bool);
    pub(super) fn subtake_set_editor_active(active: bool);
    pub(super) fn subtake_activate_launcher();
    pub(super) fn subtake_install_status_item(callback: extern "C" fn(*const std::ffi::c_char));
    pub(super) fn subtake_set_app_icon(bytes: *const u8, length: usize);
    pub(super) fn subtake_position_launcher(view: *mut std::ffi::c_void);
    pub(super) fn subtake_position_launcher_options(
        options: *mut std::ffi::c_void,
        launcher: *mut std::ffi::c_void,
    );
    pub(super) fn subtake_launcher_options_are_attached(
        options: *mut std::ffi::c_void,
        launcher: *mut std::ffi::c_void,
    ) -> bool;
}

/// Native material masked to the two visible recorder cards; margins/text stay clear.
pub fn update_recorder_glass(
    window: &crate::ui_runtime::Window,
    bar: f32,
    options: f32,
    height: f32,
    expanded: bool,
) {
    #[cfg(target_os = "macos")]
    {
        // `sync_launcher` runs after every callback, so this is on the path of
        // every click and keystroke; the mask itself changes only when the
        // recorder is resized. Re-sending an identical geometry still costs an
        // objc dispatch and makes AppKit redo the layer mask, so skip it.
        thread_local! {
            static LAST: std::cell::Cell<Option<(*mut std::ffi::c_void, u64, u64, u64, bool)>> =
                const { std::cell::Cell::new(None) };
        }
        if let Ok(view) = native_view(window) {
            let shape = (
                view,
                bar.to_bits() as u64,
                options.to_bits() as u64,
                height.to_bits() as u64,
                expanded,
            );
            if LAST.with(|last| last.replace(Some(shape))) == Some(shape) {
                return;
            }
            unsafe {
                subtake_update_recorder_glass(
                    view,
                    bar as f64,
                    options as f64,
                    height as f64,
                    expanded,
                );
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, bar, options, height, expanded);
}

/// Apply the same native frosted material to an independent recorder options window.
pub fn update_options_glass(window: &crate::ui_runtime::Window) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(view) = native_view(window) {
            unsafe {
                subtake_update_options_glass(view);
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    pub(super) fn subtake_update_recorder_glass(
        view: *mut std::ffi::c_void,
        bar: f64,
        options: f64,
        height: f64,
        expanded: bool,
    );
    pub(super) fn subtake_update_options_glass(view: *mut std::ffi::c_void);
}
