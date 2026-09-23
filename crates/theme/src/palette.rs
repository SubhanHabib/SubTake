//! The colour tokens of the light and dark palettes.
//!
//! Alpha is part of every surface tone: these are tints over the window's
//! vibrancy material, not paints. Both palettes carry the same key set.
//!
//! Values come from the "Stage" redesign handoff. The source authored the
//! accent and the record red in OKLCH, so the OKLCH is quoted beside each one
//! and the sRGB below it is what that converts to — the three the handoff
//! also gave as hex (`accent`, dark `accent`, `rec`) round-trip exactly, so
//! the derived hover and press steps are computed the same way.

use std::sync::LazyLock;

use super::Theme;
use super::appearance::Appearance;
use super::css::css;

/// The palettes, parsed from their CSS tokens on first use.
pub(super) static LIGHT: LazyLock<Theme> = LazyLock::new(Theme::build_light);
pub(super) static DARK: LazyLock<Theme> = LazyLock::new(Theme::build_dark);

impl Theme {
    pub(super) fn build_light() -> Self {
        Self {
            appearance: Appearance::Light,

            // Not drawn by the design: the shell at 40%, and the floats at
            // about half the handoff's alpha, so the desktop shows through.
            bg: css("#fafaf966"),
            glass: css("#ffffff46"),
            card: css("#ffffff5a"),
            sunk: css("#ececeeb8"),
            sunk2: css("#ffffffe6"),
            raise: css("#ffffffc7"),
            raise_line: css("#14141a14"),
            seg_active: css("#ffffff"),
            slider_fill: css("#14141a0f"),
            plate: css("#ffffff8c"),
            switch_on: css("#5b5d65"),

            hover: css("#14141a0d"),
            press: css("#14141a1a"),

            text: css("#1b1c20"),
            muted: css("#4c4e55"),
            line: css("#14141a17"),

            // oklch(0.56 0.18 258)
            accent: css("#2370db"),
            // oklch(0.50 0.18 258)
            accent_hover: css("#085dc7"),
            // oklch(0.45 0.18 258)
            accent_press: css("#004eb6"),
            accent_soft: css("#2370db24"),
            on_accent: css("#ffffff"),
            // oklch(0.56 0.18 258 / 0.35)
            accent_glow: css("#2370db59"),

            ink: css("#1b1c20"),
            on_ink: css("#ffffff"),

            // oklch(0.58 0.20 22) — identical in both themes, so white on it
            // stays at 4.7:1 either way.
            rec: css("#d73240"),
            danger: css("#b30023"),
        }
    }

    pub(super) fn build_dark() -> Self {
        Self {
            appearance: Appearance::Dark,

            // Not drawn by the design: as the light shell and floats, but
            // `glass` at 41% — at half the handoff's it read too thin on dark.
            bg: css("#0d0d1066"),
            glass: css("#1e1e2469"),
            card: css("#22222964"),
            // Recesses lighten on dark: a darkening under `card` was darker
            // than the card and the plates on it vanished.
            sunk: css("#ffffff0d"),
            sunk2: css("#ffffff1a"),
            raise: css("#ffffff1a"),
            raise_line: css("#ffffff24"),
            seg_active: css("#ffffff24"),
            slider_fill: css("#ffffff12"),
            plate: css("#ffffff1f"),
            switch_on: css("#6e707a"),

            hover: css("#ffffff12"),
            press: css("#ffffff1f"),

            text: css("#f2f2f4"),
            muted: css("#9b9ca5"),
            line: css("#ffffff1a"),

            // oklch(0.72 0.15 258)
            accent: css("#66a5ff"),
            // oklch(0.78 0.14 258)
            accent_hover: css("#7db9ff"),
            // oklch(0.84 0.12 258)
            accent_press: css("#99cdff"),
            accent_soft: css("#66a5ff2e"),
            on_accent: css("#0c1022"),
            // oklch(0.72 0.15 258 / 0.45)
            accent_glow: css("#66a5ff73"),

            ink: css("#f2f2f4"),
            on_ink: css("#17171a"),

            rec: css("#d73240"),
            danger: css("#ff6b7f"),
        }
    }
}
