//! Standalone GPUI text probe; no SubTake runtime, styles, state or platform bridge.
//! Run with current features, then repeat with `--features gpui/font-kit`.
//! Optional: GPUI_TEXT_PROBE_SNAPSHOT=/tmp/gpui-text-probe.png captures the window.
use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};

struct TextProbe;

impl Render for TextProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111111))
            .text_size(px(28.))
            .child(div().w(px(80.)).h(px(12.)).bg(rgb(0xe04040)))
            .child(
                div()
                    .font_family("Helvetica")
                    .child("Helvetica: GPUI text 0123456789"),
            )
            .child(
                div()
                    .font_family(".SystemUIFont")
                    .child("System: GPUI text 0123456789"),
            )
            .child(
                div()
                    .font_family("Menlo")
                    .child("Menlo: GPUI text 0123456789"),
            )
            .child(
                div()
                    .p_4()
                    .bg(rgb(0x182033))
                    .text_color(rgb(0xffffff))
                    .font_family("Helvetica")
                    .child("White Helvetica on dark background"),
            )
            .child(div().w(px(80.)).h(px(12.)).bg(rgb(0x3060e0)))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(760.), px(420.)),
                    cx,
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("GPUI standalone text probe".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                #[cfg(target_os = "macos")]
                {
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    if let Ok(handle) = window.window_handle() {
                        if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                            // Borrowed only on GPUI's main thread. Pass the numeric
                            // WindowServer ID, never an NSView pointer, to the worker.
                            let number: isize = unsafe {
                                let view =
                                    handle.ns_view.as_ptr().cast::<objc2::runtime::AnyObject>();
                                let native: *mut objc2::runtime::AnyObject =
                                    objc2::msg_send![view, window];
                                objc2::msg_send![native, windowNumber]
                            };
                            eprintln!("GPUI_TEXT_PROBE window={number}");
                            if let Some(path) = std::env::var_os("GPUI_TEXT_PROBE_SNAPSHOT") {
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_secs(2));
                                    let result =
                                        std::process::Command::new("/usr/sbin/screencapture")
                                            .args([
                                                "-x",
                                                "-o",
                                                "-t",
                                                "png",
                                                "-l",
                                                &number.to_string(),
                                            ])
                                            .arg(&path)
                                            .output();
                                    match result {
                                        Ok(output) => eprintln!(
                                            "GPUI_TEXT_PROBE capture={} path={} stderr={}",
                                            output.status,
                                            std::path::Path::new(&path).display(),
                                            String::from_utf8_lossy(&output.stderr)
                                        ),
                                        Err(error) => {
                                            eprintln!("GPUI_TEXT_PROBE capture failed: {error}")
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                cx.new(|_| TextProbe)
            },
        )
        .expect("open GPUI text probe");
        cx.activate(true);
    });
}
