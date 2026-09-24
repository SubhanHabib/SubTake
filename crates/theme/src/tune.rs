//! Live overrides for the metrics, for the component catalogue.
//!
//! Every `f32` metric has a reader generated beside it (`Theme::GAP_SMALL` →
//! `Theme::gap_small()`, see `build.rs`). Until [`enable`] is called a reader
//! is the constant and nothing more. Once the catalogue enables tuning, a
//! reader returns its override if one is set and otherwise its own formula, so
//! a derived size follows the sizes it is built from. Reads made inside
//! [`record`] are collected, which is how the catalogue learns which metrics a
//! component uses without a list kept by hand.
//!
//! Overrides are never saved. The constants in `metrics.rs` stay the source of
//! truth; [`as_rust`] prints what changed so it can be pasted back there.
//!
//! Not drawn by the design: this is a gallery tool.

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
    sync::{
        LazyLock, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use super::Theme;

include!(concat!(env!("OUT_DIR"), "/tuned.rs"));

/// One tunable metric.
#[derive(Debug)]
pub struct Metric {
    pub name: &'static str,
    /// The `// ---- heading ----` it sits under in `metrics.rs`.
    pub group: &'static str,
    /// The first sentence of its doc comment, or empty.
    pub doc: &'static str,
    pub default: f32,
    /// Whether `metrics.rs` writes it in terms of other metrics.
    pub derived: bool,
    read: fn() -> f32,
}

impl Metric {
    /// Its value now: the override, or its formula over the current values.
    pub fn value(&self) -> f32 {
        (self.read)()
    }

    pub fn overridden(&self) -> bool {
        overrides().contains_key(self.name)
    }
}

/// The set a [`record`] call collects into.
pub type Reads = Rc<RefCell<BTreeSet<&'static str>>>;

static ENABLED: AtomicBool = AtomicBool::new(false);
static OVERRIDES: LazyLock<Mutex<HashMap<&'static str, f32>>> = LazyLock::new(Default::default);

thread_local! {
    /// The innermost [`record`] or [`unrecorded`] call; `None` for the latter.
    static RECORDING: RefCell<Vec<Option<Reads>>> = const { RefCell::new(Vec::new()) };
}

/// Every tunable metric, in `metrics.rs` order.
pub fn metrics() -> &'static [Metric] {
    METRICS
}

pub fn metric(name: &str) -> Option<&'static Metric> {
    METRICS.iter().find(|metric| metric.name == name)
}

/// Turn tuning on for the rest of the process.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn set(name: &'static str, value: f32) {
    overrides().insert(name, value);
}

pub fn reset(name: &str) {
    overrides().remove(name);
}

pub fn reset_all() {
    overrides().clear();
}

/// How many metrics carry an override.
pub fn changed() -> usize {
    overrides().len()
}

/// Run `f`, adding the name of every metric it reads to `reads`.
pub fn record<R>(reads: &Reads, f: impl FnOnce() -> R) -> R {
    recording(Some(reads.clone()), f)
}

/// Run `f` without adding its reads to the [`record`] call around it: the
/// catalogue's own chrome, which is not the component being tuned.
pub fn unrecorded<R>(f: impl FnOnce() -> R) -> R {
    recording(None, f)
}

fn recording<R>(reads: Option<Reads>, f: impl FnOnce() -> R) -> R {
    RECORDING.with(|stack| stack.borrow_mut().push(reads));
    let result = f();
    RECORDING.with(|stack| stack.borrow_mut().pop());
    result
}

/// The overrides as `metrics.rs` lines, in `metrics.rs` order.
pub fn as_rust() -> String {
    let overrides = overrides();
    let mut out = String::new();
    for metric in METRICS {
        let Some(value) = overrides.get(metric.name) else {
            continue;
        };
        if metric.derived {
            out.push_str("// Derived in metrics.rs; tuned to a fixed value.\n");
        }
        out.push_str(&format!("pub const {}: f32 = {value:?};\n", metric.name));
    }
    out
}

/// A generated reader's body.
#[inline]
pub(crate) fn read(name: &'static str, default: f32, formula: impl FnOnce() -> f32) -> f32 {
    if !ENABLED.load(Ordering::Relaxed) {
        return default;
    }
    RECORDING.with(|stack| {
        if let Some(Some(reads)) = stack.borrow().last() {
            reads.borrow_mut().insert(name);
        }
    });
    let pinned = overrides().get(name).copied();
    pinned.unwrap_or_else(formula)
}

fn overrides() -> MutexGuard<'static, HashMap<&'static str, f32>> {
    OVERRIDES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
