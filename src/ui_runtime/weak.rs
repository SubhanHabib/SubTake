//! Weak handles that workers may hold to a UI surface without touching
//! main-thread state.

use super::*;

pub trait FromUiData {
    fn from_data(data: Rc<UiData>) -> Self;
}

pub struct Weak<T> {
    id: u64,
    thread: std::thread::ThreadId,
    marker: PhantomData<fn() -> T>,
}

thread_local! {pub(super) static WEAK_DATA:RefCell<BTreeMap<u64,std::rc::Weak<UiData>>>=const { RefCell::new(BTreeMap::new()) };}
impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            thread: self.thread,
            marker: PhantomData,
        }
    }
}

impl<T: FromUiData> Weak<T> {
    pub fn new(data: &Rc<UiData>) -> Self {
        let id = data.window.0.id;
        WEAK_DATA.with(|registry| {
            registry.borrow_mut().insert(id, Rc::downgrade(data));
        });
        Self {
            id,
            thread: std::thread::current().id(),
            marker: PhantomData,
        }
    }

    pub fn upgrade(&self) -> Option<T> {
        // Worker threads may carry this identity, but UI ownership never crosses threads.
        if std::thread::current().id() != self.thread {
            return None;
        }
        WEAK_DATA
            .with(|registry| {
                registry
                    .borrow()
                    .get(&self.id)
                    .and_then(std::rc::Weak::upgrade)
            })
            .map(T::from_data)
    }
}
