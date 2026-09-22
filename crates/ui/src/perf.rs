//! Interaction-latency probes, on only when `SUBTAKE_PERF` is set.
//!
//! gpui is immediate-mode: the only way to see where the time between an
//! input and the frame that answers it goes is to stamp both ends. Every probe
//! prints one line, `perf +<ms since start> <what>`, to stderr, so a
//! reproduction can be read back as a timeline. With the variable unset every
//! probe is one cached boolean load.

use std::sync::OnceLock;
use std::time::Instant;

fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// Whether probes print. Cached after the first call.
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        start();
        std::env::var_os("SUBTAKE_PERF").is_some()
    })
}

/// Milliseconds since the first probe, for stamping a line.
pub fn now_ms() -> f64 {
    start().elapsed().as_secs_f64() * 1000.0
}

/// Print one timeline line.
pub fn log(what: impl std::fmt::Display) {
    if enabled() {
        eprintln!("perf +{:>9.2} {what}", now_ms());
    }
}

/// Print `what` with how long `began` took, but only past `threshold_ms` so a
/// quiet frame loop stays quiet.
pub fn log_took(what: impl std::fmt::Display, began: Instant, threshold_ms: f64) {
    if enabled() {
        let took = began.elapsed().as_secs_f64() * 1000.0;
        if took >= threshold_ms {
            eprintln!("perf +{:>9.2} {what} took {took:.2}ms", now_ms());
        }
    }
}
