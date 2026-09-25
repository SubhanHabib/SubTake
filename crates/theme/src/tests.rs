use super::*;

#[test]
fn surfaces_keep_the_alpha_they_were_authored_with() {
    // The frosted look lives in these alphas; an opaque token would hide
    // the vibrancy material the whole palette is designed to sit on.
    let light = Theme::light();
    assert!(light.bg.a < 1.0, "window background must stay translucent");
    assert!(
        light.glass.a < 1.0,
        "floating controls must stay translucent"
    );
    assert!(
        light.card.a < 1.0,
        "floating text surfaces must stay translucent"
    );
    assert!(light.sunk.a < 1.0, "recesses must stay translucent");
    assert_eq!(light.text.a, 1.0, "text stays fully opaque");

    let dark = Theme::dark();
    assert!(dark.bg.a < 1.0);
    assert!(dark.glass.a < 1.0);
    assert!(dark.card.a < 1.0);
}

#[test]
fn the_type_and_icon_scales_are_closed() {
    // Every size the interface may use, and nothing between them.
    let fonts = [
        Theme::FONT_SMALL,
        Theme::FONT_SECONDARY,
        Theme::FONT_BODY,
        Theme::FONT_ACTION,
        Theme::FONT_HEADING,
        Theme::FONT_PANEL,
        Theme::FONT_DISPLAY,
    ];
    assert!(fonts.windows(2).all(|w| w[0] < w[1]), "scale must ascend");
    assert_eq!(Theme::FONT_BODY, Theme::FONT_CONTROL, "one reading size");
    let icons = [
        Theme::ICON_SIZE_SMALL,
        Theme::ICON_SIZE,
        Theme::ICON_SIZE_MEDIUM,
        Theme::ICON_SIZE_LARGE,
    ];
    assert!(icons.windows(2).all(|w| w[0] < w[1]));
    // A glyph has to leave room inside the control it sits in.
    assert!(Theme::ICON_SIZE_LARGE < Theme::CONTROL_HEIGHT_SMALL);
}

#[test]
fn the_radius_ladder_leaves_its_gap() {
    // Nothing lands between 14 and 20: that gap is what keeps a container
    // and its contents readable as separate layers.
    let ladder = [
        Theme::RADIUS_REGION,
        Theme::RADIUS_LANE,
        Theme::RADIUS_INNER,
        Theme::RADIUS_MENU,
        Theme::RADIUS_ROW,
        Theme::RADIUS_FRAME,
        Theme::RADIUS_PLATE,
        Theme::RADIUS_PANEL,
        Theme::RADIUS_POD,
        Theme::RADIUS_BAR,
    ];
    assert!(ladder.windows(2).all(|w| w[0] < w[1]), "ladder must ascend");
    assert!(
        !ladder
            .iter()
            .any(|r| *r > Theme::RADIUS_INNER && *r < Theme::RADIUS_MENU),
        "the 14-to-20 gap must stay empty"
    );
}

#[test]
fn a_control_height_picks_its_own_glyph_and_padding() {
    // The four button heights ascend, and each leaves room for the glyph
    // that pairs with it. Nothing sets a glyph size or a padding directly.
    let heights = [
        Theme::CONTROL_HEIGHT_SMALL,
        Theme::CONTROL_HEIGHT,
        Theme::CONTROL_HEIGHT_LARGE,
        Theme::CONTROL_HEIGHT_HERO,
    ];
    assert!(heights.windows(2).all(|w| w[0] < w[1]));
    for (height, glyph) in heights.iter().zip([
        Theme::ICON_SIZE_SMALL,
        Theme::ICON_SIZE,
        Theme::ICON_SIZE_MEDIUM,
        Theme::ICON_SIZE_LARGE,
    ]) {
        assert!(glyph * 2.0 < *height, "a glyph must sit inside its control");
    }
    // Record is the tallest thing in the app, and the transport the largest
    // round one: neither may be mistaken for an ordinary button.
    assert!(Theme::RECORD_HEIGHT > Theme::CONTROL_HEIGHT_HERO);
    assert!(Theme::TRANSPORT_SIZE > Theme::CONTROL_HEIGHT_LARGE);
}

#[test]
fn the_toggle_thumb_travels_the_width_it_is_given() {
    // The track, the inset and the thumb derive one from the other, so the
    // travel cannot drift away from the geometry that produces it.
    assert_eq!(Theme::TOGGLE_TRAVEL, 18.0);
    assert_eq!(
        Theme::TOGGLE_INSET * 2.0 + Theme::TOGGLE_THUMB,
        Theme::TOGGLE_HEIGHT,
        "the thumb must fit its track exactly"
    );
    assert!(Theme::TOGGLE_THUMB_HELD > Theme::TOGGLE_THUMB);
}

#[test]
fn the_accent_is_the_only_hue_in_the_interface() {
    // Everything that is not the accent, the record red or the glyph on
    // one of them takes its colour from the desktop behind the glass. The
    // greys carry a trace of blue so they sit on that glass rather than
    // fighting it, which is what the loose bound below allows for.
    for t in [Theme::light(), Theme::dark()] {
        for (name, colour) in [
            ("bg", t.bg),
            ("glass", t.glass),
            ("card", t.card),
            ("dialog", t.dialog),
            ("sunk", t.sunk),
            ("sunk2", t.sunk2),
            ("seg_active", t.seg_active),
            ("slider_fill", t.slider_fill),
            ("plate", t.plate),
            ("switch_on", t.switch_on),
            ("raise", t.raise),
            ("text", t.text),
            ("muted", t.muted),
            ("line", t.line),
            ("ink", t.ink),
        ] {
            assert!(colour.s < 0.2, "{name} must read as grey, not as a colour");
        }
        for (name, colour) in [("accent", t.accent), ("rec", t.rec)] {
            assert!(colour.s > 0.6, "{name} must read as a colour, not as grey");
        }
    }
}

#[test]
fn the_filled_plate_inverts_with_the_appearance() {
    // `ink` is the achromatic filled plate — the transport button, the
    // active segment — so filled means near-black on light and near-white
    // on dark, with the glyph on it taking the opposite end.
    let light = Theme::light();
    assert!(light.ink.l < 0.2 && light.on_ink.l > 0.9);
    let dark = Theme::dark();
    assert!(dark.ink.l > 0.9 && dark.on_ink.l < 0.2);
}

#[test]
fn the_accent_steps_away_from_the_surface_it_sits_on() {
    // Hover and press deepen the blue on light and lift it on dark, so in
    // both appearances the plate separates further from its background
    // rather than sliding towards it.
    let light = Theme::light();
    assert!(light.accent_press.l < light.accent_hover.l);
    assert!(light.accent_hover.l < light.accent.l);
    let dark = Theme::dark();
    assert!(dark.accent_press.l > dark.accent_hover.l);
    assert!(dark.accent_hover.l > dark.accent.l);
    // The glyph on the accent contrasts with it in both.
    assert!(light.on_accent.l > light.accent.l);
    assert!(dark.on_accent.l < dark.accent.l);
}

#[test]
fn glass_is_on_so_the_translucent_tokens_composite() {
    assert!(Theme::WINDOW_GLASS_SUPPORTED);
    assert_eq!(
        Theme::light().window_background_appearance(),
        gpui::WindowBackgroundAppearance::Transparent
    );
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

#[test]
fn every_colour_token_is_tunable_and_prints_back_as_it_was_written() {
    // The catalogue's colour table is generated from `struct Theme`; a field
    // the generator missed could not be tuned, and a hex that did not parse
    // back to itself would paste a different colour into `palette.rs`.
    assert_eq!(tune::colours().len(), 35);
    for appearance in [Appearance::Light, Appearance::Dark] {
        for colour in tune::colours() {
            let written = tune::hex(colour.default(appearance));
            let parsed = tune::parse_colour(&written).expect("hex parses");
            assert_eq!(tune::hex(parsed), written, "{}", colour.name);
        }
    }
}
