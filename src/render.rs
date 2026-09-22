//! One composition engine for preview and export. All coordinates are output-relative.
use crate::{
    media::{Decoder, MediaInfo, resources},
    project::{Project, parse_color},
    timeline::n,
};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde_json::Value;
use skia_safe::textlayout::{
    FontCollection, ParagraphBuilder, ParagraphStyle, TextAlign, TextStyle,
};
use skia_safe::{
    self as sk, AlphaType, Canvas, Color, ColorType, Data, FontMgr, FontStyle, ImageInfo, Paint,
    RRect, Rect,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct Scene {
    source: Decoder,
    motion_filter: crate::effects::Filter,
    frame_rate: f64,
    camera: crate::motion::CameraTrack,
    webcam: Option<(PathBuf, Decoder, u32, u32, f64)>,
    background_video: Option<(String, Decoder, MediaInfo)>,
    info: MediaInfo,
    pub width: u32,
    pub height: u32,
    source_width: u32,
    source_height: u32,
    surface: Option<sk::Surface>,
    #[cfg(target_os = "macos")]
    gpu: Option<sk::gpu::DirectContext>,
    cursor_assets: HashMap<String, (sk::Image, f32, f32)>,
    cursor_track: crate::motion::CursorTrack,
    assets: HashMap<String, sk::Image>,
    base: PathBuf,
    cursor: Vec<Value>,
    fonts: FontCollection,
    font_key: Value,
}
fn paint(color: Color) -> Paint {
    let mut p = Paint::default();
    p.set_anti_alias(true).set_color(color);
    p
}
fn color(s: &str) -> Color {
    let [r, g, b, a] = parse_color(s);
    Color::from_argb(a, r, g, b)
}
fn image(bytes: Vec<u8>, w: u32, h: u32) -> Result<sk::Image> {
    sk::images::raster_from_data(
        &ImageInfo::new(
            (w as i32, h as i32),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        ),
        Data::new_copy(&bytes),
        w as usize * 4,
    )
    .context("Create decoded frame")
}
fn text(v: &Value, key: &str, default: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or(default).into()
}
fn squircle(rect: Rect, radius: f32) -> sk::Path {
    let mut path = sk::PathBuilder::new();
    let r = radius.clamp(0., rect.width().min(rect.height()) / 2.);
    if r <= 0.5 {
        path.add_rect(rect, None, None);
        return path.detach();
    }
    path.move_to((rect.left + r, rect.top));
    for (cx, cy, start) in [
        (rect.right - r, rect.top + r, -std::f32::consts::FRAC_PI_2),
        (rect.right - r, rect.bottom - r, 0.),
        (rect.left + r, rect.bottom - r, std::f32::consts::FRAC_PI_2),
        (rect.left + r, rect.top + r, std::f32::consts::PI),
    ] {
        for i in 1..=10 {
            let angle = start + std::f32::consts::FRAC_PI_2 * i as f32 / 10.;
            let (sin, cos) = angle.sin_cos();
            path.line_to((
                cx + cos.signum() * r * cos.abs().powf(2. / 4.5),
                cy + sin.signum() * r * sin.abs().powf(2. / 4.5),
            ));
        }
    }
    path.close();
    path.detach()
}
impl Scene {
    /// Normalized edit bounds use the same camera transform as the composed image.
    pub fn edit_bounds(
        &mut self,
        p: &Project,
        time: f64,
        selected: Option<&(String, String)>,
        panel: &str,
    ) -> Option<[f32; 5]> {
        if panel == "Crop" {
            let [x, y, w, h] = crate::editing::crop_rect(p);
            return Some([x as f32, y as f32, w as f32, h as f32, 1.]);
        }
        let (w, h) = (self.width as f64, self.height as f64);
        let frame =
            crate::geometry::frame(p, w, h, self.info.width as f64, self.info.height as f64);
        let cam = self.camera.at(p, &self.cursor, time * 1000., w, h, &frame);
        if panel == "Webcam" {
            let wc = p.editor.get("webcam")?;
            if !wc["enabled"].as_bool().unwrap_or(false) {
                return None;
            }
            let unit = w / 1920.;
            let reactive = if wc["reactToZoom"].as_bool().unwrap_or(true) {
                1. / cam.scale
            } else {
                1.
            };
            let width =
                (n(wc, "width", n(wc, "size", 40.)) / 100. * w.min(h) * reactive).max(56. * unit);
            let height = (n(wc, "height", 40.) / 100. * w.min(h) * reactive).max(56. * unit);
            let margin = n(wc, "margin", 24.) * unit;
            let preset = wc["positionPreset"]
                .as_str()
                .or(wc["corner"].as_str())
                .unwrap_or("custom");
            let (x, y) = match preset {
                "top-left" => (0., 0.),
                "top-center" => (0.5, 0.),
                "top-right" => (1., 0.),
                "center-left" => (0., 0.5),
                "center" => (0.5, 0.5),
                "center-right" => (1., 0.5),
                "bottom-left" => (0., 1.),
                "bottom-center" => (0.5, 1.),
                "bottom-right" => (1., 1.),
                _ => (n(wc, "positionX", 1.), n(wc, "positionY", 1.)),
            };
            return Some([
                ((margin + (w - width - 2. * margin).max(0.) * x) / w) as f32,
                ((margin + (h - height - 2. * margin).max(0.) * y) / h) as f32,
                (width / w) as f32,
                (height / h) as f32,
                reactive as f32,
            ]);
        }
        let (kind, id) = selected?;
        if kind != "annotationRegions" {
            return None;
        }
        let a = p.regions(kind).iter().find(|a| {
            a["id"] == *id
                && n(a, "startMs", 0.) <= time * 1000.
                && n(a, "endMs", 0.) > time * 1000.
        })?;
        Some([
            (cam.x / w + cam.scale * n(&a["position"], "x", 50.) / 100.) as f32,
            (cam.y / h + cam.scale * n(&a["position"], "y", 50.) / 100.) as f32,
            (cam.scale * n(&a["size"], "width", 30.) / 100.) as f32,
            (cam.scale * n(&a["size"], "height", 20.) / 100.) as f32,
            cam.scale as f32,
        ])
    }
    pub fn new(source: PathBuf, info: MediaInfo, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= 8192 && height <= 8192,
            "Output dimensions must be between 1 and 8192"
        );
        let ratio = (8192. / info.width.max(info.height) as f64).min(1.);
        let sw = (info.width as f64 * ratio).round().max(2.) as u32;
        let sh = (info.height as f64 * ratio).round().max(2.) as u32;
        let mut fonts = FontCollection::new();
        fonts.set_default_font_manager(FontMgr::new(), None);
        let mut sidecar = source.as_os_str().to_os_string();
        sidecar.push(".cursor.json");
        let telemetry = std::fs::read(PathBuf::from(sidecar))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let mut cursor = telemetry
            .as_ref()
            .and_then(|v| {
                v.as_array()
                    .or_else(|| v.get("samples").and_then(Value::as_array))
            })
            .cloned()
            .unwrap_or_default();
        cursor.sort_by(|a, b| n(a, "timeMs", 0.).total_cmp(&n(b, "timeMs", 0.)));
        Ok(Self {
            base: source.parent().unwrap_or(Path::new(".")).into(),
            source: Decoder::new(source, sw, sh).with_rate(info.fps),
            motion_filter: crate::effects::Filter::new()?,
            frame_rate: 30.,
            camera: crate::motion::CameraTrack::default(),
            info,
            width,
            height,
            source_width: sw,
            source_height: sh,
            webcam: None,
            background_video: None,
            assets: HashMap::new(),
            cursor,
            fonts,
            font_key: Value::Null,
            surface: None,
            cursor_assets: HashMap::new(),
            cursor_track: crate::motion::CursorTrack::default(),
            #[cfg(target_os = "macos")]
            gpu: metal_context(),
        })
    }
    pub fn backend(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        if self.gpu.is_some() {
            return "Metal";
        }
        "Skia CPU"
    }
    pub fn with_frame_rate(mut self, rate: f64) -> Self {
        self.frame_rate = rate.clamp(1., 120.);
        self
    }
    fn create_surface(&mut self) -> Result<sk::Surface> {
        #[cfg(target_os = "macos")]
        if let Some(gpu) = &mut self.gpu {
            return sk::gpu::surfaces::render_target(
                gpu,
                sk::gpu::Budgeted::Yes,
                &ImageInfo::new_n32_premul((self.width as i32, self.height as i32), None),
                None,
                sk::gpu::SurfaceOrigin::TopLeft,
                None,
                false,
                false,
            )
            .context("Allocate Metal composition target");
        }
        sk::surfaces::raster_n32_premul((self.width as i32, self.height as i32))
            .context("Allocate software composition target")
    }
    pub fn asset(&mut self, name: &str) -> Result<sk::Image> {
        if let Some(i) = self.assets.get(name) {
            return Ok(i.clone());
        }
        let bytes = if name.starts_with("data:") {
            let (_, encoded) = name.split_once(',').context("Malformed image data URI")?;
            base64::engine::general_purpose::STANDARD.decode(encoded)?
        } else {
            let root = resources();
            let candidates = [
                crate::project::local_path(name),
                self.base.join(name),
                root.join("public").join(name.trim_start_matches('/')),
                root.join(name.trim_start_matches('/')),
            ];
            let path = candidates
                .iter()
                .find(|p| p.is_file())
                .with_context(|| format!("Image is missing: {name}"))?;
            std::fs::read(path)?
        };
        let i = if String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).contains("<svg") {
            let svg = String::from_utf8_lossy(&bytes);
            let attribute = |key: &str| {
                svg.split(&format!("{key}=\""))
                    .nth(1)
                    .and_then(|s| s.split('"').next())
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(32.)
            };
            let height = 512;
            let width = (512. * attribute("width") / attribute("height"))
                .round()
                .max(1.) as i32;
            let mut surface = sk::surfaces::raster_n32_premul((width, height))
                .context("Allocate cursor raster")?;
            let mut dom = sk::svg::Dom::from_bytes(&bytes, FontMgr::new())
                .map_err(|_| anyhow::anyhow!("Invalid SVG asset"))?;
            let native_width = attribute("width");
            let native_height = attribute("height");
            dom.set_container_size((native_width, native_height));
            surface.canvas().clear(Color::TRANSPARENT);
            surface
                .canvas()
                .scale((width as f32 / native_width, height as f32 / native_height));
            dom.render(surface.canvas());
            surface.image_snapshot()
        } else {
            sk::images::deferred_from_encoded_data(Data::new_copy(&bytes), None)
                .context("Unsupported image")?
        };
        self.assets.insert(name.into(), i.clone());
        Ok(i)
    }
    pub fn render(&mut self, p: &Project, source_time: f64) -> Result<Vec<u8>> {
        let font_key = p.editor.get("nativeFonts").unwrap_or(&Value::Null);
        if &self.font_key != font_key {
            let mut provider = sk::textlayout::TypefaceFontProvider::new();
            if let Some(fonts) = font_key.as_array() {
                for font in fonts {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(font["data"].as_str().context("Font has no data")?)?;
                    let face = FontMgr::new()
                        .new_from_data(&bytes, None)
                        .context("Invalid embedded font")?;
                    provider.register_typeface(face, font["family"].as_str());
                }
            }
            self.fonts.set_asset_font_manager(Some(provider.into()));
            self.font_key = font_key.clone();
        }
        let mut surface = if let Some(surface) = self.surface.take() {
            surface
        } else {
            self.create_surface()?
        };
        let canvas = surface.canvas();
        let w = self.width as f32;
        let h = self.height as f32;
        let unit = w / 1920.;
        let bg = p.text("wallpaper", "#171c35");
        canvas.clear(color(bg));
        if !bg.starts_with('#') && !bg.is_empty() {
            if bg.starts_with("linear-gradient") {
                let colors: Vec<_> = bg
                    .split([' ', ',', ')'])
                    .filter(|s| s.starts_with('#'))
                    .map(color)
                    .collect();
                if colors.len() > 1 {
                    let mut fill = Paint::default();
                    let colors: Vec<sk::Color4f> = colors.into_iter().map(Into::into).collect();
                    let gradient = sk::gradient::Gradient::new(
                        sk::gradient::Colors::new_evenly_spaced(&colors, sk::TileMode::Clamp, None),
                        sk::gradient::Interpolation::default(),
                    );
                    let angle = bg
                        .split('(')
                        .nth(1)
                        .and_then(|s| s.split(',').next())
                        .map(|s| s.trim())
                        .unwrap_or("180deg");
                    let angle = angle
                        .strip_suffix("deg")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(match angle {
                            "to right" => 90.,
                            "to left" => 270.,
                            "to top" => 0.,
                            _ => 180.,
                        })
                        .to_radians();
                    let dx = angle.sin();
                    let dy = -angle.cos();
                    let half = (w * dx.abs() + h * dy.abs()) / 2.;
                    fill.set_shader(sk::gradient::shaders::linear_gradient(
                        (
                            (w / 2. - dx * half, h / 2. - dy * half),
                            (w / 2. + dx * half, h / 2. + dy * half),
                        ),
                        &gradient,
                        None,
                    ));
                    canvas.draw_rect(Rect::from_wh(w, h), &fill);
                }
            } else {
                let extension = Path::new(bg)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let i = if matches!(extension.as_str(), "mp4" | "mov" | "webm" | "mkv") {
                    if self
                        .background_video
                        .as_ref()
                        .is_none_or(|(name, _, _)| name != bg)
                    {
                        let path = [
                            crate::project::local_path(bg),
                            self.base.join(bg),
                            resources().join("public").join(bg.trim_start_matches('/')),
                        ]
                        .into_iter()
                        .find(|p| p.is_file())
                        .context("Background video is missing")?;
                        let info = crate::media::probe(&path)?;
                        let ratio = (self.width as f64 / info.width as f64).min(1.);
                        let mut scaled = info.clone();
                        scaled.width = (info.width as f64 * ratio).round().max(2.) as u32;
                        scaled.height = (info.height as f64 * ratio).round().max(2.) as u32;
                        self.background_video = Some((
                            bg.into(),
                            Decoder::new(path, scaled.width, scaled.height),
                            scaled,
                        ));
                    }
                    let (_, decoder, info) = self.background_video.as_mut().unwrap();
                    image(
                        decoder.frame(source_time.rem_euclid(info.duration))?,
                        info.width,
                        info.height,
                    )?
                } else {
                    self.asset(bg)?
                };
                let mut fill = Paint::default();
                let blur = p.number("backgroundBlur", 0.) as f32 * unit;
                if blur > 0. {
                    fill.set_image_filter(sk::image_filters::blur((blur, blur), None, None, None));
                }
                let scale = (w / i.width() as f32).max(h / i.height() as f32);
                let iw = i.width() as f32 * scale;
                let ih = i.height() as f32 * scale;
                canvas.draw_image_rect(
                    &i,
                    None,
                    Rect::from_xywh((w - iw) / 2., (h - ih) / 2., iw, ih),
                    &fill,
                );
            }
        }
        let crop = p.editor.get("cropRegion").cloned().unwrap_or(Value::Null);
        let cw = n(&crop, "width", 1.).clamp(0.001, 1.) as f32;
        let ch = n(&crop, "height", 1.).clamp(0.001, 1.) as f32;
        let layout = crate::geometry::frame(
            p,
            w as f64,
            h as f64,
            self.info.width as f64,
            self.info.height as f64,
        );
        let frame = Rect::from_xywh(
            layout.x as f32,
            layout.y as f32,
            layout.width as f32,
            layout.height as f32,
        );
        let radius = layout.radius as f32;
        let cam = self.camera.at(
            p,
            &self.cursor,
            source_time * 1000.,
            w as f64,
            h as f64,
            &layout,
        );
        let previous = self.camera.at(
            p,
            &self.cursor,
            (source_time * 1000. - 1000. / self.frame_rate).max(0.),
            w as f64,
            h as f64,
            &layout,
        );
        let blur = crate::effects::camera(
            p,
            previous,
            cam,
            &layout,
            w as f64,
            h as f64,
            1. / self.frame_rate,
        );
        let mut blur_paint = Paint::default();
        blur_paint.set_image_filter(self.motion_filter.image_filter(&blur, w as f64, h as f64)?);
        canvas.save_layer(&sk::canvas::SaveLayerRec::default().paint(&blur_paint));
        canvas.save();
        let tx = cam.x;
        let ty = cam.y;
        canvas.translate((tx as f32, ty as f32));
        canvas.scale((cam.scale as f32, cam.scale as f32));
        let shadow = p.number("shadowIntensity", 0.3).clamp(0., 1.) as f32;
        if shadow > 0. {
            let mut s = paint(Color::from_argb((shadow * 180.) as u8, 0, 0, 0));
            s.set_image_filter(sk::image_filters::blur(
                (18. * unit, 18. * unit),
                None,
                None,
                None,
            ));
            canvas.draw_rrect(
                RRect::new_rect_xy(frame.with_offset((0., 12. * unit)), radius, radius),
                &s,
            );
        }
        canvas.save();
        canvas.clip_path(&squircle(frame, radius), None, true);
        let frame_image = image(
            self.source.frame(source_time)?,
            self.source_width,
            self.source_height,
        )?;
        let source_rect = Rect::from_xywh(
            n(&crop, "x", 0.) as f32 * self.source_width as f32,
            n(&crop, "y", 0.) as f32 * self.source_height as f32,
            cw * self.source_width as f32,
            ch * self.source_height as f32,
        );
        canvas.draw_image_rect(
            &frame_image,
            Some((&source_rect, sk::canvas::SrcRectConstraint::Strict)),
            frame,
            &Paint::default(),
        );
        if p.flag("showCursor", true) {
            self.draw_cursor(canvas, p, source_time * 1000., frame, &crop)?;
        }
        canvas.restore();
        canvas.restore();
        canvas.restore();
        if let Some(webcam) = p
            .editor
            .get("webcam")
            .filter(|v| v["enabled"].as_bool() == Some(true))
            && let Some(path) = webcam["sourcePath"].as_str()
        {
            let path = crate::project::local_path(path);
            let path = if path.is_absolute() {
                path
            } else {
                self.base.join(path)
            };
            if self
                .webcam
                .as_ref()
                .is_none_or(|(p, _, _, _, _)| *p != path)
            {
                let info = crate::media::probe(&path)?;
                let height = (640. * info.height as f64 / info.width as f64)
                    .round()
                    .max(2.) as u32;
                self.webcam = Some((
                    path.clone(),
                    Decoder::new(path, 640, height).with_rate(info.fps),
                    640,
                    height,
                    info.duration,
                ));
            }
            let time = (source_time - n(webcam, "timeOffsetMs", 0.) / 1000.).max(0.);
            let (_, decoder, iw, ih, duration) = self.webcam.as_mut().unwrap();
            let i = image(
                decoder.frame(time.min((*duration - 1. / 60.).max(0.)))?,
                *iw,
                *ih,
            )?;
            let reactive = if webcam["reactToZoom"].as_bool().unwrap_or(true) {
                1. / cam.scale as f32
            } else {
                1.
            };
            let ww =
                (n(webcam, "width", n(webcam, "size", 40.)) as f32 / 100. * w.min(h) * reactive)
                    .max(56. * unit);
            let wh = (n(webcam, "height", 40.) as f32 / 100. * w.min(h) * reactive).max(56. * unit);
            let margin = n(webcam, "margin", 24.) as f32 * unit;
            let preset = webcam["positionPreset"]
                .as_str()
                .or(webcam["corner"].as_str())
                .unwrap_or("custom");
            let (px, py) = match preset {
                "top-left" => (0., 0.),
                "top-center" => (0.5, 0.),
                "top-right" => (1., 0.),
                "center-left" => (0., 0.5),
                "center" => (0.5, 0.5),
                "center-right" => (1., 0.5),
                "bottom-left" => (0., 1.),
                "bottom-center" => (0.5, 1.),
                "bottom-right" => (1., 1.),
                _ => (
                    n(webcam, "positionX", 1.) as f32,
                    n(webcam, "positionY", 1.) as f32,
                ),
            };
            let x = margin + (w - ww - 2. * margin).max(0.) * px;
            let y = margin + (h - wh - 2. * margin).max(0.) * py;
            let rect = Rect::from_xywh(x, y, ww, wh);
            let rad = (n(webcam, "roundness", 100.).clamp(0., 100.) as f32 / 100.).sqrt()
                * ww.min(wh)
                * 0.5;
            let mut shadow = paint(Color::from_argb(
                (n(webcam, "shadow", 0.3).clamp(0., 1.) * 200.) as u8,
                0,
                0,
                0,
            ));
            shadow.set_image_filter(sk::image_filters::blur(
                (12. * unit, 12. * unit),
                None,
                None,
                None,
            ));
            canvas.draw_rrect(
                RRect::new_rect_xy(rect.with_offset((0., 8. * unit)), rad, rad),
                &shadow,
            );
            let crop = &webcam["cropRegion"];
            let cx = n(crop, "x", 0.).clamp(0., 0.99) as f32 * i.width() as f32;
            let cy = n(crop, "y", 0.).clamp(0., 0.99) as f32 * i.height() as f32;
            let cw = (n(crop, "width", 1.).clamp(0.01, 1.) as f32 * i.width() as f32)
                .min(i.width() as f32 - cx);
            let ch = (n(crop, "height", 1.).clamp(0.01, 1.) as f32 * i.height() as f32)
                .min(i.height() as f32 - cy);
            let scale = (ww / cw).max(wh / ch);
            let sw = ww / scale;
            let sh = wh / scale;
            let src = Rect::from_xywh(cx + (cw - sw) / 2., cy + (ch - sh) / 2., sw, sh);
            canvas.save();
            canvas.clip_rrect(RRect::new_rect_xy(rect, rad, rad), None, true);
            if webcam["mirror"].as_bool().unwrap_or(true) {
                canvas.translate((rect.center_x() * 2., 0.));
                canvas.scale((-1., 1.));
            }
            canvas.draw_image_rect(
                &i,
                Some((&src, sk::canvas::SrcRectConstraint::Strict)),
                rect,
                &Paint::default(),
            );
            canvas.restore();
        }
        let mut annotations: Vec<_> = p
            .regions("annotationRegions")
            .iter()
            .filter(|r| {
                n(r, "startMs", 0.) <= source_time * 1000.
                    && n(r, "endMs", 0.) > source_time * 1000.
            })
            .collect();
        annotations.sort_by(|a, b| n(a, "zIndex", 0.).total_cmp(&n(b, "zIndex", 0.)));
        for a in annotations {
            canvas.save();
            canvas.translate((tx as f32, ty as f32));
            canvas.scale((cam.scale as f32, cam.scale as f32));
            let aw = n(&a["size"], "width", 30.) as f32 / 100. * w;
            let ah = n(&a["size"], "height", 20.) as f32 / 100. * h;
            let rect = Rect::from_xywh(
                n(&a["position"], "x", 50.) as f32 / 100. * w,
                n(&a["position"], "y", 50.) as f32 / 100. * h,
                aw,
                ah,
            );
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
            canvas.restore();
        }
        if p.editor
            .get("autoCaptionSettings")
            .and_then(|s| s["enabled"].as_bool())
            .unwrap_or(false)
        {
            let settings = &p.editor["autoCaptionSettings"];
            self.draw_captions(
                canvas,
                p.regions("autoCaptions"),
                settings,
                source_time * 1000.,
            );
        }

        let mut bytes = vec![0; self.width as usize * self.height as usize * 4];
        ensure!(
            surface.read_pixels(
                &ImageInfo::new(
                    (self.width as i32, self.height as i32),
                    ColorType::RGBA8888,
                    AlphaType::Unpremul,
                    None
                ),
                &mut bytes,
                self.width as usize * 4,
                (0, 0)
            ),
            "Read composed frame"
        );
        self.surface = Some(surface);
        Ok(bytes)
    }
    fn draw_captions(&self, canvas: &Canvas, cues: &[Value], settings: &Value, time: f64) {
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
    fn draw_text(
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
        paragraph.layout((rect.width() - 16. * unit).max(1.));
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
            (
                rect.left + 8. * unit,
                y + (height - paragraph.height()) / 2.,
            ),
        );
        canvas.restore();
    }
    fn cursor_asset(&mut self, style: &str, kind: &str) -> Result<(sk::Image, f32, f32)> {
        let key = format!("{style}/{kind}");
        if let Some(asset) = self.cursor_assets.get(&key) {
            return Ok(asset.clone());
        }
        let manifest: Value = serde_json::from_str(include_str!("../assets/cursors.json"))?;
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
    fn draw_cursor(
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

#[cfg(target_os = "macos")]
fn metal_context() -> Option<sk::gpu::DirectContext> {
    use objc2::rc::Retained;
    use objc2_metal::MTLDevice;
    if std::env::var("SUBTAKE_RENDERER").as_deref() == Ok("software") {
        return None;
    }
    let device = objc2_metal::MTLCreateSystemDefaultDevice()?;
    let queue = device.newCommandQueue()?;
    // SAFETY: both Objective-C objects are live retained objects. Skia's backend
    // and direct context retain their own references before these locals drop.
    let backend = unsafe {
        sk::gpu::mtl::BackendContext::new(
            Retained::as_ptr(&device) as sk::gpu::mtl::Handle,
            Retained::as_ptr(&queue) as sk::gpu::mtl::Handle,
        )
    };
    sk::gpu::direct_contexts::make_metal(&backend, None)
}
