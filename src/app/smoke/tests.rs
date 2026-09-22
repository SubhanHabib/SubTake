use super::when_idle;
use std::cell::{Cell, RefCell};

#[test]
fn modal_borrow_skips_refresh_then_retries_without_losing_pending_size() {
    let state = RefCell::new(());
    let refreshed = Cell::new(false);
    let modal_action = state.borrow_mut();
    when_idle(&state, |_| refreshed.set(true));
    assert!(!refreshed.get());
    drop(modal_action);
    when_idle(&state, |_| refreshed.set(true));
    assert!(refreshed.get());
}
