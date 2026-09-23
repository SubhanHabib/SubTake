//! Procedural preview, strip and waveform imagery so no media file is needed.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum Style {
    /// Diagonal two-tone gradient with a lighter "window" card in the middle.
    Preview,
    /// Horizontal gradient chopped into frame-like cells.
    Strip,
    /// A symmetrical pseudo-random waveform on transparent.
    Waveform,
}

pub(super) fn gradient(width: u32, height: u32, from: [u8; 3], to: [u8; 3], style: Style) -> Image {
    let mut bytes = vec![0u8; (width * height * 4) as usize];
    let mut seed = 0x9e37_79b9u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 1000) as f32 / 1000.
    };
    let columns: Vec<f32> = (0..width).map(|_| noise()).collect();
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let (rgb, alpha) = match style {
                Style::Preview => {
                    let t = (fx * 0.7 + fy * 0.3).clamp(0., 1.);
                    let mut c = lerp(from, to, t);
                    // A pale inset card reads as the recorded window, drawn
                    // as the renderer composes the source: rounded, and
                    // casting a soft shadow onto the background below it.
                    let (w, h) = (width as f32, height as f32);
                    let card = (0.12 * w, 0.14 * h, 0.88 * w, 0.86 * h);
                    let inside = rounded_distance(x as f32, y as f32, card, CARD_RADIUS);
                    let below =
                        rounded_distance(x as f32, y as f32 - SHADOW_OFFSET, card, CARD_RADIUS);
                    let shadow = SHADOW_ALPHA * (-(below.max(0.) / SHADOW_BLUR).powi(2)).exp();
                    c = lerp(c, [0, 0, 0], shadow);
                    // A pixel of coverage across the edge keeps the corners
                    // smooth rather than stepped.
                    let cover = (0.5 - inside).clamp(0., 1.);
                    let mut window = lerp(c, [0xf4, 0xf4, 0xf7], 0.85);
                    if y as f32 - card.1 < 0.06 * h {
                        window = lerp(window, [0xd8, 0xd8, 0xde], 0.5);
                    }
                    (lerp(c, window, cover), 255)
                }
                Style::Strip => {
                    let cell = (fx * 24.).floor() / 24.;
                    let mut c = lerp(from, to, cell);
                    if (x % (width / 24).max(1)) < 2 {
                        c = [0x10, 0x10, 0x14];
                    }
                    (c, 255)
                }
                Style::Waveform => {
                    let amp =
                        0.15 + 0.8 * (columns[x as usize] * (0.5 + 0.5 * (fx * 12.).sin().abs()));
                    let inside = (fy - 0.5).abs() * 2. < amp;
                    (from, if inside { 255 } else { 0 })
                }
            };
            bytes[i..i + 3].copy_from_slice(&rgb);
            bytes[i + 3] = alpha;
        }
    }
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &bytes, width, height,
    ))
}

/// A thumbnail's fake source: its corner radius, shadow drop, shadow spread
/// and shadow strength, in thumbnail pixels.
const CARD_RADIUS: f32 = 9.;
const SHADOW_OFFSET: f32 = 6.;
const SHADOW_BLUR: f32 = 16.;
const SHADOW_ALPHA: f32 = 0.4;

/// The editor's stage as a new recording opens: the Apricot wallpaper, the
/// source inset by padding 56 with radius 4, the three tinted shadow layers
/// at 50% and the hairline inside the frame's edge, all as the renderer
/// draws them at `width` over 1920. The recorded window is a pale card.
pub(super) fn scene(width: u32, height: u32) -> Image {
    const STOPS: [[u8; 3]; 3] = [[0xf7, 0xdc, 0xc2], [0xed, 0xa8, 0x8f], [0xd2, 0x7b, 0x86]];
    const TINT: [u8; 3] = [0x5c, 0x22, 0x24];
    const EDGE: [u8; 3] = [0x46, 0x14, 0x14];
    const EDGE_ALPHA: f32 = 0x1a as f32 / 255.;
    const SHADOW: f32 = 0.5;
    // Drop, blur, inset and strength per layer, in 1920-wide pixels.
    const LAYERS: [(f32, f32, f32, f32); 3] = [
        (2., 2., 0., 0.28),
        (18., 22., 8., 0.44),
        (50., 60., 20., 0.40),
    ];
    let (w, h) = (width as f32, height as f32);
    let unit = w / 1920.;
    let pad = 56. * unit;
    let radius = 4. * unit;
    // The source is 16:9 like the frame, fitted into what the padding leaves.
    let fit = ((w - 2. * pad) / 16.).min((h - 2. * pad) / 9.);
    let (cw, ch) = (16. * fit, 9. * fit);
    let card = ((w - cw) / 2., (h - ch) / 2., (w + cw) / 2., (h + ch) / 2.);
    let mut bytes = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let (px, py) = (x as f32, y as f32);
            // 135°: top left to bottom right, along the diagonal.
            let t = ((px + py) / (w + h)).clamp(0., 1.);
            let mut c = if t < 0.5 {
                lerp(STOPS[0], STOPS[1], t * 2.)
            } else {
                lerp(STOPS[1], STOPS[2], t * 2. - 1.)
            };
            for (down, sigma, inset, alpha) in LAYERS {
                let (inset, sigma) = (inset * unit, (sigma * unit).max(0.5));
                let layer = (
                    card.0 + inset,
                    card.1 + inset,
                    card.2 - inset,
                    card.3 - inset,
                );
                let d = rounded_distance(px, py - down * unit, layer, radius);
                // A blurred edge's coverage: the normal CDF, logistic-fitted.
                let cover = 1. / (1. + (1.702 * d / sigma).exp());
                c = lerp(c, TINT, SHADOW * alpha * cover);
            }
            let inside = rounded_distance(px, py, card, radius);
            let mut window = [0xf6, 0xf3, 0xf1];
            if py - card.1 < 0.06 * h {
                window = [0xe6, 0xe0, 0xdd];
            }
            if inside > -3. * unit {
                window = lerp(window, EDGE, EDGE_ALPHA);
            }
            let rgb = lerp(c, window, (0.5 - inside).clamp(0., 1.));
            let i = ((y * width + x) * 4) as usize;
            bytes[i..i + 3].copy_from_slice(&rgb);
            bytes[i + 3] = 255;
        }
    }
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &bytes, width, height,
    ))
}

/// How far a point is outside a rounded rectangle (negative inside).
fn rounded_distance(
    x: f32,
    y: f32,
    (left, top, right, bottom): (f32, f32, f32, f32),
    r: f32,
) -> f32 {
    let dx = (left + r - x).max(x - (right - r)).max(0.);
    let dy = (top + r - y).max(y - (bottom - r)).max(0.);
    let outside = (dx * dx + dy * dy).sqrt();
    let inner = (left + r - x)
        .max(x - (right - r))
        .max((top + r - y).max(y - (bottom - r)));
    if dx > 0. || dy > 0. {
        outside - r
    } else {
        inner.min(0.) - r
    }
}

pub(super) fn lerp(from: [u8; 3], to: [u8; 3], t: f32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (from[i] as f32 + (to[i] as f32 - from[i] as f32) * t)
            .round()
            .clamp(0., 255.) as u8;
    }
    out
}
