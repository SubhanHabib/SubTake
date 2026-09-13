use crate::project::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub source_start: f64,
    pub source_end: f64,
    pub output_start: f64,
    pub speed: f64,
    pub muted: bool,
}
impl Span {
    pub fn duration(&self) -> f64 {
        (self.source_end - self.source_start) / self.speed
    }
}

pub fn n(v: &Value, key: &str, d: f64) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .unwrap_or(d)
}

/// Output clock is contiguous; trim gaps disappear. Region times remain on the source clock.
pub fn spans(project: &Project, duration: f64) -> Vec<Span> {
    let mut kept = vec![];
    if !project.regions("clipRegions").is_empty() {
        for c in project.regions("clipRegions") {
            let a = n(c, "startMs", 0.) / 1000.;
            let speed = n(c, "speed", 1.).max(0.01);
            let b = (a + (n(c, "endMs", 0.) / 1000. - a) * speed).min(duration);
            if b > a {
                kept.push((a, b, speed, c["muted"].as_bool().unwrap_or(false)));
            }
        }
    } else {
        let mut cuts: Vec<_> = project
            .regions("trimRegions")
            .iter()
            .map(|t| (n(t, "startMs", 0.) / 1000., n(t, "endMs", 0.) / 1000.))
            .collect();
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut at: f64 = 0.;
        for (a, b) in cuts {
            if a > at {
                kept.push((at, a.min(duration), 1., false));
            }
            at = at.max(b).min(duration);
        }
        if at < duration {
            kept.push((at, duration, 1., false));
        }
    }
    kept.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut result = vec![];
    let mut output = 0.;
    for (a, b, base, muted) in kept {
        let mut points = vec![a, b];
        for s in project.regions("speedRegions") {
            for p in [n(s, "startMs", 0.) / 1000., n(s, "endMs", 0.) / 1000.] {
                if p > a && p < b {
                    points.push(p)
                }
            }
        }
        points.sort_by(f64::total_cmp);
        points.dedup();
        for pair in points.windows(2) {
            let mid = (pair[0] + pair[1]) * 500.;
            let speed = project
                .regions("speedRegions")
                .iter()
                .find(|s| n(s, "startMs", 0.) <= mid && n(s, "endMs", 0.) > mid)
                .map(|s| n(s, "speed", base))
                .unwrap_or(base)
                .clamp(0.01, 100.);
            let span = Span {
                source_start: pair[0],
                source_end: pair[1],
                output_start: output,
                speed,
                muted,
            };
            output += span.duration();
            result.push(span);
        }
    }
    result
}
pub fn duration(spans: &[Span]) -> f64 {
    spans
        .last()
        .map(|s| s.output_start + s.duration())
        .unwrap_or(0.)
}
pub fn source_time(spans: &[Span], output: f64) -> f64 {
    spans
        .iter()
        .find(|s| output < s.output_start + s.duration())
        .map(|s| s.source_start + (output - s.output_start).max(0.) * s.speed)
        .unwrap_or_else(|| spans.last().map(|s| s.source_end).unwrap_or(0.))
}
pub fn output_time(spans: &[Span], source: f64) -> f64 {
    for s in spans {
        if source < s.source_start {
            return s.output_start;
        }
        if source <= s.source_end {
            return s.output_start + (source - s.source_start) / s.speed;
        }
    }
    duration(spans)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Camera {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
    pub progress: f64,
    pub target_scale: f64,
    pub follow: bool,
}
pub fn ease_out_zoom(t: f64) -> f64 {
    fn sample(a: f64, b: f64, t: f64) -> f64 {
        3. * a * (1. - t) * (1. - t) * t + 3. * b * (1. - t) * t * t + t * t * t
    }
    fn derivative(a: f64, b: f64, t: f64) -> f64 {
        3. * a * (1. - t) * (1. - t) + 6. * (b - a) * (1. - t) * t + 3. * (1. - b) * t * t
    }
    let target = t.clamp(0., 1.);
    let mut solved = target;
    for _ in 0..8 {
        let x = sample(0.16, 0.3, solved) - target;
        let d = derivative(0.16, 0.3, solved);
        if x.abs() < 1e-6 || d.abs() < 1e-6 {
            break;
        }
        solved -= x / d;
    }
    let (mut lower, mut upper) = (0., 1.);
    solved = solved.clamp(0., 1.);
    for _ in 0..10 {
        let x = sample(0.16, 0.3, solved);
        if (x - target).abs() < 1e-6 {
            break;
        }
        if x < target {
            lower = solved
        } else {
            upper = solved
        }
        solved = (lower + upper) / 2.;
    }
    sample(1., 1., solved)
}
pub fn region_strength(region: &Value, time: f64, enter: f64, exit: f64) -> f64 {
    let time = time - 200.;
    let start = n(region, "startMs", 0.) + 1000. - 1522.575;
    let (mut zoom_in_end, mut zoom_out_start) = (start + enter, n(region, "endMs", 0.) - 500.);
    if zoom_in_end > zoom_out_start {
        let middle = (zoom_in_end + zoom_out_start) / 2.;
        zoom_in_end = middle;
        zoom_out_start = middle;
    }
    if time < start || time > zoom_out_start + exit {
        return 0.;
    }
    if time < zoom_in_end {
        return ease_out_zoom((time - start) / enter);
    }
    if time <= zoom_out_start {
        return 1.;
    }
    1. - ease_out_zoom((time - zoom_out_start) / exit)
}
pub fn camera(project: &Project, time: f64) -> Camera {
    let mut regions: Vec<_> = project.regions("zoomRegions").iter().collect();
    regions.sort_by(|a, b| n(a, "startMs", 0.).total_cmp(&n(b, "startMs", 0.)));
    let pairs: Vec<_> = if project.flag("connectZooms", true) {
        regions
            .windows(2)
            .filter(|p| n(p[1], "startMs", 0.) - n(p[0], "endMs", 0.) <= 1350.)
            .map(|p| (p[0], p[1], n(p[0], "endMs", 0.) + 200.))
            .collect()
    } else {
        vec![]
    };
    let make = |z: &Value, progress: f64| {
        let depth = n(z, "depth", 3.).round().clamp(1., 6.) as usize;
        let target_scale = [1.25, 1.5, 1.8, 2.2, 3.5, 5.][depth - 1];
        let margin = 1. / (2. * target_scale);
        Camera {
            scale: 1. + (target_scale - 1.) * progress,
            target_scale,
            follow: z["mode"] != "manual",
            progress,
            x: n(&z["focus"], "cx", 0.5).clamp(margin, 1. - margin),
            y: n(&z["focus"], "cy", 0.5).clamp(margin, 1. - margin),
        }
    };
    for (_, next, start) in &pairs {
        if time >= start + 1000. && time < n(next, "startMs", 0.) {
            return make(next, 1.);
        }
    }
    let mut selected = None;
    let mut strength = 0.;
    let mut selected_start = -1.;
    for region in regions {
        let outgoing = pairs
            .iter()
            .find(|(current, _, _)| current["id"] == region["id"]);
        let incoming = pairs.iter().find(|(_, next, _)| next["id"] == region["id"]);
        let progress =
            if let Some((_, _, start)) = outgoing.filter(|(_, _, start)| time >= *start - 500.) {
                if time >= *start { 0. } else { 1. }
            } else if let Some((_, _, start)) = incoming {
                if time < *start {
                    0.
                } else if time < n(region, "endMs", 0.) - 300. {
                    1.
                } else {
                    region_strength(
                        region,
                        time,
                        project.number("zoomInDurationMs", 1522.575).max(1.),
                        project.number("zoomOutDurationMs", 1015.05).max(1.),
                    )
                }
            } else {
                region_strength(
                    region,
                    time,
                    project.number("zoomInDurationMs", 1522.575).max(1.),
                    project.number("zoomOutDurationMs", 1015.05).max(1.),
                )
            };
        if progress > 0.
            && (progress > strength
                || (progress == strength && n(region, "startMs", 0.) > selected_start))
        {
            strength = progress;
            selected_start = n(region, "startMs", 0.);
            selected = Some(make(region, progress));
        }
    }
    selected.unwrap_or(Camera {
        scale: 1.,
        x: 0.5,
        y: 0.5,
        progress: 0.,
        target_scale: 1.,
        follow: false,
    })
}
