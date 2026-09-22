//! Port of videoPlayback/layoutUtils.ts. Keep padding in percent, not pixels.
use crate::{project::Project, timeline::n};
use serde_json::Value;
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

pub fn frame(
    project: &Project,
    width: f64,
    height: f64,
    video_width: f64,
    video_height: f64,
) -> Frame {
    let padding = project.editor.get("padding").unwrap_or(&Value::Null);
    let legacy = padding.as_f64().unwrap_or(20.);
    let advanced = padding.get("linked").and_then(Value::as_bool) == Some(false);
    let top = n(padding, "top", legacy).clamp(0., if advanced { 250. } else { 100. });
    let bottom = n(padding, "bottom", legacy).clamp(0., if advanced { 250. } else { 100. });
    let left = n(padding, "left", legacy).clamp(0., 100.) * 0.002;
    let right = n(padding, "right", legacy).clamp(0., 100.) * 0.002;
    let crop = project.editor.get("cropRegion").unwrap_or(&Value::Null);
    let vw = video_width * n(crop, "width", 1.).clamp(0.001, 1.);
    let vh = video_height * n(crop, "height", 1.).clamp(0.001, 1.);
    let available_width = width * (1. - left - right);
    let available_height = height * (1. - top.min(100.) * 0.002 - bottom.min(100.) * 0.002);
    let scale = (available_width / vw).min(available_height / vh);
    let fw = vw * scale;
    let fh = vh * scale;
    let x = left * width + (available_width - fw) / 2.;
    let y = if advanced {
        let travel = (height - fh).max(0.);
        (travel / 2. + (top - bottom) / 250. * travel / 2.).clamp(0., travel)
    } else {
        top.min(100.) * 0.002 * height + (available_height - fh) / 2.
    };
    Frame {
        x,
        y,
        width: fw,
        height: fh,
        radius: fw.min(fh) * project.number("borderRadius", 0.).clamp(0., 50.) / 100.,
    }
}
