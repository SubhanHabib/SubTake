//! Platform boundary. Windows must implement capture/window integration here;
//! documents, timeline, rendering, export and the UI remain shared.
use crate::media::{ManagedChild, resources};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

mod access;
mod companion;
mod devices;
mod microphone;
mod recording;
mod windows;

pub use access::{Access, has_access, open_access_settings};
use companion::*;
pub use devices::watch_devices;
pub use microphone::meter_microphone;
pub use recording::Recording;
pub use windows::*;

pub fn helper(name: &str) -> Result<PathBuf> {
    let root = resources();
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "win32"
    };
    let candidates = [
        root.join("bin").join(name),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("dist/SubTake.app/Contents/Resources/bin")
            .join(name),
        root.join(format!("electron/native/bin/{os}-{arch}"))
            .join(name),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .with_context(|| format!("Native helper is missing: {name}"))
}

pub fn sources() -> Result<Vec<Value>> {
    sources_cancellable(&std::sync::atomic::AtomicBool::new(false), true)
}

pub fn sources_cancellable(
    cancel: &std::sync::atomic::AtomicBool,
    request_access: bool,
) -> Result<Vec<Value>> {
    #[cfg(target_os = "macos")]
    {
        let bytes = crate::media::capture_output_cancellable(
            Command::new(helper("subtake-platform")?).arg(if request_access {
                "sources"
            } else {
                "sources-passive"
            }),
            Duration::from_secs(75),
            8 * 1024 * 1024,
            cancel,
        )?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[cfg(not(target_os = "macos"))]
    {
        bail!("Native recording source enumeration is not implemented on this platform")
    }
}

pub fn devices() -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        let bytes = crate::media::capture_output(
            Command::new(helper("subtake-platform")?).arg("devices"),
            Duration::from_secs(15),
            1024 * 1024,
        )?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[cfg(not(target_os = "macos"))]
    {
        bail!("Native device enumeration is pending on this platform")
    }
}

pub fn reveal(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open").arg("-R").arg(path).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path.parent().unwrap_or(path))
            .spawn()?;
    }
    Ok(())
}
