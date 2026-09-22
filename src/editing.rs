//! Timeline edits are shared commands expressed on the source clock.
use crate::{project::Project, timeline::n};
use anyhow::{Context, Result};
use serde_json::{Value, json};
pub fn move_region(
    p: &mut Project,
    kind: &str,
    id: &str,
    delta: f64,
    mode: i32,
    duration: f64,
) -> Result<()> {
    let r = p
        .regions(kind)
        .iter()
        .find(|r| r["id"] == id)
        .context("Region not found")?;
    let start = n(r, "startMs", 0.);
    let speed = if kind == "clipRegions" {
        n(r, "speed", 1.)
    } else {
        1.
    };
    let end = start + (n(r, "endMs", 0.) - start) * speed;
    let (a, b) = if mode == 1 {
        (
            start,
            (end + delta).clamp(start + 1., duration.max(start + 1.)),
        )
    } else if mode == 2 {
        ((start + delta).clamp(0., (end - 1.).max(0.)), end)
    } else {
        let delta = delta.clamp(-start, (duration - end).max(-start));
        (start + delta, end + delta)
    };
    p.change_region(kind, id, json!({"startMs":a,"endMs":a+(b-a)/speed}))
}

pub fn snap_delta(
    project: &Project,
    kind: &str,
    id: &str,
    delta: f64,
    mode: i32,
    playhead: f64,
    duration: f64,
    threshold: f64,
) -> f64 {
    let Some(region) = project.regions(kind).iter().find(|r| r["id"] == id) else {
        return delta;
    };
    let start = n(region, "startMs", 0.);
    let end = start
        + (n(region, "endMs", 0.) - start)
            * if kind == "clipRegions" {
                n(region, "speed", 1.)
            } else {
                1.
            };
    let mut anchors = vec![0., duration, playhead];
    for key in [
        "zoomRegions",
        "trimRegions",
        "clipRegions",
        "speedRegions",
        "annotationRegions",
        "audioRegions",
        "autoCaptions",
        "nativeMarkers",
    ] {
        for r in project.regions(key) {
            if key == kind && r["id"] == id {
                continue;
            }
            anchors.push(n(r, "startMs", 0.));
            anchors.push(source_end(key, r));
        }
    }
    let target = if mode == 1 {
        end + delta
    } else {
        start + delta
    };
    anchors
        .into_iter()
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
        .filter(|a| (a - target).abs() < threshold)
        .map(|a| delta + a - target)
        .unwrap_or(delta)
}

pub fn source_end(kind: &str, region: &Value) -> f64 {
    let start = n(region, "startMs", 0.);
    start
        + (n(region, "endMs", 0.) - start)
            * if kind == "clipRegions" {
                n(region, "speed", 1.)
            } else {
                1.
            }
}
pub fn move_group(
    p: &mut Project,
    keys: &[(String, String)],
    delta: f64,
    mode: i32,
    duration: f64,
) -> Result<()> {
    let mut delta = delta;
    if mode == 0 {
        let mut first = f64::INFINITY;
        let mut last = 0_f64;
        for (kind, id) in keys {
            let r = p
                .regions(kind)
                .iter()
                .find(|r| r["id"] == *id)
                .context("Selected region no longer exists")?;
            first = first.min(n(r, "startMs", 0.));
            last = last.max(source_end(kind, r));
        }
        delta = delta.clamp(-first, (duration - last).max(-first));
    }
    for (kind, id) in keys {
        move_region(p, kind, id, delta, mode, duration)?;
    }
    Ok(())
}

/// A crop that remains inside the source even for malformed imported settings.
pub fn crop_rect(project: &Project) -> [f64; 4] {
    let crop = project.editor.get("cropRegion").unwrap_or(&Value::Null);
    let x = n(crop, "x", 0.).clamp(0., 0.99);
    let y = n(crop, "y", 0.).clamp(0., 0.99);
    [
        x,
        y,
        n(crop, "width", 1.).clamp(0.01, 1. - x),
        n(crop, "height", 1.).clamp(0.01, 1. - y),
    ]
}
pub fn adjust_crop(project: &mut Project, dx: f64, dy: f64, resize: bool) -> Result<()> {
    anyhow::ensure!(
        dx.is_finite() && dy.is_finite(),
        "Crop movement must be finite"
    );
    let [mut x, mut y, mut width, mut height] = crop_rect(project);
    if resize {
        width = (width + dx).clamp(0.01, 1. - x);
        height = (height + dy).clamp(0.01, 1. - y);
    } else {
        x = (x + dx).clamp(0., 1. - width);
        y = (y + dy).clamp(0., 1. - height);
    }
    project.set(
        "cropRegion",
        json!({"x":x,"y":y,"width":width,"height":height}),
    );
    Ok(())
}
