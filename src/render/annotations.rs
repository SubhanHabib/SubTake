//! Annotations painted over the composed frame, in output pixels under the
//! camera's transform, so they zoom with the picture.

use super::*;

/// A region's corner radius, `style.borderRadius` in 1080p pixels.
fn radius(a: &Value, unit: f32) -> f32 {
    n(&a["style"], "borderRadius", 0.) as f32 * unit
}

impl Scene {
    pub(super) fn draw_annotation(
        &mut self,
        canvas: &Canvas,
        a: &Value,
        rect: Rect,
        unit: f32,
    ) -> Result<()> {
        let (aw, ah) = (rect.width(), rect.height());
        match a["type"].as_str().unwrap_or("text") {
            "image" => {
                let name = a["imageContent"]
                    .as_str()
                    .or(a["content"].as_str())
                    .unwrap_or("");
                let i = self.asset(name)?;
                canvas.draw_image_rect(&i, None, rect, &Paint::default());
            }
            "figure" => {
                let mut pen = paint(color(&text(&a["figureData"], "color", "#2563eb")));
                pen.set_stroke_width(n(&a["figureData"], "strokeWidth", 4.) as f32 * unit)
                    .set_style(sk::paint::Style::Stroke)
                    .set_stroke_cap(sk::paint::Cap::Round);
                let angle = match text(&a["figureData"], "arrowDirection", "right").as_str() {
                    "left" => 180.,
                    "up" => -90.,
                    "down" => 90.,
                    "up-right" => -45.,
                    "up-left" => -135.,
                    "down-right" => 45.,
                    "down-left" => 135.,
                    _ => 0.,
                };
                canvas.save();
                canvas.rotate(angle, Some(rect.center()));
                canvas.draw_line(
                    (rect.left, rect.center_y()),
                    (rect.right, rect.center_y()),
                    &pen,
                );
                canvas.draw_line(
                    (rect.right, rect.center_y()),
                    (rect.right - aw * 0.2, rect.center_y() - ah * 0.3),
                    &pen,
                );
                canvas.draw_line(
                    (rect.right, rect.center_y()),
                    (rect.right - aw * 0.2, rect.center_y() + ah * 0.3),
                    &pen,
                );
                canvas.restore();
            }
            "blur" => {
                let amount = n(a, "blurIntensity", 20.) as f32 * unit;
                let filter = if a["blurMode"] == "pixelate" {
                    // Shrunk and grown back without smoothing, the picture
                    // turns to square blocks of its own colours, aligned to
                    // the region's corner.
                    let cell = amount.max(2.);
                    let corner = sk::Point::new(rect.left, rect.top);
                    let mut shrink = sk::Matrix::new_identity();
                    shrink.set_scale((1. / cell, 1. / cell), corner);
                    let mut grow = sk::Matrix::new_identity();
                    grow.set_scale((cell, cell), corner);
                    sk::image_filters::matrix_transform(
                        &grow,
                        sk::SamplingOptions::new(sk::FilterMode::Nearest, sk::MipmapMode::None),
                        sk::image_filters::matrix_transform(
                            &shrink,
                            sk::SamplingOptions::new(sk::FilterMode::Linear, sk::MipmapMode::None),
                            None,
                        ),
                    )
                } else {
                    sk::image_filters::blur((amount, amount), None, None, None)
                };
                canvas.save();
                canvas.clip_path(&squircle(rect, radius(a, unit)), None, true);
                if let Some(filter) = filter {
                    canvas.save_layer(
                        &sk::canvas::SaveLayerRec::default()
                            .bounds(&rect)
                            .backdrop(&filter),
                    );
                    canvas.restore();
                }
                canvas.restore();
            }
            "highlight" => {
                let stroke = n(&a["figureData"], "strokeWidth", 4.) as f32 * unit;
                let shape = squircle(rect, radius(a, unit));
                canvas.draw_path(
                    &shape,
                    &paint(color(&text(&a["style"], "backgroundColor", "transparent"))),
                );
                let mut pen = paint(color(&text(&a["figureData"], "color", "#facc15")));
                pen.set_stroke_width(stroke)
                    .set_style(sk::paint::Style::Stroke);
                canvas.draw_path(&shape, &pen);
            }
            "spotlight" => {
                // Everything but the region dims, the wallpaper included.
                let dim = n(a, "dimOpacity", 60.).clamp(0., 100.) / 100.;
                canvas.save();
                canvas.clip_path(
                    &squircle(rect, radius(a, unit)),
                    sk::ClipOp::Difference,
                    true,
                );
                canvas.draw_paint(&paint(Color::from_argb((dim * 255.) as u8, 0, 0, 0)));
                canvas.restore();
            }
            "step" => {
                // A disc as wide as the region's shorter side, centred in it.
                let diameter = aw.min(ah);
                canvas.draw_circle(
                    rect.center(),
                    diameter / 2.,
                    &paint(color(&text(&a["figureData"], "color", "#2563eb"))),
                );
                let content = a["textContent"].as_str().unwrap_or("1");
                let style = serde_json::json!({
                    "fontSize": diameter * 0.55 / unit,
                    "fontFamily": text(&a["style"], "fontFamily", "Helvetica"),
                    "fontWeight": "bold",
                    "color": text(&a["style"], "color", "#ffffff"),
                    "backgroundColor": "transparent",
                });
                let disc = Rect::from_xywh(
                    rect.center_x() - diameter / 2.,
                    rect.center_y() - diameter / 2.,
                    diameter,
                    diameter,
                );
                self.draw_text(
                    canvas,
                    content,
                    &style,
                    disc.with_outset((8. * unit, 0.)),
                    unit,
                    false,
                );
            }
            _ => {
                let content = a["textContent"]
                    .as_str()
                    .or(a["content"].as_str())
                    .unwrap_or("");
                self.draw_text(canvas, content, &a["style"], rect, unit, false);
            }
        }
        Ok(())
    }
}
