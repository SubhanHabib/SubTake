//! What SubTake may capture, over `native/Access.swift`, and the way to the
//! System Settings pane that changes it.

use super::*;

/// Something capture needs the system's permission for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    Screen,
    Microphone,
    Camera,
}

impl Access {
    /// Its Privacy & Security pane.
    fn pane(self) -> &'static str {
        match self {
            Self::Screen => "Privacy_ScreenCapture",
            Self::Microphone => "Privacy_Microphone",
            Self::Camera => "Privacy_Camera",
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_capture_access(kind: i32) -> bool;
    fn subtake_request_screen_access();
}

/// Whether `access` is allowed, without asking. The screen reads as off
/// until it is allowed; a microphone or camera nobody has been asked about
/// yet reads as allowed, since recording asks for it.
#[cfg(target_os = "macos")]
pub fn has_access(access: Access) -> bool {
    let kind = match access {
        Access::Screen => 0,
        Access::Microphone => 1,
        Access::Camera => 2,
    };
    unsafe { subtake_capture_access(kind) }
}

/// Not wired: only macOS asks, so elsewhere everything reads as allowed.
#[cfg(not(target_os = "macos"))]
pub fn has_access(_access: Access) -> bool {
    true
}

/// Opens System Settings where `access` is turned on. For the screen it asks
/// first, which is what lists SubTake there.
pub fn open_access_settings(access: Access) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if access == Access::Screen {
            unsafe { subtake_request_screen_access() };
        }
        Command::new("/usr/bin/open")
            .arg(format!(
                "x-apple.systempreferences:com.apple.preference.security?{}",
                access.pane()
            ))
            .spawn()?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = access.pane();
        bail!("System Settings is macOS's")
    }
}
