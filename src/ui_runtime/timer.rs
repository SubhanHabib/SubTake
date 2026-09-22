//! Repeating and one-shot timers driven from the event loop.

use super::*;

pub(super) type TimerCallback = Rc<RefCell<Box<dyn FnMut()>>>;
pub(super) struct Scheduled {
    pub(super) deadline: Instant,
    pub(super) interval: Option<Duration>,
    pub(super) callback: TimerCallback,
}
thread_local! {
    pub(super) static TIMERS: RefCell<BTreeMap<u64, Scheduled>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}
pub(super) fn next_id() -> u64 {
    NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}
#[derive(Clone, Copy)]
pub enum TimerMode {
    SingleShot,
    Repeated,
}
#[derive(Default)]
pub struct Timer {
    id: Cell<Option<u64>>,
}
impl Timer {
    pub fn start(&self, mode: TimerMode, duration: Duration, f: impl FnMut() + 'static) {
        self.stop();
        let id = next_id();
        self.id.set(Some(id));
        schedule(id, mode, duration, f);
    }
    pub fn stop(&self) {
        if let Some(id) = self.id.take() {
            TIMERS.with(|t| {
                t.borrow_mut().remove(&id);
            });
        }
    }
    pub fn single_shot(duration: Duration, f: impl FnOnce() + 'static) {
        let mut f = Some(f);
        schedule(next_id(), TimerMode::SingleShot, duration, move || {
            if let Some(f) = f.take() {
                f();
            }
        });
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.stop();
    }
}
pub(super) fn schedule(id: u64, mode: TimerMode, duration: Duration, f: impl FnMut() + 'static) {
    TIMERS.with(|t| {
        t.borrow_mut().insert(
            id,
            Scheduled {
                deadline: Instant::now() + duration,
                interval: match mode {
                    TimerMode::SingleShot => None,
                    TimerMode::Repeated => Some(duration),
                },
                callback: Rc::new(RefCell::new(Box::new(f))),
            },
        );
    });
    wake();
}
