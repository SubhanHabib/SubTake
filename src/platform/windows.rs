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

/// Make the countdown window a click-through sheet over the display the
/// capture will record, just under the recorder bar.
pub fn configure_countdown_overlay(window: &crate::ui_runtime::Window) -> Result<()> {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_configure_countdown_overlay(native_view(window)?);
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}

/// Which display the countdown covers, by its CoreGraphics id; an id no
/// screen has puts it on the recorder bar's screen. Moves an open overlay at
/// once, so the same borrow rule as `set_launcher_options_anchor` applies.
pub fn set_countdown_display(display: u32) {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_set_countdown_display(display)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = display;
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
/// Put the recorder's options card over the bar control that opened it:
/// `anchor` is that control's centre in points from the bar's left edge, or
/// negative to centre the card on the bar. Moves an open card at once, so it
/// must not be called while GPUI holds the app borrow (AppKit reports the
/// move synchronously).
pub fn set_launcher_options_anchor(anchor: f32) {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_set_launcher_options_anchor(anchor as f64)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = anchor;
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
ui_window_operation!(ui_window_make_key, subtake_window_make_key);
ui_window_operation!(ui_window_minimize, subtake_window_minimize, minimized: bool);
ui_window_operation!(ui_window_set_transparent, subtake_window_set_transparent, transparent: bool);
ui_window_operation!(ui_window_set_blur, subtake_window_set_blur, enabled: bool);
ui_window_operation!(ui_window_set_corner_radius, subtake_window_set_corner_radius, radius: f64);
ui_window_operation!(ui_resize_launcher_options, subtake_resize_launcher_options, width: f64, height: f64);
ui_window_operation!(ui_fade_launcher_options, subtake_fade_launcher_options, alpha: f64, seconds: f64);

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
    pub(super) fn subtake_window_make_key(view: *mut std::ffi::c_void);
    pub(super) fn subtake_window_minimize(view: *mut std::ffi::c_void, minimized: bool);
    pub(super) fn subtake_window_set_transparent(view: *mut std::ffi::c_void, transparent: bool);
    pub(super) fn subtake_window_set_blur(view: *mut std::ffi::c_void, enabled: bool);
    pub(super) fn subtake_window_set_corner_radius(view: *mut std::ffi::c_void, radius: f64);
    pub(super) fn subtake_window_set_glass(view: *mut std::ffi::c_void, blur: f64, saturation: f64);
    pub(super) fn subtake_resize_launcher_options(
        view: *mut std::ffi::c_void,
        width: f64,
        height: f64,
    );
    pub(super) fn subtake_fade_launcher_options(
        view: *mut std::ffi::c_void,
        alpha: f64,
        seconds: f64,
    );
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
    pub(super) fn subtake_set_launcher_options_anchor(anchor: f64);
    pub(super) fn subtake_configure_countdown_overlay(view: *mut std::ffi::c_void);
    pub(super) fn subtake_set_countdown_display(display: u32);
    pub(super) fn subtake_launcher_options_are_attached(
        options: *mut std::ffi::c_void,
        launcher: *mut std::ffi::c_void,
    ) -> bool;
}

/// Put the native frosted material under a recorder window's plate, masked
/// to the plate's own shape: the whole window at `radius`. Installed once;
/// the mask follows the window through every resize on its own.
pub fn update_recorder_glass(window: &crate::ui_runtime::Window, radius: f32) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(view) = native_view(window) {
            unsafe {
                subtake_update_recorder_glass(view, radius as f64);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, radius);
}

/// Shrink a recorder window's material to the bottom `height` points, for a
/// plate that can fill less than its window; 0 fills it again.
pub fn set_recorder_glass_height(window: &crate::ui_runtime::Window, height: f32) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(view) = native_view(window) {
            unsafe {
                subtake_set_recorder_glass_height(view, height as f64);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, height);
}

/// Tint a recorder window's material for a dark or a light theme, whatever
/// the system's own appearance.
/// Frosts the editor window behind its paint. Installs the material on the
/// first call and only retunes it after, so it is safe to call every frame.
pub fn set_window_glass(window: &crate::ui_runtime::Window, blur: f32, saturation: f32) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(view) = native_view(window) {
            unsafe {
                subtake_window_set_glass(view, f64::from(blur), f64::from(saturation));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, blur, saturation);
}

/// `set_window_glass` for a window opened straight on gpui, outside the
/// surface registry — the gallery's component catalogue — so it frosts as the
/// editor does. Safe to call every frame, as that one is.
pub fn set_gpui_window_glass(window: &gpui::Window, blur: f32, saturation: f32) {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::AppKit(handle) = handle.as_raw()
        {
            unsafe {
                subtake_window_set_glass(
                    handle.ns_view.as_ptr(),
                    f64::from(blur),
                    f64::from(saturation),
                );
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, blur, saturation);
}

pub fn set_recorder_glass_dark(window: &crate::ui_runtime::Window, dark: bool) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(view) = native_view(window) {
            unsafe {
                subtake_set_recorder_glass_dark(view, dark);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, dark);
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    pub(super) fn subtake_set_recorder_glass_dark(view: *mut std::ffi::c_void, dark: bool);
    pub(super) fn subtake_update_recorder_glass(view: *mut std::ffi::c_void, radius: f64);
    pub(super) fn subtake_set_recorder_glass_height(view: *mut std::ffi::c_void, height: f64);
}
