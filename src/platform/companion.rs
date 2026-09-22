//! The companion helper process that owns the status item and agent workspace.

use super::*;

pub(super) struct Companion {
    pub(super) process: ManagedChild,
    pub(super) events: crossbeam_channel::Receiver<String>,
}
impl Companion {
    pub(super) fn start(
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
    pub(super) fn wait(&mut self, marker: &str, timeout: Duration) -> Result<()> {
        self.wait_cancellable(marker, timeout, &std::sync::atomic::AtomicBool::new(false))
    }
    pub(super) fn wait_cancellable(
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
            if let Ok(line) = self.events.recv_timeout(Duration::from_millis(20))
                && line == marker
            {
                return Ok(());
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
    pub(super) fn command(&mut self, command: &str) -> Result<()> {
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
