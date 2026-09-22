//! Native port of Recordly's damped spring and cursor sway math (AGPL-3.0).
//! Reference: src/components/video-editor/videoPlayback/motionSmoothing.ts,
//! originally authored in Recordly by @webadderall.
use crate::project::Project;
use crate::timeline::n;
use serde_json::Value;
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub value: f64,
    velocity: f64,
    initialized: bool,
}
impl Spring {
    pub fn step(&mut self, target: f64, ms: f64, stiffness: f64, damping: f64, mass: f64) -> f64 {
        self.step_with_rest(target, ms, (stiffness, damping, mass), (0.0002, 0.01))
    }
    pub fn step_with_rest(
        &mut self,
        target: f64,
        ms: f64,
        config: (f64, f64, f64),
        rest: (f64, f64),
    ) -> f64 {
        let (k, c, m) = config;
        let (rest_delta, rest_speed) = rest;
        if !self.initialized {
            self.value = target;
            self.initialized = true;
            return target;
        }
        if (target - self.value).abs() <= rest_delta && self.velocity.abs() <= rest_speed {
            self.value = target;
            self.velocity = 0.;
            return target;
        }
        let w = (k / m).sqrt();
        let z = c / (2. * (k * m).sqrt());
        let delta = target - self.value;
        let velocity = -self.velocity;
        let time = ms.clamp(1., 80.) / 1000.;
        let position = |t: f64| {
            if z < 1. {
                let wd = w * (1. - z * z).sqrt();
                target
                    - (-z * w * t).exp()
                        * ((velocity + z * w * delta) / wd * (wd * t).sin()
                            + delta * (wd * t).cos())
            } else if (z - 1.).abs() < 1e-10 {
                target - (-w * t).exp() * (delta + (velocity + w * delta) * t)
            } else {
                let wd = w * (z * z - 1.).sqrt();
                let wt = (wd * t).min(300.);
                target
                    - (-z * w * t).exp()
                        * ((velocity + z * w * delta) * wt.sinh() + wd * delta * wt.cosh())
                        / wd
            }
        };
        let current = position(time);
        if z >= 1.
            && ((self.value <= target && current > target)
                || (self.value >= target && current < target))
        {
            self.value = target;
            self.velocity = 0.;
            return target;
        }
        let velocity = (position(time + 0.0001) - current) / 0.0001;
        if velocity.abs() <= rest_speed && (target - current).abs() <= rest_delta {
            self.value = target;
            self.velocity = 0.;
        } else {
            self.value = current;
            self.velocity = velocity;
        }
        self.value
    }
}
pub fn cursor_config(project: &Project) -> (f64, f64, f64) {
    let s = project.number("cursorSmoothing", 0.67).clamp(0., 2.);
    let (k, c, m) = if s <= 0. {
        (1000., 100., 1.)
    } else if s <= 0.5 {
        let t = s / 0.5;
        ((760. - t * 420.) * 1.12, 34. + t * 24., 0.85 + t * 0.55)
    } else {
        let t = (s - 0.5) / 1.5;
        ((340. - t * 180.) * 1.12, 58. + t * 22., 1.35 + t * 0.45)
    };
    (
        k * project
            .number("cursorSpringStiffnessMultiplier", 1.)
            .clamp(0.25, 3.),
        c * project
            .number("cursorSpringDampingMultiplier", 1.)
            .clamp(0.25, 3.),
        m * project
            .number("cursorSpringMassMultiplier", 1.)
            .clamp(0.25, 3.),
    )
}
pub fn smooth_cursor(project: &Project, points: &[Value]) -> Vec<(f64, f64)> {
    let (k, c, m) = cursor_config(project);
    let (mut x, mut y) = (Spring::default(), Spring::default());
    let mut last = 0.;
    points
        .iter()
        .map(|point| {
            let time = n(point, "timeMs", 0.);
            let dt = if time <= last {
                1000. / 60.
            } else {
                time - last
            };
            last = time;
            if project.number("cursorSmoothing", 0.67) == 0. {
                (n(point, "cx", 0.5), n(point, "cy", 0.5))
            } else {
                (
                    x.step(n(point, "cx", 0.5), dt, k, c, m),
                    y.step(n(point, "cy", 0.5), dt, k, c, m),
                )
            }
        })
        .collect()
}
pub fn sway(dx: f64, dy: f64, ms: f64, amount: f64) -> f64 {
    let distance = dx.hypot(dy);
    if distance < 0.01 || amount <= 0. {
        return 0.;
    }
    let speed = (distance / (ms.clamp(1., 80.) / 1000.) / 1400.).clamp(0., 1.);
    ((dx + dy * 0.65) / distance).clamp(-1., 1.) * speed * 10. * amount * 3.
}

#[derive(Default, Clone, Copy)]
pub struct Follow {
    initialized: bool,
    last: f64,
    x: f64,
    y: f64,
    was_zoomed: bool,
    full: bool,
}
impl Follow {
    pub fn step(
        &mut self,
        cursor: Option<(f64, f64)>,
        time: f64,
        scale: f64,
        strength: f64,
        focus: (f64, f64),
    ) -> (f64, f64) {
        let margin = 0.5 / scale.max(1.);
        let focus = (
            focus.0.clamp(margin, 1. - margin),
            focus.1.clamp(margin, 1. - margin),
        );
        if strength < 0.01 {
            if self.was_zoomed {
                self.was_zoomed = false;
                self.initialized = false;
                self.full = false;
            }
            return focus;
        }
        let Some(cursor) = cursor else {
            return if self.initialized {
                (self.x, self.y)
            } else {
                focus
            };
        };
        if strength >= 0.99 {
            self.full = true;
        }
        if self.full && strength < 0.99 {
            return (self.x, self.y);
        }
        if !self.initialized || !self.was_zoomed || time + 0.5 < self.last {
            self.initialized = true;
            self.was_zoomed = true;
            self.last = time;
            self.x = focus.0;
            self.y = focus.1;
            return focus;
        }
        self.last = time;
        let safe = 0.25 / scale.max(1.);
        if (cursor.0 - self.x).abs() > safe {
            self.x = cursor.0.clamp(margin, 1. - margin)
        }
        if (cursor.1 - self.y).abs() > safe {
            self.y = cursor.1.clamp(margin, 1. - margin)
        }
        (self.x, self.y)
    }
}
pub fn cursor_at(points: &[Value], time: f64) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let i = points
        .partition_point(|p| n(p, "timeMs", 0.) <= time)
        .saturating_sub(1);
    let a = &points[i];
    let b = &points[(i + 1).min(points.len() - 1)];
    let t = ((time - n(a, "timeMs", 0.)) / (n(b, "timeMs", 0.) - n(a, "timeMs", 0.)).max(1.))
        .clamp(0., 1.);
    Some((
        n(a, "cx", 0.5) + (n(b, "cx", 0.5) - n(a, "cx", 0.5)) * t,
        n(a, "cy", 0.5) + (n(b, "cy", 0.5) - n(a, "cy", 0.5)) * t,
    ))
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Transform {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}
/// Canonical media-time sampling makes scrubbing and export use the same motion.
/// No wall-clock state or preceding UI seek can change a rendered frame.
#[derive(Default)]
pub struct CameraTrack {
    key: Option<Value>,
    frames: Vec<Transform>,
    follow: Follow,
    springs: [Spring; 3],
}
impl CameraTrack {
    pub fn at(
        &mut self,
        p: &Project,
        points: &[Value],
        time: f64,
        width: f64,
        height: f64,
        frame: &crate::geometry::Frame,
    ) -> Transform {
        let key = serde_json::json!([
            p.regions("zoomRegions"),
            p.flag("connectZooms", true),
            p.number("zoomInDurationMs", 1522.575),
            p.number("zoomOutDurationMs", 1015.05),
            p.number("zoomSmoothness", 0.5),
            p.flag("zoomClassicMode", false),
            p.number("cameraSpringStiffnessMultiplier", 1.),
            p.number("cameraSpringDampingMultiplier", 1.),
            p.number("cameraSpringMassMultiplier", 1.),
            width,
            height,
            frame.x,
            frame.y,
            frame.width,
            frame.height
        ]);
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.frames.clear();
            self.follow = Follow::default();
            self.springs = [Spring::default(); 3];
        }
        let at = time.max(0.) * 60. / 1000.;
        let last = at.ceil() as usize;
        let smooth = p.number("zoomSmoothness", 0.5).clamp(0., 1.);
        let classic = p.flag("zoomClassicMode", false);
        let config = if smooth == 0. {
            (1000., 100., 1.)
        } else {
            (100. / (smooth * 2.), 21., smooth * 2.)
        };
        let config = (
            config.0
                * p.number("cameraSpringStiffnessMultiplier", 1.)
                    .clamp(0.25, 3.),
            config.1
                * p.number("cameraSpringDampingMultiplier", 1.)
                    .clamp(0.25, 3.),
            config.2 * p.number("cameraSpringMassMultiplier", 1.).clamp(0.25, 3.),
        );
        let rest = if smooth == 0. {
            (0.0001, 0.001)
        } else {
            (0.0005, 0.015)
        };
        while self.frames.len() <= last {
            let t = self.frames.len() as f64 * 1000. / 60.;
            let mut camera = crate::timeline::camera(p, t);
            if camera.follow && !classic && camera.progress > 0. && !points.is_empty() {
                (camera.x, camera.y) = self.follow.step(
                    cursor_at(points, t),
                    t,
                    camera.target_scale,
                    camera.progress,
                    (camera.x, camera.y),
                );
            }
            let target = [
                camera.scale,
                (width / 2. - (frame.x + frame.width * camera.x) * camera.target_scale)
                    * camera.progress,
                (height / 2. - (frame.y + frame.height * camera.y) * camera.target_scale)
                    * camera.progress,
            ];
            let values = if classic {
                target
            } else {
                [
                    self.springs[0].step_with_rest(target[0], 1000. / 60., config, rest),
                    self.springs[1].step_with_rest(target[1], 1000. / 60., config, rest),
                    self.springs[2].step_with_rest(target[2], 1000. / 60., config, rest),
                ]
            };
            self.frames.push(Transform {
                scale: values[0],
                x: values[1],
                y: values[2],
            });
        }
        let a = self.frames[at.floor() as usize];
        let b = self.frames[last];
        let progress = at.fract();
        Transform {
            scale: a.scale + (b.scale - a.scale) * progress,
            x: a.x + (b.x - a.x) * progress,
            y: a.y + (b.y - a.y) * progress,
        }
    }
}

/// Cursor springs are sampled on a fixed media clock, independently of seek order.
#[derive(Default)]
pub struct CursorTrack {
    key: Option<Value>,
    frames: Vec<(f64, f64, f64)>,
    position: [Spring; 2],
    rotation: Spring,
}
impl CursorTrack {
    pub fn at(
        &mut self,
        p: &Project,
        points: &[Value],
        time: f64,
        width: f64,
        height: f64,
    ) -> (f64, f64, f64) {
        let config = cursor_config(p);
        let amount = p.number("cursorSway", 0.4);
        let key = serde_json::json!([config, amount, width, height]);
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.frames.clear();
            self.position = [Spring::default(); 2];
            self.rotation = Spring::default();
        }
        let mut sway_project = p.clone();
        sway_project.set(
            "cursorSmoothing",
            serde_json::json!((p.number("cursorSmoothing", 0.67) * 0.7 + 0.18).clamp(0.15, 2.)),
        );
        let sway_config = cursor_config(&sway_project);
        let sway_config = (
            sway_config.0,
            sway_config.1 * 0.9,
            (sway_config.2 * 0.8).max(0.55),
        );
        let at = time.max(0.) * 60. / 1000.;
        let last = at.ceil() as usize;
        while self.frames.len() <= last {
            let time = self.frames.len() as f64 * 1000. / 60.;
            let target = cursor_at(points, time).unwrap_or((0.5, 0.5));
            let (x, y) = if p.number("cursorSmoothing", 0.67) == 0. {
                target
            } else {
                (
                    self.position[0].step(target.0, 1000. / 60., config.0, config.1, config.2),
                    self.position[1].step(target.1, 1000. / 60., config.0, config.1, config.2),
                )
            };
            let target_rotation = self
                .frames
                .last()
                .map(|&(a, b, _)| {
                    sway((x - a) * width, (y - b) * height, 1000. / 60., amount).to_radians()
                })
                .unwrap_or(0.);
            let rotation = self.rotation.step_with_rest(
                target_rotation,
                1000. / 60.,
                sway_config,
                (0.0005, 0.02),
            );
            self.frames.push((x, y, rotation.to_degrees()));
        }
        let a = self.frames[at.floor() as usize];
        let b = self.frames[last];
        let t = at.fract();
        (
            a.0 + (b.0 - a.0) * t,
            a.1 + (b.1 - a.1) * t,
            a.2 + (b.2 - a.2) * t,
        )
    }
}
