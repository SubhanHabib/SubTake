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

use gpui::{
    Animation, AnimationElement, AnimationExt, App, ElementId, Hsla, IntoElement, SharedString,
    Styled, Window, ease_out_quint, px,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::{Duration, Instant},
};

/// CSS `transition-colors` default duration.
pub const HOVER_FADE_MS: u64 = 150;

/// Tailwind's default transition curve — CSS `cubic-bezier(0.4, 0, 0.2, 1)`.
pub const EASE: Curve = Curve::Bezier(CubicBezier {
    x1: 0.4,
    y1: 0.0,
    x2: 0.2,
    y2: 1.0,
});

/// CSS `ease-out` — `cubic-bezier(0, 0, 0.58, 1)`.
pub const EASE_OUT: Curve = Curve::Bezier(CubicBezier {
    x1: 0.0,
    y1: 0.0,
    x2: 0.58,
    y2: 1.0,
});

/// A gentle overshoot, in the shape CSS `linear()` springs are authored in:
/// mass 1, and the stiffness/damping a UI spring usually wants.
pub const SPRING: Curve = Curve::Spring {
    stiffness: 180.0,
    damping: 20.0,
};

/// How a tween gets from 0 to 1. `eval` takes normalised time and returns
/// normalised progress, so a spring may legitimately return a value above 1
/// while it overshoots.
#[derive(Clone, Copy)]
pub enum Curve {
    Bezier(CubicBezier),
    /// A damped harmonic oscillator, as CSS spring easings and SwiftUI
    /// `.spring` describe one. Mass is fixed at 1.
    Spring {
        stiffness: f32,
        damping: f32,
    },
}

impl Curve {
    pub fn eval(&self, t: f32) -> f32 {
        match self {
            Self::Bezier(bezier) => bezier.eval(t),
            Self::Spring { stiffness, damping } => spring(t, *stiffness, *damping),
        }
    }
}

/// Progress of a unit spring at normalised time `t`.
///
/// Mass is 1, so the system is `x'' + damping * x' + stiffness * x = 0`
/// released from rest at -1. Return displacement from the target expressed as
/// progress: 0 at rest, 1 at the target, and above 1 while it overshoots.
fn spring(t: f32, stiffness: f32, damping: f32) -> f32 {
    // TODO(human)
    let _ = (stiffness, damping);
    t
}

#[derive(Clone, Copy)]
pub struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezier {
    /// Solve y for a given x by Newton iteration, as browsers do.
    pub fn eval(&self, x: f32) -> f32 {
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

/// Linear interpolation, also used by callers tweening geometry rather
/// than colour (the toggle thumb's travel, a chevron's rotation).
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// How far through a move that began at `started` and lasts `ms` it is at
/// `now`, from 0 to 1 and held at 1 once it is over.
pub fn progress(started: Instant, ms: u64, now: Instant) -> f32 {
    (now.saturating_duration_since(started).as_secs_f32() * 1000. / ms as f32).min(1.)
}

/// Where a move from `from` to `to` over `ms`, begun at `started`, stands at
/// `now` on [`EASE_OUT`] — exactly `to` once it is over. For the moves the
/// app times itself: the recorder card's height, a sliding inspector, the
/// export tick.
pub fn ease_toward(from: f32, to: f32, started: Instant, ms: u64, now: Instant) -> f32 {
    let t = progress(started, ms, now);
    if t >= 1. {
        to
    } else {
        lerp(from, to, EASE_OUT.eval(t))
    }
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
    /// Frame counter at the last re-anchor, used to catch a contested key.
    anchored: u64,
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
                anchored: seen,
            },
        );
    }

    /// Drive `key` toward `on` from a RENDER pass rather than an event.
    ///
    /// [`Self::set_at`] re-anchors unconditionally, which is right for a
    /// pointer flip but wrong here: render runs every frame, so re-anchoring
    /// would restart the tween each frame and it would never arrive. This
    /// only re-anchors when the target actually changes, and adopts the
    /// state outright the first time a key is seen so a panel that opens
    /// with a switch already on does not play the switch-on animation.
    fn set_state_at(&mut self, key: &str, on: bool, reduced: bool, now: Instant) -> f32 {
        let target = if on { 1.0 } else { 0.0 };
        let duration = Self::duration();
        let frame = self.frame;
        if let Some(entry) = self.entries.get_mut(key) {
            if entry.target != target {
                // Two controls are driving one key and disagree — they share
                // an element id. Each would re-anchor the other every render,
                // so the tween would never settle and the root render would
                // request frames forever: the window repaints at full rate
                // with nothing moving on it. Snap instead. The controls still
                // fight over the value, but only one of them looks wrong,
                // which is a great deal cheaper than pegging a core.
                let contested = entry.anchored == frame;
                let origin = if reduced || contested {
                    target
                } else {
                    entry.value(now, duration)
                };
                *entry = FadeEntry {
                    origin,
                    target,
                    started: now,
                    seen: frame,
                    anchored: frame,
                };
            } else {
                entry.seen = frame;
            }
            return entry.value(now, duration);
        }
        self.entries.insert(
            key.to_string(),
            FadeEntry {
                origin: target,
                target,
                started: now,
                seen: frame,
                anchored: frame,
            },
        );
        target
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
    static WINDOW: Cell<u64> = const { Cell::new(0) };
}

/// Scope the tween store to the window now rendering.
///
/// An element id is unique within a window, not across windows, and this
/// store is one thread-local shared by every window on the main thread. The
/// editor's close button and the recording overlay's are both `"close"`, so
/// without a scope they share a tween key and disagree about it every frame.
/// The root render calls this before it builds its tree; gpui draws one
/// window at a time, so every element painted afterwards belongs to it.
pub fn enter_window(id: u64) {
    WINDOW.with(|window| window.set(id));
}

fn scope() -> u64 {
    WINDOW.with(|window| window.get())
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

/// Progress (0..1) of `key`'s tween toward `on`, driven from render.
///
/// Use this for state a control *holds* — switched on, selected, expanded —
/// where there is no enter/leave event to hang a listener on. Reading it is
/// what advances it, so call it unconditionally in the render that uses it.
pub fn state_fade(key: &str, on: bool) -> f32 {
    let reduced = reduced_motion();
    HOVER_FADES.with(|fades| {
        fades
            .borrow_mut()
            .set_state_at(key, on, reduced, Instant::now())
    })
}

/// Interpolate two colours at `t`, premultiplied so a fade out of a
/// zero-alpha wash keeps its hue instead of dipping through transparent black.
pub fn blend(from: Hsla, to: Hsla, t: f32) -> Hsla {
    mix(from, to, t)
}

/// A stable tween key for `id`'s `slot`.
///
/// Element ids are already unique within a window, which is exactly the
/// scope the tween store needs; `slot` separates the several tweens one
/// control runs at once (its hover wash and its selected fill).
pub fn tween_key(id: &ElementId, slot: &str) -> String {
    format!("{}@{id:?}#{slot}", scope())
}

/// The standard hover blend: rest → hover colour at `key`'s current progress.
pub fn hover_blend(key: &str, rest: Hsla, hover: Hsla) -> Hsla {
    mix(rest, hover, hover_t(key))
}

/// Whether the OS asks for reduced motion.
///
/// gpui keeps a reduce-motion flag of its own for [`AnimationExt`] but never
/// reads the platform's setting into it, so this reads the macOS
/// accessibility setting once and the app hands it to gpui at launch.
/// `SUBTAKE_REDUCE_MOTION=1` turns it on without changing the Mac's setting,
/// for checking every transition lands in place. Other platforms fall back to
/// full motion.
pub fn reduced_motion() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        if std::env::var_os("SUBTAKE_REDUCE_MOTION").is_some_and(|v| v == "1") {
            return true;
        }
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
        crate::perf::log(format_args!(
            "hover {} {key}",
            if *hovered { "in " } else { "out" }
        ));
        set_hover(&key, *hovered, reduced_motion());
        // Event-dispatch context: `request_animation_frame` is draw-phase only,
        // so mark the whole window dirty and let the render tail keep frames
        // coming while the fade is mid-flight.
        window.refresh();
    }
}

// ---------------------------------------------------------------------------
// entrances
// ---------------------------------------------------------------------------

/// How long a floating surface takes to arrive.
pub const MENU_IN_MS: u64 = 140;

/// The distance a menu travels as it settles, in pixels.
const MENU_IN_RISE: f32 = 4.0;

/// How long a floating surface takes to fade once dismissed: quicker than
/// it arrives, since by then the eye has moved on.
pub const MENU_OUT_MS: u64 = 100;

/// A floating surface's way out. Its owner reports each render whether it
/// is open; while it is, or for [`MENU_OUT_MS`] after it closes, this gives
/// the opacity to draw it at, and `None` once it has gone. `opens` counts
/// openings, for an entrance keyed afresh each time.
#[derive(Default)]
pub struct Leave {
    was_open: bool,
    closed: Option<Instant>,
    pub opens: usize,
}

impl Leave {
    pub fn shown(&mut self, open: bool) -> Option<f32> {
        if open {
            if !self.was_open {
                self.opens += 1;
            }
            self.was_open = true;
            self.closed = None;
            return Some(1.);
        }
        if std::mem::take(&mut self.was_open) && !reduced_motion() {
            self.closed = Some(Instant::now());
        }
        let opacity = ease_toward(1., 0., self.closed?, MENU_OUT_MS, Instant::now());
        if opacity == 0. {
            self.closed = None;
            return None;
        }
        Some(opacity)
    }
}

/// Fade `element` in over [`MENU_IN_MS`] — tooltips and other surfaces that
/// appear in place.
///
/// gpui runs these through [`AnimationExt`], which honours the platform's
/// reduce-motion setting on its own: the element is simply rendered in its
/// end state and no frames are scheduled.
pub fn fade_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(id, menu_curve(), |el, t| el.opacity(t))
}

/// Fade `element` in and settle it down onto `top`.
///
/// Only for a surface that is already absolutely positioned — the rise is
/// applied to `top`, so on an in-flow element it would shove its siblings
/// around for the length of the animation. gpui at this revision has no
/// scale transform for divs (only svgs), so the reference's
/// `scale(0.96) → 1` is approximated by this short travel, which reads the
/// same at menu size.
///
/// `leave` is the surface's opacity on its way out ([`Leave`]), 1 while open.
pub fn menu_in<E>(id: impl Into<ElementId>, top: f32, leave: f32, element: E) -> AnimationElement<E>
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(id, menu_curve(), move |el, t| {
        el.opacity(t * leave)
            .top(px(top - MENU_IN_RISE * (1.0 - t)))
    })
}

/// Menus alone settle on a quint rather than [`EASE_OUT`]: nearly all of
/// their short travel lands in the first frames, so a menu is there the
/// moment it is asked for and only its last pixel eases.
fn menu_curve() -> Animation {
    Animation::new(Duration::from_millis(MENU_IN_MS)).with_easing(ease_out_quint())
}

/// Once-per-frame tick from the root render. Returns true while any fade is
/// still in flight, so the caller can request another frame.
pub fn tick_hover_fades() -> bool {
    HOVER_FADES.with(|fades| fades.borrow_mut().tick_at(Instant::now()))
}

#[cfg(test)]
mod tests;
