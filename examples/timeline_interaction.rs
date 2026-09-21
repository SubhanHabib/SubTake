//! Source/output clock and EditorWindow callback regression harness.
//! Direct invocation does not test GPUI pointer capture, pinch or hit targets.
use serde_json::json;
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};
use subtake_native::{
    project::Project,
    timeline,
    ui_runtime::{self, Timer},
    ui_state::EditorWindow,
};

fn check_clock_mapping() {
    let mut project = Project::new(Path::new("timeline-fixture.mp4"));
    project.set("trimRegions", json!([{ "startMs": 2000, "endMs": 4000 }]));
    let spans = timeline::spans(&project, 12.0);
    assert_eq!(timeline::duration(&spans), 10.0);
    assert_eq!(timeline::source_time(&spans, 2.0), 4.0);
    assert_eq!(timeline::output_time(&spans, 3.0), 2.0);
    for source in [0.0, 1.0, 4.0, 6.0, 12.0] {
        let output = timeline::output_time(&spans, source);
        assert!((timeline::source_time(&spans, output) - source).abs() < 1e-9);
    }
}

fn main() -> anyhow::Result<()> {
    check_clock_mapping();
    let ui = EditorWindow::new()?;
    ui.on_translate(|text, _| text);
    ui.set_has_video(true);
    ui.set_duration(12.0);
    let seeks = Rc::new(RefCell::new(Vec::new()));
    let observed = seeks.clone();
    let weak = ui.as_weak();
    ui.on_seek(move |time| {
        observed.borrow_mut().push(time);
        weak.upgrade()
            .expect("editor must stay alive during seek callback")
            .set_playhead(time);
    });
    let selections = Rc::new(RefCell::new(Vec::new()));
    let observed = selections.clone();
    ui.on_select_region(move |kind, id, extend| {
        observed.borrow_mut().push((kind, id, extend));
    });
    ui.show()?;
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(800), move || {
        let ui = weak
            .upgrade()
            .expect("editor must stay alive during timeline check");
        for time in [0.0, 3.0, 8.0, 12.0] {
            ui.invoke_seek(time);
            assert_eq!(ui.get_playhead(), time);
        }
        assert_eq!(*seeks.borrow(), vec![0.0, 3.0, 8.0, 12.0]);
        assert!(
            selections.borrow().is_empty(),
            "seek must not select a region"
        );
        ui.invoke_select_region("zoom".into(), "test".into(), true);
        let selections = selections.borrow();
        assert_eq!(selections.len(), 1);
        assert_eq!(selections[0].0, "zoom");
        assert_eq!(selections[0].1, "test");
        assert!(selections[0].2, "extend-selection flag was lost");
        assert!(!ui.get_dirty());
        println!("TIMELINE_MODEL_CALLBACK_PASSED (no pointer/gesture coverage)");
        ui_runtime::quit_event_loop().unwrap();
    });
    ui_runtime::run_event_loop_until_quit()?;
    Ok(())
}

#[test]
fn trimmed_source_output_clock_roundtrip() {
    check_clock_mapping();
}
