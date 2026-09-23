//! Preview playback: the off-thread frame renderer, seeking, the clock and
//! the full property refresh that follows every model change.

use super::*;

pub(super) struct FrameRequest {
    project: Project,
    path: PathBuf,
    info: MediaInfo,
    time: f64,
    epoch: u64,
    selected: Option<(String, String)>,
    source_revision: u64,
    panel: String,
    width: u32,
    height: u32,
}

pub(super) struct Preview {
    slot: Arc<(Mutex<Option<FrameRequest>>, Condvar)>,
}

impl Preview {
    pub(super) fn new() -> Self {
        let slot: Arc<(Mutex<Option<FrameRequest>>, Condvar)> =
            Arc::new((Mutex::new(None), Condvar::new()));
        let worker = slot.clone();
        std::thread::spawn(move || {
            let mut scene: Option<(PathBuf, u32, u32, u64, Scene)> = None;
            loop {
                let request = {
                    let (lock, wake) = &*worker;
                    let mut pending = lock.lock().unwrap();
                    while pending.is_none() {
                        pending = wake.wait(pending).unwrap()
                    }
                    pending.take().unwrap()
                };
                let result = (|| -> Result<(Vec<u8>, Option<[f32; 5]>)> {
                    if scene.as_ref().is_none_or(|(path, w, h, revision, _)| {
                        *path != request.path
                            || *w != request.width
                            || *h != request.height
                            || *revision != request.source_revision
                    }) {
                        scene = Some((
                            request.path.clone(),
                            request.width,
                            request.height,
                            request.source_revision,
                            Scene::new(
                                request.path.clone(),
                                request.info.clone(),
                                request.width,
                                request.height,
                            )?,
                        ));
                    }
                    let scene = &mut scene.as_mut().unwrap().4;
                    let mut drawing = request.project.clone();
                    if request.panel == "Crop" {
                        drawing.set("cropRegion", json!({"x":0,"y":0,"width":1,"height":1}));
                        drawing.set("padding", json!(0));
                        drawing.set("borderRadius", json!(0));
                        drawing.set("shadowIntensity", json!(0));
                        drawing.set("wallpaper", json!("#000000"));
                        drawing.set("showCursor", json!(false));
                        for key in ["zoomRegions", "annotationRegions", "autoCaptions"] {
                            drawing.set(key, json!([]));
                        }
                        drawing.set("webcam", json!({"enabled":false}));
                    }
                    let pixels = scene.render(&drawing, request.time)?;
                    let bounds = scene.edit_bounds(
                        &request.project,
                        request.time,
                        request.selected.as_ref(),
                        &request.panel,
                    );
                    Ok((pixels, bounds))
                })();
                post(move |app, ui| {
                    if app.epoch != request.epoch {
                        return;
                    }
                    match result {
                        Ok((pixels, bounds)) => {
                            ui.set_edit_visible(bounds.is_some());
                            if let Some([x, y, w, h, scale]) = bounds {
                                ui.set_edit_x(x);
                                ui.set_edit_y(y);
                                ui.set_edit_width(w);
                                ui.set_edit_height(h);
                                ui.set_edit_scale(scale);
                            }
                            let buffer =
                                ui_runtime::SharedPixelBuffer::<ui_runtime::Rgba8Pixel>::clone_from_slice(
                                    &pixels,
                                    request.width,
                                    request.height,
                                );
                            ui.set_preview(ui_runtime::Image::from_rgba8(buffer));
                        }
                        Err(e) => {
                            app.stop(ui);
                            ui.set_status(format!("Preview: {e:#}"));
                        }
                    }
                });
            }
        });
        Self { slot }
    }

    fn request(&self, frame: FrameRequest) {
        let (lock, wake) = &*self.slot;
        *lock.lock().unwrap() = Some(frame);
        wake.notify_one();
    }
}

impl App {
    pub(super) fn request(&self) {
        if let (Some(h), Some(source), Some(info)) = (&self.history, &self.source, &self.info) {
            let panel = STATE.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .and_then(|(_, ui)| ui.upgrade())
                    .map(|ui| ui.get_panel().to_string())
                    .unwrap_or_default()
            });
            let aspect = if panel == "Crop" {
                info.width as f64 / info.height as f64
            } else {
                aspect_ratio(&h.project, info)
            };
            let physical_width = STATE.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .and_then(|(_, ui)| ui.upgrade())
                    .map(|ui| {
                        ui.get_preview_pixel_width() as f64 * ui.window().scale_factor() as f64
                    })
                    .unwrap_or(info.width as f64)
            });
            // Match display pixels, retaining only the renderer's allocation safety bound.
            let width = physical_width.ceil().max(2.).min(8192.).min(8192. * aspect) as u32;
            let height = (width as f64 / aspect).round().max(2.) as u32;
            self.preview.request(FrameRequest {
                project: h.project.clone(),
                path: source.clone(),
                info: info.clone(),
                time: self.source_time.min((info.duration - 0.001).max(0.)),
                epoch: self.epoch,
                selected: self.selected.clone(),
                source_revision: self.source_revision,
                panel,
                width,
                height,
            });
        }
    }

    pub(super) fn stop(&mut self, ui: &EditorWindow) {
        self.playback.stop();
        self.started = None;
        self.audio_cancel.store(true, Ordering::Relaxed);
        ui.set_playing(false);
    }

    pub(super) fn seek(&mut self, ui: &EditorWindow, time: f64) {
        self.stop(ui);
        self.epoch += 1;
        self.source_time = time.clamp(0., self.info.as_ref().map(|i| i.duration).unwrap_or(0.));
        self.update_time(ui);
        self.request();
    }

    pub(super) fn update_time(&self, ui: &EditorWindow) {
        ui.set_playhead(self.source_time as f32);
        let visible = ui.get_timeline_visible();
        let offset = ui.get_timeline_offset();
        if (self.source_time as f32) < offset || (self.source_time as f32) > offset + visible {
            ui.set_timeline_offset(
                ((self.source_time as f32 - visible * 0.1).max(0.))
                    .min((ui.get_duration() - visible).max(0.)),
            );
        }
        let t = self.source_time;
        ui.set_time_label(format!(
            "{:02}:{:06.3} / {:02}:{:06.3}",
            (t / 60.) as u64,
            t % 60.,
            (ui.get_duration() / 60.) as u64,
            ui.get_duration() % 60.
        ));
    }

    pub(super) fn refresh(&self, ui: &EditorWindow) {
        ui.set_language(self.preferences.language.as_str().into());
        ui.set_appearance(self.preferences.appearance.as_str().into());
        ui.set_auto_apply_zooms(self.preferences.auto_apply_zooms);
        ui.set_look_choice(
            self.history
                .as_ref()
                .map(|h| subtake_native::presets::appearance_choice(&h.project))
                .unwrap_or("")
                .into(),
        );
        ui.set_connect_zooms(
            self.history
                .as_ref()
                .map(|h| {
                    h.project
                        .editor
                        .get("connectZooms")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .unwrap_or(true),
        );
        ui.set_motion_choice(
            self.history
                .as_ref()
                .map(|h| subtake_native::presets::motion_choice(&h.project))
                .unwrap_or("")
                .into(),
        );
        ui.set_can_undo(self.history.as_ref().is_some_and(|h| h.can_undo()));
        ui.set_can_redo(self.history.as_ref().is_some_and(|h| h.can_redo()));
        let aspect = self
            .history
            .as_ref()
            .map(|h| h.project.text("aspectRatio", "native"))
            .unwrap_or("native");
        ui.set_aspect_index(
            ["native", "16:9", "9:16", "1:1", "4:3", "3:2"]
                .iter()
                .position(|a| *a == aspect)
                .unwrap_or(0) as i32,
        );
        ui.set_background_value(
            self.history
                .as_ref()
                .map(|h| h.project.text("wallpaper", "#17171c"))
                .unwrap_or("#17171c")
                .into(),
        );
        // The inspector's panels, in the order the rail lists them.
        //
        // TODO(redesign): the "Stage" handoff draws three of these — Frame,
        // Recent and Wallpapers; its fourth, Presets, is a dialog now — and
        // round 2 draws Export, Selection, Cursor and Camera (Webcam). The rest are marked below;
        // they are carried into the new design by the
        // generic field renderer rather than left on the old one, so they
        // are correct but undesigned: their grouping, their density and
        // which of them the rail should still offer are open questions for
        // the designer. The order is load-bearing — `panel_index` is a
        // position in this list, mirrored in `src/ui_state.rs` — so the
        // marking is per line rather than a regrouping.
        ui.set_panel_index(
            [
                "Frame",       // drawn
                "Cursor",      // drawn
                "Webcam",      // drawn, as Camera
                "Captions",    // not drawn
                "Selection",   // drawn
                "Recording",   // not drawn
                "Export",      // drawn
                "Audio",       // not drawn
                "Preferences", // not drawn
                "Recent",      // drawn
                "Wallpapers",  // drawn
                "Crop",        // not drawn
                "Shortcuts",   // not drawn
            ]
            .iter()
            .position(|p| *p == ui.get_panel().as_str())
            .unwrap_or(0) as i32,
        );
        ui.set_has_video(self.history.is_some());
        if let Some(history) = &self.history {
            ui.set_dirty(history.dirty());
            let title = self
                .document
                .as_deref()
                .or(self.source.as_deref())
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy();
            ui.set_document_title(title.as_ref().into());
            if let Some(info) = &self.info {
                ui.set_preview_aspect(if ui.get_panel() == "Crop" {
                    info.width as f32 / info.height as f32
                } else {
                    aspect_ratio(&history.project, info) as f32
                });
            }
            ui.set_duration(self.info.as_ref().map(|i| i.duration as f32).unwrap_or(1.));
            let mut regions = vec![];
            let selected_keys = self.selected_keys();
            for (key, row, tint, title) in [
                // Track tints identify a clip's category, so unlike the
                // controls they stay coloured; the chrome around them is
                // achromatic, which is what lets them read at all.
                ("zoomRegions", 0, "#397afa", "Zoom"),
                ("trimRegions", 1, "#ee5261", "Trim"),
                ("clipRegions", 1, "#357c65", "Clip"),
                ("speedRegions", 1, "#dc922d", "Speed"),
                ("annotationRegions", 2, "#cbb44f", "Annotation"),
                ("autoCaptions", 3, "#6396dc", "Caption"),
                ("audioRegions", 4, "#a468e9", "Audio"),
                ("nativeMarkers", 0, "#f5bb6b", "◆"),
            ] {
                // The take itself (`Region::TAKE_CLIP`, `TAKE_AUDIO`). Not
                // drawn by the design, which shows a take already split into
                // clips and its sound as a voice-over.
                let duration = self.info.as_ref().map_or(0., |i| i.duration);
                let [red, green, blue, _] = subtake_native::project::parse_color(tint);
                let take = |id: String, start: f64, end: f64| Region {
                    id,
                    kind: if key == "audioRegions" {
                        Region::TAKE_AUDIO
                    } else {
                        Region::TAKE_CLIP
                    }
                    .into(),
                    label: "Recording".into(),
                    start: start as f32,
                    end: end as f32,
                    row,
                    tint: ui_runtime::Color::from_rgb_u8(red, green, blue),
                    selected: false,
                    arrow: false,
                };
                if key == "clipRegions" && history.project.regions(key).is_empty() {
                    for (i, span) in timeline::spans(&history.project, duration)
                        .iter()
                        .enumerate()
                    {
                        regions.push(take(
                            format!("take-{i}"),
                            span.source_start,
                            span.source_end,
                        ));
                    }
                }
                if key == "audioRegions" && self.info.as_ref().is_some_and(|i| i.audio_tracks > 0) {
                    regions.push(take("take".into(), 0., duration));
                }
                for r in history.project.regions(key) {
                    let [red, green, blue, _] = subtake_native::project::parse_color(tint);
                    // A speed region is labelled by its speed, `2×`.
                    let speed = format!("{}×", n(r, "speed", 1.));
                    regions.push(Region {
                        id: r["id"].as_str().unwrap_or("").into(),
                        kind: key.into(),
                        label: r["text"]
                            .as_str()
                            .or(r["textContent"].as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(if key == "speedRegions" { &speed } else { title })
                            .into(),
                        start: (n(r, "startMs", 0.) / 1000.) as f32,
                        end: ((n(r, "startMs", 0.)
                            + (n(r, "endMs", 0.) - n(r, "startMs", 0.))
                                * if key == "clipRegions" {
                                    n(r, "speed", 1.)
                                } else {
                                    1.
                                })
                            / 1000.) as f32,
                        row,
                        tint: ui_runtime::Color::from_rgb_u8(red, green, blue),
                        selected: selected_keys
                            .iter()
                            .any(|(kind, id)| kind == key && r["id"] == *id),
                        arrow: key == "annotationRegions" && r["type"] == "figure",
                    });
                }
            }
            // Put overlapping overlays on separate visible lanes while preserving source timing.
            let mut labels = Vec::new();
            let base_rows = regions.iter().map(|r| r.row).collect::<Vec<_>>();
            for (base, label) in ["Zoom", "Clip", "Annotation", "Caption", "Audio"]
                .into_iter()
                .enumerate()
            {
                let mut indices = regions
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| base_rows[*i] == base as i32)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                indices.sort_by(|a, b| regions[*a].start.total_cmp(&regions[*b].start));
                let mut ends = vec![f32::NEG_INFINITY];
                for index in indices {
                    let lane = if base == 2 || base == 4 {
                        ends.iter()
                            .position(|end| *end <= regions[index].start)
                            .unwrap_or(ends.len())
                    } else {
                        0
                    };
                    if lane == ends.len() {
                        ends.push(f32::NEG_INFINITY);
                    }
                    ends[lane] = regions[index].end;
                    regions[index].row = (labels.len() + lane) as i32;
                }
                labels.extend((0..ends.len()).map(|_| SharedString::from(label)));
            }
            ui.set_track_labels(ModelRc::new(VecModel::from(labels)));
            ui.set_regions(ModelRc::new(VecModel::from(regions)));
            ui.set_selected_id(
                self.selected
                    .as_ref()
                    .map(|s| s.1.as_str())
                    .unwrap_or("")
                    .into(),
            );
        }
        ui.set_saved_presets(ModelRc::new(VecModel::from(
            self.presets
                .iter()
                .map(|p| {
                    SharedString::from(p.file_stem().unwrap_or_default().to_string_lossy().as_ref())
                })
                .collect::<Vec<_>>(),
        )));
        if self.history.is_none() {
            ui.set_recents(ModelRc::new(VecModel::from(self.recents())));
            ui.set_dirty(false);
            ui.set_document_title("Untitled".into());
            ui.set_duration(0.);
            ui.set_regions(ModelRc::default());
            ui.set_edit_visible(false);
        }
        ui.set_fields(ModelRc::new(VecModel::from(self.fields(&ui.get_panel()))));
        self.update_time(ui);
    }
}

pub(super) fn aspect_ratio(project: &Project, info: &MediaInfo) -> f64 {
    if project.text("aspectRatio", "native") == "native" {
        let crop = project.editor.get("cropRegion").unwrap_or(&Value::Null);
        return (info.width as f64 * n(crop, "width", 1.)
            / (info.height as f64 * n(crop, "height", 1.)))
        .clamp(0.25, 4.);
    }
    project
        .text("aspectRatio", "16:9")
        .split_once(':')
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .filter(|v| v.is_finite() && *v >= 0.25 && *v <= 4.)
        .unwrap_or(info.width as f64 / info.height as f64)
}
