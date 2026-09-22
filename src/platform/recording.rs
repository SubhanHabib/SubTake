//! Driving the native recorder helper and reading its telemetry.

use super::*;

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
                if let Ok(line) = events.recv_timeout(Duration::from_millis(50))
                    && line.contains("Recording started")
                {
                    break;
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

    pub(super) fn send(&mut self, command: &str) -> Result<()> {
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
            if let Ok(line) = self.events.recv_timeout(Duration::from_millis(50))
                && line.contains(marker)
            {
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

    pub(super) fn finish_recording(&mut self) -> Result<PathBuf> {
        let mut issues = vec![];
        let mut companion_ok = true;
        if let Some(t) = &mut self.telemetry
            && let Err(e) = t.command("stop")
        {
            issues.push(format!("Cursor telemetry: {e}"));
        }
        if let Some(c) = &mut self.companion
            && let Err(e) = c.command("stop")
        {
            companion_ok = false;
            issues.push(format!("Camera/microphone: {e}"));
        }
        // Always ask the screen recorder to finalize, even if another helper failed.
        let stop_result = self.send("stop");
        self.process.child.stdin.take();
        let finish_result = self.process.finish(Duration::from_secs(30));
        stop_result?;
        finish_result?;
        if let Some(c) = &mut self.companion
            && let Err(e) = c.process.finish(Duration::from_secs(30))
        {
            companion_ok = false;
            issues.push(format!("Camera/microphone: {e}"));
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
        if let Some(c) = &mut self.companion
            && c.process.child.try_wait().ok().flatten().is_some()
        {
            return Some(format!(
                "Camera/microphone recording ended: {}",
                c.process.errors.lock().unwrap()
            ));
        }
        self.process.child.try_wait().ok().flatten().map(|s| {
            format!(
                "Recording ended ({s}): {}",
                self.process.errors.lock().unwrap()
            )
        })
    }
}

pub(super) struct Telemetry {
    process: ManagedChild,
    monitor: Option<ManagedChild>,
    samples: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Telemetry {
    pub(super) fn start(source: &Value) -> Result<Self> {
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

    pub(super) fn command(&mut self, command: &str) -> Result<()> {
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

    pub(super) fn finish(&mut self) -> Result<Vec<Value>> {
        self.process.child.stdin.take();
        self.process.finish(Duration::from_secs(5))?;
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.monitor.take();
        Ok(std::mem::take(&mut *self.samples.lock().unwrap()))
    }
}
