use serde_json::json;
use std::{
    path::Path,
    process::Command,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use subtake_native::{
    export::ExportSettings,
    geometry,
    media::{self, ManagedChild},
    motion::CameraTrack,
    project::Project,
};

#[test]
fn export_options_roundtrip_through_project_history() {
    let mut p = Project::new(Path::new("video.mp4"));
    p.set("exportQuality", json!("source"));
    p.set("exportEncodingMode", json!("fast"));
    let settings = ExportSettings {
        width: 1280,
        height: 720,
        fps: 60,
        gif: false,
        gif_loop: false,
        quality: "medium".into(),
        hardware: true,
    };
    settings.store(&mut p);
    assert_eq!(p.text("exportQuality", ""), "source");
    assert_eq!(p.text("exportEncodingMode", ""), "fast");
    let restored = ExportSettings::from_project(&p);
    assert_eq!(
        (restored.width, restored.height, restored.fps),
        (1280, 720, 60)
    );
    assert!(restored.hardware);
    assert_eq!(restored.quality, "medium");
    assert!(!restored.gif_loop);
}

#[test]
fn file_urls_decode_spaces_and_unicode() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("hello world 🐝.mp4");
    let mut p = Project::new(&file);
    p.video_path = url::Url::from_file_path(&file).unwrap().to_string();
    assert_eq!(p.source_path(None), file);
}

#[test]
fn camera_seek_order_does_not_change_composition() {
    let mut p = Project::new(Path::new("video.mp4"));
    p.set("zoomRegions",json!([{"id":"z","startMs":0,"endMs":5000,"depth":4,"focus":{"cx":0.3,"cy":0.4},"mode":"auto"}]));
    let points = vec![
        json!({"timeMs":0,"cx":0.2,"cy":0.2}),
        json!({"timeMs":2000,"cx":0.9,"cy":0.8}),
        json!({"timeMs":6000,"cx":0.5,"cy":0.5}),
    ];
    let frame = geometry::frame(&p, 1920., 1080., 1920., 1080.);
    let mut sequential = CameraTrack::default();
    for i in 0..=180 {
        sequential.at(&p, &points, i as f64 * 1000. / 30., 1920., 1080., &frame);
    }
    for time in [4200., 100., 2600., 0., 1600.] {
        let actual = sequential.at(&p, &points, time, 1920., 1080., &frame);
        let fresh = CameraTrack::default().at(&p, &points, time, 1920., 1080., &frame);
        assert_eq!(
            (actual.scale, actual.x, actual.y),
            (fresh.scale, fresh.x, fresh.y)
        );
    }
}

#[cfg(unix)]
#[test]
fn process_cancellation_and_output_limits_are_enforced() {
    let start = Instant::now();
    let mut process = ManagedChild::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
    let cancel = AtomicBool::new(true);
    assert!(
        process
            .finish_cancellable(Duration::from_secs(10), &cancel)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    drop(process);
    assert!(start.elapsed() < Duration::from_secs(2));
    let error = media::capture_output(
        Command::new("/usr/bin/printf").arg("123456789"),
        Duration::from_secs(2),
        4,
    )
    .unwrap_err();
    assert!(error.to_string().contains("exceeded"));
    let error = media::capture_output(
        Command::new("/bin/sleep").arg("30"),
        Duration::from_millis(60),
        100,
    )
    .unwrap_err();
    assert!(error.to_string().contains("timed out"));
}

#[test]
fn subtitle_output_clock_follows_cuts_and_speed() {
    use subtake_native::{subtitles, timeline};
    let mut p = Project::new(Path::new("fixture.mp4"));
    p.set(
        "autoCaptions",
        json!([{"id":"c","startMs":500,"endMs":3500,"text":"A caption"}]),
    );
    p.set(
        "trimRegions",
        json!([{"id":"t","startMs":1000,"endMs":1500}]),
    );
    p.set(
        "speedRegions",
        json!([{"id":"s","startMs":2000,"endMs":3000,"speed":2}]),
    );
    let spans = timeline::spans(&p, 4.);
    let cues = subtitles::mapped_cues(&p, &spans);
    let ranges: Vec<_> = cues
        .iter()
        .map(|v| (v["startMs"].as_f64().unwrap(), v["endMs"].as_f64().unwrap()))
        .collect();
    assert_eq!(
        ranges,
        vec![
            (500., 1000.),
            (1000., 1500.),
            (1500., 2000.),
            (2000., 2500.)
        ]
    );
    assert!(subtitles::format(&cues, false).contains("00:00:02,000 --> 00:00:02,500"));
    assert!(subtitles::format(&cues, true).starts_with("WEBVTT\n\n"));
}

#[test]
fn recovery_preserves_saved_document_and_unknown_fields() {
    use subtake_native::{project::History, recovery};
    let dir = tempfile::tempdir().unwrap();
    let saved = dir.path().join("project.recordly");
    let snapshot = dir.path().join("recovery.recordly");
    let mut p = Project::new(Path::new("fixture.mp4"));
    p.set("future", json!({"nested":[1,2,3]}));
    p.save(&saved).unwrap();
    let original = std::fs::read(&saved).unwrap();
    p.set("padding", json!(50));
    recovery::save(&snapshot, &p, Some(&saved)).unwrap();
    assert_eq!(std::fs::read(&saved).unwrap(), original);
    let restored = Project::load(&snapshot).unwrap();
    assert_eq!(restored.editor["future"], p.editor["future"]);
    assert_eq!(restored.extra["nativeRecoveryDocument"], json!(saved));
    let mut history = History::new(restored);
    history.mark_unsaved();
    assert!(history.dirty());
    history.mark_saved();
    assert!(!history.dirty());
    recovery::remove(&snapshot).unwrap();
    assert!(!snapshot.exists());
    assert!(saved.exists());
}

#[test]
fn transcription_prefers_microphone_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("a.mp4");
    for ext in ["mp4", "mic.m4a", "system.m4a"] {
        std::fs::write(source.with_extension(ext), b"fixture").unwrap();
    }
    assert_eq!(
        subtake_native::transcription::audio_candidates(&source),
        vec![
            source.with_extension("mic.m4a"),
            source.with_extension("system.m4a"),
            source
        ]
    );
}

#[test]
fn replacing_a_take_removes_old_companion_tracks_and_keeps_rollback_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let video = dir.path().join("take.mp4");
    let mic = dir.path().join("take.mic.m4a");
    let new = work.join("new.mp4");
    std::fs::write(&video, b"old video").unwrap();
    std::fs::write(&mic, b"old mic").unwrap();
    std::fs::write(&new, b"new video").unwrap();
    subtake_native::file_group::replace(&work, &[(mic.clone(), None), (video.clone(), Some(new))])
        .unwrap();
    assert!(!mic.exists());
    assert_eq!(std::fs::read(video).unwrap(), b"new video");
    assert_eq!(
        std::fs::read(work.join("previous-recording/0")).unwrap(),
        b"old mic"
    );
}

#[test]
fn caption_moves_retime_words_and_text_edits_replace_old_recognition() {
    let mut p = Project::new(Path::new("fixture.mp4"));
    p.set("autoCaptions",json!([{"id":"c","startMs":0,"endMs":1000,"text":"Hello world","words":[{"text":"Hello","startMs":0,"endMs":400},{"text":"world","startMs":500,"endMs":1000,"leadingSpace":true}]}]));
    subtake_native::editing::move_region(&mut p, "autoCaptions", "c", 500., 0, 4000.).unwrap();
    let cue = &p.regions("autoCaptions")[0];
    assert_eq!(cue["words"][1]["startMs"], json!(1000.));
    p.change_region("autoCaptions", "c", json!({"text":"123"}))
        .unwrap();
    assert!(p.regions("autoCaptions")[0].get("words").is_none());
}

#[test]
fn clip_resize_keeps_encoded_duration_consistent_with_source_speed() {
    let mut p = Project::new(Path::new("fixture.mp4"));
    p.set(
        "clipRegions",
        json!([{"id":"c","startMs":1000,"endMs":2000,"speed":2}]),
    );
    subtake_native::editing::move_region(&mut p, "clipRegions", "c", 500., 2, 5000.).unwrap();
    let cue = &p.regions("clipRegions")[0];
    assert_eq!(cue["startMs"], json!(1500.));
    assert_eq!(cue["endMs"], json!(2250.));
    subtake_native::editing::move_region(&mut p, "clipRegions", "c", 1000., 1, 5000.).unwrap();
    assert_eq!(p.regions("clipRegions")[0]["endMs"], json!(2750.));
}

#[test]
fn cursor_sway_is_stable_across_backward_seeks() {
    use subtake_native::motion::CursorTrack;
    let p = Project::new(Path::new("fixture.mp4"));
    let points = vec![
        json!({"timeMs":0,"cx":0.2,"cy":0.4}),
        json!({"timeMs":1200,"cx":0.8,"cy":0.5}),
        json!({"timeMs":3000,"cx":0.3,"cy":0.7}),
    ];
    let mut track = CursorTrack::default();
    track.at(&p, &points, 2900., 1920., 1080.);
    for time in [1400., 600., 50., 0.] {
        let a = track.at(&p, &points, time, 1920., 1080.);
        let b = CursorTrack::default().at(&p, &points, time, 1920., 1080.);
        assert_eq!(a, b);
    }
}

#[test]
fn relative_media_follows_document_folder_while_wallpapers_stay_portable() {
    let mut p = Project::new(Path::new("video.mp4"));
    p.set("wallpaper", json!("wallpapers/tahoe.jpg"));
    p.set(
        "audioRegions",
        json!([{"id":"a","audioPath":"media/music.wav","startMs":0,"endMs":1000}]),
    );
    let dir = tempfile::tempdir().unwrap();
    p.resolve_assets(&dir.path().join("edit.recordly"));
    assert_eq!(p.editor["wallpaper"], json!("wallpapers/tahoe.jpg"));
    assert_eq!(
        p.regions("audioRegions")[0]["audioPath"],
        json!(dir.path().join("media/music.wav"))
    );
}

#[test]
fn editor_shortcuts_reject_conflicts_and_respect_modifiers() {
    use subtake_native::shortcuts;
    let bindings = shortcuts::defaults();
    assert_eq!(
        shortcuts::action(&bindings, "z", false, false, false).as_deref(),
        Some("add-zoom")
    );
    assert_eq!(shortcuts::action(&bindings, "z", true, false, false), None);
    assert!(shortcuts::validate("play", "Primary+S", &bindings).is_err());
    assert!(shortcuts::validate("play", "Z", &bindings).is_err());
    assert!(shortcuts::validate("play", "Shift+P", &bindings).is_ok());
}

#[test]
fn silence_logs_keep_trailing_silence_and_metadata_times() {
    use subtake_native::segmentation::parse_silences;
    let result = parse_silences(
        "[silencedetect] silence_start: -0.02\n[filter] silence_end: 1.234 | silence_duration: 1.234\n[filter] silence_start: 5",
    );
    assert_eq!(result.len(), 2);
    assert_eq!((result[0].start_ms, result[0].end_ms), (0., 1234.));
    assert_eq!(result[1].start_ms, 5000.);
    assert!(result[1].end_ms.is_infinite());
    let metadata = parse_silences(
        "frame:3\nlavfi.silence_start=0.21\nframe:5\nlavfi.silence_end=2.93\nlavfi.silence_duration=2.72",
    );
    assert_eq!((metadata[0].start_ms, metadata[0].end_ms), (210., 2930.));
}

#[cfg(feature = "native-ffmpeg")]
#[test]
fn native_decoder_rejects_invalid_dimensions() {
    use subtake_native::native_decoder::NativeDecoder;
    assert!(NativeDecoder::open(std::path::Path::new("absent.mp4"), 0, 1080).is_err());
    assert!(NativeDecoder::open(std::path::Path::new("absent.mp4"), u32::MAX, 1080).is_err());
}

#[test]
fn grouped_moves_preserve_offsets_and_clip_speed_at_source_edges() {
    let mut p = subtake_native::project::Project::new(std::path::Path::new("source.mp4"));
    p.set(
        "clipRegions",
        serde_json::json!([{"id":"clip","startMs":1000,"endMs":2000,"speed":2}]),
    );
    p.set("autoCaptions",serde_json::json!([{"id":"caption","startMs":2000,"endMs":3000,"text":"two words","words":[{"text":"two","startMs":2000,"endMs":2400},{"text":"words","startMs":2500,"endMs":3000,"leadingSpace":true}]}]));
    let keys = vec![
        ("clipRegions".into(), "clip".into()),
        ("autoCaptions".into(), "caption".into()),
    ];
    subtake_native::editing::move_group(&mut p, &keys, -5000., 0, 8000.).unwrap();
    assert_eq!(
        p.regions("clipRegions")[0]["startMs"],
        serde_json::json!(0.)
    );
    assert_eq!(
        p.regions("autoCaptions")[0]["words"][0]["startMs"],
        serde_json::json!(1000.)
    );
    subtake_native::editing::move_group(&mut p, &keys, 99999., 0, 8000.).unwrap();
    assert_eq!(
        subtake_native::editing::source_end("clipRegions", &p.regions("clipRegions")[0]),
        8000.
    );
    assert_eq!(
        p.regions("autoCaptions")[0]["startMs"],
        serde_json::json!(7000.)
    );
}

#[test]
fn appearance_presets_preserve_recordings_regions_and_unknown_project_fields() {
    let mut a = subtake_native::project::Project::new(std::path::Path::new("first.mp4"));
    a.set("wallpaper", serde_json::json!("#112233"));
    a.set(
        "webcam",
        serde_json::json!({"width":20,"sourcePath":"first-camera.mp4"}),
    );
    a.set(
        "annotationRegions",
        serde_json::json!([{"id":"keep","startMs":0,"endMs":1000}]),
    );
    let mut b = subtake_native::project::Project::new(std::path::Path::new("second.mp4"));
    b.set(
        "webcam",
        serde_json::json!({"sourcePath":"second-camera.mp4"}),
    );
    b.set("futureField", serde_json::json!(7));
    subtake_native::presets::apply(&mut b, &subtake_native::presets::snapshot(&a)).unwrap();
    assert_eq!(b.video_path, "second.mp4");
    assert!(b.regions("annotationRegions").is_empty());
    assert_eq!(b.editor["futureField"], 7);
    assert_eq!(b.editor["webcam"]["sourcePath"], "second-camera.mp4");
    assert_eq!(b.editor["webcam"]["width"], 20);
}

#[test]
fn localization_reuses_catalogs_and_preserves_unknown_content() {
    use subtake_native::localization::translate;
    assert_eq!(translate("Save", "fr"), "Enregistrer");
    assert_eq!(translate("Cancel", "es"), "Cancelar");
    assert_eq!(translate("Save", "en"), "Save");
    assert_eq!(
        translate("My own project title", "fr"),
        "My own project title"
    );
    for locale in subtake_native::localization::LOCALES {
        assert!(!translate("Export", locale).is_empty());
    }
}

#[test]
fn crop_drag_stays_in_source_and_repairs_imported_bounds() {
    use subtake_native::editing::{adjust_crop, crop_rect};
    let mut p = Project::new(Path::new("video.mp4"));
    p.set("cropRegion", json!({"x":3,"y":-2,"width":8,"height":-1}));
    adjust_crop(&mut p, 2., -2., true).unwrap();
    for (origin, extent) in [
        (crop_rect(&p)[0], crop_rect(&p)[2]),
        (crop_rect(&p)[1], crop_rect(&p)[3]),
    ] {
        assert!(origin >= 0. && extent >= 0.01 && origin + extent <= 1.000001);
    }
    p.set("cropRegion", json!({"x":0,"y":0,"width":0.4,"height":0.5}));
    adjust_crop(&mut p, 10., 10., false).unwrap();
    assert_eq!(crop_rect(&p), [0.6, 0.5, 0.4, 0.5]);
    assert!(adjust_crop(&mut p, f64::NAN, 0., false).is_err());
}

#[test]
fn folder_library_search_filters_sidecars_and_deduplicates_recents() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "Demo.RECORDLY",
        "Second.MOV",
        "take.WEBCAM.MP4",
        "notes.txt",
    ] {
        std::fs::write(dir.path().join(name), b"fixture").unwrap();
    }
    let recent = dir.path().join("Demo.RECORDLY");
    let entries =
        subtake_native::library::entries(Some(dir.path()), &[recent.clone(), recent.clone()], "")
            .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], recent);
    assert_eq!(
        subtake_native::library::entries(Some(dir.path()), &[], "DEMO").unwrap(),
        vec![recent]
    );
}

#[test]
fn a_moved_region_snaps_whichever_edge_lands_nearest_an_anchor() {
    use subtake_native::editing::snap;
    // A 2–5 region moved 2.75 on: its start would land at 4.75, its end at
    // 7.75. The end is nearer an anchor, 8, so it is the end that catches.
    assert_eq!(snap(&[2., 5.], 2.75, &[5.5, 8.], 0., 0.5), (3., Some(8.)));
    // Out of reach of every anchor, the drag is left as it is.
    assert_eq!(snap(&[2., 5.], 1.5, &[5.5, 8.], 0., 0.5), (1.5, None));
    // A trim drags one edge, and only that edge snaps.
    assert_eq!(snap(&[5.], 2.75, &[5.5, 8.], 0., 0.5), (3., Some(8.)));
}

#[test]
fn a_region_out_of_reach_of_every_anchor_snaps_to_the_ruler_ticks() {
    use subtake_native::editing::snap;
    // Moved 1.4375 on, a 2–4.75 region's start lands at 3.4375 and its end
    // at 6.1875, with no anchor in reach. The start is nearer a tick, 3.5,
    // and a tick is not reported as an anchor.
    assert_eq!(snap(&[2., 4.75], 1.4375, &[10.], 0.5, 0.25), (1.5, None));
    // An anchor in reach wins over a nearer tick.
    assert_eq!(
        snap(&[2., 4.75], 1.4375, &[6.3125], 0.5, 0.25),
        (1.5625, Some(6.3125))
    );
    // Out of reach of a tick as well, the drag is left as it is.
    assert_eq!(snap(&[2.], 1.25, &[10.], 0.5, 0.2), (1.25, None));
}
