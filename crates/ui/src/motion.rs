//! Hover colour fades — CSS `transition-colors` parity.
//!
//! Ported from the Zeron reference implementation (`zeronsh/zeron`,
//! `crates/ui/src/motion.rs`, MIT © 2026 Wing).
//!
//! gpui `.hover()` styles snap by construction — the style applies the frame
//! the pointer enters. The reference puts Tailwind `transition-colors` (150ms,
//! `cubic-bezier(0.4, 0, 0.2, 1)`) on every interactive wash, so hover states
//! FADE. This is the manual-drive tween for that: a per-element-key hover
//! progress advanced from wall time on each evaluation, with the render tail
//! requesting frames while any fade is mid-flight.
//!
//! The store is a main-thread `thread_local` rather than a gpui Global so the
//! free-function element builders can blend colours without threading `cx`
//! through every signature. All access happens on the UI thread.
//!
//! Staleness: an element that unmounts mid-hover never gets its leave event,
//! so entries are stamped with a frame counter on every read and pruned by
//! [`tick_hover_fades`] when a full frame passes without a read.

use gpui::{App, Hsla, SharedString, Window};
use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

/// CSS `transition-colors` default duration.
pub const HOVER_FADE_MS: u64 = 150;

/// Tailwind's default transition curve — CSS `cubic-bezier(0.4, 0, 0.2, 1)`.
const EASE: CubicBezier = CubicBezier {
    x1: 0.4,
    y1: 0.0,
    x2: 0.2,
    y2: 1.0,
};

#[derive(Clone, Copy)]
struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezier {
    /// Solve y for a given x by Newton iteration, as browsers do.
    fn eval(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..8 {
            let cx = Self::bezier(t, self.x1, self.x2) - x;
            if cx.abs() < 1e-4 {
                break;
            }
            let dx = Self::slope(t, self.x1, self.x2);
            if dx.abs() < 1e-6 {
                break;
            }
            t -= cx / dx;
            t = t.clamp(0.0, 1.0);
        }
        Self::bezier(t, self.y1, self.y2)
    }

    fn bezier(t: f32, a1: f32, a2: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * t * a1 + 3.0 * u * t * t * a2 + t * t * t
    }

    fn slope(t: f32, a1: f32, a2: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * a1 + 6.0 * u * t * (a2 - a1) + 3.0 * t * t * (1.0 - a2)
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// One element's hover fade: progress runs `origin → target` over the fade
/// duration, re-anchored whenever the pointer flips direction mid-flight so
/// the blend stays continuous.
#[derive(Debug, Clone, Copy)]
struct FadeEntry {
    origin: f32,
    target: f32,
    started: Instant,
    /// Frame counter at the last read (liveness stamp).
    seen: u64,
}

impl FadeEntry {
    fn value(&self, now: Instant, duration: Duration) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        if duration.is_zero() || elapsed >= duration {
            return self.target;
        }
        let raw = elapsed.as_secs_f32() / duration.as_secs_f32();
        lerp(self.origin, self.target, EASE.eval(raw))
    }

    fn settled(&self, now: Instant, duration: Duration) -> bool {
        self.origin == self.target || now.saturating_duration_since(self.started) >= duration
    }
}

#[derive(Default)]
struct HoverFades {
    entries: HashMap<String, FadeEntry>,
    frame: u64,
}

impl HoverFades {
    fn duration() -> Duration {
        Duration::from_millis(HOVER_FADE_MS)
    }

    fn set_at(&mut self, key: &str, hovered: bool, reduced: bool, now: Instant) {
        let target = if hovered { 1.0 } else { 0.0 };
        let duration = Self::duration();
        let current = self
            .entries
            .get(key)
            .map(|e| e.value(now, duration))
            .unwrap_or(0.0);
        if target == 0.0 && !self.entries.contains_key(key) {
            return; // never-hovered element reporting a leave
        }
        let origin = if reduced { target } else { current };
        let seen = self.frame;
        self.entries.insert(
            key.to_string(),
            FadeEntry {
                origin,
                target,
                started: now,
                seen,
            },
        );
    }

    fn value_at(&mut self, key: &str, now: Instant) -> f32 {
        let frame = self.frame;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.seen = frame;
                entry.value(now, Self::duration())
            }
            None => 0.0,
        }
    }

    /// Advance the frame counter; report whether any fade is still in flight.
    /// Prunes settled entries that went unread for a whole frame.
    fn tick_at(&mut self, now: Instant) -> bool {
        let duration = Self::duration();
        let frame = self.frame;
        self.entries
            .retain(|_, entry| entry.seen >= frame || !entry.settled(now, duration));
        self.frame = self.frame.wrapping_add(1);
        self.entries
            .values()
            .any(|entry| !entry.settled(now, duration))
    }
}

thread_local! {
    static HOVER_FADES: RefCell<HoverFades> = RefCell::new(HoverFades::default());
}

/// Hover progress (0..1) for `key` this frame.
fn hover_t(key: &str) -> f32 {
    HOVER_FADES.with(|fades| fades.borrow_mut().value_at(key, Instant::now()))
}

/// Record a hover flip for `key` (reduced motion snaps).
fn set_hover(key: &str, hovered: bool, reduced: bool) {
    HOVER_FADES.with(|fades| {
        fades
            .borrow_mut()
            .set_at(key, hovered, reduced, Instant::now())
    });
}

/// Premultiplied-alpha mix so a fade from a zero-alpha wash keeps its hue
/// instead of dipping toward transparent black.
fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let a = lerp(from.a, to.a, t);
    if a <= f32::EPSILON {
        return Hsla { a: 0.0, ..to };
    }
    Hsla {
        h: lerp(from.h, to.h, t),
        s: lerp(from.s, to.s, t),
        l: lerp(from.l, to.l, t),
        a,
    }
}

/// The standard hover blend: rest → hover colour at `key`'s current progress.
pub fn hover_blend(key: &str, rest: Hsla, hover: Hsla) -> Hsla {
    mix(rest, hover, hover_t(key))
}

/// Whether the OS asks for reduced motion.
///
/// The reference reads gpui's `cx.is_reduce_motion()`, which its fork adds;
/// stock gpui 0.2.2 has no such API, so read the macOS accessibility setting
/// once instead. Other platforms fall back to full motion.
pub fn reduced_motion() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("defaults")
                .args(["read", "com.apple.universalaccess", "reduceMotion"])
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|value| value.trim() == "1")
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    })
}

/// An `.on_hover` listener driving the fade for `key` — pair with a
/// [`hover_blend`] read of the same key in the same element.
pub fn hover_listener(
    key: impl Into<SharedString>,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let key = key.into();
    move |hovered, window, _cx| {
        set_hover(&key, *hovered, reduced_motion());
        // Event-dispatch context: `request_animation_frame` is draw-phase only,
        // so mark the whole window dirty and let the render tail keep frames
        // coming while the fade is mid-flight.
        window.refresh();
    }
}

/// Once-per-frame tick from the root render. Returns true while any fade is
/// still in flight, so the caller can request another frame.
pub fn tick_hover_fades() -> bool {
    HOVER_FADES.with(|fades| fades.borrow_mut().tick_at(Instant::now()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_runs_from_rest_to_hover_over_the_duration() {
        let mut fades = HoverFades::default();
        let t0 = Instant::now();
        fades.set_at("a", true, false, t0);
        assert_eq!(fades.value_at("a", t0), 0.0);
        assert_eq!(
            fades.value_at("a", t0 + Duration::from_millis(HOVER_FADE_MS)),
            1.0
        );
    }

    #[test]
    fn reduced_motion_snaps() {
        let mut fades = HoverFades::default();
        let t0 = Instant::now();
        fades.set_at("a", true, true, t0);
        assert_eq!(fades.value_at("a", t0), 1.0);
    }

    #[test]
    fn a_leave_on_a_never_hovered_key_creates_nothing() {
        let mut fades = HoverFades::default();
        fades.set_at("ghost", false, false, Instant::now());
        assert!(fades.entries.is_empty());
    }

    #[test]
    fn tailwind_curve_is_monotonic_between_the_endpoints() {
        assert_eq!(EASE.eval(0.0), 0.0);
        assert!((EASE.eval(1.0) - 1.0).abs() < 1e-3);
        let mut previous = 0.0;
        for step in 0..=20 {
            let value = EASE.eval(step as f32 / 20.0);
            assert!(value >= previous - 1e-4, "curve must not go backwards");
            previous = value;
        }
    }
}
