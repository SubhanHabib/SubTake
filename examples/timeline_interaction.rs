use slint::{ComponentHandle, Timer};
use std::{cell::Cell, rc::Rc, time::Duration};
slint::include_modules!();
fn event(ui: &EditorWindow, kind: u8, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    ui.window().dispatch_event(match kind {
        0 => WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        },
        1 => WindowEvent::PointerMoved { position },
        _ => WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        },
    });
}
fn main() -> Result<(), slint::PlatformError> {
    let ui = EditorWindow::new()?;
    ui.set_has_video(true);
    ui.set_duration(12.);
    ui.set_track_labels(slint::ModelRc::new(slint::VecModel::from(vec![
        "Zoom".into(),
        "Clip".into(),
    ])));
    ui.set_regions(slint::ModelRc::new(slint::VecModel::from(vec![Region {
        id: "test".into(),
        kind: "zoom".into(),
        label: "Zoom".into(),
        start: 0.,
        end: 12.,
        row: 0,
        tint: slint::Color::from_rgb_u8(100, 150, 230),
        selected: false,
    }])));
    let weak = ui.as_weak();
    ui.on_seek(move |time| weak.unwrap().set_playhead(time));
    let selections = Rc::new(Cell::new(0));
    let count = selections.clone();
    ui.on_select_region(move |_, _, _| count.set(count.get() + 1));
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1360., 880.));
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(800), move || {
        let ui = weak.unwrap();
        // Filmstrip: preview changes while held, not just after releasing.
        event(&ui, 0, 400., 650.);
        let start = ui.get_playhead();
        assert!(
            start > 1. && start < 5.,
            "filmstrip press did not seek: {start}"
        );
        event(&ui, 1, 700., 650.);
        assert!(
            ui.get_playhead() > start + 2.,
            "filmstrip drag did not scrub"
        );
        event(&ui, 2, 900., 650.);
        let released = ui.get_playhead();
        assert!(released > 7. && released < 9., "release did not seek");
        let weak = ui.as_weak();
        Timer::single_shot(Duration::from_millis(100), move || {
            let ui = weak.unwrap();
            // The playhead must own its hit target even over an effect region.
            event(&ui, 0, 900., 706.);
            event(&ui, 1, 1050., 706.);
            event(&ui, 2, 1050., 706.);
            assert!(
                ui.get_playhead() > released + 1.,
                "track playhead did not drag"
            );
            assert_eq!(
                selections.get(),
                0,
                "playhead drag selected an underlying effect"
            );
            // Ruler and pointer capture beyond the right edge.
            event(&ui, 0, 500., 597.);
            event(&ui, 1, 1500., 597.);
            assert!(
                (ui.get_playhead() - 12.).abs() < 0.01,
                "scrub did not clamp to duration"
            );
            event(&ui, 2, 1500., 597.);
            println!("TIMELINE_INTERACTION_PASSED");
            slint::quit_event_loop().unwrap();
        });
    });
    slint::run_event_loop()
}
