//! Caption cues laid out and painted over the composed frame.

use super::*;

impl Scene {
    pub(super) fn draw_captions(
        &self,
        canvas: &Canvas,
        cues: &[Value],
        settings: &Value,
        time: f64,
    ) {
        let w = self.width as f64;
        let h = self.height as f64;
        let max_width = n(settings, "maxWidth", 62.).clamp(5., 100.);
        let font_size = (n(settings, "fontSize", 30.) * (w * max_width / 100.) / (1920. * 0.62))
            .max(14.) as f32;
        let mut style = TextStyle::new();
        let family = text(settings, "fontFamily", "Helvetica");
        let families: Vec<_> = family
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .collect();
        style
            .set_font_size(font_size)
            .set_font_families(&families)
            .set_color(color(&text(settings, "textColor", "#ffffff")));
        let mut ps = ParagraphStyle::new();
        ps.set_text_style(&style);
        let paragraph = |text: &str| {
            let mut builder = ParagraphBuilder::new(&ps, self.fonts.clone());
            builder.add_text(text);
            let mut p = builder.build();
            p.layout(1_000_000.);
            p
        };
        let px = font_size as f64 * 1.1;
        let py = font_size as f64 * 0.78;
        let measure = |text: &str| paragraph(text).max_intrinsic_width() as f64;
        let Some(layout) = crate::captions::layout(
            cues,
            time,
            (w * max_width / 100. - px * 2.).max(font_size as f64 * 4.),
            n(settings, "maxRows", 1.) as usize,
            &text(settings, "animationStyle", "fade"),
            measure,
        ) else {
            return;
        };
        let line_height = font_size as f64 * 1.32;
        let box_height = layout.lines.len() as f64 * line_height + py * 2.;
        let box_width = (w * max_width / 100. + px * 2.)
            .min(layout.lines.iter().map(|l| l.width).fold(0., f64::max) + px * 2.);
        canvas.save();
        canvas.translate((
            w as f32 / 2.,
            (h - h * n(settings, "bottomOffset", 3.) / 100. - box_height / 2. + layout.translate_y)
                as f32,
        ));
        canvas.scale((layout.scale as f32, layout.scale as f32));
        let mut alpha = Paint::default();
        alpha.set_alpha_f(layout.opacity as f32);
        canvas.save_layer(&sk::canvas::SaveLayerRec::default().paint(&alpha));
        let background = paint(Color::from_argb(
            (n(settings, "backgroundOpacity", 0.9).clamp(0., 1.) * 255.) as u8,
            0,
            0,
            0,
        ));
        canvas.draw_path(
            &squircle(
                Rect::from_xywh(
                    -box_width as f32 / 2.,
                    -box_height as f32 / 2.,
                    box_width as f32,
                    box_height as f32,
                ),
                (n(settings, "boxRadius", 17.5) * font_size as f64 / 30.) as f32,
            ),
            &background,
        );
        // The shipping Electron renderer deliberately uses uniform word colour.
        // Preserve its word timings for page changes without re-enabling highlighting.
        for (i, line) in layout.lines.iter().enumerate() {
            let mut x = -line.width / 2.;
            for word in &line.words {
                let segment = format!("{}{}", if word.leading_space { " " } else { "" }, word.text);
                let p = paragraph(&segment);
                let y = -box_height / 2.
                    + py
                    + line_height * i as f64
                    + (line_height - p.height() as f64) / 2.;
                p.paint(canvas, (x as f32, y as f32));
                x += p.max_intrinsic_width() as f64;
            }
        }
        canvas.restore();
        canvas.restore();
    }

    pub(super) fn draw_text(
        &self,
        canvas: &Canvas,
        content: &str,
        settings: &Value,
        rect: Rect,
        unit: f32,
        caption: bool,
    ) {
        let mut style = TextStyle::new();
        style
            .set_font_size(n(settings, "fontSize", 32.) as f32 * unit)
            .set_color(color(&text(
                settings,
                if caption { "textColor" } else { "color" },
                "#ffffff",
            )));
        let family = text(settings, "fontFamily", "Helvetica");
        let families: Vec<_> = family
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .collect();
        style.set_font_families(&families);
        let bold = text(settings, "fontWeight", "bold") == "bold";
        let italic = text(settings, "fontStyle", "normal") == "italic";
        style.set_font_style(match (bold, italic) {
            (true, true) => FontStyle::bold_italic(),
            (true, false) => FontStyle::bold(),
            (false, true) => FontStyle::italic(),
            _ => FontStyle::normal(),
        });
        if text(settings, "textDecoration", "none") == "underline" {
            style.set_decoration_type(skia_safe::textlayout::TextDecoration::UNDERLINE);
        }
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_style(&style).set_text_align(
            match text(settings, "textAlign", "center").as_str() {
                "left" => TextAlign::Left,
                "right" => TextAlign::Right,
                _ => TextAlign::Center,
            },
        );
        if caption {
            paragraph_style.set_max_lines(n(settings, "maxRows", 2.) as usize);
        }
        let mut builder = ParagraphBuilder::new(&paragraph_style, self.fonts.clone());
        builder.add_text(content);
        let mut paragraph = builder.build();
        // The inset from the box's sides: a caption's is fixed, an
        // annotation's is its style's `padding` (1080p px).
        let inset = if caption {
            8.
        } else {
            n(settings, "padding", 8.) as f32
        } * unit;
        paragraph.layout((rect.width() - 2. * inset).max(1.));
        let height = if caption {
            paragraph.height() + 16. * unit
        } else {
            rect.height()
        };
        let y = rect.center_y() - height / 2.;
        let background = if caption {
            Color::from_argb(
                (n(settings, "backgroundOpacity", 0.9) * 255.) as u8,
                0,
                0,
                0,
            )
        } else {
            color(&text(settings, "backgroundColor", "transparent"))
        };
        let radius = n(
            settings,
            if caption { "boxRadius" } else { "borderRadius" },
            8.,
        ) as f32
            * unit;
        canvas.draw_rrect(
            RRect::new_rect_xy(
                Rect::from_xywh(rect.left, y, rect.width(), height),
                radius,
                radius,
            ),
            &paint(background),
        );
        canvas.save();
        canvas.clip_rect(
            Rect::from_xywh(rect.left, y, rect.width(), height),
            None,
            true,
        );
        paragraph.paint(
            canvas,
            (rect.left + inset, y + (height - paragraph.height()) / 2.),
        );
        canvas.restore();
    }
}
