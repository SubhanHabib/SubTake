use serde_json::json;
use std::path::Path;
use subtake_native::{preferences::Preferences, presets, project::Project};

#[test]
fn older_preferences_keep_shortcuts_and_gain_appearance_defaults() {
    let mut prefs: Preferences = serde_json::from_value(json!({
        "language":"fr", "record_shortcut":"Super+Shift+9",
        "recent_projects":["/video/one.recordly"]
    }))
    .unwrap();
    assert_eq!(prefs.appearance, "system");
    assert!(prefs.auto_apply_zooms);
    prefs.appearance = "light".into();
    prefs.auto_apply_zooms = false;
    let restored: Preferences =
        serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
    assert_eq!(restored.appearance, "light");
    assert!(!restored.auto_apply_zooms);
    assert_eq!(restored.language, "fr");
    assert_eq!(restored.record_shortcut, "Super+Shift+9");
    assert_eq!(restored.recent_projects, prefs.recent_projects);
}

#[test]
fn motion_cards_report_custom_after_tuning_without_losing_source_or_regions() {
    let mut project = Project::new(Path::new("/video/keep.mp4"));
    project
        .add("zoomRegions", json!({"startMs":100,"endMs":1000,"depth":2}))
        .unwrap();
    let regions = project.regions("zoomRegions").to_vec();
    for (smooth, expected) in [(false, "focused"), (true, "smooth")] {
        presets::motion(&mut project, smooth);
        assert_eq!(presets::motion_choice(&project), expected);
        assert_eq!(project.video_path, "/video/keep.mp4");
        assert_eq!(project.regions("zoomRegions"), regions);
        project.set("cursorSize", json!(4.0));
        assert_eq!(presets::motion_choice(&project), "");
    }
}

#[test]
fn appearance_cards_preserve_media_timing_and_motion() {
    let mut p = Project::new(Path::new("/video/keep.mp4"));
    p.add("zoomRegions", json!({"startMs":100,"endMs":1000,"depth":2}))
        .unwrap();
    let original = p.clone();
    for name in ["studio", "minimal", "bold"] {
        presets::appearance(&mut p, name).unwrap();
        assert_eq!(presets::appearance_choice(&p), name);
        assert_eq!(p.video_path, original.video_path);
        assert_eq!(p.extra, original.extra);
        for (key, value) in &original.editor {
            if ![
                "wallpaper",
                "padding",
                "borderRadius",
                "shadowIntensity",
                "shadowColor",
                "frameEdgeColor",
            ]
            .contains(&key.as_str())
            {
                assert_eq!(p.editor.get(key), Some(value));
            }
        }
    }
    let before = p.clone();
    assert!(presets::appearance(&mut p, "unknown").is_err());
    assert_eq!(p, before);
}
