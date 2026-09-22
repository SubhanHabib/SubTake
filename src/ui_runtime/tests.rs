#[test]
fn completion_batches_rearm_until_all_work_is_delivered() {
    let (sender, receiver) = crossbeam_channel::unbounded::<Completion>();
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for _ in 0..257 {
        let count = count.clone();
        sender
            .send(Box::new(move || {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }))
            .unwrap();
    }
    let wakes = std::cell::Cell::new(0);
    for expected in [128, 256, 257] {
        super::drain_completions(&receiver, || wakes.set(wakes.get() + 1));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), expected);
    }
    assert_eq!(wakes.get(), 2);
    assert!(receiver.is_empty());
}
use super::*;
#[test]
fn preview_upload_swizzles_red_and_blue_without_changing_alpha() {
    let image = Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
        &[255, 20, 10, 128, 1, 2, 3, 255],
        2,
        1,
    ));
    assert_eq!(
        image.0.unwrap().as_bytes(0).unwrap(),
        &[10, 20, 255, 128, 3, 2, 1, 255]
    );
}
#[test]
fn worker_can_return_ui_identity_but_cannot_access_main_thread_state() {
    let ui = crate::ui_state::EditorWindow::new().unwrap();
    let weak = ui.as_weak();
    let returned = std::thread::spawn(move || {
        assert!(weak.upgrade().is_none());
        weak
    })
    .join()
    .unwrap();
    returned.upgrade().unwrap().set_status("completion".into());
    assert_eq!(ui.get_status(), "completion");
}
#[test]
fn restarting_and_stopping_timers_discards_obsolete_callbacks() {
    let count = Rc::new(Cell::new(0));
    let timer = Timer::default();
    let c = count.clone();
    timer.start(TimerMode::SingleShot, Duration::ZERO, move || {
        c.set(c.get() + 100)
    });
    let c = count.clone();
    timer.start(TimerMode::SingleShot, Duration::ZERO, move || {
        c.set(c.get() + 1)
    });
    drain_commands();
    assert_eq!(count.get(), 1);
    let c = count.clone();
    timer.start(TimerMode::Repeated, Duration::ZERO, move || {
        c.set(c.get() + 1)
    });
    drain_commands();
    assert_eq!(count.get(), 2);
    timer.stop();
    drain_commands();
    assert_eq!(count.get(), 2);
}
#[test]
fn callback_can_schedule_another_timer_without_borrowing_the_scheduler() {
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    Timer::single_shot(Duration::ZERO, move || {
        c.set(1);
        Timer::single_shot(Duration::ZERO, move || c.set(2));
    });
    drain_commands();
    assert_eq!(count.get(), 1);
    drain_commands();
    assert_eq!(count.get(), 2);
}
