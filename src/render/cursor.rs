//! Cursor sprites: loading the styled asset and drawing it at the recorded position.

use super::*;

impl Scene {
    pub(super) fn cursor_asset(
        &mut self,
        style: &str,
        kind: &str,
    ) -> Result<(sk::Image, f32, f32)> {
        let key = format!("{style}/{kind}");
        if let Some(asset) = self.cursor_assets.get(&key) {
            return Ok(asset.clone());
        }
        let manifest: Value = serde_json::from_str(include_str!("../../assets/cursors.json"))?;
        let entry = if style == "figma" {
            serde_json::json!({"path":"src/assets/cursors/custom/minimal-cursor.svg","x":0.1,"y":0.05})
        } else {
            let set = if style == "tahoe-inverted" {
                "tahoe"
            } else {
                style
            };
            manifest[set]
                .get(kind)
                .or_else(|| manifest[set].get("arrow"))
                .unwrap_or(&manifest["tahoe"]["arrow"])
                .clone()
        };
        let original = self.asset(
            entry["path"]
                .as_str()
                .context("Missing cursor asset path")?,
        )?;
        let (width, height) = (original.width() as usize, original.height() as usize);
        let mut bytes = vec![0; width * height * 4];
        ensure!(
            original.read_pixels(
                &ImageInfo::new(
                    (width as i32, height as i32),
                    ColorType::RGBA8888,
                    AlphaType::Unpremul,
                    None
                ),
                &mut bytes,
                width * 4,
                (0, 0),
                sk::image::CachingHint::Allow
            ),
            "Read cursor asset"
        );
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0, 0);
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 4;
                if bytes[offset + 3] > 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
                if style == "tahoe-inverted" {
                    for channel in 0..3 {
                        bytes[offset + channel] = 255 - bytes[offset + channel];
                    }
                }
            }
        }
        if entry["preserveCanvas"].as_bool().unwrap_or(false) {
            min_x = 0;
            min_y = 0;
            max_x = width - 1;
            max_y = height - 1;
        }
        ensure!(min_x <= max_x && min_y <= max_y, "Cursor asset is empty");
        let cw = max_x - min_x + 1;
        let ch = max_y - min_y + 1;
        let mut cropped = vec![0; cw * ch * 4];
        for y in 0..ch {
            cropped[y * cw * 4..(y + 1) * cw * 4].copy_from_slice(
                &bytes[((y + min_y) * width + min_x) * 4..((y + min_y) * width + min_x + cw) * 4],
            );
        }
        let hx = (n(&entry, "x", 0.) * width as f64 - min_x as f64) / cw as f64;
        let hy = (n(&entry, "y", 0.) * height as f64 - min_y as f64) / ch as f64;
        let asset = (image(cropped, cw as u32, ch as u32)?, hx as f32, hy as f32);
        self.cursor_assets.insert(key, asset.clone());
        Ok(asset)
    }

    pub(super) fn draw_cursor(
        &mut self,
        canvas: &Canvas,
        p: &Project,
        time: f64,
        frame: Rect,
        crop: &Value,
    ) -> Result<()> {
        if self.cursor.is_empty() {
            return Ok(());
        }
        let index = self
            .cursor
            .partition_point(|c| n(c, "timeMs", 0.) <= time)
            .saturating_sub(1);
        let cursor_type = self.cursor[index]["cursorType"]
            .as_str()
            .unwrap_or("arrow")
            .to_owned();
        let fw = frame.width() as f64 / n(crop, "width", 1.);
        let fh = frame.height() as f64 / n(crop, "height", 1.);
        let (mut x, mut y, rotation) = self.cursor_track.at(p, &self.cursor, time, fw, fh);
        let delta_ms = 1000. / self.frame_rate;
        let (ax, ay, _) = self
            .cursor_track
            .at(p, &self.cursor, (time - delta_ms).max(0.), fw, fh);
        let (bx, by) = (x, y);
        if p.flag("loopCursor", false) {
            let tail = (self.info.duration * 1000. - time) / 350.;
            if tail < 1. {
                let t = (1. - tail).clamp(0., 1.);
                let t = t * t * (3. - 2. * t);
                let first = self.cursor_track.at(p, &self.cursor, 0., fw, fh);
                x += (first.0 - x) * t;
                y += (first.1 - y) * t;
            }
        }
        let x = frame.left as f64
            + (x - n(crop, "x", 0.)) / n(crop, "width", 1.) * frame.width() as f64;
        let y = frame.top as f64
            + (y - n(crop, "y", 0.)) / n(crop, "height", 1.) * frame.height() as f64;
        let click_sample = self.cursor[..=index].iter().rev().find(|c| {
            c["interactionType"]
                .as_str()
                .is_some_and(|s| s.contains("click"))
        });
        let click = click_sample
            .map(|c| time - n(c, "timeMs", 0.))
            .unwrap_or(f64::INFINITY);
        let duration = p.number("cursorClickBounceDuration", 350.).max(1.);
        let bounce = if click < duration {
            1. - 0.08
                * p.number("cursorClickBounce", 2.)
                * (std::f64::consts::PI * click / duration).sin()
        } else {
            1.
        };
        let size = (p.number("cursorSize", 3.) * 28. * frame.width() as f64 / 1920.
            * bounce.max(0.72)) as f32;
        let age = click - duration * 0.5;
        let effect_duration = p.number("cursorClickEffectDurationMs", 600.).max(1.);
        if age >= 0. && age < effect_duration && p.text("cursorClickEffect", "none") != "none" {
            let progress = (1. - age / effect_duration) as f32;
            let reveal = 1. - progress;
            let effect_scale = p.number("cursorClickEffectScale", 1.).clamp(0.25, 4.) as f32;
            let opacity = p.number("cursorClickEffectOpacity", 1.).clamp(0., 1.) as f32;
            let style_multiplier = match p.text("cursorStyle", "tahoe") {
                "macos" => (851. / 958.) / (386. / 746.),
                "windows11" => 32. / 19.0625,
                _ => 1.,
            };
            let cursor_size = (p.number("cursorSize", 3.) * 28. * frame.width() as f64 / 1920.)
                as f32
                * style_multiplier;
            let base = (cursor_size * 0.55 * effect_scale).max(12.);
            let stroke = (cursor_size * 0.08).max(2.);
            let point = click_sample.map(|c| {
                (
                    frame.left as f64
                        + (n(c, "cx", 0.5) - n(crop, "x", 0.)) / n(crop, "width", 1.)
                            * frame.width() as f64,
                    frame.top as f64
                        + (n(c, "cy", 0.5) - n(crop, "y", 0.)) / n(crop, "height", 1.)
                            * frame.height() as f64,
                )
            });
            let (ex, ey) = point.unwrap_or((x, y));
            let circle = |radius: f32, width: f32, alpha: f32, fill: bool| {
                let mut pen = paint(color(p.text("cursorClickEffectColor", "#2563eb")));
                pen.set_alpha_f(alpha)
                    .set_style(if fill {
                        sk::paint::Style::Fill
                    } else {
                        sk::paint::Style::Stroke
                    })
                    .set_stroke_width(width);
                canvas.draw_circle((ex as f32, ey as f32), radius, &pen);
            };
            match p.text("cursorClickEffect", "ripple") {
                "ripple" => {
                    let fade = progress.powi(3);
                    circle(
                        ((1. - fade) * cursor_size * 1.95 * effect_scale).max(0.5),
                        (2. * fade).max(1.),
                        fade * 0.6 * opacity,
                        false,
                    );
                }
                "spotlight" => {
                    let radius = base + reveal * cursor_size * effect_scale;
                    circle(
                        radius,
                        (stroke * 0.68).max(1.25),
                        progress * opacity * 0.28,
                        false,
                    );
                    circle(
                        (base * 0.72).max(radius * 0.76),
                        (stroke * 0.75).max(1.5),
                        progress * opacity * 0.5,
                        false,
                    );
                }
                _ => {
                    let radius = base + reveal * cursor_size * 1.22 * effect_scale;
                    circle(radius, stroke, progress * opacity * 0.72, false);
                    circle(
                        (base * 0.58).max(radius * 0.62),
                        (stroke * 0.72).max(1.4),
                        progress * opacity * 0.42,
                        false,
                    );
                    circle((base * 0.18).max(3.), 0., progress * opacity * 0.14, true);
                }
            }
        }
        let style = p.text("cursorStyle", "tahoe");
        if style == "dot" {
            let mut border = paint(Color::from_argb(225, 15, 23, 42));
            border
                .set_style(sk::paint::Style::Stroke)
                .set_stroke_width(size * 0.09);
            canvas.draw_circle((x as f32, y as f32), size * 0.23, &paint(Color::WHITE));
            canvas.draw_circle((x as f32, y as f32), size * 0.23, &border);
        } else {
            let (asset, hx, hy) = self.cursor_asset(style, &cursor_type)?;
            let multiplier = match style {
                "macos" => (851. / 958.) / (386. / 746.),
                "windows11" => 32. / 19.0625,
                _ => 1.,
            };
            let height = size * multiplier;
            let width = height * asset.width() as f32 / asset.height() as f32;
            let factor =
                p.number("cursorMotionBlur", 0.6).clamp(0., 2.) * 2. * (1000. / delta_ms / 60.);
            let velocity = [
                (bx - ax) * frame.width() as f64 / n(crop, "width", 1.) * factor,
                (by - ay) * frame.height() as f64 / n(crop, "height", 1.) * factor,
            ];
            let mut blur_paint = Paint::default();
            blur_paint.set_image_filter(self.motion_filter.image_filter(
                &crate::effects::Blur {
                    velocity,
                    ..Default::default()
                },
                self.width as f64,
                self.height as f64,
            )?);
            canvas.save_layer(&sk::canvas::SaveLayerRec::default().paint(&blur_paint));
            canvas.save();
            canvas.translate((x as f32, y as f32));
            canvas.rotate(rotation as f32, None);
            let rect = Rect::from_xywh(-hx * width, -hy * height, width, height);
            if p.text("cursorStyle", "tahoe") != "figma" {
                let mut shadow = Paint::default();
                shadow.set_color_filter(sk::color_filters::blend(
                    Color::from_argb(89, 0, 0, 0),
                    sk::BlendMode::SrcIn,
                ));
                shadow.set_image_filter(sk::image_filters::blur((3., 3.), None, None, None));
                canvas.draw_image_rect(&asset, None, rect.with_offset((0., 2.)), &shadow);
            }
            canvas.draw_image_rect(&asset, None, rect, &Paint::default());
            canvas.restore();
            canvas.restore();
        }
        Ok(())
    }
}
