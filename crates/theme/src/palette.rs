//! The colour tokens of the light and dark palettes.
//!
//! Alpha is part of every surface tone: these are tints over the window's
//! vibrancy material, not paints.

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
            bg: css("hsla(60, 9.1%, 97.8%, 0.4)"),
            header: css("hsla(0, 0%, 100%, 0.333)"),
            panel: css("hsla(0, 0%, 100%, 0.282)"),
            surface: css("hsla(0, 0%, 90.6%, 0.4)"),
            overlay: css("hsla(0, 0%, 98.8%, 0.969)"),
            popup: css("hsla(0, 0%, 98%, 0.8)"),
            overlay_shadow: css("hsla(0, 0%, 0%, 0.094)"),
            border: css("hsla(0, 0%, 13.3%, 0.11)"),
            text: css("hsla(225, 4.7%, 16.9%, 1)"),
            muted: css("hsla(223, 2.9%, 48%, 1)"),
            accent: css("hsla(0, 0%, 14.1%, 1)"),
            accent_hover: css("hsla(0, 0%, 23.1%, 1)"),
            accent_text: css("hsla(0, 0%, 14.1%, 1)"),
            selection: css("hsla(0, 0%, 0%, 0.102)"),
            on_accent: css("hsla(0, 0%, 100%, 1)"),
            hover: css("hsla(0, 0%, 0%, 0.047)"),
            input: css("hsla(0, 0%, 0%, 0.071)"),
            unchecked: css("hsla(0, 0%, 0%, 0.188)"),
            toggle_thumb: css("hsla(0, 0%, 100%, 1)"),
            danger: css("hsla(348, 75.3%, 47.6%, 1)"),
            warning: css("hsla(39, 76.6%, 61.4%, 1)"),
            canvas_dot: css("hsla(225, 4.7%, 16.9%, 0.075)"),
            track: css("hsla(60, 2.6%, 92.4%, 0.4)"),
            playhead: css("hsla(84, 2.2%, 44.9%, 1)"),
        }
    }

    pub(super) fn build_dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            bg: css("hsla(240, 5.6%, 7.1%, 0.533)"),
            header: css("hsla(240, 4.5%, 8.6%, 0.4)"),
            panel: css("hsla(240, 4.3%, 9%, 0.333)"),
            surface: css("hsla(240, 5.9%, 13.3%, 0.4)"),
            overlay: css("hsla(228, 8.2%, 12%, 0.969)"),
            popup: css("hsla(230, 8.6%, 13.7%, 0.8)"),
            overlay_shadow: css("hsla(0, 0%, 0%, 0.333)"),
            border: css("hsla(0, 0%, 100%, 0.133)"),
            text: css("hsla(240, 5.3%, 96.3%, 1)"),
            muted: css("hsla(240, 5.7%, 65.9%, 1)"),
            accent: css("hsla(0, 0%, 94.1%, 1)"),
            accent_hover: css("hsla(0, 0%, 100%, 1)"),
            accent_text: css("hsla(0, 0%, 94.1%, 1)"),
            selection: css("hsla(0, 0%, 100%, 0.122)"),
            on_accent: css("hsla(0, 0%, 9%, 1)"),
            hover: css("hsla(0, 0%, 100%, 0.055)"),
            input: css("hsla(0, 0%, 0%, 0.22)"),
            unchecked: css("hsla(0, 0%, 100%, 0.133)"),
            toggle_thumb: css("hsla(0, 0%, 100%, 1)"),
            danger: css("hsla(350, 89.2%, 60.2%, 1)"),
            warning: css("hsla(39, 76.6%, 61.4%, 1)"),
            canvas_dot: css("hsla(0, 0%, 100%, 0.063)"),
            track: css("hsla(0, 0%, 100%, 0.035)"),
            playhead: css("hsla(60, 1.2%, 83.7%, 1)"),
        }
    }
}
