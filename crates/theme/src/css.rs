//! Colour tokens written in CSS notation so editors draw a swatch beside them.

use gpui::Hsla;

/// A colour written the way CSS writes it, so editor extensions that draw a
/// swatch next to CSS colours (Color Highlight and friends) draw one here too.
/// Accepts `hsla(240, 5.6%, 7.1%, 0.533)`, `hsl(240, 5.6%, 7.1%)`,
/// `#rrggbbaa`, `#rrggbb`, `#rgba` and `#rgb`.
///
/// The two palettes are parsed once into [`LIGHT`] and [`DARK`]; a malformed
/// token is a programming error caught the first time either theme is built,
/// which every theme test does.
pub(super) fn css(text: &str) -> Hsla {
    parse_css(text).unwrap_or_else(|| panic!("theme token is not a CSS colour: {text}"))
}

pub(super) fn parse_css(text: &str) -> Option<Hsla> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    let (name, rest) = text.split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    if name != "hsl" && name != "hsla" {
        return None;
    }
    let mut parts = inner.split(',').map(str::trim);
    let hue: f32 = parts.next()?.parse().ok()?;
    let saturation: f32 = parts.next()?.strip_suffix('%')?.parse().ok()?;
    let lightness: f32 = parts.next()?.strip_suffix('%')?.parse().ok()?;
    let alpha: f32 = match parts.next() {
        Some(alpha) => alpha.parse().ok()?,
        None => 1.,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(gpui::hsla(
        hue / 360.,
        saturation / 100.,
        lightness / 100.,
        alpha,
    ))
}

fn parse_hex(hex: &str) -> Option<Hsla> {
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    let channels: [u8; 4] = match digits.len() {
        3 | 4 => {
            let mut wide = [0xff; 4];
            for (i, d) in digits.iter().enumerate() {
                wide[i] = d * 17;
            }
            wide
        }
        6 | 8 => {
            let mut wide = [0xff; 4];
            for (i, pair) in digits.chunks(2).enumerate() {
                wide[i] = pair[0] * 16 + pair[1];
            }
            wide
        }
        _ => return None,
    };
    let [r, g, b, a] = channels.map(|v| v as f32 / 255.);
    Some(gpui::Rgba { r, g, b, a }.into())
}
