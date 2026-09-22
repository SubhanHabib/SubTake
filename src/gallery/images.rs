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
                    // A pale inset card reads as the recorded window.
                    if (0.12..0.88).contains(&fx) && (0.14..0.86).contains(&fy) {
                        c = lerp(c, [0xf4, 0xf4, 0xf7], 0.85);
                        if fy < 0.2 {
                            c = lerp(c, [0xd8, 0xd8, 0xde], 0.5);
                        }
                    }
                    (c, 255)
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

pub(super) fn lerp(from: [u8; 3], to: [u8; 3], t: f32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (from[i] as f32 + (to[i] as f32 - from[i] as f32) * t)
            .round()
            .clamp(0., 255.) as u8;
    }
    out
}
