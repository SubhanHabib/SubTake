//! Process startup: wire the windows to `App`, install the native hooks and
//! run the event loop.

use super::*;

pub fn run(path: Option<PathBuf>) -> Result<()> {
    let ui = EditorWindow::new()?;
    ui.set_mac_titlebar(cfg!(target_os = "macos"));
    ui.on_translate(|text, locale| subtake_native::localization::translate(&text, &locale));
    let state = Rc::new(RefCell::new(App::new()));
    STATE.with(|s| *s.borrow_mut() = Some((state.clone(), ui.as_weak())));
    // A walkthrough is filmed in one appearance from its first frame.
    if let Ok(appearance) = std::env::var("SUBTAKE_WALKTHROUGH") {
        state.borrow_mut().preferences.appearance = appearance;
    }
    let wallpaper_paths = state.borrow().wallpapers.clone();
    std::thread::spawn(move || {
        let tiles = wallpaper_paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| {
                let relative = path
                    .strip_prefix(media::resources().join("public/wallpapers"))
                    .ok()?;
                let bundled = media::resources().join("assets/wallpaper-thumbnails");
                let thumbnail_root = if bundled.is_dir() {
                    bundled
                } else {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/wallpaper-thumbnails")
                };
                let thumbnail = thumbnail_root.join(relative);
                let img = image::open(if thumbnail.is_file() {
                    &thumbnail
                } else {
                    path
                })
                .ok()?
                .thumbnail(160, 100)
                .to_rgba8();
                // The same string picking it writes into the project.
                let value = path
                    .strip_prefix(media::resources().join("public"))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                Some((
                    index,
                    path.file_stem()?.to_string_lossy().replace(['-', '_'], " "),
                    value,
                    img.width(),
                    img.height(),
                    img.into_raw(),
                ))
            })
            .collect::<Vec<_>>();
        post(move |_, ui| {
            ui.set_wallpapers(ModelRc::new(VecModel::from(
                tiles
                    .into_iter()
                    .map(|(index, title, value, w, h, pixels)| Wallpaper {
                        key: format!("wallpaper-{index}"),
                        title,
                        value,
                        source: ui_runtime::Image::from_rgba8(ui_runtime::SharedPixelBuffer::<
                            ui_runtime::Rgba8Pixel,
                        >::clone_from_slice(
                            &pixels, w, h
                        )),
                    })
                    .collect::<Vec<_>>(),
            )));
        });
    });
    if let Err(e) = state.borrow_mut().register_hotkeys() {
        ui.set_status(format!("Global shortcuts: {e}"));
    }
    global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(
        |event: global_hotkey::GlobalHotKeyEvent| {
            if event.state == global_hotkey::HotKeyState::Pressed {
                post(move |app, ui| {
                    let action = app
                        .hotkey_ids
                        .iter()
                        .find(|(id, _)| *id == event.id)
                        .map(|(_, a)| a.clone());
                    if let Some(mut action) = action {
                        if action == "record" && app.recording.is_some() {
                            action = "stop-recording".into();
                        } else if action == "record"
                            && app
                                .launcher
                                .as_ref()
                                .is_some_and(|l| l.window().is_visible())
                            && !app.sources.is_empty()
                        {
                            action = "start-recording".into();
                        }
                        let result = app.action(ui, &action);
                        report(ui, result);
                    }
                });
            }
        },
    ));
    #[cfg(target_os = "macos")]
    unsafe {
        subtake_install_document_events(open_document_event);
    }
    ui.window().on_drop_file(|path| {
        post(move |app, ui| {
            if !ui.get_busy() && app.recording.is_none() && app.can_replace(ui) {
                let result = app.load(ui, path);
                report(ui, result);
            }
        });
    });
    let tray = AppTray::new()?;
    #[cfg(not(target_os = "macos"))]
    tray.show()?;
    tray.on_action(|action| {
        with_app(|app, ui| {
            let result = app.action(ui, &action);
            report(ui, result);
        })
    });
    state.borrow_mut().tray = Some(tray);
    ui.on_action(|a| {
        with_app(|app, ui| {
            let result = app.action(ui, &a);
            report(ui, result);
        })
    });
    ui.on_seek(|time| with_app(|app, ui| app.seek(ui, time as f64)));
    {
        let preferences = &state.borrow().preferences;
        if let (Some(lanes), Some(inspector)) =
            (preferences.lane_height, preferences.inspector_width)
        {
            ui.set_saved_layout(lanes, inspector);
        }
    }
    ui.on_layout_change(|lanes, inspector| {
        with_app(|app, _| {
            app.preferences.lane_height = Some(lanes);
            app.preferences.inspector_width = Some(inspector);
            if let Err(error) = app.preferences.save() {
                eprintln!("Save the editor layout: {error:#}");
            }
        })
    });
    ui.on_panel_change(|panel| {
        with_app(|app, ui| {
            ui.set_panel(panel.clone());
            if panel == "Recent" {
                app.recoveries = subtake_native::recovery::list().unwrap_or_default();
                let result = app.reload_library();
                report(ui, result);
            }
            app.refresh(ui);
            app.epoch += 1;
            app.request();
        })
    });
    ui.on_field_change(|key, value| {
        with_app(|app, ui| {
            let result = app.field(ui, &key, &value);
            report(ui, result);
        })
    });
    ui.on_select_region(|kind, id, extend| {
        with_app(|app, ui| {
            let key = (kind.to_string(), id.to_string());
            let mut keys = app.selected_keys();
            if extend {
                if keys.contains(&key) {
                    keys.retain(|k| k != &key);
                } else {
                    keys.push(key.clone());
                }
            } else if !keys.contains(&key) {
                keys = vec![key.clone()];
            }
            app.selected = if keys.contains(&key) {
                Some(key)
            } else {
                keys.last().cloned()
            };
            app.extra_selection = keys;
            app.show_selection(ui);
            app.refresh(ui);
            app.epoch += 1;
            app.request();
        })
    });
    ui.on_move_region(|kind, id, delta, mode| {
        with_app(|app, ui| {
            if delta.abs() < 0.005 {
                return;
            }
            let duration = app.info.as_ref().map(|i| i.duration * 1000.).unwrap_or(0.);
            let mut snapping = match app.project() {
                Ok(p) => p.clone(),
                Err(_) => return,
            };
            if mode == 0 {
                for (other_kind, other_id) in app.selected_keys() {
                    if other_kind != kind.as_str() || other_id != id.as_str() {
                        let _ = snapping.remove_region(&other_kind, &other_id);
                    }
                }
            }
            let delta = if ui.get_snap() {
                subtake_native::editing::snap_delta(
                    &snapping,
                    &kind,
                    &id,
                    delta as f64 * 1000.,
                    mode,
                    app.source_time * 1000.,
                    duration,
                    ui.get_timeline_visible() as f64 * 5.,
                ) / 1000.
            } else {
                delta as f64
            };
            let keys = if mode == 0 {
                app.selected_keys()
            } else {
                vec![(kind.to_string(), id.to_string())]
            };
            let result = app.edit(ui, |p| {
                subtake_native::editing::move_group(p, &keys, delta * 1000., mode, duration)
            });
            report(ui, result);
        })
    });
    ui.on_canvas_edit(|dx,dy,resize| {
        with_app(|s,ui| {
            let result=(||->Result<()> {
                ensure!(ui.get_edit_visible(),"Select a visible annotation or webcam");
                let scale=(ui.get_edit_scale() as f64).max(0.01);
                let (dx,dy)=(dx as f64,dy as f64);
                if ui.get_panel()=="Crop" {
                    return s.edit(ui,|p| subtake_native::editing::adjust_crop(p,dx,dy,resize));
                }
                if ui.get_panel()=="Webcam" {
                    let aspect=aspect_ratio(s.project()?,s.info.as_ref().unwrap());
                    let w=960.;let h=(w/aspect).round().clamp(100.,1920.);let unit=w/1920.;
                    let x=ui.get_edit_x() as f64;let y=ui.get_edit_y() as f64;
                    let bw=ui.get_edit_width() as f64;let bh=ui.get_edit_height() as f64;
                    s.edit(ui,|p| {
                        let mut wc=p.editor.get("webcam").cloned().unwrap_or(json!({}));
                        if resize {wc["width"]=json!((n(&wc,"width",40.)+dx*w/(w.min(h)*scale)*100.).clamp(5.,100.));wc["height"]=json!((n(&wc,"height",40.)+dy*h/(w.min(h)*scale)*100.).clamp(5.,100.));}
                        else {let margin=n(&wc,"margin",24.)*unit;wc["positionPreset"]=json!("custom");wc["positionX"]=json!(((x+dx-margin/w)/(1.-bw-2.*margin/w).max(0.001)).clamp(0.,1.));wc["positionY"]=json!(((y+dy-margin/h)/(1.-bh-2.*margin/h).max(0.001)).clamp(0.,1.));}
                        p.set("webcam",wc);Ok(())
                    })
                } else {
                    let (kind,id)=s.selected.clone().context("Select an annotation")?;
                    ensure!(kind=="annotationRegions","Select an annotation");
                    s.edit(ui,|p| {let r=p.regions(&kind).iter().find(|r|r["id"]==id).context("Annotation no longer exists")?;
                        let patch=if resize {json!({"size":{"width":(n(&r["size"],"width",30.)+dx/scale*100.).clamp(1.,120.),"height":(n(&r["size"],"height",20.)+dy/scale*100.).clamp(1.,120.)}})}else{json!({"position":{"x":(n(&r["position"],"x",50.)+dx/scale*100.).clamp(-100.,200.),"y":(n(&r["position"],"y",50.)+dy/scale*100.).clamp(-100.,200.)}})};
                        p.change_region(&kind,&id,patch)
                    })
                }
            })();report(ui,result);
        })
    });
    ui.on_preview_click(|x, y| {
        with_app(|app, ui| {
            if ui.get_panel() == "Crop" {
                return;
            }
            if let Some((kind, id)) = app.selected.clone()
                && kind == "zoomRegions"
            {
                let result = (|| -> Result<()> {
                    let p = app.project()?;
                    let info = app.info.as_ref().context("Open a video first")?;
                    let width = 960.;
                    let height = (width / aspect_ratio(p, info)).round().clamp(100., 1920.);
                    let frame = subtake_native::geometry::frame(
                        p,
                        width,
                        height,
                        info.width as f64,
                        info.height as f64,
                    );
                    let mut sidecar = app.source.as_ref().unwrap().as_os_str().to_os_string();
                    sidecar.push(".cursor.json");
                    let telemetry: Value = std::fs::read(PathBuf::from(sidecar))
                        .ok()
                        .and_then(|b| serde_json::from_slice(&b).ok())
                        .unwrap_or(Value::Null);
                    let samples = telemetry
                        .as_array()
                        .or_else(|| telemetry["samples"].as_array())
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let camera = subtake_native::motion::CameraTrack::default().at(
                        p,
                        samples,
                        app.source_time * 1000.,
                        width,
                        height,
                        &frame,
                    );
                    let cx = (((x as f64 * width - camera.x) / camera.scale - frame.x)
                        / frame.width)
                        .clamp(0., 1.);
                    let cy = (((y as f64 * height - camera.y) / camera.scale - frame.y)
                        / frame.height)
                        .clamp(0., 1.);
                    app.edit(ui, |p| {
                        p.change_region(&kind, &id, json!({"focus":{"cx":cx,"cy":cy}}))
                    })
                })();
                report(ui, result);
            } else if ui.get_panel() != "Webcam" {
                // The topmost annotation under the press is selected, as a
                // click on its region would; a press on none lets go of one.
                let hit = app
                    .shown_annotations
                    .iter()
                    .rev()
                    .find(|(_, [ax, ay, aw, ah])| {
                        (*ax..=ax + aw).contains(&x) && (*ay..=ay + ah).contains(&y)
                    });
                if let Some((id, _)) = hit {
                    app.extra_selection.clear();
                    app.selected = Some(("annotationRegions".into(), id.clone()));
                    app.show_selection(ui);
                    app.refresh(ui);
                } else if app
                    .selected
                    .as_ref()
                    .is_some_and(|(kind, _)| kind == "annotationRegions")
                {
                    app.deselect(ui);
                }
            }
        })
    });
    ui.on_keyboard(|key, command, shift, alt| {
        let mut handled = false;
        with_app(|app, ui| {
            let configured = subtake_native::shortcuts::action(
                &app.preferences.editor_shortcuts,
                &key,
                command,
                shift,
                alt,
            );
            let action = if let Some(action) = configured.as_deref() {
                Some(action)
            } else if command && !alt {
                match key.to_lowercase().as_str() {
                    "o" => Some("open"),
                    "a" => Some("select-all"),
                    "c" => Some("copy"),
                    "x" => Some("cut"),
                    "v" => Some("paste"),
                    "d" if shift => Some("duplicate"),
                    "b" => Some("split-clip"),
                    "s" => Some(if shift { "save-as" } else { "save" }),
                    "z" => Some(if shift { "redo" } else { "undo" }),
                    _ => None,
                }
            } else if key == " " {
                Some("play")
            } else if matches!(key.as_str(), "delete" | "backspace" | "\u{7f}" | "\u{8}") {
                Some("delete")
            } else {
                None
            };
            if let Some(action) = action {
                handled = true;
                let result = app.action(ui, action);
                report(ui, result);
            }
        });
        handled
    });
    ui.window().on_close_requested(|| {
        let mut close = false;
        with_app(|app, ui| {
            if app.recording.is_some() || ui.get_busy() {
                ui.set_status(
                    "Finish or cancel the current operation before closing SubTake.".into(),
                );
                return;
            }
            close = true;
            app.stop(ui);
            platform::set_editor_active(false);
        });
        if close {
            ui_runtime::CloseRequestResponse::HideWindow
        } else {
            ui_runtime::CloseRequestResponse::KeepWindowShown
        }
    });
    if let Some(snapshot) = std::env::var_os("SUBTAKE_UI_SNAPSHOT") {
        if let Ok(language) = std::env::var("SUBTAKE_UI_LANGUAGE") {
            ensure!(
                subtake_native::localization::LOCALES.contains(&language.as_str()),
                "Unsupported smoke-test language"
            );
            state.borrow_mut().preferences.language = language;
        }
        let smoke_ui = ui.as_weak();
        Timer::single_shot(Duration::from_secs(3), move || {
            if let Some(ui) = smoke_ui.upgrade() {
                // Exercise controller callbacks. Native pointer hit testing
                // requires separate GPUI view interaction validation.
                for panel in ["Cursor", "Webcam", "Frame"] {
                    ui.invoke_panel_change(panel.into());
                    if ui.get_panel() != panel {
                        eprintln!("UI_SMOKE_FAILED: panel callback did not open {panel}");
                        std::process::exit(1);
                    }
                }
            }
            with_app(|app, ui| {
                let result = (|| -> Result<()> {
                    ensure!(app.history.is_some(), "UI fixture did not load");
                    if let Some(size) = std::env::var("SUBTAKE_UI_SIZE").ok().and_then(|v| {
                        v.split_once('x').and_then(|(a, b)| {
                            Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
                        })
                    }) {
                        ui.window()
                            .set_size(ui_runtime::LogicalSize::new(size.0, size.1));
                    }
                    // Exercise discovery completion, failure, empty results, cancellation and
                    // configuration reopening without recording the user's desktop.
                    let fixtures = vec![
                        json!({"kind":"display","nativeId":1,"name":"Built-in display · 1920 × 1080"}),
                        json!({"kind":"window","nativeId":2,"name":"SubTake — demo window"}),
                    ];
                    for (result, cancelled) in [
                        (Err(anyhow::anyhow!("Permission denied")), false),
                        (Ok(vec![]), false),
                        (Ok(fixtures.clone()), true),
                        (Ok(fixtures.clone()), false),
                    ] {
                        ui.set_busy(true);
                        ui.set_sources_loading(true);
                        app.finish_sources(ui, result, cancelled);
                        ensure!(
                            !ui.get_busy() && !ui.get_sources_loading(),
                            "Source discovery left recording disabled"
                        );
                    }
                    ui.set_source_index(1);
                    app.finish_sources(ui, Ok(fixtures.clone()), false);
                    ensure!(
                        ui.get_source_index() == 1,
                        "Refresh changed selected recording source"
                    );
                    app.action(ui, "record")?;
                    ensure!(
                        ui.get_panel() == "Recording" && app.recording.is_none() && !ui.get_busy(),
                        "Record must open ready configuration without starting capture"
                    );
                    ensure!(!ui.window().is_visible(), "Record should hide the editor");
                    app.show_editor(ui)?;
                    ui.set_panel("Frame".into());
                    app.refresh(ui);
                    let snapshot_project = app.project()?.clone();
                    let count = app.project()?.regions("zoomRegions").len();
                    app.action(ui, "add-zoom")?;
                    ensure!(
                        app.project()?.regions("zoomRegions").len() == count + 1,
                        "Add zoom callback failed"
                    );
                    app.action(ui, "undo")?;
                    ensure!(
                        app.project()?.regions("zoomRegions").len() == count,
                        "Undo failed"
                    );
                    app.action(ui, "redo")?;
                    ensure!(
                        app.project()?.regions("zoomRegions").len() == count + 1,
                        "Redo failed"
                    );
                    app.action(ui, "copy")?;
                    app.seek(ui, 0.8);
                    app.action(ui, "paste")?;
                    ensure!(
                        app.project()?.regions("zoomRegions").len() == count + 2,
                        "Paste failed"
                    );
                    app.action(ui, "cut")?;
                    ensure!(
                        app.project()?.regions("zoomRegions").len() == count + 1,
                        "Cut failed"
                    );
                    app.action(ui, "split-clip")?;
                    ensure!(
                        app.project()?.regions("clipRegions").len() >= 2,
                        "Split clip failed"
                    );
                    app.action(ui, "undo")?;
                    let caption = app
                        .project()?
                        .regions("autoCaptions")
                        .first()
                        .context("Caption fixture missing")?["id"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    app.extra_selection.clear();
                    app.selected = Some(("autoCaptions".into(), caption.clone()));
                    let captions = app.project()?.regions("autoCaptions").len();
                    app.action(ui, "split-caption")?;
                    ensure!(
                        app.project()?.regions("autoCaptions").len() == captions + 1,
                        "Caption split failed"
                    );
                    app.action(ui, "merge-caption")?;
                    ensure!(
                        app.project()?.regions("autoCaptions").len() == captions,
                        "Caption merge failed"
                    );
                    app.field(ui, "word.0.text", "Edited")?;
                    ensure!(
                        app.project()?.regions("autoCaptions")[0]["text"]
                            .as_str()
                            .unwrap()
                            .starts_with("Edited"),
                        "Word edit failed"
                    );
                    app.action(ui, "undo")?;
                    app.action(ui, "undo")?;
                    app.action(ui, "undo")?;
                    app.action(ui, "select-all")?;
                    let selections = app.selected_keys().len();
                    ensure!(selections > 3, "Group selection failed");
                    app.action(ui, "copy")?;
                    ensure!(app.clipboard.len() == selections, "Group copy failed");
                    app.action(ui, "delete")?;
                    ensure!(app.selected_keys().is_empty(), "Group delete failed");
                    app.action(ui, "undo")?;
                    app.extra_selection.clear();
                    app.selected = None;
                    app.field(ui, "cursorStyle", "figma")?;
                    let style = app
                        .fields("Cursor")
                        .into_iter()
                        .find(|f| f.key == "cursorStyle")
                        .context("Cursor style selector missing")?;
                    ensure!(
                        style.choice == 4 && style.value == "figma",
                        "Cursor style selector lost its value"
                    );
                    app.action(ui, "undo")?;
                    app.field(ui, "padding.all", "32")?;
                    ensure!(
                        app.project()?.editor["padding"]["right"] == 32.,
                        "Linked padding failed"
                    );
                    app.field(ui, "padding.linked", "false")?;
                    app.field(ui, "padding.left", "12")?;
                    ensure!(
                        app.project()?.editor["padding"]["right"] == 32.,
                        "Independent padding changed the opposite side"
                    );
                    app.action(ui, "undo")?;
                    app.action(ui, "undo")?;
                    app.action(ui, "undo")?;
                    let original_crop = app.project()?.editor.get("cropRegion").cloned();
                    app.action(ui, "visual-crop")?;
                    ensure!(
                        ui.get_panel_index() == 11,
                        "Crop inspector selection failed"
                    );
                    app.field(ui, "cropRegion.width", "0.8")?;
                    let info = app.info.as_ref().unwrap();
                    ensure!(
                        (ui.get_preview_aspect() as f64 - info.width as f64 / info.height as f64)
                            .abs()
                            < 0.001,
                        "Crop must show the whole source"
                    );
                    app.action(ui, "finish-crop")?;
                    ensure!(ui.get_panel_index() == 0, "Finish crop failed");
                    app.action(ui, "undo")?;
                    ensure!(
                        app.project()?.editor.get("cropRegion").cloned() == original_crop,
                        "Crop undo failed"
                    );
                    let folder = tempfile::tempdir()?;
                    std::fs::write(folder.path().join("Library Test.recordly"), "{}")?;
                    app.preferences.library_directory = Some(folder.path().to_owned());
                    app.field(ui, "library.query", "Library Test")?;
                    ensure!(app.library.len() == 1, "Folder library search failed");
                    app.preferences.library_directory = None;
                    app.library_query.clear();
                    app.reload_library()?;
                    app.history = Some(History::new(snapshot_project));
                    ui.set_panel("Cursor".into());
                    app.refresh(ui);
                    app.seek(ui, 0.0);
                    // Settings callbacks must persist outside the project and never dirty it.
                    let original_project = app.project()?.clone();
                    for appearance in ["light", "dark", "system"] {
                        app.field(ui, "prefs.appearance", appearance)?;
                        ensure!(
                            ui.get_appearance() == appearance,
                            "Appearance did not reach UI"
                        );
                        ensure!(
                            subtake_native::preferences::Preferences::load()?.appearance
                                == appearance,
                            "Appearance did not persist"
                        );
                    }
                    app.field(ui, "prefs.auto_apply_zooms", "false")?;
                    ensure!(
                        !ui.get_auto_apply_zooms()
                            && !subtake_native::preferences::Preferences::load()?.auto_apply_zooms,
                        "Automatic zoom preference did not persist"
                    );
                    app.field(ui, "prefs.auto_apply_zooms", "true")?;
                    ensure!(
                        app.project()? == &original_project,
                        "Application preferences altered project content"
                    );
                    for (key, value) in [
                        ("cursorStyle", "dot"),
                        ("webcam.positionPreset", "top-left"),
                        ("nativeCaptionLanguage", "fr"),
                        ("autoCaptionSettings.fontFamily", "Georgia"),
                    ] {
                        app.field(ui, key, value)?;
                        app.action(ui, "undo")?;
                        ensure!(
                            app.project()? == &original_project,
                            "Visual choice failed to undo: {key}"
                        );
                    }
                    if let Ok(appearance) = std::env::var("SUBTAKE_UI_APPEARANCE") {
                        app.field(ui, "prefs.appearance", &appearance)?;
                    }
                    if let Ok(panel) = std::env::var("SUBTAKE_UI_PANEL") {
                        if panel == "Crop" {
                            app.action(ui, "visual-crop")?;
                            app.field(ui, "cropRegion.width", "0.8")?;
                            app.field(ui, "cropRegion.height", "0.8")?;
                        } else {
                            ui.set_panel(panel);
                            app.refresh(ui);
                            app.epoch += 1;
                            app.request();
                        }
                    }
                    if std::env::var_os("SUBTAKE_UI_EMPTY").is_some() {
                        app.stop(ui);
                        app.epoch += 1;
                        app.source_time = 0.;
                        app.history = None;
                        app.info = None;
                        app.source = None;
                        app.document = None;
                        app.selected = None;
                        app.refresh(ui);
                        ui.set_preview(ui_runtime::Image::default());
                        ui.set_status("Choose a source and press Start recording".into());
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    eprintln!("UI_SMOKE_FAILED: {e:#}");
                    std::process::exit(1);
                }
                Timer::single_shot(Duration::from_secs(2), move || {
                    with_app(|app, ui| {
                        let panel = std::env::var("SUBTAKE_UI_PANEL").unwrap_or("Cursor".into());
                        let expected = [
                            "Frame",
                            "Cursor",
                            "Webcam",
                            "Captions",
                            "Selection",
                            "Recording",
                            "Export",
                            "Audio",
                            "Preferences",
                            "Recent",
                            "Wallpapers",
                            "Crop",
                            "Shortcuts",
                            "Add",
                        ]
                        .iter()
                        .position(|p| *p == panel)
                        .unwrap_or(1) as i32;
                        if ui.get_panel().as_str() != panel || ui.get_panel_index() != expected {
                            eprintln!(
                                "UI_SMOKE_FAILED: inspector selection and displayed panel diverged"
                            );
                            std::process::exit(1);
                        }
                        if panel == "Wallpapers" && ui.get_wallpapers().row_count() == 0 {
                            eprintln!("UI_SMOKE_FAILED: wallpaper thumbnails were not loaded");
                            std::process::exit(1);
                        }
                        app.discard_recovery();
                        app.recovery.flush();
                        match ui.window().take_snapshot() {
                            Ok(buffer) => {
                                if let Err(e) = image::save_buffer(
                                    PathBuf::from(snapshot),
                                    buffer.as_bytes(),
                                    buffer.width(),
                                    buffer.height(),
                                    image::ColorType::Rgba8,
                                ) {
                                    {
                                        eprintln!("UI_SNAPSHOT_FAILED: {e}");
                                        std::process::exit(1)
                                    }
                                } else {
                                    println!(
                                        "UI_SMOKE_PASSED: controller callbacks and native snapshot; pointer hit testing not exercised"
                                    )
                                }
                            }
                            Err(e) => {
                                eprintln!("UI_SNAPSHOT_FAILED: {e}");
                                std::process::exit(1)
                            }
                        }
                        if std::env::var_os("SUBTAKE_UI_KEEP_OPEN").is_none() {
                            let _ = ui_runtime::quit_event_loop();
                        }
                    })
                });
            })
        });
    }
    // The empty state lists the newest projects, so the library is read
    // before the first frame rather than on first opening Recent.
    let result = state.borrow_mut().reload_library();
    report(&ui, result);
    if path.is_none() && !state.borrow().recoveries.is_empty() {
        ui.set_panel("Recent".into());
        ui.set_status("Unsaved project recovery is available in Recent.".into());
    }
    state.borrow().refresh(&ui);
    if let Some(path) = path {
        let result = state.borrow_mut().load(&ui, path);
        report(&ui, result);
    } else {
        let result = state.borrow_mut().show_launcher(&ui);
        report(&ui, result);
        state.borrow().sync_launcher(&ui);
    }

    #[cfg(target_os = "macos")]
    Timer::single_shot(Duration::from_millis(100), || {
        // AppKit status items must be created on the running main event loop.
        platform::install_status_item(status_menu_action);
    });
    // The development watcher requests a normal quit only after a successful
    // build. Never interrupt capture/export; unsaved projects use the existing
    // save/discard/cancel prompt. This timer is absent in normal app launches.
    // Refresh paused previews after window resizing, monitor scale changes or pinch zoom.
    let preview_size_timer = Timer::default();
    let mut last_preview_size = (0u32, 0u32);
    preview_size_timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
        STATE.with(|slot| {
            let context = slot.borrow().clone();
            if let Some((state, weak)) = context
                && let Some(ui) = weak.upgrade()
            {
                when_idle(&state, |s| {
                    let size = (
                        (ui.get_preview_pixel_width() * ui.window().scale_factor()).ceil() as u32,
                        (ui.get_preview_aspect() * 10000.) as u32,
                    );
                    if size != last_preview_size {
                        last_preview_size = size;
                        s.request();
                    }
                });
            }
        });
    });
    let dev_restart_timer = Timer::default();
    if let Some(request) = std::env::var_os("SUBTAKE_DEV_RESTART_FILE") {
        let request = PathBuf::from(request);
        dev_restart_timer.start(TimerMode::Repeated, Duration::from_millis(300), move || {
            if !request.is_file() {
                return;
            }
            with_app(|app, ui| {
                if app.recording.is_some() || ui.get_busy() {
                    return;
                }
                let result = app.action(ui, "quit");
                report(ui, result);
                // Leave the request present while a save dialog is open.
                let _ = std::fs::remove_file(&request);
            });
        });
    }
    if std::env::var_os("SUBTAKE_LAUNCHER_SMOKE").is_some() {
        Timer::single_shot(Duration::from_secs(3), || launcher_smoke_step(0));
    }
    if std::env::var_os("SUBTAKE_WALKTHROUGH").is_some() {
        Timer::single_shot(Duration::from_secs(3), walkthrough::start);
    }
    ui_runtime::run_event_loop_until_quit()?;
    state.borrow().recovery.flush();
    Ok(())
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn subtake_install_document_events(callback: extern "C" fn(*const std::ffi::c_char));
}

#[cfg(target_os = "macos")]
extern "C" fn open_document_event(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    // SAFETY: the Objective-C bridge passes a live, NUL-terminated UTF-8 string;
    // copying it here finishes before the autoreleased URL leaves that callback.
    let path = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .to_string();
    post(move |app, ui| {
        if ui.get_busy() || app.recording.is_some() {
            ui.set_status("Finish the current operation before opening another project.".into());
            return;
        }
        if app.can_replace(ui) {
            let result = app.load(ui, PathBuf::from(path));
            report(ui, result);
        }
    });
}

#[cfg(target_os = "macos")]
extern "C" fn status_menu_action(action: *const std::ffi::c_char) {
    if action.is_null() {
        return;
    }
    let action = unsafe { std::ffi::CStr::from_ptr(action) }
        .to_string_lossy()
        .to_string();
    post(move |app, ui| {
        let result = app.action(ui, &action);
        report(ui, result);
    });
}
