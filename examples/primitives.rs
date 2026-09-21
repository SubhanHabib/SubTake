//! GPUI editor surface harness. --smoke checks callbacks, not hit testing.
use std::{cell::Cell, rc::Rc, time::Duration};
use subtake_native::{
    ui_runtime::{self, Timer},
    ui_state::EditorWindow,
};

fn main() -> anyhow::Result<()> {
    let ui = EditorWindow::new()?;
    ui.on_translate(|text, _| text);
    let clicks = Rc::new(Cell::new(0));
    let observed = clicks.clone();
    ui.on_preview_click(move |x, y| {
        assert!((0.0..=1.0).contains(&x));
        assert!((0.0..=1.0).contains(&y));
        observed.set(observed.get() + 1);
    });
    ui.show()?;
    if std::env::args().any(|arg| arg == "--smoke") {
        let weak = ui.as_weak();
        Timer::single_shot(Duration::from_millis(700), move || {
            let ui = weak
                .upgrade()
                .expect("editor must stay alive during smoke check");
            ui.invoke_preview_click(0.25, 0.75);
            assert_eq!(clicks.get(), 1, "callback must be delivered exactly once");
            assert!(
                !ui.get_dirty(),
                "preview callback must not edit the document"
            );
            println!("GPUI_SURFACE_CALLBACK_SMOKE_PASSED (no pointer/keyboard coverage)");
            ui_runtime::quit_event_loop().unwrap();
        });
    }
    ui_runtime::run_event_loop_until_quit()?;
    Ok(())
}
