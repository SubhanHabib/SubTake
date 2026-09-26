//! Exercise timestamp caching and cancellation with real, generated FFmpeg media.
use std::{path::Path, process::Command, sync::atomic::AtomicBool};
use subtake_native::{
    export::{self, ExportSettings},
    media,
    project::Project,
};

fn fixture(path: &Path) {
    let result = Command::new(media::binary("ffmpeg").unwrap())
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=3:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(result.success());
}

#[test]
fn decoder_reuses_source_frames_for_higher_output_rates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.mp4");
    fixture(&path);
    let mut decoder = media::Decoder::new(path, 64, 48).with_rate(3.);
    let first = decoder.frame(0.).unwrap();
    assert_eq!(first, decoder.frame(0.1).unwrap());
    assert_eq!(first, decoder.frame(0.32).unwrap());
    let second = decoder.frame(0.34).unwrap();
    assert_ne!(first, second);
    assert_eq!(second, decoder.frame(0.6).unwrap());
    assert_eq!(first, decoder.frame(0.).unwrap());
}

#[test]
fn cancelled_export_preserves_existing_destination() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let output = dir.path().join("output.mp4");
    std::fs::write(&output, b"previous export").unwrap();
    let p = Project::new(&source);
    let settings = ExportSettings {
        width: 64,
        height: 48,
        ..Default::default()
    };
    let error = export::export(
        &p,
        &source,
        &settings,
        &output,
        &AtomicBool::new(true),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(std::fs::read(output).unwrap(), b"previous export");
}

#[cfg(feature = "native-ffmpeg")]
#[test]
fn native_decoder_preserves_rotation_and_last_frame_at_eof() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    let rotated = dir.path().join("rotated.mp4");
    fixture(&source);
    let ffmpeg = media::binary("ffmpeg").unwrap();
    assert!(
        Command::new(&ffmpeg)
            .args(["-v", "error", "-y", "-display_rotation", "90", "-i"])
            .arg(&source)
            .args(["-c", "copy"])
            .arg(&rotated)
            .status()
            .unwrap()
            .success()
    );
    let info = media::probe(&rotated).unwrap();
    assert_eq!((info.width, info.height), (48, 64));
    let mut decoder =
        subtake_native::native_decoder::NativeDecoder::open(&rotated, 48, 64).unwrap();
    let actual = decoder.frame(0.).unwrap();
    let output = Command::new(&ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(&rotated)
        .args([
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), actual.len());
    let error = actual
        .iter()
        .zip(output.stdout.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / actual.len() as f64;
    assert!(error < 3., "Rotation/color error: {error}");
    let last = decoder.frame(0.8).unwrap();
    assert_eq!(last, decoder.frame(4.).unwrap());
}

#[test]
fn preview_edit_outline_matches_rendered_annotation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let mut p = subtake_native::project::Project::new(&source);
    p.set("annotationRegions",serde_json::json!([{"id":"box","type":"text","startMs":0,"endMs":1000,"textContent":"","position":{"x":20,"y":25},"size":{"width":30,"height":25},"style":{"backgroundColor":"#ff0000","color":"#ff0000","fontSize":12}}]));
    let info = media::probe(&source).unwrap();
    let mut scene = subtake_native::render::Scene::new(source, info, 128, 96).unwrap();
    let rgba = scene.render(&p, 0.1).unwrap();
    let bounds = scene
        .edit_bounds(
            &p,
            0.1,
            Some(&("annotationRegions".into(), "box".into())),
            "Selection",
        )
        .unwrap();
    assert!((bounds[0] - 0.2).abs() < 0.0001);
    assert!((bounds[1] - 0.25).abs() < 0.0001);
    let x = ((bounds[0] + bounds[2] / 2.) * 128.) as usize;
    let y = ((bounds[1] + bounds[3] / 2.) * 96.) as usize;
    let color = &rgba[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4];
    assert!(
        color[0] > 240 && color[1] < 10 && color[2] < 10,
        "Edit outline does not enclose the rendered red box: {color:?}"
    );
}

#[test]
fn a_scene_in_bgra_hands_back_the_same_frame_reordered() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let mut p = Project::new(&source);
    p.set("annotationRegions",serde_json::json!([{"id":"box","type":"text","startMs":0,"endMs":1000,"textContent":"","position":{"x":20,"y":25},"size":{"width":30,"height":25},"style":{"backgroundColor":"#ff0000","color":"#ff0000","fontSize":12}}]));
    let info = media::probe(&source).unwrap();
    let render = |scene: subtake_native::render::Scene| {
        let mut scene = scene;
        scene.render(&p, 0.1).unwrap()
    };
    let new = || subtake_native::render::Scene::new(source.clone(), info.clone(), 128, 96).unwrap();
    let rgba = render(new());
    let mut bgra = render(new().in_bgra());
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    assert_eq!(bgra, rgba);
}

#[test]
fn spotlight_step_and_pixelate_annotations_render() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let info = media::probe(&source).unwrap();
    let mut scene = subtake_native::render::Scene::new(source.clone(), info, 128, 96).unwrap();
    let mut p = Project::new(&source);
    let plain = scene.render(&p, 0.1).unwrap();
    p.set("annotationRegions", serde_json::json!([
        {"id":"spot","type":"spotlight","startMs":0,"endMs":1000,"position":{"x":0,"y":0},"size":{"width":50,"height":50},"dimOpacity":60},
        {"id":"step","type":"step","startMs":0,"endMs":1000,"textContent":"","position":{"x":60,"y":60},"size":{"width":30,"height":30},"figureData":{"color":"#ff0000"}},
        {"id":"px","type":"blur","blurMode":"pixelate","startMs":0,"endMs":1000,"position":{"x":0,"y":50},"size":{"width":50,"height":50},"blurIntensity":2000}
    ]));
    let annotated = scene.render(&p, 0.1).unwrap();
    let at =
        |rgba: &[u8], x: usize, y: usize| rgba[(y * 128 + x) * 4..(y * 128 + x) * 4 + 3].to_vec();
    let luma = |c: Vec<u8>| c.iter().map(|&v| v as u32).sum::<u32>();
    // Inside the spotlight the picture is untouched; outside it dims.
    assert_eq!(at(&annotated, 20, 20), at(&plain, 20, 20));
    assert!(luma(at(&annotated, 100, 20)) * 10 < luma(at(&plain, 100, 20)) * 6);
    // The badge's disc fills its centre.
    let centre = at(&annotated, 96, 72);
    assert!(
        centre[0] > 90 && centre[1] < 10 && centre[2] < 10,
        "{centre:?}"
    );
    // Blocks of 2000 at 1080p are wider than the region at this size, so
    // the whole region takes one colour.
    assert_eq!(at(&annotated, 10, 60), at(&annotated, 50, 90));
    // A click finds them in drawing order, the spotlight at the bottom.
    let shown = scene.annotation_bounds(&p, 0.1);
    let ids: Vec<_> = shown.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["spot", "step", "px"]);
    assert_eq!(shown[1].1, [0.6, 0.6, 0.3, 0.3]);
    assert!(scene.annotation_bounds(&p, 1.5).is_empty());
}

#[test]
fn hidden_lanes_leave_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let info = media::probe(&source).unwrap();
    let mut scene = subtake_native::render::Scene::new(source.clone(), info, 128, 96).unwrap();
    let mut p = Project::new(&source);
    p.set("wallpaper", serde_json::json!("#204060"));
    p.set("shadowIntensity", serde_json::json!(0));
    p.set("annotationRegions", serde_json::json!([
        {"id":"step","type":"step","startMs":0,"endMs":1000,"textContent":"","position":{"x":60,"y":60},"size":{"width":30,"height":30},"figureData":{"color":"#ff0000"}}
    ]));
    let at =
        |rgba: &[u8], x: usize, y: usize| rgba[(y * 128 + x) * 4..(y * 128 + x) * 4 + 3].to_vec();
    let shown = scene.render(&p, 0.1).unwrap();
    p.toggle_lane("Annotation");
    let no_annotations = scene.render(&p, 0.1).unwrap();
    assert_ne!(at(&shown, 96, 72), at(&no_annotations, 96, 72));
    // With the clip lane hidden too, only the wallpaper is left.
    p.toggle_lane("Clip");
    let wallpaper = scene.render(&p, 0.1).unwrap();
    assert_eq!(at(&wallpaper, 64, 48), [0x20, 0x40, 0x60]);
    assert_ne!(at(&no_annotations, 64, 48), [0x20, 0x40, 0x60]);
}

#[test]
fn muted_audio_lane_mixes_silence() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    fixture(&source);
    let voice = dir.path().join("voice.wav");
    let made = Command::new(media::binary("ffmpeg").unwrap())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=duration=1"])
        .arg(&voice)
        .status()
        .unwrap();
    assert!(made.success());
    let info = media::probe(&source).unwrap();
    let mut p = Project::new(&source);
    p.set(
        "audioRegions",
        serde_json::json!([{"id":"v","startMs":0,"endMs":1000,"audioPath":voice}]),
    );
    let spans = subtake_native::timeline::spans(&p, info.duration);
    let graph = |p: &Project| {
        let command = export::audio_command(p, &source, &info, &spans).unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let at = args.iter().position(|a| a == "-filter_complex").unwrap();
        args[at + 1].clone()
    };
    assert!(graph(&p).contains("amix=inputs=2"));
    p.toggle_lane("Audio");
    assert!(graph(&p).contains("amix=inputs=1"));
}

#[test]
fn preview_proxy_matches_the_take_frame_for_frame() {
    // The proxy goes in a throwaway cache, not the user's.
    unsafe { std::env::set_var("SUBTAKE_UI_SNAPSHOT", "1") };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("large.mp4");
    let made = Command::new(media::binary("ffmpeg").unwrap())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=size=2560x1440:rate=30:duration=2")
        .args(["-c:v", "libx264", "-g", "60", "-pix_fmt", "yuv420p"])
        .arg(&source)
        .status()
        .unwrap();
    assert!(made.success());
    let info = media::probe(&source).unwrap();
    let cancel = AtomicBool::new(false);
    let proxy = media::proxy(&source, &info, &cancel).unwrap().unwrap();
    let proxied = media::probe(&proxy).unwrap();
    assert_eq!((proxied.width, proxied.height), (1920, 1080));
    assert!((proxied.duration - info.duration).abs() < 0.05);
    // Cached: asked again, it is the same file.
    assert_eq!(
        media::proxy(&source, &info, &cancel).unwrap().unwrap(),
        proxy
    );
    let mut take = media::Decoder::new(source.clone(), 320, 180).with_rate(30.);
    let mut copy = media::Decoder::new(proxy, 320, 180).with_rate(30.);
    for time in [0.1, 0.97, 1.53] {
        let (a, b) = (take.frame(time).unwrap(), copy.frame(time).unwrap());
        // A frame out would move the pattern by thousands of these.
        let off = a
            .iter()
            .zip(&b)
            .filter(|(x, y)| x.abs_diff(**y) > 48)
            .count();
        assert!(off < 100, "{time}s: {off} bytes differ");
    }
    // A take no larger than the proxy plays itself.
    let small = dir.path().join("small.mp4");
    fixture(&small);
    let small_info = media::probe(&small).unwrap();
    assert!(
        media::proxy(&small, &small_info, &cancel)
            .unwrap()
            .is_none()
    );
}

#[test]
#[ignore = "requires a real audio output device; sends silence only"]
fn silent_output_device_clock_and_cancel() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("silent-video.mp4");
    fixture(&source);
    let p = Project::new(&source);
    let info = media::probe(&source).unwrap();
    assert_eq!(info.audio_tracks, 0);
    let clock = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    export::play_audio(
        p.clone(),
        source.clone(),
        info.clone(),
        0.,
        Arc::new(AtomicBool::new(false)),
        clock.clone(),
    )
    .unwrap();
    assert_eq!(clock.load(Ordering::Relaxed), 1_000_000);
    assert!(start.elapsed() >= Duration::from_millis(800));
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let clock = Arc::new(AtomicU64::new(0));
    let timer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        trigger.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    export::play_audio(p, source, info, 0., cancel, clock.clone()).unwrap();
    timer.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(
        clock.load(Ordering::Relaxed) < 1_000_000,
        "Cancellation jumped to the end of the video"
    );
}

#[test]
fn source_discovery_process_can_be_cancelled_promptly() {
    use std::{
        sync::{Arc, atomic::Ordering},
        time::{Duration, Instant},
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let timer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        signal.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    let result = media::capture_output_cancellable(
        Command::new(media::binary("ffmpeg").unwrap()).args([
            "-v",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "color=size=16x16:rate=1",
            "-f",
            "null",
            "-",
        ]),
        Duration::from_secs(30),
        1024,
        &cancel,
    );
    timer.join().unwrap();
    assert!(format!("{:#}", result.unwrap_err()).contains("cancelled"));
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "Cancellation waited for process timeout"
    );
}
