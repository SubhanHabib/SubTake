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

/// The fake source's corner radius, shadow drop, shadow spread and shadow
/// strength, in preview pixels: the gallery Scene's radius 18 and shadow 40%
/// at the preview's half scale.
const CARD_RADIUS: f32 = 9.;
const SHADOW_OFFSET: f32 = 6.;
const SHADOW_BLUR: f32 = 16.;
const SHADOW_ALPHA: f32 = 0.4;

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
