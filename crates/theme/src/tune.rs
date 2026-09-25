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
//! The colour tokens are tunable too, per appearance: [`Theme::light`] and
//! [`Theme::dark`] hand back their palette with any override laid over it
//! (see [`tinted`]), so every window that builds its theme each frame follows.
//! A field read is not a call, so colour reads are not recorded.
//!
//! Overrides are never saved. The constants in `metrics.rs` stay the source of
//! truth, with `palette.rs` for the colours; [`as_rust`] prints what changed
//! so it can be pasted back there.
//!
//! Not drawn by the design: this is a gallery tool.

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
    sync::{
        LazyLock, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use gpui::{Hsla, Rgba};

use super::{
    Appearance, Theme,
    css::parse_css,
    palette::{DARK, LIGHT},
};

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

/// One tunable colour token: a field of [`Theme`].
#[derive(Debug)]
pub struct Colour {
    pub name: &'static str,
    /// The `// ---- heading ----` it sits under in `struct Theme`.
    pub group: &'static str,
    /// The first sentence of its doc comment, or empty.
    pub doc: &'static str,
    get: fn(&Theme) -> Hsla,
    set: fn(&mut Theme, Hsla),
}

impl Colour {
    /// Its value in `palette.rs`.
    pub fn default(&self, appearance: Appearance) -> Hsla {
        (self.get)(palette(appearance))
    }

    /// Its value now: the override, or its value in `palette.rs`.
    pub fn value(&self, appearance: Appearance) -> Hsla {
        let pinned = tints().get(&(appearance.is_dark(), self.name)).copied();
        pinned.unwrap_or_else(|| self.default(appearance))
    }

    pub fn overridden(&self, appearance: Appearance) -> bool {
        tints().contains_key(&(appearance.is_dark(), self.name))
    }
}

/// The set a [`record`] call collects into.
pub type Reads = Rc<RefCell<BTreeSet<&'static str>>>;

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Showing `metrics.rs` and `palette.rs` as written, overrides kept aside.
static ORIGINAL: AtomicBool = AtomicBool::new(false);
static OVERRIDES: LazyLock<Mutex<HashMap<&'static str, f32>>> = LazyLock::new(Default::default);
/// Colour overrides, keyed by whether they tint the dark palette.
static TINTS: LazyLock<Mutex<HashMap<(bool, &'static str), Hsla>>> =
    LazyLock::new(Default::default);
/// Bumped by every change, so a view holding a built theme knows to rebuild.
static GENERATION: AtomicU64 = AtomicU64::new(0);

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
    changed_now();
}

pub fn reset(name: &str) {
    overrides().remove(name);
    changed_now();
}

/// Show the metrics and palette as the files write them — the before of a
/// before and after — keeping every override to come back to. Any new
/// override ends it, so what is being tuned is what is on show.
pub fn show_original(on: bool) {
    ORIGINAL.store(on, Ordering::Relaxed);
    bump();
}

pub fn showing_original() -> bool {
    ORIGINAL.load(Ordering::Relaxed)
}

/// Every tunable colour, in `struct Theme` order.
pub fn colours() -> &'static [Colour] {
    COLOURS
}

pub fn colour(name: &str) -> Option<&'static Colour> {
    COLOURS.iter().find(|colour| colour.name == name)
}

pub fn set_colour(appearance: Appearance, name: &'static str, value: Hsla) {
    tints().insert((appearance.is_dark(), name), value);
    changed_now();
}

pub fn reset_colour(appearance: Appearance, name: &str) {
    tints().retain(|&(dark, key), _| dark != appearance.is_dark() || key != name);
    changed_now();
}

/// Drop every override, sizes and colours alike.
pub fn reset_all() {
    overrides().clear();
    tints().clear();
    changed_now();
}

/// How many metrics and colours carry an override.
pub fn changed() -> usize {
    overrides().len() + tints().len()
}

/// A count that moves whenever an override does.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// A colour as CSS writes it — `hsla(…)`, `hsl(…)` or hex — or `None`.
pub fn parse_colour(text: &str) -> Option<Hsla> {
    parse_css(text)
}

/// `#rrggbb`, or `#rrggbbaa` when it is not opaque: how `palette.rs` writes it.
pub fn hex(colour: Hsla) -> String {
    let Rgba { r, g, b, a } = colour.into();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    let mut out = format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b));
    if byte(a) != 0xff {
        out.push_str(&format!("{:02x}", byte(a)));
    }
    out
}

/// `theme` with the colour overrides for its appearance laid over it.
pub(crate) fn tinted(mut theme: Theme) -> Theme {
    if !ENABLED.load(Ordering::Relaxed) {
        return theme;
    }
    if showing_original() {
        return theme;
    }
    let dark = theme.appearance.is_dark();
    for (&(tint_dark, name), &value) in tints().iter() {
        if tint_dark == dark
            && let Some(colour) = colour(name)
        {
            (colour.set)(&mut theme, value);
        }
    }
    theme
}

fn palette(appearance: Appearance) -> &'static Theme {
    match appearance {
        Appearance::Dark => &DARK,
        Appearance::Light => &LIGHT,
    }
}

fn bump() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// An override moved: back to showing the overrides, and a rebuild.
fn changed_now() {
    ORIGINAL.store(false, Ordering::Relaxed);
    bump();
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

/// One override, as the catalogue lists it: what it was, what it is now,
/// and the line to paste over the old one.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub name: &'static str,
    /// Where `line` goes: `palette.rs, Theme::build_dark`, or `metrics.rs`.
    pub file: &'static str,
    pub value: ChangeValue,
    pub line: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChangeValue {
    Colour {
        appearance: Appearance,
        from: Hsla,
        to: Hsla,
    },
    Size {
        from: f32,
        to: f32,
    },
}

/// Every override: the colours under the palette each tints, then the
/// metrics, each in its file's own order.
pub fn changes() -> Vec<Change> {
    let mut out = Vec::new();
    let tints = tints();
    for (appearance, file) in [
        (Appearance::Light, "palette.rs, Theme::build_light"),
        (Appearance::Dark, "palette.rs, Theme::build_dark"),
    ] {
        for colour in COLOURS {
            let Some(&to) = tints.get(&(appearance.is_dark(), colour.name)) else {
                continue;
            };
            out.push(Change {
                name: colour.name,
                file,
                value: ChangeValue::Colour {
                    appearance,
                    from: colour.default(appearance),
                    to,
                },
                line: format!("{}: css(\"{}\"),", colour.name, hex(to)),
            });
        }
    }
    let overrides = overrides();
    for metric in METRICS {
        let Some(&to) = overrides.get(metric.name) else {
            continue;
        };
        let mut line = String::new();
        if metric.derived {
            line.push_str("// Derived in metrics.rs; tuned to a fixed value.\n");
        }
        line.push_str(&format!("pub const {}: f32 = {to:?};", metric.name));
        out.push(Change {
            name: metric.name,
            file: "metrics.rs",
            value: ChangeValue::Size {
                from: metric.default,
                to,
            },
            line,
        });
    }
    out
}

/// Every override as lines to paste, under a comment naming the file and
/// palette each group belongs to.
pub fn as_rust() -> String {
    let mut out = String::new();
    let mut file = "";
    for change in changes() {
        if change.file != file {
            file = change.file;
            out.push_str(&format!("// {file}\n"));
        }
        out.push_str(&change.line);
        out.push('\n');
    }
    out
}

/// Every override as a theme file writes it: `light.accent=#ff6a00`,
/// `dark.card=#22222980` and `GAP_SMALL=5`, one to a line.
pub fn as_text() -> String {
    let mut out = String::new();
    for change in changes() {
        let line = match change.value {
            ChangeValue::Colour { appearance, to, .. } => {
                let palette = if appearance.is_dark() {
                    "dark"
                } else {
                    "light"
                };
                format!("{palette}.{}={}", change.name, hex(to))
            }
            ChangeValue::Size { to, .. } => format!("{}={to}", change.name),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Replace every override with those `text` lists, as `as_text` writes
/// them; commas separate entries as well as lines, and `#` starts a comment
/// line. Returns each entry it could not read, leaving the rest applied.
pub fn load_text(text: &str) -> Vec<String> {
    let mut unread = Vec::new();
    let mut sizes = HashMap::new();
    let mut colours = HashMap::new();
    for entry in text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let Some((name, value)) = entry.split_once('=') else {
            unread.push(entry.to_owned());
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        let colour = name.split_once('.').and_then(|(palette, name)| {
            let dark = match palette {
                "light" => false,
                "dark" => true,
                _ => return None,
            };
            Some((dark, colour(name)?.name, parse_colour(value)?))
        });
        if let Some((dark, name, value)) = colour {
            colours.insert((dark, name), value);
        } else if let Some((metric, value)) = metric(name).zip(value.parse::<f32>().ok()) {
            sizes.insert(metric.name, value);
        } else {
            unread.push(entry.to_owned());
        }
    }
    *overrides() = sizes;
    *tints() = colours;
    changed_now();
    unread
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
    if showing_original() {
        return formula();
    }
    let pinned = overrides().get(name).copied();
    pinned.unwrap_or_else(formula)
}

fn tints() -> MutexGuard<'static, HashMap<(bool, &'static str), Hsla>> {
    TINTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn overrides() -> MutexGuard<'static, HashMap<&'static str, f32>> {
    OVERRIDES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
