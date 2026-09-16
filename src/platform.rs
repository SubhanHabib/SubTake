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
pub fn sources_cancellable(cancel: &std::sync::atomic::AtomicBool, request_access: bool) -> Result<Vec<Value>> {
    #[cfg(target_os = "macos")]
    {
        let bytes = crate::media::capture_output_cancellable(
            Command::new(helper("subtake-platform")?).arg(if request_access { "sources" } else { "sources-passive" }),
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
pub struct Recording {
    telemetry: Option<Telemetry>,
    companion: Option<Companion>,
    work: Option<tempfile::TempDir>,
    process: ManagedChild,
    pub output: PathBuf,
    pub paused: bool,
    events: crossbeam_channel::Receiver<String>,
}
impl Recording {
    pub fn start(
        source: &Value,
        output: PathBuf,
        mic: bool,
        system: bool,
        camera: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (source, output, mic, system, camera);
            bail!("Windows capture integration remains in WINDOWS-HANDOFF.md")
        }
        #[cfg(target_os = "macos")]
        {
            ensure!(
                !cancel.load(std::sync::atomic::Ordering::Relaxed),
                "Recording cancelled"
            );
            let work = tempfile::Builder::new()
                .prefix("SubTake-recording-")
                .tempdir_in(output.parent().unwrap_or(Path::new(".")))?;
            std::fs::write(
                work.path().join("session.json"),
                serde_json::to_vec_pretty(
                    &json!({"output":output,"source":source,"microphone":mic,"systemAudio":system,"camera":camera}),
                )?,
            )?;
            let captured = work.path().join("recording.mp4");
            let mut telemetry = Some(Telemetry::start(source)?);
            let mut companion = if mic || camera {
                Some(Companion::start(work.path(), mic, camera, source, cancel)?)
            } else {
                None
            };
            let mut config = json!({"fps":60,"outputPath":captured,"systemAudioOutputPath":work.path().join("recording.system.m4a"),"microphoneOutputPath":work.path().join("recording.mic.m4a"),"capturesMicrophone":false,"capturesSystemAudio":system,"excludedProcessIds":[std::process::id()]});
            if source["kind"] == "window" {
                config["windowId"] = source["nativeId"].clone();
            } else {
                config["displayId"] = source["nativeId"].clone();
            }
            let mut process = ManagedChild::spawn(
                Command::new(helper("recordly-screencapturekit-helper")?)
                    .arg(serde_json::to_string(&config)?)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped()),
            )?;
            let stdout = process.child.stdout.take().unwrap();
            let (tx, events) = crossbeam_channel::unbounded();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            loop {
                ensure!(
                    !cancel.load(std::sync::atomic::Ordering::Relaxed),
                    "Recording cancelled"
                );
                if let Ok(line) = events.recv_timeout(Duration::from_millis(50)) {
                    if line.contains("Recording started") {
                        break;
                    }
                }
                if process.child.try_wait()?.is_some() {
                    bail!("Recording failed: {}", process.errors.lock().unwrap())
                }
                if std::time::Instant::now() > deadline {
                    bail!(
                        "Recording did not start within 20 seconds: {}",
                        process.errors.lock().unwrap()
                    )
                }
            }
            if let Some(c) = &mut companion {
                c.command("start")?;
            }
            if let Some(t) = &mut telemetry {
                t.command("start")?;
            }
            Ok(Self {
                process,
                output,
                paused: false,
                events,
                work: Some(work),
                telemetry,
                companion,
            })
        }
    }
    fn send(&mut self, command: &str) -> Result<()> {
        writeln!(
            self.process
                .child
                .stdin
                .as_mut()
                .context("Recorder input closed")?,
            "{command}"
        )?;
        Ok(())
    }
    pub fn pause(&mut self) -> Result<()> {
        self.send(if self.paused { "resume" } else { "pause" })?;
        let marker = if self.paused {
            "Recording resumed"
        } else {
            "Recording paused"
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(line) = self.events.recv_timeout(Duration::from_millis(50)) {
                if line.contains(marker) {
                    self.paused = !self.paused;
                    if let Some(c) = &mut self.companion {
                        c.command(if self.paused { "pause" } else { "resume" })?;
                    }
                    if let Some(t) = &mut self.telemetry {
                        t.command(if self.paused { "pause" } else { "resume" })?;
                    }
                    return Ok(());
                }
            }
        }
        bail!("Recorder did not acknowledge pause/resume")
    }
    pub fn stop(mut self) -> Result<PathBuf> {
        match self.finish_recording() {
            Ok(path) => Ok(path),
            Err(error) => {
                let path = self.work.take().unwrap().keep();
                Err(error.context(format!(
                    "Recording files retained for recovery at {}",
                    path.display()
                )))
            }
        }
    }
    fn finish_recording(&mut self) -> Result<PathBuf> {
        let mut issues = vec![];
        let mut companion_ok = true;
        if let Some(t) = &mut self.telemetry {
            if let Err(e) = t.command("stop") {
                issues.push(format!("Cursor telemetry: {e}"));
            }
        }
        if let Some(c) = &mut self.companion {
            if let Err(e) = c.command("stop") {
                companion_ok = false;
                issues.push(format!("Camera/microphone: {e}"));
            }
        }
        // Always ask the screen recorder to finalize, even if another helper failed.
        let stop_result = self.send("stop");
        self.process.child.stdin.take();
        let finish_result = self.process.finish(Duration::from_secs(30));
        stop_result?;
        finish_result?;
        if let Some(c) = &mut self.companion {
            if let Err(e) = c.process.finish(Duration::from_secs(30)) {
                companion_ok = false;
                issues.push(format!("Camera/microphone: {e}"));
            }
        }
        let captured = self.work.as_ref().unwrap().path().join("recording.mp4");
        crate::media::probe(&captured)?;
        let work = self.work.as_ref().unwrap().path();
        let mut files = vec![];
        for track in ["system", "mic"] {
            for extension in ["m4a", "wav", "webm"] {
                let from = work.join(format!("recording.{track}.{extension}"));
                files.push((
                    self.output.with_extension(format!("{track}.{extension}")),
                    (from.is_file() && (track != "mic" || companion_ok)).then_some(from),
                ));
            }
        }
        let webcam = work.join("recording.webcam.mp4");
        files.push((
            self.output.with_extension("webcam.mp4"),
            (webcam.is_file() && companion_ok).then_some(webcam),
        ));
        let mut cursor_path = self.output.as_os_str().to_os_string();
        cursor_path.push(".cursor.json");
        let cursor = if let Some(mut telemetry) = self.telemetry.take() {
            match telemetry.finish() {
                Ok(samples) => {
                    let path = work.join("cursor.json");
                    std::fs::write(
                        &path,
                        serde_json::to_vec(&json!({"version":1,"samples":samples}))?,
                    )?;
                    Some(path)
                }
                Err(e) => {
                    issues.push(format!("Cursor telemetry: {e}"));
                    None
                }
            }
        } else {
            None
        };
        files.push((PathBuf::from(cursor_path), cursor));
        files.push((self.output.clone(), Some(captured)));
        crate::file_group::replace(work, &files)?;
        ensure!(
            issues.is_empty(),
            "Video saved to {}, but companion capture needs review: {}",
            self.output.display(),
            issues.join("; ")
        );
        Ok(self.output.clone())
    }
    pub fn error(&mut self) -> Option<String> {
        if let Some(c) = &mut self.companion {
            if c.process.child.try_wait().ok().flatten().is_some() {
                return Some(format!(
                    "Camera/microphone recording ended: {}",
                    c.process.errors.lock().unwrap()
                ));
            }
        }
        self.process.child.try_wait().ok().flatten().map(|s| {
            format!(
                "Recording ended ({s}): {}",
                self.process.errors.lock().unwrap()
            )
        })
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

struct Telemetry {
    process: ManagedChild,
    monitor: Option<ManagedChild>,
    samples: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Telemetry {
    fn start(source: &Value) -> Result<Self> {
        let state = std::sync::Arc::new(std::sync::Mutex::new(String::from("arrow")));
        let mut monitor = ManagedChild::spawn(
            Command::new(helper("recordly-native-cursor-monitor")?).stdout(Stdio::piped()),
        )
        .ok();
        if let Some(m) = &mut monitor {
            let output = m.child.stdout.take().unwrap();
            let state = state.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(output).lines().map_while(Result::ok) {
                    if let Some(value) = line.strip_prefix("STATE:") {
                        *state.lock().unwrap() = value.trim().into();
                    }
                }
            });
        }
        let mut process = ManagedChild::spawn(
            Command::new(helper("subtake-platform")?)
                .arg("telemetry")
                .arg(serde_json::to_string(source)?)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped()),
        )?;
        let output = process.child.stdout.take().unwrap();
        let samples = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results = samples.clone();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Ok(mut point) = serde_json::from_str::<Value>(&line) {
                    point["cursorType"] = json!(state.lock().unwrap().clone());
                    let mut samples = results.lock().unwrap();
                    if samples.len() < 2_000_000 {
                        samples.push(point);
                    }
                }
            }
        });
        Ok(Self {
            process,
            monitor,
            samples,
            reader: Some(reader),
        })
    }
    fn command(&mut self, command: &str) -> Result<()> {
        writeln!(
            self.process
                .child
                .stdin
                .as_mut()
                .context("Telemetry input closed")?,
            "{command}"
        )?;
        Ok(())
    }
    fn finish(&mut self) -> Result<Vec<Value>> {
        self.process.child.stdin.take();
        self.process.finish(Duration::from_secs(5))?;
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.monitor.take();
        Ok(std::mem::take(&mut *self.samples.lock().unwrap()))
    }
}

struct Companion {
    process: ManagedChild,
    events: crossbeam_channel::Receiver<String>,
}
impl Companion {
    fn start(
        folder: &Path,
        microphone: bool,
        camera: bool,
        source: &Value,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        let config = json!({"folder":folder,"microphone":microphone,"camera":camera,"cameraId":source["cameraId"],"microphoneId":source["microphoneId"]});
        let mut process = ManagedChild::spawn(
            Command::new(helper("subtake-companion")?)
                .arg(config.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped()),
        )?;
        let stdout = process
            .child
            .stdout
            .take()
            .context("Open companion events")?;
        let (tx, events) = crossbeam_channel::bounded(64);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut companion = Self { process, events };
        companion.wait_cancellable("Companion ready", Duration::from_secs(75), cancel)?;
        Ok(companion)
    }
    fn wait(&mut self, marker: &str, timeout: Duration) -> Result<()> {
        self.wait_cancellable(marker, timeout, &std::sync::atomic::AtomicBool::new(false))
    }
    fn wait_cancellable(
        &mut self,
        marker: &str,
        timeout: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            ensure!(
                !cancel.load(std::sync::atomic::Ordering::Relaxed),
                "Recording cancelled"
            );
            if let Ok(line) = self.events.recv_timeout(Duration::from_millis(20)) {
                if line == marker {
                    return Ok(());
                }
            }
            ensure!(
                self.process.child.try_wait()?.is_none(),
                "Camera/microphone: {}",
                self.process.errors.lock().unwrap()
            );
            ensure!(
                std::time::Instant::now() < deadline,
                "Camera/microphone did not acknowledge {marker}"
            );
        }
    }
    fn command(&mut self, command: &str) -> Result<()> {
        writeln!(
            self.process
                .child
                .stdin
                .as_mut()
                .context("Companion input closed")?,
            "{command}"
        )?;
        if command != "stop" {
            self.wait(
                match command {
                    "start" => "Companion started",
                    "pause" => "Companion paused",
                    _ => "Companion resumed",
                },
                Duration::from_secs(5),
            )?;
        }
        Ok(())
    }
}

/// Keep recording controls reachable in fullscreen Spaces. This runs only on
/// the UI thread, while Slint owns and retains the NSView/NSWindow handles.
pub fn configure_recording_hud(window: &slint::Window, movable: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let host = window.window_handle();
        if let RawWindowHandle::AppKit(handle) = host.window_handle()?.as_raw() {
            // Slint owns the content view. AppKit applies native non-activating
            // floating-panel behavior to its enclosing NSWindow.
            unsafe { subtake_configure_recorder_overlay(handle.ns_view.as_ptr(), movable); }
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
        let icon = include_bytes!("../assets/branding/app-icon.png");
        subtake_set_app_icon(icon.as_ptr(), icon.len());
        subtake_install_status_item(callback);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = callback;
}
pub fn position_launcher(window: &slint::Window) -> Result<()> {
    configure_recording_hud(window, true)?;
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let RawWindowHandle::AppKit(handle) = window.window_handle().window_handle()?.as_raw() {
            // Slint retains this NSView on the UI thread throughout the call.
            unsafe {
                subtake_position_launcher(handle.ns_view.as_ptr());
            }
        }
    }
    Ok(())
}
/// Position the custom options surface above the fixed recorder bar. The two
/// Slint render trees remain separate, while AppKit makes the options window a
/// native child of the overlay host for movement and ordering.
pub fn position_launcher_options(options: &slint::Window, launcher: &slint::Window) -> Result<()> {
    configure_recording_hud(options, false)?;
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let options_winit = options.window_handle();
        let launcher_winit = launcher.window_handle();
        let option_handle = options_winit.window_handle()?;
        let launcher_handle = launcher_winit.window_handle()?;
        if let (RawWindowHandle::AppKit(options), RawWindowHandle::AppKit(launcher)) = (option_handle.as_raw(), launcher_handle.as_raw()) {
            // Both handles are owned by Slint on this event-loop thread.
            unsafe { subtake_position_launcher_options(options.ns_view.as_ptr(), launcher.ns_view.as_ptr()); }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (options, launcher);
    Ok(())
}
/// Check actual native window geometry rather than Winit's cached logical
/// coordinates, which can lag behind an AppKit child-window move.
pub fn launcher_options_are_attached(options: &slint::Window, launcher: &slint::Window) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let options_winit = options.window_handle();
        let launcher_winit = launcher.window_handle();
        let option_handle = options_winit.window_handle()?;
        let launcher_handle = launcher_winit.window_handle()?;
        if let (RawWindowHandle::AppKit(options), RawWindowHandle::AppKit(launcher)) =
            (option_handle.as_raw(), launcher_handle.as_raw())
        {
            return Ok(unsafe {
                subtake_launcher_options_are_attached(options.ns_view.as_ptr(), launcher.ns_view.as_ptr())
            });
        }
        return Ok(false);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let option_position = options.position();
        let option_size = options.size();
        let launcher_position = launcher.position();
        let launcher_size = launcher.size();
        Ok(option_position.y + option_size.height as i32 <= launcher_position.y - 12
            && (option_position.x + option_size.width as i32 / 2
                - (launcher_position.x + launcher_size.width as i32 / 2))
                .abs()
                <= 2)
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_configure_recorder_overlay(view: *mut std::ffi::c_void, movable: bool);
    fn subtake_set_editor_active(active: bool);
    fn subtake_activate_launcher();
    fn subtake_install_status_item(callback: extern "C" fn(*const std::ffi::c_char));
    fn subtake_set_app_icon(bytes: *const u8, length: usize);
    fn subtake_position_launcher(view: *mut std::ffi::c_void);
    fn subtake_position_launcher_options(options: *mut std::ffi::c_void, launcher: *mut std::ffi::c_void);
    fn subtake_launcher_options_are_attached(options: *mut std::ffi::c_void, launcher: *mut std::ffi::c_void) -> bool;
}

/// Native material masked to the two visible recorder cards; margins/text stay clear.
pub fn update_recorder_glass(window: &slint::Window, bar: f32, options: f32, height: f32, expanded: bool) {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = window.window_handle().window_handle() {
            if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                unsafe {
                    subtake_update_recorder_glass(handle.ns_view.as_ptr(), bar as f64,
                        options as f64, height as f64, expanded);
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, bar, options, height, expanded);
}

/// Apply the same native frosted material to an independent recorder options window.
pub fn update_options_glass(window: &slint::Window) {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(host) = window.window_handle().window_handle() {
            if let RawWindowHandle::AppKit(handle) = host.as_raw() {
                unsafe { subtake_update_options_glass(handle.ns_view.as_ptr()); }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_update_recorder_glass(view: *mut std::ffi::c_void, bar: f64, options: f64, height: f64, expanded: bool);
    fn subtake_update_options_glass(view: *mut std::ffi::c_void);
}
