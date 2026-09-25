use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f64,
    pub audio_tracks: usize,
}

pub fn resources() -> PathBuf {
    if let Some(path) = std::env::var_os("SUBTAKE_RESOURCES") {
        return path.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.parent().unwrap().join("../Resources");
        if p.is_dir() {
            return p;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("legacy-electron")
}

pub fn binary(name: &str) -> Result<PathBuf> {
    let env = format!("SUBTAKE_{}", name.to_uppercase().replace('-', "_"));
    if let Some(path) = std::env::var_os(env) {
        let p = PathBuf::from(path);
        ensure!(p.is_file(), "Configured {name} executable is missing");
        return Ok(p);
    }
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let root = resources();
    let mut candidates = vec![root.join("bin").join(&executable), root.join(&executable)];
    if name == "ffmpeg" {
        candidates.push(root.join("node_modules/ffmpeg-static").join(&executable));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| p.join(&executable)));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin").join(&executable),
        PathBuf::from("/usr/local/bin").join(&executable),
    ]);
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .with_context(|| {
            format!(
                "{name} was not found. Install the bundled runtime or set SUBTAKE_{}.",
                name.to_uppercase()
            )
        })
}

pub fn probe(path: &Path) -> Result<MediaInfo> {
    ensure!(path.is_file(), "Media file is missing: {}", path.display());
    let output = capture_output(
        Command::new(binary("ffprobe")?)
            .args([
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ])
            .arg(path),
        Duration::from_secs(30),
        8 * 1024 * 1024,
    )?;
    let data: Value = serde_json::from_slice(&output)?;
    let streams = data["streams"].as_array().context("No media streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .context("File has no video stream")?;
    let duration = data["format"]["duration"]
        .as_str()
        .or(video["duration"].as_str())
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.);
    ensure!(
        duration.is_finite() && duration > 0.,
        "Video has no finite duration"
    );
    let rate = video["avg_frame_rate"].as_str().unwrap_or("30/1");
    let (a, b) = rate.split_once('/').unwrap_or(("30", "1"));
    let fps = a.parse::<f64>().unwrap_or(30.) / b.parse::<f64>().unwrap_or(1.).max(1.);
    let mut width = video["width"].as_u64().unwrap_or(0) as u32;
    let mut height = video["height"].as_u64().unwrap_or(0) as u32;
    if video["side_data_list"].as_array().is_some_and(|a| {
        a.iter()
            .any(|s| s["rotation"].as_i64().unwrap_or(0).abs() % 180 == 90)
    }) {
        std::mem::swap(&mut width, &mut height);
    }
    ensure!(
        width > 0 && height > 0 && width <= 32768 && height <= 32768,
        "Unsupported video dimensions"
    );
    Ok(MediaInfo {
        width,
        height,
        duration,
        fps: fps.clamp(1., 240.),
        audio_tracks: streams
            .iter()
            .filter(|s| s["codec_type"] == "audio")
            .count(),
    })
}

pub struct ManagedChild {
    pub child: Child,
    pub errors: Arc<Mutex<String>>,
}

impl ManagedChild {
    pub fn spawn(command: &mut Command) -> Result<Self> {
        let mut child = command.stderr(Stdio::piped()).spawn()?;
        let mut stderr = child.stderr.take().unwrap();
        let errors = Arc::new(Mutex::new(String::new()));
        let log = errors.clone();
        thread::spawn(move || {
            let mut bytes = [0; 2048];
            while let Ok(count) = stderr.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                let mut s = log.lock().unwrap();
                s.push_str(&String::from_utf8_lossy(&bytes[..count]));
                if s.len() > 16384 {
                    let boundary = s
                        .char_indices()
                        .find(|(i, _)| *i >= s.len() - 8192)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    s.drain(..boundary);
                }
            }
        });
        Ok(Self { child, errors })
    }

    pub fn finish(&mut self, timeout: Duration) -> Result<()> {
        self.finish_cancellable(timeout, &std::sync::atomic::AtomicBool::new(false))
    }

    pub fn finish_cancellable(
        &mut self,
        timeout: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                bail!("Operation cancelled")
            }
            if let Some(status) = self.child.try_wait()? {
                ensure!(
                    status.success(),
                    "Media process failed: {}",
                    self.errors.lock().unwrap()
                );
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "Media process timed out: {}",
                self.errors.lock().unwrap()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[allow(dead_code)]
pub struct Decoder {
    #[cfg(feature = "native-ffmpeg")]
    native: Option<crate::native_decoder::NativeDecoder>,
    path: PathBuf,
    width: u32,
    height: u32,
    rate: f64,
    process: Option<ManagedChild>,
    stdout: Option<ChildStdout>,
    next_index: u64,
    last_index: Option<u64>,
    last: Option<Vec<u8>>,
}

impl Decoder {
    pub fn new(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            #[cfg(feature = "native-ffmpeg")]
            native: None,
            path,
            width,
            height,
            rate: 60.,
            process: None,
            stdout: None,
            next_index: 0,
            last_index: None,
            last: None,
        }
    }

    pub fn with_rate(mut self, rate: f64) -> Self {
        self.rate = rate.clamp(1., 240.);
        self
    }

    #[allow(dead_code)]
    fn start(&mut self, time: f64) -> Result<()> {
        self.stdout = None;
        self.process = None;
        let filter = format!(
            "fps={},scale={}:{}:flags=bilinear",
            self.rate, self.width, self.height
        );
        let mut child = ManagedChild::spawn(
            Command::new(binary("ffmpeg")?)
                .args([
                    "-v",
                    "error",
                    "-nostdin",
                    "-hwaccel",
                    "auto",
                    "-ss",
                    &format!("{time:.6}"),
                    "-i",
                ])
                .arg(&self.path)
                .args([
                    "-map", "0:v:0", "-an", "-sn", "-vf", &filter, "-pix_fmt", "rgba", "-f",
                    "rawvideo", "pipe:1",
                ])
                .stdout(Stdio::piped()),
        )?;
        self.stdout = child.child.stdout.take();
        self.process = Some(child);
        self.next_index = (time * self.rate).round() as u64;
        Ok(())
    }

    pub fn frame(&mut self, time: f64) -> Result<Vec<u8>> {
        let index = (time.max(0.) * self.rate + 1e-6).floor() as u64;
        if self.last_index == Some(index)
            && let Some(frame) = &self.last
        {
            return Ok(frame.clone());
        }

        #[cfg(feature = "native-ffmpeg")]
        {
            if self.native.is_none() {
                self.native = Some(crate::native_decoder::NativeDecoder::open(
                    &self.path,
                    self.width,
                    self.height,
                )?);
            }
            let bytes = self
                .native
                .as_mut()
                .unwrap()
                .frame(index as f64 / self.rate)?;
            self.last_index = Some(index);
            self.last = Some(bytes.clone());
            Ok(bytes)
        }

        #[cfg(not(feature = "native-ffmpeg"))]
        {
            if self.process.is_none()
                || index < self.next_index
                || index.saturating_sub(self.next_index) as f64 > self.rate * 0.25
            {
                self.start(index as f64 / self.rate)?;
            }
            let mut bytes = vec![0; self.width as usize * self.height as usize * 4];
            loop {
                if let Err(error) = self.stdout.as_mut().unwrap().read_exact(&mut bytes) {
                    if self.last_index.is_some_and(|last| {
                        index >= last && (index - last) as f64 / self.rate < 0.05
                    }) {
                        if let Some(last) = &self.last {
                            return Ok(last.clone());
                        }
                    }
                    return Err(error).with_context(|| {
                        format!(
                            "Decode video frame: {}",
                            self.process.as_ref().unwrap().errors.lock().unwrap()
                        )
                    });
                }
                let decoded = self.next_index;
                self.next_index += 1;
                if decoded >= index {
                    self.last_index = Some(decoded);
                    break;
                }
            }
            self.last = Some(bytes.clone());
            Ok(bytes)
        }
    }
}

/// Drain subprocess output concurrently, limiting both memory and execution time.
pub fn capture_output(command: &mut Command, timeout: Duration, limit: usize) -> Result<Vec<u8>> {
    capture_output_cancellable(
        command,
        timeout,
        limit,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

pub fn capture_output_cancellable(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>> {
    let mut child = ManagedChild::spawn(command.stdout(Stdio::piped()).stdin(Stdio::null()))?;
    let mut stdout = child.child.stdout.take().context("Open process output")?;
    let reader = thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        let mut result = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut exceeded = false;
        loop {
            let count = stdout.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            let remaining = limit.saturating_sub(result.len());
            result.extend_from_slice(&chunk[..count.min(remaining)]);
            exceeded |= count > remaining;
        }
        Ok((result, exceeded))
    });
    let status = child.finish_cancellable(timeout, cancel);
    if status.is_err() {
        let _ = child.child.kill();
        let _ = child.child.wait();
    }
    let (bytes, exceeded) = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Process output reader failed"))??;
    status?;
    ensure!(!exceeded, "Process output exceeded {limit} bytes");
    Ok(bytes)
}

/// How many frames the timeline's thumbnail strip holds, side by side, each
/// taken from the middle of its tenth of the recording.
pub const TIMELINE_FRAMES: u32 = 10;

/// Source thumbnails and waveform are cached outside the document. Cache identity
/// includes file metadata so replacing media cannot reuse the previous artwork.
pub fn timeline_artwork(source: &Path, info: &MediaInfo) -> Result<(PathBuf, Option<PathBuf>)> {
    let (directory, name) = cache_entry(source)?;
    let thumbs = directory.join(format!("{name}-thumbs.png"));
    let waveform = directory.join(format!("{name}-wave.png"));
    if !thumbs.exists() {
        let height = (128. * info.height as f64 / info.width as f64)
            .round()
            .max(2.) as u32;
        let mut decoder = Decoder::new(source.into(), 128, height);
        let mut strip = image::RgbaImage::new(128 * TIMELINE_FRAMES, 72);
        for i in 0..TIMELINE_FRAMES {
            let time = (i as f64 + 0.5) * info.duration / TIMELINE_FRAMES as f64;
            let pixels = decoder.frame(time)?;
            let frame = image::RgbaImage::from_raw(128, height, pixels)
                .context("Invalid thumbnail pixels")?;
            let frame =
                image::imageops::resize(&frame, 128, 72, image::imageops::FilterType::Triangle);
            image::imageops::overlay(&mut strip, &frame, i64::from(i) * 128, 0);
        }
        let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
        strip.write_to(&mut temp, image::ImageFormat::Png)?;
        temp.persist(&thumbs).map_err(|e| e.error)?;
    }
    let audio = [
        source.with_extension("system.m4a"),
        source.with_extension("mic.m4a"),
        source.with_extension("system.wav"),
        source.with_extension("mic.wav"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .or_else(|| (info.audio_tracks > 0).then(|| source.into()));
    let waveform = if let Some(audio) = audio {
        if !waveform.exists() {
            let temp = tempfile::NamedTempFile::new_in(&directory)?;
            let mut process = ManagedChild::spawn(
                Command::new(binary("ffmpeg")?)
                    .args(["-v", "error", "-nostdin", "-y", "-i"])
                    .arg(audio)
                    .args([
                        "-filter_complex",
                        "aformat=channel_layouts=mono,showwavespic=s=1280x36:colors=0x58bfbd",
                        "-frames:v",
                        "1",
                        "-f",
                        "image2",
                        "-vcodec",
                        "png",
                    ])
                    .arg(temp.path())
                    .stdout(Stdio::null()),
            )?;
            process.finish(Duration::from_secs(120))?;
            temp.persist(&waveform).map_err(|e| e.error)?;
        }
        Some(waveform)
    } else {
        None
    };
    Ok((thumbs, waveform))
}

/// How wide a library card's still is decoded, twice the card's drawn width
/// so it stays sharp on a Retina display.
pub const STILL_WIDTH: u32 = 480;

/// A library card's picture and running time.
pub struct LibraryStill {
    pub still: PathBuf,
    /// In seconds.
    pub duration: f64,
}

/// A still for a library card: one frame a quarter of the way into the take,
/// cached beside the timeline artwork with the take's running time. A
/// project reads its source video.
pub fn library_still(path: &Path) -> Result<LibraryStill> {
    let source = if matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("recordly" | "openscreen")
    ) {
        let mut project = crate::project::Project::load(path)?;
        project.resolve_assets(path);
        project.source_path(Some(path))
    } else {
        path.to_owned()
    };
    let (directory, name) = cache_entry(&source)?;
    let still = directory.join(format!("{name}-still.png"));
    let length = directory.join(format!("{name}-duration.txt"));
    let cached = std::fs::read_to_string(&length)
        .ok()
        .and_then(|text| text.trim().parse::<f64>().ok());
    if still.exists()
        && let Some(duration) = cached
    {
        return Ok(LibraryStill { still, duration });
    }
    let info = probe(&source)?;
    if !still.exists() {
        let height = (STILL_WIDTH as f64 * info.height as f64 / info.width.max(1) as f64)
            .round()
            .max(2.) as u32;
        let pixels = Decoder::new(source, STILL_WIDTH, height).frame(info.duration / 4.)?;
        let frame = image::RgbaImage::from_raw(STILL_WIDTH, height, pixels)
            .context("Invalid still pixels")?;
        let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
        frame.write_to(&mut temp, image::ImageFormat::Png)?;
        temp.persist(&still).map_err(|e| e.error)?;
    }
    std::fs::write(&length, info.duration.to_string())?;
    Ok(LibraryStill {
        still,
        duration: info.duration,
    })
}

/// The timeline cache and the name a take's artwork goes under there, from
/// its path, size and last change.
fn cache_entry(source: &Path) -> Result<(PathBuf, String)> {
    use std::hash::{Hash, Hasher};
    let metadata = std::fs::metadata(source)?;
    let mut key = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut key);
    metadata.len().hash(&mut key);
    metadata.modified()?.hash(&mut key);
    let directory = crate::preferences::Preferences::directory()?.join("timeline-cache");
    std::fs::create_dir_all(&directory)?;
    Ok((directory, format!("{:016x}", key.finish())))
}

/// The longest edge of a preview proxy. A take larger than this plays in the
/// preview from a copy this size, so playback decodes a fraction of the
/// pixels. A paused frame and the export still read the take itself.
pub const PROXY_EDGE: u32 = 1920;

/// How many proxies the cache keeps, newest first. Each is a whole take, so
/// older ones go as new ones are made.
const PROXIES_KEPT: usize = 3;

/// A size scaled to fit `edge` on its longest side, never up, in even pixels.
pub fn fit(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let ratio = (edge as f64 / width.max(height) as f64).min(1.);
    let even = |side: u32| ((side as f64 * ratio / 2.).round() as u32 * 2).max(2);
    (even(width), even(height))
}

/// The preview proxy for a take larger than `PROXY_EDGE`: an H.264 copy at
/// that size with a keyframe every quarter second, so a seek decodes little,
/// and the take's own timestamps. Made on first ask and kept in the timeline
/// cache; `None` for a take small enough to play itself.
pub fn proxy(source: &Path, info: &MediaInfo, cancel: &AtomicBool) -> Result<Option<PathBuf>> {
    if info.width.max(info.height) <= PROXY_EDGE {
        return Ok(None);
    }
    let (directory, name) = cache_entry(source)?;
    let proxy = directory.join(format!("{name}-proxy.mp4"));
    if proxy.is_file() {
        return Ok(Some(proxy));
    }
    let (width, height) = fit(info.width, info.height, PROXY_EDGE);
    let keyframes = (info.fps / 4.).round().max(1.).to_string();
    let temp = tempfile::Builder::new()
        .suffix(".mp4")
        .tempfile_in(&directory)?;
    let encoders: &[&[&str]] = if cfg!(target_os = "macos") {
        &[
            &["-c:v", "h264_videotoolbox", "-b:v", "6M"],
            &["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"],
        ]
    } else {
        &[&["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"]]
    };
    let mut failure = None;
    for encoder in encoders {
        let mut process = ManagedChild::spawn(
            Command::new(binary("ffmpeg")?)
                .args(["-v", "error", "-nostdin", "-y", "-i"])
                .arg(source)
                .args(["-map", "0:v:0", "-an", "-sn", "-vf"])
                .arg(format!("scale={width}:{height}:flags=bicubic"))
                .args(["-fps_mode", "passthrough", "-pix_fmt", "yuv420p", "-g"])
                .arg(&keyframes)
                .args(*encoder)
                .args(["-f", "mp4"])
                .arg(temp.path())
                .stdout(Stdio::null()),
        )?;
        match process.finish_cancellable(Duration::from_secs(6 * 3600), cancel) {
            Ok(()) => {
                failure = None;
                break;
            }
            Err(error) if cancel.load(std::sync::atomic::Ordering::Relaxed) => return Err(error),
            Err(error) => failure = Some(error),
        }
    }
    if let Some(error) = failure {
        return Err(error.context("Make the preview proxy"));
    }
    temp.persist(&proxy).map_err(|e| e.error)?;
    prune_proxies(&directory);
    Ok(Some(proxy))
}

/// Removes all but the newest `PROXIES_KEPT` proxies.
fn prune_proxies(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut proxies: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with("-proxy.mp4"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    proxies.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, path) in proxies.into_iter().skip(PROXIES_KEPT) {
        let _ = std::fs::remove_file(path);
    }
}

/// Discover bundled wallpapers at runtime so asset additions require no Rust changes.
pub fn wallpapers() -> Vec<PathBuf> {
    fn walk(dir: &Path, output: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, output);
                } else if path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                    matches!(
                        s.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "mp4" | "mov" | "webm"
                    )
                }) {
                    output.push(path);
                }
            }
        }
    }
    let mut paths = vec![];
    walk(&resources().join("public/wallpapers"), &mut paths);
    paths.sort();
    paths
}
