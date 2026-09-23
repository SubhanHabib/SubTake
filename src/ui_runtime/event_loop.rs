//! Starting, running and quitting the gpui application.

use super::*;

pub fn quit_event_loop() -> Result<()> {
    QUIT.with(|q| q.set(true));
    wake();
    Ok(())
}

pub fn run_event_loop_until_quit() -> Result<()> {
    gpui_platform::application().with_assets(Assets).run(|cx| {
        // gpui's own animations (panel entrances, menus, the Presets dialog)
        // only land in place when told to.
        cx.set_reduce_motion(subtake_ui::motion::reduced_motion());
        install_menus(cx);
        CONTEXT.with(|c| *c.borrow_mut() = Some(cx.to_async()));
        if let Err(error) = sync_windows(cx) {
            eprintln!("Open GPUI window: {error:#}");
            cx.quit();
            return;
        }
        cx.spawn(async move |cx| {
            loop {
                let delay = TIMERS
                    .with(|t| {
                        t.borrow()
                            .values()
                            .map(|t| t.deadline.saturating_duration_since(Instant::now()))
                            .min()
                    })
                    .unwrap_or(Duration::from_secs(3600));
                let timer = cx.background_executor().timer(delay);
                let receiver = &WAKE.get_or_init(|| async_channel::bounded(1)).1;
                futures_lite::future::race(
                    async {
                        timer.await;
                    },
                    async {
                        let _ = receiver.recv().await;
                    },
                )
                .await;
                // Run application callbacks outside GPUI's app borrow, allowing native modal dialogs.
                subtake_ui::perf::log("pump wake");
                let began = Instant::now();
                drain_commands();
                subtake_ui::perf::log_took("pump drain_commands", began, 1.0);
                let quit = QUIT.with(Cell::get);
                let began = Instant::now();
                cx.update(|cx| {
                    if quit {
                        cx.quit();
                    } else if let Err(error) = sync_windows(cx) {
                        eprintln!("GPUI update: {error:#}");
                    }
                });
                subtake_ui::perf::log_took("pump sync_windows", began, 1.0);
                if quit {
                    break;
                }
            }
        })
        .detach();
    });
    CONTEXT.with(|c| c.borrow_mut().take());
    Ok(())
}

pub fn run_event_loop() -> Result<()> {
    run_event_loop_until_quit()
}
