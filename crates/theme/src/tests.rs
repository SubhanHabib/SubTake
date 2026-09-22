use super::*;

#[test]
fn surfaces_keep_the_alpha_they_were_authored_with() {
    // The frosted look lives in these alphas; an opaque token would hide
    // the vibrancy material the whole palette is designed to sit on.
    let light = Theme::light();
    assert!(light.bg.a < 1.0, "window background must stay translucent");
    assert!(light.panel.a < 1.0, "panels must stay translucent");
    assert!(
        light.surface.a < 1.0,
        "control plates must stay translucent"
    );
    assert_eq!(light.text.a, 1.0, "text stays fully opaque");

    let dark = Theme::dark();
    assert!(dark.bg.a < 1.0);
    assert!(dark.panel.a < 1.0);
}

#[test]
fn the_type_and_icon_scales_are_closed() {
    // Every size the interface may use, and nothing between them.
    let fonts = [
        Theme::FONT_SMALL,
        Theme::FONT_CONTROL,
        Theme::FONT_BODY,
        Theme::FONT_HEADING,
        Theme::FONT_DISPLAY,
    ];
    assert!(fonts.windows(2).all(|w| w[0] <= w[1]), "scale must ascend");
    assert_eq!(Theme::FONT_BODY, Theme::FONT_CONTROL, "one reading size");
    let icons = [
        Theme::ICON_SIZE_SMALL,
        Theme::ICON_SIZE,
        Theme::ICON_SIZE_LARGE,
    ];
    assert!(icons.windows(2).all(|w| w[0] < w[1]));
    // A glyph has to leave room inside the control it sits in.
    assert!(Theme::ICON_SIZE_LARGE < Theme::CONTROL_HEIGHT);
}

#[test]
fn control_geometry_is_unified() {
    assert_eq!(Theme::CONTROL_HEIGHT, 40.0);
    assert_eq!(Theme::RADIUS_CONTROL, 16.0);
    assert_eq!(Theme::ICON_SIZE, 16.0);
    // A 16px glyph centred in a 40px control.
    assert_eq!(Theme::CONTROL_PADDING, 12.0);
}

#[test]
fn every_control_colour_is_achromatic() {
    // The interface takes its colour from the desktop behind the glass,
    // so no control may introduce a hue of its own.
    for t in [Theme::light(), Theme::dark()] {
        for (name, colour) in [
            ("accent", t.accent),
            ("accent_hover", t.accent_hover),
            ("accent_text", t.accent_text),
            ("on_accent", t.on_accent),
            ("selection", t.selection),
            ("unchecked", t.unchecked),
            ("toggle_thumb", t.toggle_thumb),
        ] {
            assert_eq!(colour.s, 0.0, "{name} must be grey, not tinted");
        }
    }
}

#[test]
fn the_filled_plate_inverts_with_the_appearance() {
    // Filled means near-black on light and near-white on dark, with the
    // glyph on it taking the opposite end.
    let light = Theme::light();
    assert!(light.accent.l < 0.2 && light.on_accent.l > 0.9);
    let dark = Theme::dark();
    assert!(dark.accent.l > 0.9 && dark.on_accent.l < 0.2);
    // Hover lifts the plate away from the background in both.
    assert!(light.accent_hover.l > light.accent.l);
    assert!(dark.accent_hover.l > dark.accent.l);
}

#[test]
fn glass_is_on_so_the_translucent_tokens_composite() {
    assert!(Theme::WINDOW_GLASS_SUPPORTED);
    if !cfg!(target_os = "linux") {
        assert_eq!(
            Theme::light().window_background_appearance(),
            gpui::WindowBackgroundAppearance::Blurred
        );
    }
}

#[test]
fn appearance_preference_overrides_the_window() {
    assert_eq!(
        Appearance::resolve("dark", WindowAppearance::Light),
        Appearance::Dark
    );
    assert_eq!(
        Appearance::resolve("system", WindowAppearance::VibrantDark),
        Appearance::Dark
    );
}

#[test]
fn css_tokens_parse_in_both_notations() {
    let from_hex = parse_css("#fafaf966").expect("hex");
    let from_hsla = parse_css("hsla(60, 9.1%, 97.8%, 0.4)").expect("hsla");
    assert!((from_hex.h - from_hsla.h).abs() < 0.01);
    assert!((from_hex.l - from_hsla.l).abs() < 0.01);
    assert!((from_hex.a - from_hsla.a).abs() < 0.01);
    assert_eq!(parse_css("hsl(0, 0%, 100%)").map(|c| c.a), Some(1.));
    assert_eq!(parse_css("#fff").map(|c| (c.l, c.a)), Some((1., 1.)));
    assert!(parse_css("rgb(1, 2, 3)").is_none());
    assert!(
        parse_css("hsla(1, 2, 3, 4)").is_none(),
        "percent signs are required"
    );
}
