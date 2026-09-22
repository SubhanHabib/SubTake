//! `SUBTAKE_GALLERY_SCREEN=tour`: the gallery walks itself through a whole
//! take, recorder to export, on timers, then quits. It exists so the flow can
//! be filmed per window (or watched) without anyone clicking.
use super::{Gallery, show_recorder};
use std::{cell::RefCell, rc::Rc, time::Duration};
use subtake_native::ui_runtime::{self, Timer};

type Step = (u64, fn(&mut Gallery));

/// Each step waits its milliseconds, then runs.
const STEPS: &[Step] = &[
    (0, |g| {
        let _ = g.editor.hide();
    }),
    (1500, |g| card(g, "sources")),
    (1800, |g| card(g, "audio")),
    (1800, |g| card(g, "camera")),
    (1800, |g| card(g, "countdown")),
    (1800, |g| card(g, "")),
    (1000, |g| g.action("record")),
    (6500, |g| g.action("pause-recording")),
    (2000, |g| g.action("resume-recording")),
    (2500, |g| g.action("stop-recording")),
    (3200, |g| g.action("show-editor")),
    (1200, |g| g.toggle_play()),
    (3000, |g| g.toggle_play()),
    (800, |g| open_panel(g, "Cursor")),
    (1800, |g| open_panel(g, "Webcam")),
    (1800, |g| open_panel(g, "Preferences")),
    (2200, |g| open_panel(g, "Shortcuts")),
    (1800, |g| open_panel(g, "Frame")),
    (1500, |g| {
        for r in &mut g.regions {
            r.selected = r.id == "z2";
        }
        g.before_selection = Some("Frame".into());
        g.editor.set_selected_id("z2".into());
        g.push_timeline();
        open_panel(g, "Selection");
    }),
    (2200, |g| g.action("deselect")),
    (1200, |g| g.action("toggle-presets")),
    (2800, |g| g.action("toggle-presets")),
    (1000, |g| open_panel(g, "Export")),
    (1500, |g| g.action("export")),
    (10000, |g| g.editor.set_export_state(String::new())),
    // Under 1280 the inspector folds to a toggle; it slides in and out.
    (1500, |g| width(g, 1100.)),
    (2000, |g| g.editor.set_inspector_open(true)),
    (2200, |g| g.editor.set_inspector_open(false)),
    (1800, |g| width(g, 1360.)),
    (2000, |_| {
        let _ = ui_runtime::quit_event_loop();
    }),
];

pub(super) fn start(gallery: Rc<RefCell<Gallery>>) {
    run(gallery, 0);
}

fn run(gallery: Rc<RefCell<Gallery>>, index: usize) {
    let Some(&(wait, step)) = STEPS.get(index) else {
        return;
    };
    Timer::single_shot(Duration::from_millis(wait), move || {
        step(&mut gallery.borrow_mut());
        run(gallery, index + 1);
    });
}

fn card(g: &mut Gallery, name: &str) {
    show_recorder(&g.launcher, &g.options);
    g.launcher.set_panel(name.into());
    g.options.set_panel(name.into());
    g.position_options();
}

fn open_panel(g: &mut Gallery, name: &str) {
    g.editor.set_panel(name.into());
    g.push_fields();
}

fn width(g: &mut Gallery, width: f32) {
    g.editor
        .window()
        .set_size(ui_runtime::LogicalSize::new(width, 880.));
}
