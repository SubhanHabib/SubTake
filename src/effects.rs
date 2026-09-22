//! Spatial motion blur ported from zoomTransform.ts and the pixi-filters kernels.
//! Temporal accumulation is disabled in the production reference and stays disabled here.
use crate::{geometry::Frame, motion::Transform, project::Project, timeline::n};
use serde::{Deserialize, Serialize};
use skia_safe as sk;
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Blur {
    pub velocity: [f64; 2],
    pub center: [f64; 2],
    pub strength: f64,
}

pub fn camera(
    p: &Project,
    previous: Transform,
    current: Transform,
    frame: &Frame,
    width: f64,
    height: f64,
    dt: f64,
) -> Blur {
    let amount = p.number("zoomMotionBlur", 0.35).max(0.);
    if amount == 0. {
        return Blur::default();
    }
    let dt = dt.clamp(0.001, 0.08);
    let tuning = p
        .editor
        .get("zoomMotionBlurTuning")
        .unwrap_or(&serde_json::Value::Null);
    let center = |t: Transform| {
        [
            t.x + (frame.x + frame.width / 2.) * t.scale,
            t.y + (frame.y + frame.height / 2.) * t.scale,
        ]
    };
    let a = center(previous);
    let b = center(current);
    let movement = [b[0] - a[0], b[1] - a[1]];
    let ratio = current.scale / previous.scale.max(0.0001);
    let zoom_distance = (frame.width * (current.scale - previous.scale))
        .hypot(frame.height * (current.scale - previous.scale));
    let mut result = Blur {
        center: b,
        ..Default::default()
    };
    if zoom_distance > 0.001
        && ratio.max(0.0001).ln().abs() / dt > n(tuning, "zoomVelocityThreshold", 0.)
    {
        result.strength = (1. - ratio).abs()
            * amount
            * (1. / dt / 60.)
            * 4.
            * n(tuning, "maxRadialBlurStrength", 1.);
        if (1. - ratio).abs() > 1e-10 {
            result.center = [
                ((current.x - ratio * previous.x) / (1. - ratio)).clamp(-width, 2. * width),
                ((current.y - ratio * previous.y) / (1. - ratio)).clamp(-height, 2. * height),
            ];
        }
    } else if movement[0].hypot(movement[1]) > 0.001
        && movement[0].hypot(movement[1]) / dt > n(tuning, "panVelocityThreshold", 0.)
    {
        let factor = amount * (1. / dt / 60.) * 4. * n(tuning, "maxDirectionalBlurPx", 41.8) / 41.8;
        result.velocity = [movement[0] * factor, movement[1] * factor];
    }
    result
}

const KERNEL: &str = r#"
uniform shader scene;
uniform float2 velocity;
uniform float2 center;
uniform float2 size;
uniform float strength;
half4 main(float2 point) {
    if (strength > 0.000001) {
        float2 uv=point/size;
        float dt=dot(uv,float2(12.9898,78.233));
        float sn=dt-floor(dt/3.14159)*3.14159;
        float offset=fract(sin(sn)*43758.5453);
        float2 dir=(center-point)*strength;
        half4 sum=half4(0);float total=0;
        for(int i=0;i<32;i++) {float t=(float(i)+offset)/32.;float weight=4.*(t-t*t);sum+=scene.eval(point+dir*t)*weight;total+=weight;}
        return sum/total;
    }
    if(length(velocity)<0.01) return scene.eval(point);
    half4 sum=scene.eval(point);
    for(int i=0;i<12;i++){sum+=scene.eval(point+velocity*(float(i)/12.+1./32.-0.5));}
    return sum/13.;
}
"#;
pub struct Filter {
    effect: sk::RuntimeEffect,
}

impl Filter {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            effect: sk::RuntimeEffect::make_for_shader(KERNEL, None)
                .map_err(|e| anyhow::anyhow!("Motion shader: {e}"))?,
        })
    }

    pub fn image_filter(
        &self,
        blur: &Blur,
        width: f64,
        height: f64,
    ) -> anyhow::Result<Option<sk::ImageFilter>> {
        if blur.strength < 0.000001 && blur.velocity[0].hypot(blur.velocity[1]) < 0.01 {
            return Ok(None);
        }
        let mut b = sk::runtime_effect::RuntimeShaderBuilder::new(self.effect.clone());
        b.set_uniform_float(
            "velocity",
            &[blur.velocity[0] as f32, blur.velocity[1] as f32],
        )
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        b.set_uniform_float("center", &[blur.center[0] as f32, blur.center[1] as f32])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        b.set_uniform_float("size", &[width as f32, height as f32])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        b.set_uniform_float("strength", &[blur.strength as f32])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        Ok(sk::image_filters::runtime_shader(&b, "scene", None))
    }
}
