//! Main-thread dispatch: workers enqueue completions, the event loop drains
//! them.

use super::*;

pub(super) type Completion = Box<dyn FnOnce() + Send>;
pub(super) static COMPLETIONS: OnceLock<(
    crossbeam_channel::Sender<Completion>,
    crossbeam_channel::Receiver<Completion>,
)> = OnceLock::new();
pub(super) static WAKE: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();
pub(super) fn wake() {
    let _ = WAKE
        .get_or_init(|| async_channel::bounded(1))
        .0
        .try_send(());
}
pub fn invoke_from_event_loop(f: impl FnOnce() + Send + 'static) -> Result<()> {
    COMPLETIONS
        .get_or_init(crossbeam_channel::unbounded)
        .0
        .send(Box::new(f))
        .map_err(|_| anyhow::anyhow!("UI event queue disconnected"))?;
    wake();
    Ok(())
}

pub(super) fn drain_completions(
    queue: &crossbeam_channel::Receiver<Completion>,
    rearm: impl FnOnce(),
) {
    // Bound each turn so a producer cannot starve native input.
    for _ in 0..128 {
        match queue.try_recv() {
            Ok(f) => f(),
            Err(_) => break,
        }
    }
    if !queue.is_empty() {
        rearm();
    }
}
pub(super) fn drain_commands() {
    let queue = &COMPLETIONS.get_or_init(crossbeam_channel::unbounded).1;
    drain_completions(queue, wake);
    let now = Instant::now();
    let due = TIMERS.with(|t| {
        t.borrow()
            .iter()
            .filter(|(_, s)| s.deadline <= now)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>()
    });
    for id in due {
        let callback = TIMERS.with(|t| {
            let mut timers = t.borrow_mut();
            let task = timers.get_mut(&id)?;
            let callback = task.callback.clone();
            if let Some(interval) = task.interval {
                task.deadline = now + interval;
            } else {
                timers.remove(&id);
            }
            Some(callback)
        });
        if let Some(callback) = callback
            && let Ok(mut f) = callback.try_borrow_mut()
        {
            f();
        }
    }
}
