//! The Camera card's live picture, over `native/CameraPreview.swift`.

#[cfg(target_os = "macos")]
use std::{
    ffi::{CString, c_char},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Where the camera's picture stands after `preview_camera`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraPreview {
    /// Nothing is streaming: none was asked for, or it could not start.
    Off,
    /// The camera asked for has just started; its first frame is to come.
    Started,
    /// The camera asked for was already streaming.
    Streaming,
}

/// Receives one mirrored BGRA frame: its rows, width, height and the bytes
/// to a row, valid only for the call, on a capture thread.
pub type CameraFrame = extern "C" fn(*const u8, i32, i32, i32);

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_camera_preview_start(device: *const c_char, frame: CameraFrame) -> bool;
    fn subtake_camera_preview_stop();
    fn subtake_camera_request_access(answer: extern "C" fn(bool));
}

/// The camera being streamed: its unique id, empty for the system default.
#[cfg(target_os = "macos")]
static STREAMING: Mutex<Option<String>> = Mutex::new(None);
/// Whether the system has been asked for the camera this run.
#[cfg(target_os = "macos")]
static ASKED: AtomicBool = AtomicBool::new(false);

/// Streams `device` (its unique id, empty for the system default) to
/// `frame`, or stops for `None`. Asking for what is already running does
/// nothing, so this can follow every sync of the card. Without access, or
/// without the camera, it is `Off`.
#[cfg(target_os = "macos")]
pub fn preview_camera(device: Option<&str>, frame: CameraFrame) -> CameraPreview {
    let Ok(mut streaming) = STREAMING.lock() else {
        return CameraPreview::Off;
    };
    if streaming.as_deref() == device {
        return if device.is_some() {
            CameraPreview::Streaming
        } else {
            CameraPreview::Off
        };
    }
    match device {
        Some(id) => {
            let Ok(id) = CString::new(id) else {
                return CameraPreview::Off;
            };
            // Unable to start, it stays off, and the next call retries.
            *streaming = if unsafe { subtake_camera_preview_start(id.as_ptr(), frame) } {
                device.map(str::to_owned)
            } else {
                unsafe { subtake_camera_preview_stop() };
                None
            };
            if streaming.is_some() {
                CameraPreview::Started
            } else {
                CameraPreview::Off
            }
        }
        None => {
            unsafe { subtake_camera_preview_stop() };
            *streaming = None;
            CameraPreview::Off
        }
    }
}

/// Asks the system for the camera, the first time in a run only; `answer`
/// then receives whether it may be used, on a system thread. Already
/// answered, the system does not ask again and `answer` receives that.
#[cfg(target_os = "macos")]
pub fn request_camera_access(answer: extern "C" fn(bool)) {
    if !ASKED.swap(true, Ordering::SeqCst) {
        unsafe { subtake_camera_request_access(answer) }
    }
}

/// Not wired: only macOS streams the camera into its card, so elsewhere the
/// picture stays on its spinner.
#[cfg(not(target_os = "macos"))]
pub fn preview_camera(_device: Option<&str>, _frame: CameraFrame) -> CameraPreview {
    CameraPreview::Off
}

#[cfg(not(target_os = "macos"))]
pub fn request_camera_access(_answer: extern "C" fn(bool)) {}
