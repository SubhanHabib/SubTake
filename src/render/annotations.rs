//! Annotations painted over the composed frame, in output pixels under the
//! camera's transform, so they zoom with the picture.

use super::*;

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
                    .set_style(sk::paint::Style::Stroke);
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
                canvas.save();
                canvas.clip_rect(rect, None, true);
                let filter = sk::image_filters::blur(
                    (
                        n(a, "blurIntensity", 20.) as f32 * unit,
                        n(a, "blurIntensity", 20.) as f32 * unit,
                    ),
                    None,
                    None,
                    None,
                );
                canvas.save_layer(
                    &sk::canvas::SaveLayerRec::default()
                        .bounds(&rect)
                        .backdrop(filter.as_ref().unwrap()),
                );
                canvas.restore();
                canvas.restore();
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
