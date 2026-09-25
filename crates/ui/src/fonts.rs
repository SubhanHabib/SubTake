//! Bundled interface typography.
//!
//! The reference ships Geist and Geist Mono as crate assets and registers them
//! with gpui's text system at startup (`zeronsh/zeron`,
//! `crates/ui/src/typography.rs`, MIT © 2026 Wing). Embedding the faces means
//! the interface renders identically whether or not the host has Geist
//! installed, and needs no resource-bundle step.
//!
//! Geist and Geist Mono are © 2024 The Geist Project Authors, SIL Open Font
//! License 1.1 — see `assets/fonts/licenses/Geist-OFL.txt`. Space Grotesk is
//! © 2020 The Space Grotesk Project Authors, same licence — see
//! `assets/fonts/licenses/SpaceGrotesk-OFL.txt`. The redesign uses it for
//! titles only and only at weight 500, so that is the one face bundled.

use gpui::App;
use std::borrow::Cow;

const GEIST: [&[u8]; 8] = [
    include_bytes!("../../../assets/fonts/Geist.ttf"),
    include_bytes!("../../../assets/fonts/Geist-Italic.ttf"),
    include_bytes!("../../../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Geist-MediumItalic.ttf"),
    include_bytes!("../../../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/Geist-SemiBoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/Geist-Bold.ttf"),
    include_bytes!("../../../assets/fonts/Geist-BoldItalic.ttf"),
];

const GEIST_MONO: [&[u8]; 4] = [
    include_bytes!("../../../assets/fonts/GeistMono.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Bold.ttf"),
];

const SPACE_GROTESK: [&[u8]; 1] = [include_bytes!(
    "../../../assets/fonts/SpaceGrotesk-Medium.ttf"
)];

/// Geist and Geist Mono at 500, the faces the area overlay's chips and
/// buttons are drawn in natively.
pub fn medium_faces() -> (&'static [u8], &'static [u8]) {
    (GEIST[2], GEIST_MONO[1])
}

/// Register the bundled faces. Safe to call more than once; gpui's text system
/// treats a repeat registration of the same face as a no-op.
pub fn register(cx: &App) {
    let faces = GEIST
        .iter()
        .chain(GEIST_MONO.iter())
        .chain(SPACE_GROTESK.iter())
        .map(|face| Cow::Borrowed(*face))
        .collect();
    if let Err(error) = cx.text_system().add_fonts(faces) {
        eprintln!("SubTake: failed to register bundled fonts: {error}");
    }
}

/// Whether the bundled families resolved — used by the UI smoke checks so a
/// silent fallback to the system face cannot pass as fidelity. This passed
/// vacuously while `FONT_SANS` named a system face: it asked whether the host
/// had Helvetica, which it always does.
pub fn families_available(cx: &App) -> (bool, bool, bool) {
    let names = cx.text_system().all_font_names();
    let has = |family: &str| names.iter().any(|n| n == family);
    (
        has(subtake_theme::FONT_SANS),
        has(subtake_theme::FONT_MONO),
        has(subtake_theme::FONT_TITLE),
    )
}
