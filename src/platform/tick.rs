//! The countdown's tick, over `native/CountdownTick.swift`.

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_countdown_tick();
}

/// Plays one second of the recorder's countdown: a soft click.
pub fn countdown_tick() {
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_countdown_tick();
    }
}
