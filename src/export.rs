use crate::{
    media::{self, ManagedChild, MediaInfo},
    project::Project,
    render::Scene,
    timeline::{self, Span, n},
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub gif: bool,
    pub gif_loop: bool,
    pub quality: String,
    pub hardware: bool,
}
impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            gif: false,
            gif_loop: true,
            quality: "high".into(),
            hardware: false,
        }
    }
}
impl ExportSettings {
    pub fn from_project(project: &Project) -> Self {
        let gif = project.text("exportFormat", "mp4") == "gif";
        Self {
            width: project.number("nativeExportWidth", 1920.) as u32,
            height: project.number("nativeExportHeight", 1080.) as u32,
            fps: project.number(
                if gif { "gifFrameRate" } else { "mp4FrameRate" },
                if gif { 15. } else { 30. },
            ) as u32,
            gif,
            gif_loop: project.flag("gifLoop", true),
            quality: project.text("nativeExportQuality", "high").into(),
            hardware: project.flag("nativeExportHardware", false),
        }
    }
    pub fn for_media(project: &Project, info: &MediaInfo) -> Self {
        let mut settings = Self::from_project(project);
        if !project.editor.contains_key("nativeExportWidth")
            || !project.editor.contains_key("nativeExportHeight")
        {
            let (w, h) =
                legacy_dimensions(project, info.width as f64, info.height as f64, settings.gif);
            settings.width = w;
            settings.height = h;
        }
        settings
    }
    pub fn store(&self, project: &mut Project) {
        use serde_json::json;
        project.set("nativeExportWidth", json!(self.width));
        project.set("nativeExportHeight", json!(self.height));
        project.set(
            if self.gif {
                "gifFrameRate"
            } else {
                "mp4FrameRate"
            },
            json!(self.fps),
        );
        project.set("exportFormat", json!(if self.gif { "gif" } else { "mp4" }));
        project.set("gifLoop", json!(self.gif_loop));
        project.set("nativeExportQuality", json!(self.quality));
        project.set("nativeExportHardware", json!(self.hardware));
    }
}
pub fn legacy_dimensions(
    p: &Project,
    source_width: f64,
    source_height: f64,
    gif: bool,
) -> (u32, u32) {
    fn even(value: f64) -> u32 {
        ((value / 2.).floor().max(1.) * 2.) as u32
    }
    if gif {
        let limit = match p.text("gifSizePreset", "medium") {
            "large" => 1080.,
            "original" => f64::INFINITY,
            _ => 720.,
        };
        if source_height <= limit {
            return (source_width as u32, source_height as u32);
        }
        let w = (limit * source_width / source_height).round() as u32;
        return (w + (w % 2), limit as u32);
    }
    let native = p.text("aspectRatio", "native") == "native";
    let crop = p
        .editor
        .get("cropRegion")
        .unwrap_or(&serde_json::Value::Null);
    let sw = even(source_width * if native { n(crop, "width", 1.) } else { 1. }) as f64;
    let sh = even(source_height * if native { n(crop, "height", 1.) } else { 1. }) as f64;
    let (width, height) = if native {
        (sw, sh)
    } else {
        let aspect = p
            .text("aspectRatio", "16:9")
            .split_once(':')
            .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
            .filter(|v| v.is_finite() && *v > 0.)
            .unwrap_or(16. / 9.);
        let (maxw, maxh) = if aspect >= 1. {
            (sw.max(sh), sw.min(sh))
        } else {
            (sw.min(sh), sw.max(sh))
        };
        if maxw / maxh > aspect {
            ((even(maxh * aspect) as f64).min(maxw), maxh)
        } else {
            (maxw, (even(maxw / aspect) as f64).min(maxh))
        }
    };
    let scale = match p.text("exportQuality", "high") {
        "source" => 1.,
        "medium" => 0.6,
        "good" => 0.75,
        _ => 0.9,
    };
    (even(width * scale), even(height * scale))
}
fn tempo(mut speed: f64) -> String {
    let mut filters = vec![];
    while speed > 2. {
        filters.push("atempo=2".into());
        speed /= 2.;
    }
    while speed < 0.5 {
        filters.push("atempo=0.5".into());
        speed /= 0.5;
    }
    filters.push(format!("atempo={speed:.8}"));
    filters.join(",")
}
pub fn audio_command(
    p: &Project,
    source: &Path,
    info: &MediaInfo,
    spans: &[Span],
) -> Result<Command> {
    let duration = timeline::duration(spans);
    ensure!(duration > 0., "The entire video has been trimmed");
    let mut command = Command::new(media::binary("ffmpeg")?);
    command.args(["-v", "error", "-nostdin", "-i"]).arg(source);
    let mut inputs = 1;
    let mut filters = vec![format!(
        "anullsrc=r=48000:cl=stereo,atrim=duration={duration:.8}[silence]"
    )];
    let mut labels = vec!["[silence]".to_owned()];
    let mut tracks = vec![];
    for key in ["system", "mic"] {
        let path = ["m4a", "wav", "webm"]
            .iter()
            .map(|ext| source.with_extension(format!("{key}.{ext}")))
            .find(|p| p.is_file());
        if let Some(path) = path {
            command.arg("-i").arg(path);
            tracks.push((inputs, 0, key.to_owned()));
            inputs += 1;
        }
    }
    if !tracks.iter().any(|(_, _, key)| key == "system") {
        for track in 0..info.audio_tracks {
            tracks.push((
                0,
                track,
                if track == 0 {
                    "mixed".into()
                } else {
                    format!("track-{track}")
                },
            ));
        }
    }
    for (track, (input, stream, key)) in tracks.iter().enumerate() {
        for (i, s) in spans.iter().enumerate() {
            let clip = p.regions("clipRegions").iter().find(|clip| {
                let start = n(clip, "startMs", 0.) / 1000.;
                let end = start + (n(clip, "endMs", 0.) / 1000. - start) * n(clip, "speed", 1.);
                s.source_start >= start && s.source_start < end
            });
            let per_clip = clip.and_then(|c| c["id"].as_str()).and_then(|id| {
                p.editor
                    .get("sourceAudioTrackSettingsByClip")
                    .and_then(|map| map.get(id))
            });
            let track_settings = per_clip.and_then(|s| s.get(key)).or_else(|| {
                p.editor
                    .get("defaultSourceAudioTrackSettings")
                    .and_then(|s| s.get(key))
            });
            let gain = track_settings
                .map(|s| n(s, "volume", 1.))
                .unwrap_or(1.)
                .clamp(0., 2.);
            let gain = if track_settings
                .and_then(|s| s["normalize"].as_bool())
                .unwrap_or(false)
            {
                (gain * 1.35).min(2.)
            } else {
                gain
            };
            let label = format!("source{track}_{i}");
            filters.push(format!("[{input}:a:{stream}]atrim=start={:.8}:end={:.8},asetpts=PTS-STARTPTS,{},volume={:.8},adelay={}:all=1[{label}]",s.source_start,s.source_end,tempo(s.speed),if s.muted{0.}else{gain},(s.output_start*1000.).round()as u64));
            labels.push(format!("[{label}]"));
        }
    }
    for (j, a) in p.regions("audioRegions").iter().enumerate() {
        let path = crate::project::local_path(
            a["audioPath"]
                .as_str()
                .context("Audio region has no path")?,
        );
        let path = if path.is_absolute() {
            path
        } else {
            source.parent().unwrap_or(Path::new(".")).join(path)
        };
        ensure!(path.is_file(), "Audio file is missing: {}", path.display());
        command.arg("-i").arg(&path);
        let input = inputs;
        inputs += 1;
        let start = n(a, "startMs", 0.) / 1000.;
        let end = n(a, "endMs", 0.) / 1000.;
        for (i, s) in spans.iter().enumerate() {
            let from = start.max(s.source_start);
            let to = end.min(s.source_end);
            if to <= from {
                continue;
            }
            let out = s.output_start + (from - s.source_start) / s.speed;
            let label = format!("extra{j}_{i}");
            let gain = (n(a, "volume", 1.)
                * if a["normalize"].as_bool().unwrap_or(false) {
                    1.35
                } else {
                    1.
                })
            .clamp(0., 1.);
            filters.push(format!("[{input}:a:0]atrim=start={:.8}:end={:.8},asetpts=PTS-STARTPTS,{},volume={gain:.8},adelay={}:all=1[{label}]",from-start,to-start,tempo(s.speed),(out*1000.).round()as u64));
            labels.push(format!("[{label}]"));
        }
    }
    filters.push(format!("{}amix=inputs={}:duration=first:normalize=0,alimiter=limit=0.98:latency=1,atrim=duration={duration:.8}[mix]",labels.join(""),labels.len()));
    command.args([
        "-filter_complex",
        &filters.join(";"),
        "-map",
        "[mix]",
        "-ar",
        "48000",
        "-ac",
        "2",
    ]);
    Ok(command)
}
pub fn render_audio(
    p: &Project,
    source: &Path,
    info: &MediaInfo,
    spans: &[Span],
    path: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut command = audio_command(p, source, info, spans)?;
    command
        .args(["-y", "-c:a", "pcm_s16le", "-f", "wav"])
        .arg(path)
        .stdout(Stdio::null());
    let mut child = ManagedChild::spawn(&mut command)?;
    child.finish_cancellable(Duration::from_secs(3600), cancel)
}
pub fn export(
    p: &Project,
    source: &Path,
    settings: &ExportSettings,
    destination: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(f32),
) -> Result<()> {
    ensure!(
        !settings.hardware || cfg!(target_os = "macos"),
        "Hardware encoding is not implemented on this platform; choose software encoding"
    );
    ensure!(
        settings.width > 0
            && settings.height > 0
            && settings.width <= 8192
            && settings.height <= 8192,
        "Invalid output dimensions"
    );
    ensure!(
        settings.width.is_multiple_of(2) && settings.height.is_multiple_of(2),
        "MP4 dimensions must be even"
    );
    ensure!(
        (1..=120).contains(&settings.fps),
        "Frame rate must be 1–120"
    );
    ensure!(
        source != destination && destination.canonicalize().ok() != Some(source.canonicalize()?),
        "Export cannot overwrite the source video"
    );
    let info = media::probe(source)?;
    let spans = timeline::spans(p, info.duration);
    let duration = timeline::duration(&spans);
    ensure!(duration > 0., "The entire video has been trimmed");
    let folder = destination.parent().unwrap_or(Path::new("."));
    let work = tempfile::tempdir_in(folder)?;
    let video = work.path().join("video.mp4");
    let audio = work.path().join("audio.wav");
    let final_file = work.path().join(if settings.gif {
        "output.gif"
    } else {
        "output.mp4"
    });
    let mut command = Command::new(media::binary("ffmpeg")?);
    command.args([
        "-v",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &format!("{}x{}", settings.width, settings.height),
        "-r",
        &settings.fps.to_string(),
        "-i",
        "pipe:0",
        "-an",
    ]);
    let crf = match settings.quality.as_str() {
        "low" => "28",
        "medium" => "23",
        "lossless" => "0",
        _ => "18",
    };
    if settings.hardware && cfg!(target_os = "macos") {
        command.args([
            "-c:v",
            "h264_videotoolbox",
            "-b:v",
            match settings.quality.as_str() {
                "low" => "4M",
                "medium" => "10M",
                _ => "24M",
            },
        ]);
    } else {
        command.args(["-c:v", "libx264", "-preset", "fast", "-crf", crf]);
    }
    command
        .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(&video)
        .stdin(Stdio::piped())
        .stdout(Stdio::null());
    let mut encoder = ManagedChild::spawn(&mut command)?;
    let mut stdin = encoder.child.stdin.take().context("Open encoder input")?;
    let mut scene = Scene::new(source.into(), info.clone(), settings.width, settings.height)?
        .with_frame_rate(settings.fps as f64);
    let frames = (duration * settings.fps as f64).ceil() as u64;
    for frame in 0..frames {
        if cancel.load(Ordering::Relaxed) {
            bail!("Export cancelled")
        }
        let time = timeline::source_time(&spans, frame as f64 / settings.fps as f64);
        let pixels = scene.render(p, time)?;
        stdin
            .write_all(&pixels)
            .with_context(|| format!("Encode frame: {}", encoder.errors.lock().unwrap()))?;
        progress(frame as f32 / frames as f32 * 0.85);
    }
    drop(stdin);
    encoder.finish_cancellable(Duration::from_secs(120), cancel)?;
    if cancel.load(Ordering::Relaxed) {
        bail!("Export cancelled")
    }
    if settings.gif {
        let mut gif=ManagedChild::spawn(Command::new(media::binary("ffmpeg")?).args(["-v","error","-y","-i"]).arg(&video).args(["-filter_complex","[0:v]split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a","-loop",if settings.gif_loop{"0"}else{"-1"}]).arg(&final_file))?;
        gif.finish_cancellable(Duration::from_secs(300), cancel)?;
    } else {
        render_audio(p, source, &info, &spans, &audio, cancel)?;
        progress(0.93);
        let mut mux = ManagedChild::spawn(
            Command::new(media::binary("ffmpeg")?)
                .args(["-v", "error", "-y", "-i"])
                .arg(&video)
                .arg("-i")
                .arg(&audio)
                .args([
                    "-map",
                    "0:v:0",
                    "-map",
                    "1:a:0",
                    "-c:v",
                    "copy",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-t",
                    &format!("{duration:.8}"),
                    "-movflags",
                    "+faststart",
                ])
                .arg(&final_file),
        )?;
        mux.finish_cancellable(Duration::from_secs(120), cancel)?;
    }
    if cancel.load(Ordering::Relaxed) {
        bail!("Export cancelled")
    }
    let mut temp = tempfile::NamedTempFile::new_in(folder)?;
    std::io::copy(&mut std::fs::File::open(&final_file)?, &mut temp)?;
    temp.as_file().sync_all()?;
    temp.persist(destination).map_err(|e| e.error)?;
    if p.flag("nativeCaptionSidecars", false) {
        crate::subtitles::write(p, &spans, destination)
            .context("Video saved, but subtitle sidecars could not be saved")?;
    }
    progress(1.);
    Ok(())
}

/// Audio is pulled lazily by the device, with bounded memory and owned child lifetime.
struct PcmSource {
    stdout: std::io::BufReader<std::process::ChildStdout>,
    cancel: Arc<AtomicBool>,
}
impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut b = [0; 4];
        self.stdout.read_exact(&mut b).ok()?;
        Some(f32::from_le_bytes(b))
    }
}
impl rodio::Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        48000
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
pub fn play_audio(
    p: Project,
    source: PathBuf,
    info: MediaInfo,
    start: f64,
    cancel: Arc<AtomicBool>,
    clock: Arc<std::sync::atomic::AtomicU64>,
) -> Result<()> {
    let spans = timeline::spans(&p, info.duration);
    let mut command = audio_command(&p, &source, &info, &spans)?;
    command
        .args(["-ss", &format!("{start:.8}"), "-f", "f32le", "pipe:1"])
        .stdout(Stdio::piped());
    let mut child = ManagedChild::spawn(&mut command)?;
    let stdout = child.child.stdout.take().context("Open audio stream")?;
    let mut stream = rodio::OutputStreamBuilder::open_default_stream()?;
    stream.log_on_drop(false);
    let sink = rodio::Sink::connect_new(stream.mixer());
    let mut stdout = std::io::BufReader::with_capacity(65536, stdout);
    std::io::BufRead::fill_buf(&mut stdout).context("Buffer preview audio")?;
    sink.append(PcmSource {
        stdout,
        cancel: cancel.clone(),
    });
    while !sink.empty() && !cancel.load(Ordering::Relaxed) {
        clock.store(sink.get_pos().as_micros() as u64, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(15));
    }
    if cancel.load(Ordering::Relaxed) {
        // Close the producer before dropping the device: an audio callback may be waiting on its pipe.
        let _ = child.child.kill();
        sink.stop();
        return Ok(());
    }
    // A decoder failure must not look like successful playback reaching the end of the video.
    child.finish(Duration::from_secs(5))?;
    clock.store(
        ((timeline::duration(&spans) - start).max(0.) * 1_000_000.) as u64,
        Ordering::Relaxed,
    );
    sink.stop();
    Ok(())
}
