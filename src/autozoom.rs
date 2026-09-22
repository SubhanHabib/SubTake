//! Explicit-click clustering from timeline/zoomSuggestionUtils.ts.
use crate::{project::Project, timeline::n};
use serde_json::{Value, json};
fn explicit(value: &Value) -> bool {
    matches!(
        value["interactionType"].as_str(),
        Some("click" | "double-click" | "right-click" | "middle-click")
    )
}
fn strength(points: &[Value], click: &Value) -> f64 {
    if click["interactionType"] == "double-click" {
        return 1500.;
    }
    let time = n(click, "timeMs", 0.);
    let x = n(click, "cx", 0.5);
    let y = n(click, "cy", 0.5);
    if let Some(up) = points.iter().find(|p| {
        p["interactionType"] == "mouseup"
            && n(p, "timeMs", 0.) > time
            && n(p, "timeMs", 0.) - time < 3000.
    }) {
        let dx = (n(up, "cx", x) - x).abs();
        let dy = (n(up, "cy", y) - y).abs();
        if n(up, "timeMs", time) - time >= 200. && dx > 0.03 && dx > dy * 1.8 {
            return 1300.;
        }
    }
    let movement = points
        .iter()
        .filter(|p| {
            n(p, "timeMs", 0.) > time + 100.
                && n(p, "timeMs", 0.) <= time + 2000.
                && (p["interactionType"].is_null() || p["interactionType"] == "move")
        })
        .collect::<Vec<_>>();
    if movement.len() < 3 {
        return 1100.;
    }
    let mut distance: f64 = 0.;
    let mut dx = 0.;
    let mut dy = 0.;
    for p in &movement {
        let a = (n(p, "cx", x) - x).abs();
        let b = (n(p, "cy", y) - y).abs();
        distance = distance.max(a.hypot(b));
        dx += a;
        dy += b;
    }
    if distance < 0.02 {
        return 1100.;
    }
    if n(movement.last().unwrap(), "cy", y) - y > 0.03 && dy > dx * 1.5 {
        return 1200.;
    }
    if dx > 0.03 && dx > dy * 1.8 {
        return 1300.;
    }
    900.
}
pub fn suggest(points: &[Value], duration: f64, reserved: &[(f64, f64)]) -> Vec<Value> {
    if duration <= 0. {
        return vec![];
    }
    let mut points = points
        .iter()
        .filter(|p| {
            ["timeMs", "cx", "cy"]
                .iter()
                .all(|k| p[*k].as_f64().is_some_and(f64::is_finite))
        })
        .cloned()
        .collect::<Vec<_>>();
    points.sort_by(|a, b| n(a, "timeMs", 0.).total_cmp(&n(b, "timeMs", 0.)));
    for p in &mut points {
        for (key, max) in [("timeMs", duration), ("cx", 1.), ("cy", 1.)] {
            p[key] = json!(n(p, key, 0.).clamp(0., max));
        }
    }
    let clicks = points
        .iter()
        .filter(|p| explicit(p))
        .map(|p| {
            (
                n(p, "timeMs", 0.).round(),
                n(p, "cx", 0.5),
                n(p, "cy", 0.5),
                strength(&points, p),
            )
        })
        .collect::<Vec<_>>();
    let mut clusters: Vec<(f64, f64, f64, f64, f64)> = vec![];
    for (time, x, y, strength) in clicks {
        if let Some(last) = clusters.last_mut()
            && time - last.1 <= 2500.
        {
            last.1 = last.1.max(time);
            if strength > last.4 {
                last.2 = x;
                last.3 = y;
                last.4 = strength;
            }
            continue;
        }
        clusters.push((time, time, x, y, strength));
    }
    let mut reserved = reserved.to_vec();
    let mut regions = vec![];
    for (start, end, x, y, _) in clusters {
        let a = (start - 500.).max(0.);
        let b = (end + 500.).min(duration);
        if b > a && !reserved.iter().any(|&(c, d)| b > c && a < d) {
            reserved.push((a, b));
            regions.push(
                json!({"startMs":a,"endMs":b,"focus":{"cx":x,"cy":y},"depth":2,"mode":"auto"}),
            );
        }
    }
    regions
}
pub fn from_source(
    p: &Project,
    source: &std::path::Path,
    duration: f64,
) -> anyhow::Result<Vec<Value>> {
    use anyhow::Context;
    let mut path = source.as_os_str().to_os_string();
    path.push(".cursor.json");
    let data: Value = serde_json::from_slice(
        &std::fs::read(std::path::PathBuf::from(path))
            .context("This video has no cursor telemetry")?,
    )?;
    let points = data
        .as_array()
        .or_else(|| data["samples"].as_array())
        .context("Invalid cursor telemetry")?;
    let reserved = p
        .regions("zoomRegions")
        .iter()
        .map(|r| (n(r, "startMs", 0.), n(r, "endMs", 0.)))
        .collect::<Vec<_>>();
    Ok(suggest(points, duration, &reserved))
}
