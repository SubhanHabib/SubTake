//! The recorder's microphone meter, over `native/MicMeter.swift`.

#[cfg(target_os = "macos")]
use std::{
    ffi::{CString, c_char},
    sync::Mutex,
};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_mic_meter_start(device: *const c_char, callback: extern "C" fn(f32)) -> bool;
    fn subtake_mic_meter_stop();
}

/// The microphone being metered: its unique id, empty for the system default.
#[cfg(target_os = "macos")]
static METERING: Mutex<Option<String>> = Mutex::new(None);

/// Meters `device` (its unique id, empty for the system default), or stops
/// metering for `None`. `level` receives the peak in dBFS about ten times a
/// second on a capture thread. Asking for what is already running does
/// nothing, so this can follow every redraw of the card. Without microphone
/// access nothing is metered, and nothing asks for access.
#[cfg(target_os = "macos")]
pub fn meter_microphone(device: Option<&str>, level: extern "C" fn(f32)) {
    let Ok(mut metering) = METERING.lock() else {
        return;
    };
    if metering.as_deref() == device {
        return;
    }
    match device {
        Some(id) => {
            let Ok(id) = CString::new(id) else {
                return;
            };
            // Without access the meter stays off, and the next call retries.
            *metering = if unsafe { subtake_mic_meter_start(id.as_ptr(), level) } {
                device.map(str::to_owned)
            } else {
                unsafe { subtake_mic_meter_stop() };
                None
            };
        }
        None => {
            unsafe { subtake_mic_meter_stop() };
            *metering = None;
        }
    }
}

/// Not wired: only macOS meters the microphone, so elsewhere the card's meter
/// reads as a dash.
#[cfg(not(target_os = "macos"))]
pub fn meter_microphone(_device: Option<&str>, _level: extern "C" fn(f32)) {}
