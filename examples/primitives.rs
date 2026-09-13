use slint::{ComponentHandle, Timer};
use std::{path::PathBuf, time::Duration};
slint::include_modules!();
fn click(ui: &PrimitiveGallery, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn key(ui: &PrimitiveGallery, text: slint::SharedString) {
    use slint::platform::WindowEvent;
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn snapshot(ui: &PrimitiveGallery, name: &str) {
    let image = ui.window().take_snapshot().unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output/primitives");
    std::fs::create_dir_all(&dir).unwrap();
    image::save_buffer(
        dir.join(name),
        image.as_bytes(),
        image.width(),
        image.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}
fn main() -> Result<(), slint::PlatformError> {
    let ui = PrimitiveGallery::new()?;
    if std::env::args().any(|v| v == "--smoke") {
        let weak = ui.as_weak();
        Timer::single_shot(Duration::from_millis(700), move || {
            let ui = weak.unwrap();
            // Public interactions rather than setters exercise actual focus and hit targets.
            click(&ui, 60., 135.);
            assert_eq!(ui.get_clicks(), 1);
            key(&ui, " ".into());
            assert_eq!(ui.get_clicks(), 2);
            click(&ui, 600., 135.);
            assert_eq!(ui.get_clicks(), 2);
            click(&ui, 40., 366.);
            assert!(ui.get_checked());
            key(&ui, " ".into());
            assert!(!ui.get_checked());
            click(&ui, 600., 366.);
            assert!(ui.get_switched());
            click(&ui, 670., 366.);
            assert!(ui.get_switched());
            click(&ui, 175., 410.);
            assert_eq!(ui.get_radio(), 1);
            click(&ui, 75., 238.);
            key(&ui, slint::platform::Key::Return.into());
            assert_eq!(ui.get_commits(), 1);
            click(&ui, 90., 284.);
            let weak = ui.as_weak();
            Timer::single_shot(Duration::from_millis(200), move || {
                let ui = weak.unwrap();
                assert!(ui.get_dropdown_open());
                key(&ui, slint::platform::Key::DownArrow.into());
                key(&ui, slint::platform::Key::Return.into());
                assert_eq!(ui.get_selected(), 2);
                assert_eq!(ui.get_selections(), 1);
                assert_eq!(ui.get_selection_text(), "Third option");
                click(&ui, 90., 284.);
                let weak = ui.as_weak();
                Timer::single_shot(Duration::from_millis(200), move || {
                    let ui = weak.unwrap();
                    key(&ui, slint::platform::Key::End.into());
                    key(&ui, slint::platform::Key::Escape.into());
                    assert_eq!(ui.get_selected(), 2);
                    assert_eq!(ui.get_selections(), 1);
                    click(&ui, 590., 284.);
                    assert!(!ui.get_dropdown_open());
                    click(&ui, 310., 284.);
                    assert!(!ui.get_dropdown_open());
                    click(&ui, 174., 463.);
                    assert!(ui.get_amount() > 0.4 && ui.get_amount() < 0.6);
                    snapshot(&ui, "dark.png");
                    ui.set_dark(false);
                    let weak = ui.as_weak();
                    Timer::single_shot(Duration::from_millis(300), move || {
                        let ui = weak.unwrap();
                        snapshot(&ui, "light.png");
                        click(&ui, 90., 284.);
                        let weak = ui.as_weak();
                        Timer::single_shot(Duration::from_millis(200), move || {
                            let ui = weak.unwrap();
                            snapshot(&ui, "dropdown.png");
                            key(&ui, slint::platform::Key::End.into());
                            key(&ui, slint::platform::Key::Return.into());
                            assert_eq!(ui.get_selected(), 11);
                            assert_eq!(ui.get_selection_text(), "Twelfth option");
                            click(&ui, 90., 284.);
                            // A long list reopens scrolled to its selected item. Move to the
                            // top before checking pointer selection of the first row.
                            key(&ui, slint::platform::Key::Home.into());
                            let weak = ui.as_weak();
                            Timer::single_shot(Duration::from_millis(200), move || {
                                let ui = weak.unwrap();
                                click(&ui, 90., 318.);
                                assert_eq!(ui.get_selected(), 0);
                                assert_eq!(ui.get_selection_text(), "First option");
                                click(&ui, 666., 546.);
                                assert!((ui.get_amount() - 0.5).abs() < 0.02);
                                key(&ui, slint::platform::Key::RightArrow.into());
                                assert!((ui.get_amount() - 0.51).abs() < 0.02);
                                println!("PRIMITIVES_SMOKE_PASSED");
                                slint::quit_event_loop().unwrap();
                            });
                        });
                    });
                });
            });
        });
    }
    ui.run()
}
