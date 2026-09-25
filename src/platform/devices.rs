//! Microphones and cameras coming and going, over `native/Access.swift`.

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_devices_watch(callback: extern "C" fn());
}

/// Calls `changed` on the main thread whenever a microphone or camera is
/// plugged in or taken away.
#[cfg(target_os = "macos")]
pub fn watch_devices(changed: extern "C" fn()) {
    unsafe { subtake_devices_watch(changed) }
}

/// Not wired: only macOS says when a device comes or goes, so elsewhere the
/// lists change when the sources are refreshed.
#[cfg(not(target_os = "macos"))]
pub fn watch_devices(_changed: extern "C" fn()) {}
