use serde_json::json;
use std::path::Path;
use subtake_native::{
    project::{History, Project, parse_srt},
    timeline,
};

fn project() -> Project {
    Project::new(Path::new("/fixture/video.mp4"))
}

#[test]
fn unknown_fields_and_legacy_settings_survive_atomic_save() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.recordly");
    let mut p = project();
    p.extra
        .insert("futureMetadata".into(), json!({"a":[1,2,3]}));
    p.set("futureEffect", json!({"enabled":true}));
    p.save(&file).unwrap();
    let original = std::fs::read(&file).unwrap();
    p.set("wallpaper", json!("#112233"));
    p.save(&file).unwrap();
    assert_eq!(Project::load(&file).unwrap(), p);
    assert_eq!(
        std::fs::read(file.with_extension("recordly.bak")).unwrap(),
        original
    );
}

#[test]
fn invalid_edits_are_transactional_and_redo_branches() {
    let mut h = History::new(project());
    let original = h.project.clone();
    assert!(
        h.edit(|p| {
            p.add("zoomRegions", json!({"startMs":20,"endMs":10}))?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(h.project, original);
    assert!(!h.dirty());
    h.edit(|p| {
        p.set("wallpaper", json!("#ffffff"));
        Ok(())
    })
    .unwrap();
    assert!(h.dirty());
    h.undo();
    assert_eq!(h.project, original);
    h.redo();
    assert!(h.dirty());
    h.undo();
    h.edit(|p| {
        p.set("wallpaper", json!("#000000"));
        Ok(())
    })
    .unwrap();
    h.redo();
    assert_eq!(h.project.text("wallpaper", ""), "#000000");
}

#[test]
fn overlapping_trims_remove_the_union() {
    let mut p = project();
    p.set(
        "trimRegions",
        json!([{ "id":"a","startMs":1000,"endMs":3000},{"id":"b","startMs":2000,"endMs":4000}]),
    );
    let spans = timeline::spans(&p, 6.);
    assert_eq!(timeline::duration(&spans), 3.);
    assert_eq!(timeline::source_time(&spans, 1.), 4.);
    assert_eq!(timeline::output_time(&spans, 3.), 1.);
}

#[test]
fn speed_boundaries_and_clip_source_end() {
    let mut p = project();
    p.set(
        "clipRegions",
        json!([{"id":"a","startMs":1000,"endMs":3000,"speed":2}]),
    );
    let spans = timeline::spans(&p, 10.);
    assert_eq!(spans[0].source_end, 5.);
    assert_eq!(timeline::duration(&spans), 2.);
    assert_eq!(timeline::source_time(&spans, 1.), 3.);
    let mut p = project();
    p.set(
        "speedRegions",
        json!([{"id":"a","startMs":1000,"endMs":3000,"speed":2}]),
    );
    let spans = timeline::spans(&p, 5.);
    assert_eq!(timeline::duration(&spans), 4.);
    assert_eq!(timeline::source_time(&spans, 2.), 3.);
}

#[test]
fn removing_entire_video_has_zero_duration() {
    let mut p = project();
    p.set("trimRegions", json!([{"id":"a","startMs":0,"endMs":10000}]));
    assert_eq!(timeline::duration(&timeline::spans(&p, 5.)), 0.);
}

#[test]
fn subtitle_import_retains_multiline_and_milliseconds() {
    let cues=parse_srt("1\r\n00:00:01,250 --> 00:00:02,750\r\nHello\r\nworld\r\n\r\n2\r\n00:01:00,000 --> 00:01:01,000\r\nNext").unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0]["startMs"], 1250.);
    assert_eq!(cues[0]["text"], "Hello\nworld");
    assert!(parse_srt("1\n00:00:02,000 --> 00:00:01,000\nx").is_err());
}

#[test]
fn missing_media_is_not_silently_rewritten() {
    let p = project();
    assert_eq!(p.source_path(None), Path::new("/fixture/video.mp4"));
    let mut p = p;
    p.video_path = "clips/a.mp4".into();
    assert_eq!(
        p.source_path(Some(Path::new("/projects/demo.recordly"))),
        Path::new("/projects/clips/a.mp4")
    );
}

#[test]
fn lanes_turned_off_are_saved_and_leave_the_picture() {
    let mut p = project();
    p.set(
        "annotationRegions",
        json!([{"id":"a","startMs":0,"endMs":1000}]),
    );
    p.set("zoomRegions", json!([{"id":"z","startMs":0,"endMs":1000}]));
    assert!(matches!(p.played(), std::borrow::Cow::Borrowed(_)));
    p.toggle_lane("Annotation");
    p.toggle_lane("Audio");
    assert!(p.lane_off("Annotation") && p.lane_off("Audio") && !p.lane_off("Zoom"));
    let played = p.played();
    assert!(played.regions("annotationRegions").is_empty());
    assert_eq!(played.regions("zoomRegions").len(), 1);
    assert_eq!(p.regions("annotationRegions").len(), 1);

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.recordly");
    p.save(&file).unwrap();
    let mut loaded = Project::load(&file).unwrap();
    assert!(loaded.lane_off("Annotation") && loaded.lane_off("Audio"));
    loaded.toggle_lane("Annotation");
    loaded.toggle_lane("Audio");
    assert!(!loaded.editor.contains_key("lanesOff"));
}
