//! Presentation-state contracts only: these tests do not launch GPUI, capture a
//! native window, or claim pointer/keyboard hit-testing coverage.
//!
//! Migration audit baseline, transcribed from the former editor/recorder UI and
//! its controls on 2026-09-21. This reference does not depend on the old UI files
//! remaining on disk. Items below describe required semantics, not passing GUI
//! checks; the executable tests cover only the state/callback subset.
//!
//! Literal action inventory (53 distinct names):
//! add-audio, add-blur, add-caption, add-figure, add-image, add-marker, add-speed,
//! add-text, add-trim, add-zoom, auto-zoom, cancel, choose-background, copy, cut,
//! delete, drag-launcher, drag-window, duplicate, export, feedback, hide-launcher,
//! import-captions, next-annotation, next-frame, next-marker, open, paste,
//! pause-recording, play, previous-annotation, previous-frame, previous-marker,
//! projects, quit, record, recording-folder, redo, save, save-as, select-all,
//! shortcut-reference, show, show-editor, sources, split-clip, start-recording,
//! stop-recording, storyboard-spike, transcribe, undo, visual-crop, wallpapers.
//! Native synchronous window dragging may replace the two drag commands.
//! Dynamic commands: wallpaper tile.key, every action-valued field.key (kind 3),
//! motion-focused/motion-smooth, look-studio/look-minimal/look-bold. Field actions
//! include controller-generated project/recovery/library/preset, crop completion,
//! caption editing and export commands; preserve the whole model-driven route.
//!
//! Panel IDs/indexes: Frame=0, Cursor=1, Webcam=2, Captions=3, Selection=4,
//! Recording=5, Export=6, Audio=7, Preferences=8, Recent=9, Wallpapers=10,
//! Crop=11, Presets=12, Shortcuts=13. show-panel publishes panel then notifies
//! panel-change; panel changes reset inspector scroll. Display aliases:
//! Frame->Scene, Preferences->Settings, Recent->Projects, Wallpapers->Background.
//! Scene rail remains active for Frame/Wallpapers/Crop/Presets; Settings remains
//! active for Preferences/Shortcuts. Back from Shortcuts goes to Preferences;
//! back from Crop/Wallpapers/Presets goes to Frame. Export menu opens its panel.
//!
//! Inspector field kinds: 0=text (commit on Enter), 1=numeric (slider commits on
//! release; typed value commits on Enter), 2=boolean ("true"/"false"), 3=action
//! (dispatch field.key; display field.value), 4=choice (display choices; dispatch
//! values[index], never the translated label), 5=section heading. Retained controls
//! must refresh callbacks/choice values/bounds when their model changes. Unknown
//! imported choice values remain selectable and must not become a false selection.
//! Special controls: cursor thumbnails for tahoe/macos/windows11/dot/figma plus
//! current unfamiliar value; webcam position 3x3 grid (top/center/bottom by
//! left/center/right), plus custom; appearance light/dark/system; automatic zooms;
//! connectZooms; focused/smooth motion cards; studio/minimal/bold look cards.
//! Project-only motion/look/connect controls are disabled without media.
//! Background: thumbnail actions, choose-background upload, six swatches
//! #17171c/#22364a/#253c32/#54324a/#784832/#ededed and arbitrary color/gradient
//! input through field-change("wallpaper", value). Aspect choices dispatch
//! native/16:9/9:16/1:1/4:3/3:2. Crop dispatches visual-crop; Done is a field action.
//! Selection footer deletes only with selected-id; Captions has import-captions
//! and transcribe; Export is enabled only with media and !busy. Recent includes
//! storyboard-spike/open and all dynamic fields. Preferences links to Shortcuts.
//!
//! Preview: fit preserves aspect, uses actual viewport bounds minus the small
//! inset, and reports rendered image width to the controller for physical-pixel
//! rendering. Pinch zoom is 1..8 with pointer anchoring. Pan is limited to image
//! overflow. Fit, aspect change, disabled/no-media and new-media reset clear zoom
//! and pan. Click coordinates and canvas-edit deltas are image-normalized;
//! canvas-edit's boolean distinguishes movement(false) from resize(true).
//! Edit overlays are hidden during playback. Preview zoom/pan never seek or edit
//! the timeline/document. Transport exposes previous-frame/play/next-frame/Audio.
//!
//! Timeline: zoom 1..100, offset 0..duration-visible, visible=max(.001,duration/zoom).
//! Pinch preserves time under the gesture anchor. Fit resets zoom=1, offset=0;
//! Snap toggles state. Ruler/source/track scrubbing seeks source time on down,
//! movement and up. Region selection forwards kind/id/shift-extension; movement
//! forwards seconds delta with modes move=0, start trim=2, end trim=1. Playhead
//! dragging must win over overlapping region hit targets. Render all model track
//! labels/rows, source thumbnails, audio-row waveform and selected region state;
//! old playhead used frosted-thumbnails as its backdrop. Position slider scrolls
//! the timeline. Add menu maps Text/Image/Arrow/Blur/Audio/Caption/Trim/Speed/Marker
//! to add-text/add-image/add-figure/add-blur/add-audio/add-caption/add-trim/
//! add-speed/add-marker and closes after selection.
//!
//! Recorder: initial panel=""; toggle same panel closes it. Panels are sources,
//! audio, camera, countdown, more. Idle Record with no sources toggles sources
//! and dispatches sources; otherwise start-recording. Busy/recording shows status
//! or REC/PAUSED elapsed time, pause/resume and stop disabled while busy, optional
//! cancel when cancellable; hide-launcher always remains available. Status has a
//! help affordance. Bar content is 692x64 with 16px horizontal/8px vertical inset;
//! outer window 724x80, transparent, frameless, always-on-top.
//! Options dimensions: sources=430x264, audio=420x264, camera=420x276,
//! countdown=400x264, more=520x284. Independent options window stays above/attached
//! to the bar across panel changes, size changes and native window movement.
//! Option callback keys: source, microphone, microphone-device, system-audio,
//! camera, camera-device, countdown. Device/source values are decimal indices;
//! toggles use "true"/"false"; countdown values are 0/3/5/10. Controller owns
//! final state and rejects changes while busy/recording. More routes storyboard-
//! spike/open/projects/show-editor/recording-folder; show-editor needs a project.
//! Options close emits panel-change(""). Recorder selections never mutate editor
//! state except through the controller's explicit synchronization/option callback.
//!
//! Editor defaults: Frame/index 0; system appearance; auto_apply_zooms/connect_zooms
//! true; duration/preview_zoom/timeline_zoom/edit_scale=1; timeline_offset/playhead=0;
//! snap=true; capture_mic/capture_system=true, capture_camera=false; device index=0
//! with camera/microphone models ["System default"]. Initial track labels are
//! Zoom/Clip/Annotation/Audio/Caption with audio row=3. Recorder countdown=3,
//! elapsed="00:00". Window preferred size 1360x880, minimum 980x680; document
//! title includes dirty indication. Status/progress and Cancel reflect busy state.
//!
//! Cross-cutting: preserve translated editor strings, accessible labels/actions,
//! enabled gates, focus traversal and Enter/Space activation. Text fields own
//! editing shortcuts/IME; editor keyboard callback returns whether it handled
//! the key. Native file drops and close requests route through controller guards.
//! Modal-capable callbacks execute outside GPUI render/event borrows; native
//! dragging executes synchronously in its mouse event. Tray Open/Quit route
//! show/quit. Callback/state tests do not certify any native interaction above.
use std::{cell::RefCell, rc::Rc};
use subtake_native::{
    ui_runtime::{ModelRc, VecModel},
    ui_state::{EditorWindow, Field, RecordingLauncher, RecordingOptions},
};

#[test]
fn panel_changes_update_selection_before_callbacks_and_do_not_emit_actions() {
    let ui = EditorWindow::new().unwrap();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let events = observed.clone();
    let weak = ui.as_weak();
    ui.on_panel_change(move |panel| {
        let ui = weak.upgrade().unwrap();
        events
            .borrow_mut()
            .push((panel, ui.get_panel(), ui.get_panel_index()));
        ui.set_status("Inspector refreshed".into());
    });
    ui.on_action(|action| panic!("Panel navigation emitted an action: {action}"));
    let panels = [
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
    ];
    for (index, panel) in panels.into_iter().enumerate() {
        // Matches the old show-panel contract: publish state, then notify.
        ui.set_panel(panel.into());
        assert_eq!(ui.get_panel_index(), index as i32, "{panel}");
        assert_eq!(
            observed.borrow().len(),
            index,
            "set_panel emitted a callback"
        );
        ui.invoke_panel_change(panel.into());
        assert_eq!(
            observed.borrow()[index],
            (panel.into(), panel.into(), index as i32),
        );
    }
    ui.set_panel("Frame".into());
    assert_eq!(ui.get_panel_index(), 0);
    assert_eq!(ui.get_status(), "Inspector refreshed");
}

#[test]
fn callback_can_replace_itself_and_dispatch_the_replacement() {
    let ui = EditorWindow::new().unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let first_events = events.clone();
    let weak = ui.as_weak();
    ui.on_action(move |action| {
        first_events.borrow_mut().push(format!("first:{action}"));
        let ui = weak.upgrade().unwrap();
        ui.set_busy(true);
        let next_events = first_events.clone();
        let weak = ui.as_weak();
        ui.on_action(move |action| {
            next_events
                .borrow_mut()
                .push(format!("replacement:{action}"));
            weak.upgrade().unwrap().set_busy(false);
        });
        ui.invoke_action("cancel".into());
    });
    ui.invoke_action("export".into());
    ui.invoke_action("save".into());
    assert_eq!(
        *events.borrow(),
        ["first:export", "replacement:cancel", "replacement:save"],
    );
    assert!(!ui.get_busy());
}

#[test]
fn recorder_commands_are_isolated_until_the_controller_updates_editor_state() {
    let editor = EditorWindow::new().unwrap();
    let launcher = RecordingLauncher::new().unwrap();
    let options = RecordingOptions::new().unwrap();
    editor.on_action(|action| panic!("Recorder command leaked into editor: {action}"));
    launcher.on_option(|key, _| panic!("Option leaked into launcher: {key}"));
    options.set_microphone(false);
    assert!(
        editor.get_capture_mic(),
        "Recorder presentation mutated the editor"
    );
    let weak = editor.as_weak();
    options.on_option(move |key, value| {
        assert_eq!(key, "microphone");
        weak.upgrade().unwrap().set_capture_mic(value == "true");
    });
    options.invoke_option("microphone".into(), "false".into());
    assert!(!editor.get_capture_mic());
    assert!(!editor.get_recording());
    assert_eq!(editor.get_panel(), "Frame");
}

#[test]
fn recorder_surfaces_start_with_no_options_panel_selected() {
    assert_eq!(EditorWindow::new().unwrap().get_panel(), "Frame");
    assert_eq!(RecordingLauncher::new().unwrap().get_panel(), "");
    assert_eq!(RecordingOptions::new().unwrap().get_panel(), "");
}

#[test]
fn editor_can_select_system_devices_before_discovery_completes() {
    let ui = EditorWindow::new().unwrap();
    assert_eq!(
        ui.get_camera_names()
            .row_data(ui.get_camera_index() as usize),
        Some("System default".into())
    );
    assert_eq!(
        ui.get_microphone_names()
            .row_data(ui.get_microphone_index() as usize),
        Some("System default".into())
    );
    assert!(ui.get_capture_mic());
    assert!(ui.get_capture_system());
    assert!(!ui.get_capture_camera());
}

#[test]
fn changing_options_panel_preserves_device_selection_and_recomputes_dimensions() {
    let ui = RecordingOptions::new().unwrap();
    ui.set_microphone_index(2);
    ui.set_camera_index(1);
    ui.set_countdown(5);
    ui.on_option(|key, _| panic!("Panel change committed an option: {key}"));
    for panel in ["sources", "audio", "camera", "countdown", "more", "sources"] {
        ui.set_panel(panel.into());
        // The card measures its own height; a panel change alone keeps the
        // window a card wide and leaves its height to that measurement.
        assert_eq!(
            (ui.get_options_width(), ui.get_options_height()),
            (subtake_theme::Theme::RECORDER_CARD_WIDTH, 264.)
        );
        assert_eq!(
            (
                ui.get_microphone_index(),
                ui.get_camera_index(),
                ui.get_countdown()
            ),
            (2, 1, 5)
        );
    }
}

#[test]
fn keyboard_result_and_modifiers_survive_callback_dispatch() {
    let ui = EditorWindow::new().unwrap();
    assert!(!ui.invoke_keyboard("z".into(), true, false, false));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let events = seen.clone();
    ui.on_keyboard(move |key, primary, shift, alt| {
        events.borrow_mut().push((key.clone(), primary, shift, alt));
        key == "z" && primary && shift && !alt
    });
    assert!(ui.invoke_keyboard("z".into(), true, true, false));
    assert!(!ui.invoke_keyboard("z".into(), true, false, true));
    assert_eq!(
        *seen.borrow(),
        [
            ("z".into(), true, true, false),
            ("z".into(), true, false, true)
        ]
    );
}

#[test]
fn region_selection_and_trim_callbacks_preserve_identity_mode_and_extend() {
    let ui = EditorWindow::new().unwrap();
    ui.set_playhead(12.);
    ui.on_seek(|_| panic!("Selection or trimming dispatched seek"));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let recorded = selections.clone();
    let weak = ui.as_weak();
    ui.on_select_region(move |kind, id, extend| {
        recorded.borrow_mut().push((kind, id.clone(), extend));
        let ui = weak.upgrade().unwrap();
        ui.set_selected_id(id);
        ui.set_panel("Selection".into());
    });
    let edits = Rc::new(RefCell::new(Vec::new()));
    let recorded = edits.clone();
    ui.on_move_region(move |kind, id, delta, mode| {
        recorded.borrow_mut().push((kind, id, delta, mode));
    });
    ui.invoke_select_region("clipRegions".into(), "clip-a".into(), false);
    ui.invoke_select_region("annotationRegions".into(), "note-b".into(), true);
    for (delta, mode) in [(1.25, 0), (-0.5, 2), (0.75, 1)] {
        ui.invoke_move_region("clipRegions".into(), "clip-a".into(), delta, mode);
    }
    assert_eq!(
        *selections.borrow(),
        [
            ("clipRegions".into(), "clip-a".into(), false),
            ("annotationRegions".into(), "note-b".into(), true),
        ],
    );
    assert_eq!(
        *edits.borrow(),
        [
            ("clipRegions".into(), "clip-a".into(), 1.25, 0),
            ("clipRegions".into(), "clip-a".into(), -0.5, 2),
            ("clipRegions".into(), "clip-a".into(), 0.75, 1),
        ],
    );
    assert_eq!(ui.get_panel_index(), 4);
    assert_eq!(ui.get_selected_id(), "note-b");
    assert_eq!(ui.get_playhead(), 12.);
}

#[test]
fn preview_selection_and_canvas_edits_are_distinct_from_timeline_commands() {
    let ui = EditorWindow::new().unwrap();
    ui.on_seek(|_| panic!("Canvas edit sought media"));
    ui.on_move_region(|_, _, _, _| panic!("Canvas edit moved a timeline region"));
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let recorded = clicks.clone();
    ui.on_preview_click(move |x, y| recorded.borrow_mut().push((x, y)));
    let edits = Rc::new(RefCell::new(Vec::new()));
    let recorded = edits.clone();
    ui.on_canvas_edit(move |dx, dy, resize| recorded.borrow_mut().push((dx, dy, resize)));
    ui.invoke_preview_click(0.25, 0.75);
    ui.invoke_canvas_edit(-0.125, 0.25, false);
    ui.invoke_canvas_edit(0.25, -0.125, true);
    assert_eq!(*clicks.borrow(), [(0.25, 0.75)]);
    assert_eq!(
        *edits.borrow(),
        [(-0.125, 0.25, false), (0.25, -0.125, true)]
    );
}

#[test]
fn preview_fit_does_not_seek_or_change_timeline_or_document_state() {
    let ui = EditorWindow::new().unwrap();
    ui.set_duration(120.);
    ui.set_playhead(37.);
    ui.set_timeline_zoom(4.);
    ui.set_timeline_offset(25.);
    ui.set_preview_zoom(3.);
    ui.set_dirty(true);
    ui.on_seek(|_| panic!("Preview fit sought media"));
    ui.on_action(|_| panic!("Preview fit emitted a document action"));
    ui.invoke_reset_preview();
    assert_eq!(ui.get_preview_zoom(), 1.);
    assert_eq!(
        (
            ui.get_playhead(),
            ui.get_timeline_zoom(),
            ui.get_timeline_offset()
        ),
        (37., 4., 25.)
    );
    assert_eq!(ui.get_timeline_visible(), 30.);
    assert!(ui.get_dirty());
}

#[test]
fn inspector_model_replacement_preserves_old_snapshot_and_raw_choice_values() {
    let ui = EditorWindow::new().unwrap();
    let field = Field {
        key: "cursorStyle".into(),
        value: "imported-custom".into(),
        kind: 4,
        choice: 1,
        choices: ModelRc::new(VecModel::from(vec![
            "macOS".into(),
            "Imported cursor".into(),
        ])),
        values: ModelRc::new(VecModel::from(vec![
            "macos".into(),
            "imported-custom".into(),
        ])),
        ..Default::default()
    };
    ui.set_fields(ModelRc::new(VecModel::from(vec![field.clone()])));
    let previous = ui.get_fields();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    let weak = ui.as_weak();
    ui.on_field_change(move |key, value| {
        recorded.borrow_mut().push((key, value));
        weak.upgrade().unwrap().set_fields(ModelRc::default());
    });
    let selected = previous.row_data(0).unwrap();
    ui.invoke_field_change(
        selected.key,
        selected.values.row_data(selected.choice as usize).unwrap(),
    );
    assert_eq!(
        *changes.borrow(),
        [("cursorStyle".into(), "imported-custom".into())]
    );
    assert_eq!(ui.get_fields().row_count(), 0);
    assert_eq!(previous.row_data(0), Some(field));
}
