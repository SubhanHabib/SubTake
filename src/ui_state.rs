//! Toolkit-independent presentation state and commands shared by the GPUI surfaces.
//! Property updates invalidate the corresponding native window; media work stays off-thread.
use crate::ui_runtime::{self, Color, Image, ModelRc, VecModel, Window};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Region {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub start: f32,
    pub end: f32,
    pub row: i32,
    pub tint: Color,
    pub selected: bool,
    /// An annotation's own glyph, by its type; empty for every other kind,
    /// which takes its kind's.
    pub glyph: &'static str,
}

impl Region {
    /// The take's own kept spans, drawn in the clip lane while the project
    /// has no clips, and the take's sound, drawn first in the audio lane.
    /// Neither is a project region, so neither is selected or dragged.
    pub const TAKE_CLIP: &str = "takeClip";
    pub const TAKE_AUDIO: &str = "takeAudio";

    pub fn is_take(&self) -> bool {
        self.kind == Self::TAKE_CLIP || self.kind == Self::TAKE_AUDIO
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub value: String,
    pub kind: i32,
    pub minimum: f32,
    pub maximum: f32,
    pub choices: ModelRc<String>,
    pub values: ModelRc<String>,
    pub choice: i32,
}

/// A project or recording the empty state offers to reopen.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Recent {
    /// The action that opens it.
    pub key: String,
    pub title: String,
    /// One mono line under the title: its kind and when it last changed.
    pub meta: String,
    /// A still from it, when one is to hand. Empty draws a placeholder.
    pub thumbnail: Image,
}

/// A display or window the recorder can capture, as its Source card draws it.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct CaptureSource {
    /// `"display"` or `"window"`: the card lists them in two groups.
    pub kind: String,
    pub name: String,
    /// A display's resolution; empty for a window.
    pub detail: String,
    /// A still of it, when one is to hand. Empty draws a placeholder.
    pub thumbnail: Image,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Wallpaper {
    pub key: String,
    pub title: String,
    /// What the project's `wallpaper` field holds while this one is in use.
    pub value: String,
    pub source: Image,
}

struct Properties {
    appearance: String,
    auto_apply_zooms: bool,
    connect_zooms: bool,
    motion_choice: String,
    look_choice: String,
    language: String,
    document_title: String,
    can_undo: bool,
    can_redo: bool,
    dirty: bool,
    has_video: bool,
    playing: bool,
    recording: bool,
    recording_paused: bool,
    busy: bool,
    sources_loading: bool,
    mac_titlebar: bool,
    edit_visible: bool,
    wallpapers: ModelRc<Wallpaper>,
    recents: ModelRc<Recent>,
    saved_presets: ModelRc<String>,
    saved_preset_parts: ModelRc<String>,
    default_preset: String,
    selected_preset: String,
    dialog: String,
    inspector_open: bool,
    aspect_index: i32,
    background_value: String,
    recording_hint: String,
    status: String,
    progress: f32,
    preview: Image,
    thumbnails: Image,
    frosted_thumbnails: Image,
    waveform: Image,
    edit_x: f32,
    edit_y: f32,
    /// The annotations on the preview, bottom first, as normalized
    /// [x, y, width, height] on the picture.
    stage_annotations: Vec<[f32; 4]>,
    /// The one of them under the pointer, or -1.
    hovered_annotation: i32,
    edit_width: f32,
    edit_height: f32,
    edit_scale: f32,
    preview_zoom: f32,
    preview_pixel_width: f32,
    preview_aspect: f32,
    duration: f32,
    playhead: f32,
    timeline_zoom: f32,
    timeline_offset: f32,
    /// The lane height and inspector width the last session left, for the
    /// editor to take up once.
    saved_layout: Option<(f32, f32)>,
    snap: bool,
    time_label: String,
    regions: ModelRc<Region>,
    track_labels: ModelRc<String>,
    /// The lanes turned off from their headers, by label, as the project
    /// saves them.
    lanes_off: Vec<String>,
    selected_id: String,
    fields: ModelRc<Field>,
    settings_fields: ModelRc<Field>,
    settings_section: String,
    source_names: ModelRc<String>,
    capture_sources: ModelRc<CaptureSource>,
    mic_level: f32,
    camera_preview: Image,
    options_width: f32,
    options_height: f32,
    /// The recorder's settings beyond source and devices, by key
    /// (`preferences::RECORDER_DEFAULTS`).
    recorder_settings: std::collections::BTreeMap<String, String>,
    camera_names: ModelRc<String>,
    microphone_names: ModelRc<String>,
    /// How each of `microphone_names` connects — "Built-in", "USB",
    /// "Bluetooth" — or empty where macOS does not say.
    microphone_kinds: ModelRc<String>,
    source_index: i32,
    camera_index: i32,
    microphone_index: i32,
    capture_mic: bool,
    capture_camera: bool,
    capture_system: bool,
    panel: String,
    panel_index: i32,
    camera: bool,
    microphone: bool,
    system_audio: bool,
    paused: bool,
    has_project: bool,
    cancellable: bool,
    elapsed: String,
    directory: String,
    /// How many projects and videos the library holds: the More card's All
    /// projects.
    project_count: i32,
    /// The global record / stop binding, as the keymap spells it
    /// ("Super+Shift+R"); empty when there is none.
    record_shortcut: String,
    countdown: i32,
    counting: i32,
    stopping: bool,
    export_state: String,
    export_name: String,
    export_detail: String,
    export_progress: f32,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            appearance: "system".into(),
            auto_apply_zooms: true,
            connect_zooms: true,
            motion_choice: String::new(),
            look_choice: String::new(),
            language: String::new(),
            document_title: "Untitled".into(),
            can_undo: false,
            can_redo: false,
            dirty: false,
            has_video: false,
            playing: false,
            recording: false,
            recording_paused: false,
            busy: false,
            sources_loading: false,
            mac_titlebar: false,
            edit_visible: false,
            wallpapers: ModelRc::default(),
            recents: ModelRc::default(),
            saved_presets: ModelRc::default(),
            saved_preset_parts: ModelRc::default(),
            default_preset: String::new(),
            selected_preset: String::new(),
            dialog: String::new(),
            inspector_open: false,
            aspect_index: 0,
            background_value: String::new(),
            recording_hint: "Choose a display or window, then start recording.".into(),
            status: "Open a video or start a recording".into(),
            progress: 0.,
            preview: Image::default(),
            thumbnails: Image::default(),
            frosted_thumbnails: Image::default(),
            waveform: Image::default(),
            edit_x: 0.,
            edit_y: 0.,
            stage_annotations: vec![],
            hovered_annotation: -1,
            edit_width: 0.,
            edit_height: 0.,
            edit_scale: 1.,
            preview_zoom: 1.,
            preview_pixel_width: 960.,
            preview_aspect: 16. / 9.,
            duration: 1.,
            playhead: 0.,
            timeline_zoom: 1.,
            timeline_offset: 0.,
            saved_layout: None,
            snap: true,
            time_label: "00:00.000".into(),
            regions: ModelRc::default(),
            track_labels: ModelRc::new(VecModel::from(vec![
                "Zoom".into(),
                "Clip".into(),
                "Annotation".into(),
                "Audio".into(),
                "Caption".into(),
            ])),
            lanes_off: vec![],
            selected_id: String::new(),
            fields: ModelRc::default(),
            settings_fields: ModelRc::default(),
            settings_section: "General".into(),
            source_names: ModelRc::default(),
            capture_sources: ModelRc::default(),
            mic_level: f32::NEG_INFINITY,
            camera_preview: Image::default(),
            options_width: subtake_theme::Theme::recorder_card_width(),
            options_height: 264.,
            recorder_settings: crate::preferences::Preferences::default().recorder_settings(),
            camera_names: ModelRc::default(),
            microphone_names: ModelRc::default(),
            microphone_kinds: ModelRc::default(),
            source_index: 0,
            camera_index: 0,
            microphone_index: 0,
            capture_mic: true,
            capture_camera: false,
            capture_system: true,
            panel: "Frame".into(),
            panel_index: 0,
            camera: false,
            microphone: false,
            system_audio: false,
            paused: false,
            has_project: false,
            cancellable: false,
            elapsed: "00:00".into(),
            directory: String::new(),
            project_count: 0,
            record_shortcut: String::new(),
            countdown: 3,
            counting: 0,
            stopping: false,
            export_state: String::new(),
            export_name: String::new(),
            export_detail: String::new(),
            export_progress: 0.,
        }
    }
}

#[derive(Default)]
struct Callbacks {
    action: Option<Rc<dyn Fn(String)>>,
    seek: Option<Rc<dyn Fn(f32)>>,
    panel_change: Option<Rc<dyn Fn(String)>>,
    field_change: Option<Rc<dyn Fn(String, String)>>,
    select_region: Option<Rc<dyn Fn(String, String, bool)>>,
    move_region: Option<Rc<dyn Fn(String, String, f32, i32)>>,
    canvas_edit: Option<Rc<dyn Fn(f32, f32, bool)>>,
    layout_change: Option<Rc<dyn Fn(f32, f32)>>,
    preview_click: Option<Rc<dyn Fn(f32, f32)>>,
    keyboard: Option<Rc<dyn Fn(String, bool, bool, bool) -> bool>>,
    translate: Option<Rc<dyn Fn(String, String) -> String>>,
    option: Option<Rc<dyn Fn(String, String)>>,
}

pub struct UiData {
    props: RefCell<Properties>,
    callbacks: RefCell<Callbacks>,
    pub(crate) window: Window,
}

#[derive(Clone)]
pub struct UiHandle(pub(crate) Rc<UiData>);
impl UiHandle {
    pub fn window(&self) -> &Window {
        &self.0.window
    }

    pub fn show(&self) -> anyhow::Result<()> {
        self.window().show();
        Ok(())
    }

    pub fn hide(&self) -> anyhow::Result<()> {
        self.window().hide();
        Ok(())
    }

    pub fn get_timeline_visible(&self) -> f32 {
        (self.get_duration() / self.get_timeline_zoom().max(1.)).max(0.001)
    }

    /// The options window is as wide as the card it holds
    /// (`set_options_width`), and as tall as that card measures itself
    /// (`set_options_height`): cards differ in both, and a list of windows
    /// differs with what is open on the Mac.
    pub fn get_options_width(&self) -> f32 {
        self.0.props.borrow().options_width
    }

    pub fn set_options_width(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.options_width != value {
            props.options_width = value;
            self.window().invalidate();
        }
    }

    pub fn get_recorder_setting(&self, key: &str) -> String {
        self.0
            .props
            .borrow()
            .recorder_settings
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// A recorder setting that is on or off.
    pub fn get_recorder_flag(&self, key: &str) -> bool {
        self.get_recorder_setting(key) == "true"
    }

    pub fn set_recorder_settings(&self, value: std::collections::BTreeMap<String, String>) {
        let mut props = self.0.props.borrow_mut();
        if props.recorder_settings != value {
            props.recorder_settings = value;
            self.window().invalidate();
        }
    }

    pub fn set_recorder_setting(&self, key: &str, value: &str) {
        let mut props = self.0.props.borrow_mut();
        if props.recorder_settings.get(key).map(String::as_str) != Some(value) {
            props
                .recorder_settings
                .insert(key.to_owned(), value.to_owned());
            self.window().invalidate();
        }
    }

    pub fn get_options_height(&self) -> f32 {
        self.0.props.borrow().options_height
    }

    pub fn set_options_height(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.options_height != value {
            props.options_height = value;
            self.window().invalidate();
        }
    }

    pub fn invoke_reset_preview(&self) {
        self.set_preview_zoom(1.);
    }

    pub fn get_appearance(&self) -> String {
        self.0.props.borrow().appearance.clone()
    }

    pub fn set_appearance(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.appearance != value {
            props.appearance = value;
            self.window().invalidate();
        }
    }

    pub fn get_auto_apply_zooms(&self) -> bool {
        self.0.props.borrow().auto_apply_zooms
    }

    pub fn set_auto_apply_zooms(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.auto_apply_zooms != value {
            props.auto_apply_zooms = value;
            self.window().invalidate();
        }
    }

    pub fn get_connect_zooms(&self) -> bool {
        self.0.props.borrow().connect_zooms
    }

    pub fn set_connect_zooms(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.connect_zooms != value {
            props.connect_zooms = value;
            self.window().invalidate();
        }
    }

    pub fn get_motion_choice(&self) -> String {
        self.0.props.borrow().motion_choice.clone()
    }

    pub fn set_motion_choice(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.motion_choice != value {
            props.motion_choice = value;
            self.window().invalidate();
        }
    }

    pub fn get_look_choice(&self) -> String {
        self.0.props.borrow().look_choice.clone()
    }

    pub fn set_look_choice(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.look_choice != value {
            props.look_choice = value;
            self.window().invalidate();
        }
    }

    pub fn get_language(&self) -> String {
        self.0.props.borrow().language.clone()
    }

    pub fn set_language(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.language != value {
            props.language = value;
            self.window().invalidate();
        }
    }

    pub fn get_document_title(&self) -> String {
        self.0.props.borrow().document_title.clone()
    }

    pub fn set_document_title(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.document_title != value {
            props.document_title = value;
            self.window().invalidate();
        }
    }

    pub fn get_can_undo(&self) -> bool {
        self.0.props.borrow().can_undo
    }

    pub fn set_can_undo(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.can_undo != value {
            props.can_undo = value;
            self.window().invalidate();
        }
    }

    pub fn get_can_redo(&self) -> bool {
        self.0.props.borrow().can_redo
    }

    pub fn set_can_redo(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.can_redo != value {
            props.can_redo = value;
            self.window().invalidate();
        }
    }

    pub fn get_dirty(&self) -> bool {
        self.0.props.borrow().dirty
    }

    pub fn set_dirty(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.dirty != value {
            props.dirty = value;
            self.window().invalidate();
        }
    }

    pub fn get_has_video(&self) -> bool {
        self.0.props.borrow().has_video
    }

    pub fn set_has_video(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.has_video != value {
            props.has_video = value;
            self.window().invalidate();
        }
    }

    pub fn get_playing(&self) -> bool {
        self.0.props.borrow().playing
    }

    pub fn set_playing(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.playing != value {
            props.playing = value;
            self.window().invalidate();
        }
    }

    pub fn get_recording(&self) -> bool {
        self.0.props.borrow().recording
    }

    pub fn set_recording(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.recording != value {
            props.recording = value;
            self.window().invalidate();
        }
    }

    pub fn get_recording_paused(&self) -> bool {
        self.0.props.borrow().recording_paused
    }

    pub fn set_recording_paused(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.recording_paused != value {
            props.recording_paused = value;
            self.window().invalidate();
        }
    }

    pub fn get_busy(&self) -> bool {
        self.0.props.borrow().busy
    }

    pub fn set_busy(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.busy != value {
            props.busy = value;
            self.window().invalidate();
        }
    }

    pub fn get_sources_loading(&self) -> bool {
        self.0.props.borrow().sources_loading
    }

    pub fn set_sources_loading(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.sources_loading != value {
            props.sources_loading = value;
            self.window().invalidate();
        }
    }

    pub fn get_mac_titlebar(&self) -> bool {
        self.0.props.borrow().mac_titlebar
    }

    pub fn set_mac_titlebar(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.mac_titlebar != value {
            props.mac_titlebar = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_visible(&self) -> bool {
        self.0.props.borrow().edit_visible
    }

    pub fn set_edit_visible(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_visible != value {
            props.edit_visible = value;
            self.window().invalidate();
        }
    }

    pub fn get_wallpapers(&self) -> ModelRc<Wallpaper> {
        self.0.props.borrow().wallpapers.clone()
    }

    pub fn set_wallpapers(&self, value: ModelRc<Wallpaper>) {
        let mut props = self.0.props.borrow_mut();
        if props.wallpapers != value {
            props.wallpapers = value;
            self.window().invalidate();
        }
    }

    /// The centred dialog over the editor: `"presets"`, `"settings"`, or
    /// empty for none.
    pub fn get_dialog(&self) -> String {
        self.0.props.borrow().dialog.clone()
    }

    pub fn set_dialog(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.dialog != value {
            props.dialog = value;
            self.window().invalidate();
        }
    }

    /// Whether the folded inspector is slid in. Only read while the window
    /// is too narrow to keep the inspector beside the stage.
    pub fn get_inspector_open(&self) -> bool {
        self.0.props.borrow().inspector_open
    }

    pub fn set_inspector_open(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.inspector_open != value {
            props.inspector_open = value;
            self.window().invalidate();
        }
    }

    pub fn get_recents(&self) -> ModelRc<Recent> {
        self.0.props.borrow().recents.clone()
    }

    pub fn set_recents(&self, value: ModelRc<Recent>) {
        let mut props = self.0.props.borrow_mut();
        if props.recents != value {
            props.recents = value;
            self.window().invalidate();
        }
    }

    /// The names of the presets saved to disk, in the order the
    /// `apply-preset-{index}` actions address them.
    pub fn get_saved_presets(&self) -> ModelRc<String> {
        self.0.props.borrow().saved_presets.clone()
    }

    pub fn set_saved_presets(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.saved_presets != value {
            props.saved_presets = value;
            self.window().invalidate();
        }
    }

    /// Each saved preset's parts, as `presets::GROUPS` keys joined by
    /// commas, in `saved_presets` order.
    pub fn get_saved_preset_parts(&self) -> ModelRc<String> {
        self.0.props.borrow().saved_preset_parts.clone()
    }

    pub fn set_saved_preset_parts(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.saved_preset_parts != value {
            props.saved_preset_parts = value;
            self.window().invalidate();
        }
    }

    /// The saved preset new videos start from, by name, or empty for none.
    pub fn get_default_preset(&self) -> String {
        self.0.props.borrow().default_preset.clone()
    }

    pub fn set_default_preset(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.default_preset != value {
            props.default_preset = value;
            self.window().invalidate();
        }
    }

    /// The saved preset the Presets dialog is showing, by name. The app
    /// sets it to a preset it has just made, copied or renamed.
    pub fn get_selected_preset(&self) -> String {
        self.0.props.borrow().selected_preset.clone()
    }

    pub fn set_selected_preset(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.selected_preset != value {
            props.selected_preset = value;
            self.window().invalidate();
        }
    }

    pub fn get_aspect_index(&self) -> i32 {
        self.0.props.borrow().aspect_index
    }

    pub fn set_aspect_index(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.aspect_index != value {
            props.aspect_index = value;
            self.window().invalidate();
        }
    }

    pub fn get_background_value(&self) -> String {
        self.0.props.borrow().background_value.clone()
    }

    pub fn set_background_value(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.background_value != value {
            props.background_value = value;
            self.window().invalidate();
        }
    }

    pub fn get_recording_hint(&self) -> String {
        self.0.props.borrow().recording_hint.clone()
    }

    pub fn set_recording_hint(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.recording_hint != value {
            props.recording_hint = value;
            self.window().invalidate();
        }
    }

    pub fn get_status(&self) -> String {
        self.0.props.borrow().status.clone()
    }

    pub fn set_status(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.status != value {
            props.status = value;
            self.window().invalidate();
        }
    }

    pub fn get_progress(&self) -> f32 {
        self.0.props.borrow().progress
    }

    pub fn set_progress(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.progress != value {
            props.progress = value;
            self.window().invalidate();
        }
    }

    pub fn get_preview(&self) -> Image {
        self.0.props.borrow().preview.clone()
    }

    pub fn set_preview(&self, value: Image) {
        let mut props = self.0.props.borrow_mut();
        if props.preview != value {
            props.preview = value;
            self.window().invalidate();
        }
    }

    pub fn get_thumbnails(&self) -> Image {
        self.0.props.borrow().thumbnails.clone()
    }

    pub fn set_thumbnails(&self, value: Image) {
        let mut props = self.0.props.borrow_mut();
        if props.thumbnails != value {
            props.thumbnails = value;
            self.window().invalidate();
        }
    }

    pub fn get_frosted_thumbnails(&self) -> Image {
        self.0.props.borrow().frosted_thumbnails.clone()
    }

    pub fn set_frosted_thumbnails(&self, value: Image) {
        let mut props = self.0.props.borrow_mut();
        if props.frosted_thumbnails != value {
            props.frosted_thumbnails = value;
            self.window().invalidate();
        }
    }

    pub fn get_waveform(&self) -> Image {
        self.0.props.borrow().waveform.clone()
    }

    pub fn set_waveform(&self, value: Image) {
        let mut props = self.0.props.borrow_mut();
        if props.waveform != value {
            props.waveform = value;
            self.window().invalidate();
        }
    }

    pub fn get_stage_annotations(&self) -> Vec<[f32; 4]> {
        self.0.props.borrow().stage_annotations.clone()
    }

    pub fn set_stage_annotations(&self, value: Vec<[f32; 4]>) {
        let mut props = self.0.props.borrow_mut();
        if props.stage_annotations != value {
            props.stage_annotations = value;
            self.window().invalidate();
        }
    }

    pub fn get_hovered_annotation(&self) -> i32 {
        self.0.props.borrow().hovered_annotation
    }

    pub fn set_hovered_annotation(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.hovered_annotation != value {
            props.hovered_annotation = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_x(&self) -> f32 {
        self.0.props.borrow().edit_x
    }

    pub fn set_edit_x(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_x != value {
            props.edit_x = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_y(&self) -> f32 {
        self.0.props.borrow().edit_y
    }

    pub fn set_edit_y(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_y != value {
            props.edit_y = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_width(&self) -> f32 {
        self.0.props.borrow().edit_width
    }

    pub fn set_edit_width(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_width != value {
            props.edit_width = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_height(&self) -> f32 {
        self.0.props.borrow().edit_height
    }

    pub fn set_edit_height(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_height != value {
            props.edit_height = value;
            self.window().invalidate();
        }
    }

    pub fn get_edit_scale(&self) -> f32 {
        self.0.props.borrow().edit_scale
    }

    pub fn set_edit_scale(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.edit_scale != value {
            props.edit_scale = value;
            self.window().invalidate();
        }
    }

    pub fn get_preview_zoom(&self) -> f32 {
        self.0.props.borrow().preview_zoom
    }

    pub fn set_preview_zoom(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.preview_zoom != value {
            props.preview_zoom = value;
            self.window().invalidate();
        }
    }

    pub fn get_preview_pixel_width(&self) -> f32 {
        self.0.props.borrow().preview_pixel_width
    }

    pub fn set_preview_pixel_width(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.preview_pixel_width != value {
            props.preview_pixel_width = value;
            self.window().invalidate();
        }
    }

    pub fn get_preview_aspect(&self) -> f32 {
        self.0.props.borrow().preview_aspect
    }

    pub fn set_preview_aspect(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.preview_aspect != value {
            props.preview_aspect = value;
            self.window().invalidate();
        }
    }

    pub fn get_duration(&self) -> f32 {
        self.0.props.borrow().duration
    }

    pub fn set_duration(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.duration != value {
            props.duration = value;
            self.window().invalidate();
        }
    }

    pub fn get_playhead(&self) -> f32 {
        self.0.props.borrow().playhead
    }

    pub fn set_playhead(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.playhead != value {
            props.playhead = value;
            self.window().invalidate();
        }
    }

    pub fn get_timeline_zoom(&self) -> f32 {
        self.0.props.borrow().timeline_zoom
    }

    pub fn set_timeline_zoom(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.timeline_zoom != value {
            props.timeline_zoom = value;
            self.window().invalidate();
        }
    }

    pub fn set_saved_layout(&self, lane_height: f32, inspector_width: f32) {
        self.0.props.borrow_mut().saved_layout = Some((lane_height, inspector_width));
        self.window().invalidate();
    }

    pub fn take_saved_layout(&self) -> Option<(f32, f32)> {
        self.0.props.borrow_mut().saved_layout.take()
    }

    pub fn get_timeline_offset(&self) -> f32 {
        self.0.props.borrow().timeline_offset
    }

    pub fn set_timeline_offset(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.timeline_offset != value {
            props.timeline_offset = value;
            self.window().invalidate();
        }
    }

    pub fn get_snap(&self) -> bool {
        self.0.props.borrow().snap
    }

    pub fn set_snap(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.snap != value {
            props.snap = value;
            self.window().invalidate();
        }
    }

    pub fn get_time_label(&self) -> String {
        self.0.props.borrow().time_label.clone()
    }

    pub fn set_time_label(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.time_label != value {
            props.time_label = value;
            self.window().invalidate();
        }
    }

    pub fn get_regions(&self) -> ModelRc<Region> {
        self.0.props.borrow().regions.clone()
    }

    pub fn set_regions(&self, value: ModelRc<Region>) {
        let mut props = self.0.props.borrow_mut();
        if props.regions != value {
            props.regions = value;
            self.window().invalidate();
        }
    }

    pub fn get_track_labels(&self) -> ModelRc<String> {
        self.0.props.borrow().track_labels.clone()
    }

    pub fn set_track_labels(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.track_labels != value {
            props.track_labels = value;
            self.window().invalidate();
        }
    }

    pub fn get_lanes_off(&self) -> Vec<String> {
        self.0.props.borrow().lanes_off.clone()
    }

    pub fn set_lanes_off(&self, value: Vec<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.lanes_off != value {
            props.lanes_off = value;
            self.window().invalidate();
        }
    }

    pub fn get_selected_id(&self) -> String {
        self.0.props.borrow().selected_id.clone()
    }

    pub fn set_selected_id(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.selected_id != value {
            props.selected_id = value;
            self.window().invalidate();
        }
    }

    pub fn get_fields(&self) -> ModelRc<Field> {
        self.0.props.borrow().fields.clone()
    }

    pub fn set_fields(&self, value: ModelRc<Field>) {
        let mut props = self.0.props.borrow_mut();
        if props.fields != value {
            props.fields = value;
            self.window().invalidate();
        }
    }

    /// The Settings dialog's rows, whatever panel the inspector shows:
    /// the app's preferences and shortcuts, and the folders it keeps.
    pub fn get_settings_fields(&self) -> ModelRc<Field> {
        self.0.props.borrow().settings_fields.clone()
    }

    pub fn set_settings_fields(&self, value: ModelRc<Field>) {
        let mut props = self.0.props.borrow_mut();
        if props.settings_fields != value {
            props.settings_fields = value;
            self.window().invalidate();
        }
    }

    /// The Settings dialog's section: `General`, `Recording`, `Shortcuts`,
    /// `Library` or `Captions`.
    pub fn get_settings_section(&self) -> String {
        self.0.props.borrow().settings_section.clone()
    }

    pub fn set_settings_section(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.settings_section != value {
            props.settings_section = value;
            self.window().invalidate();
        }
    }

    /// The recorder's sources with what the Source card draws for each, in
    /// the same order as `source_names` (the index `source` options address).
    pub fn get_capture_sources(&self) -> ModelRc<CaptureSource> {
        self.0.props.borrow().capture_sources.clone()
    }

    pub fn set_capture_sources(&self, value: ModelRc<CaptureSource>) {
        let mut props = self.0.props.borrow_mut();
        if props.capture_sources != value {
            props.capture_sources = value;
            self.window().invalidate();
        }
    }

    /// The microphone's current peak in dBFS; negative infinity while
    /// nothing is metering it.
    pub fn get_mic_level(&self) -> f32 {
        self.0.props.borrow().mic_level
    }

    pub fn set_mic_level(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.mic_level != value {
            props.mic_level = value;
            self.window().invalidate();
        }
    }

    /// A frame from the selected camera; empty draws a placeholder.
    pub fn get_camera_preview(&self) -> Image {
        self.0.props.borrow().camera_preview.clone()
    }

    pub fn set_camera_preview(&self, value: Image) {
        let mut props = self.0.props.borrow_mut();
        if props.camera_preview != value {
            props.camera_preview = value;
            self.window().invalidate();
        }
    }

    pub fn get_source_names(&self) -> ModelRc<String> {
        self.0.props.borrow().source_names.clone()
    }

    pub fn set_source_names(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.source_names != value {
            props.source_names = value;
            self.window().invalidate();
        }
    }

    pub fn get_camera_names(&self) -> ModelRc<String> {
        self.0.props.borrow().camera_names.clone()
    }

    pub fn set_camera_names(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.camera_names != value {
            props.camera_names = value;
            self.window().invalidate();
        }
    }

    pub fn get_microphone_names(&self) -> ModelRc<String> {
        self.0.props.borrow().microphone_names.clone()
    }

    pub fn set_microphone_names(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.microphone_names != value {
            props.microphone_names = value;
            self.window().invalidate();
        }
    }

    pub fn get_microphone_kinds(&self) -> ModelRc<String> {
        self.0.props.borrow().microphone_kinds.clone()
    }

    pub fn set_microphone_kinds(&self, value: ModelRc<String>) {
        let mut props = self.0.props.borrow_mut();
        if props.microphone_kinds != value {
            props.microphone_kinds = value;
            self.window().invalidate();
        }
    }

    pub fn get_source_index(&self) -> i32 {
        self.0.props.borrow().source_index
    }

    pub fn set_source_index(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.source_index != value {
            props.source_index = value;
            self.window().invalidate();
        }
    }

    pub fn get_camera_index(&self) -> i32 {
        self.0.props.borrow().camera_index
    }

    pub fn set_camera_index(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.camera_index != value {
            props.camera_index = value;
            self.window().invalidate();
        }
    }

    pub fn get_microphone_index(&self) -> i32 {
        self.0.props.borrow().microphone_index
    }

    pub fn set_microphone_index(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.microphone_index != value {
            props.microphone_index = value;
            self.window().invalidate();
        }
    }

    pub fn get_capture_mic(&self) -> bool {
        self.0.props.borrow().capture_mic
    }

    pub fn set_capture_mic(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.capture_mic != value {
            props.capture_mic = value;
            self.window().invalidate();
        }
    }

    pub fn get_capture_camera(&self) -> bool {
        self.0.props.borrow().capture_camera
    }

    pub fn set_capture_camera(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.capture_camera != value {
            props.capture_camera = value;
            self.window().invalidate();
        }
    }

    pub fn get_capture_system(&self) -> bool {
        self.0.props.borrow().capture_system
    }

    pub fn set_capture_system(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.capture_system != value {
            props.capture_system = value;
            self.window().invalidate();
        }
    }

    pub fn get_panel(&self) -> String {
        self.0.props.borrow().panel.clone()
    }

    pub fn set_panel(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.panel != value {
            props.panel_index = match value.as_str() {
                "Frame" => 0,
                "Cursor" => 1,
                "Webcam" => 2,
                "Captions" => 3,
                "Selection" => 4,
                "Recording" => 5,
                "Export" => 6,
                "Audio" => 7,
                "Recent" => 8,
                "Wallpapers" => 9,
                "Crop" => 10,
                "Add" => 11,
                _ => 12,
            };
            props.panel = value;
            self.window().invalidate();
        }
    }

    pub fn get_panel_index(&self) -> i32 {
        self.0.props.borrow().panel_index
    }

    pub fn set_panel_index(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.panel_index != value {
            props.panel_index = value;
            self.window().invalidate();
        }
    }

    pub fn get_camera(&self) -> bool {
        self.0.props.borrow().camera
    }

    pub fn set_camera(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.camera != value {
            props.camera = value;
            self.window().invalidate();
        }
    }

    pub fn get_microphone(&self) -> bool {
        self.0.props.borrow().microphone
    }

    pub fn set_microphone(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.microphone != value {
            props.microphone = value;
            self.window().invalidate();
        }
    }

    pub fn get_system_audio(&self) -> bool {
        self.0.props.borrow().system_audio
    }

    pub fn set_system_audio(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.system_audio != value {
            props.system_audio = value;
            self.window().invalidate();
        }
    }

    pub fn get_paused(&self) -> bool {
        self.0.props.borrow().paused
    }

    pub fn set_paused(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.paused != value {
            props.paused = value;
            self.window().invalidate();
        }
    }

    pub fn get_has_project(&self) -> bool {
        self.0.props.borrow().has_project
    }

    pub fn set_has_project(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.has_project != value {
            props.has_project = value;
            self.window().invalidate();
        }
    }

    pub fn get_cancellable(&self) -> bool {
        self.0.props.borrow().cancellable
    }

    pub fn set_cancellable(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.cancellable != value {
            props.cancellable = value;
            self.window().invalidate();
        }
    }

    pub fn get_elapsed(&self) -> String {
        self.0.props.borrow().elapsed.clone()
    }

    pub fn set_elapsed(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.elapsed != value {
            props.elapsed = value;
            self.window().invalidate();
        }
    }

    pub fn get_directory(&self) -> String {
        self.0.props.borrow().directory.clone()
    }

    pub fn set_directory(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.directory != value {
            props.directory = value;
            self.window().invalidate();
        }
    }

    pub fn get_project_count(&self) -> i32 {
        self.0.props.borrow().project_count
    }

    pub fn set_project_count(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.project_count != value {
            props.project_count = value;
            self.window().invalidate();
        }
    }

    pub fn get_record_shortcut(&self) -> String {
        self.0.props.borrow().record_shortcut.clone()
    }

    pub fn set_record_shortcut(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.record_shortcut != value {
            props.record_shortcut = value;
            self.window().invalidate();
        }
    }

    pub fn get_countdown(&self) -> i32 {
        self.0.props.borrow().countdown
    }

    pub fn set_countdown(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.countdown != value {
            props.countdown = value;
            self.window().invalidate();
        }
    }

    /// The seconds left on a countdown that is running, or 0 when none is.
    pub fn get_counting(&self) -> i32 {
        self.0.props.borrow().counting
    }

    pub fn set_counting(&self, value: i32) {
        let mut props = self.0.props.borrow_mut();
        if props.counting != value {
            props.counting = value;
            self.window().invalidate();
        }
    }

    /// Capture has ended and the recording is being written out.
    pub fn get_stopping(&self) -> bool {
        self.0.props.borrow().stopping
    }

    pub fn set_stopping(&self, value: bool) {
        let mut props = self.0.props.borrow_mut();
        if props.stopping != value {
            props.stopping = value;
            self.window().invalidate();
        }
    }

    /// The titlebar's export pill: `"exporting"`, `"done"`, `"failed"`,
    /// or empty while there is none.
    pub fn get_export_state(&self) -> String {
        self.0.props.borrow().export_state.clone()
    }

    pub fn set_export_state(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.export_state != value {
            props.export_state = value;
            self.window().invalidate();
        }
    }

    /// The file the pill is about, by name.
    pub fn get_export_name(&self) -> String {
        self.0.props.borrow().export_name.clone()
    }

    pub fn set_export_name(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.export_name != value {
            props.export_name = value;
            self.window().invalidate();
        }
    }

    /// The pill's second line: time left while exporting, the reason after
    /// a failure.
    pub fn get_export_detail(&self) -> String {
        self.0.props.borrow().export_detail.clone()
    }

    pub fn set_export_detail(&self, value: String) {
        let mut props = self.0.props.borrow_mut();
        if props.export_detail != value {
            props.export_detail = value;
            self.window().invalidate();
        }
    }

    /// How far the running export has got, 0 to 1.
    pub fn get_export_progress(&self) -> f32 {
        self.0.props.borrow().export_progress
    }

    pub fn set_export_progress(&self, value: f32) {
        let mut props = self.0.props.borrow_mut();
        if props.export_progress != value {
            props.export_progress = value;
            self.window().invalidate();
        }
    }

    pub fn on_action(&self, callback: impl Fn(String) + 'static) {
        self.0.callbacks.borrow_mut().action = Some(Rc::new(callback));
    }

    pub fn invoke_action(&self, a0: String) {
        let callback = self.0.callbacks.borrow().action.clone();
        if let Some(callback) = callback {
            callback(a0)
        } else {
            Default::default()
        }
    }

    pub fn on_seek(&self, callback: impl Fn(f32) + 'static) {
        self.0.callbacks.borrow_mut().seek = Some(Rc::new(callback));
    }

    pub fn invoke_seek(&self, a0: f32) {
        let callback = self.0.callbacks.borrow().seek.clone();
        if let Some(callback) = callback {
            callback(a0)
        } else {
            Default::default()
        }
    }

    pub fn on_panel_change(&self, callback: impl Fn(String) + 'static) {
        self.0.callbacks.borrow_mut().panel_change = Some(Rc::new(callback));
    }

    pub fn invoke_panel_change(&self, a0: String) {
        let callback = self.0.callbacks.borrow().panel_change.clone();
        if let Some(callback) = callback {
            callback(a0)
        } else {
            Default::default()
        }
    }

    pub fn on_field_change(&self, callback: impl Fn(String, String) + 'static) {
        self.0.callbacks.borrow_mut().field_change = Some(Rc::new(callback));
    }

    pub fn invoke_field_change(&self, a0: String, a1: String) {
        let callback = self.0.callbacks.borrow().field_change.clone();
        if let Some(callback) = callback {
            callback(a0, a1)
        } else {
            Default::default()
        }
    }

    pub fn on_select_region(&self, callback: impl Fn(String, String, bool) + 'static) {
        self.0.callbacks.borrow_mut().select_region = Some(Rc::new(callback));
    }

    pub fn invoke_select_region(&self, a0: String, a1: String, a2: bool) {
        let callback = self.0.callbacks.borrow().select_region.clone();
        if let Some(callback) = callback {
            callback(a0, a1, a2)
        } else {
            Default::default()
        }
    }

    pub fn on_move_region(&self, callback: impl Fn(String, String, f32, i32) + 'static) {
        self.0.callbacks.borrow_mut().move_region = Some(Rc::new(callback));
    }

    pub fn invoke_move_region(&self, a0: String, a1: String, a2: f32, a3: i32) {
        let callback = self.0.callbacks.borrow().move_region.clone();
        if let Some(callback) = callback {
            callback(a0, a1, a2, a3)
        } else {
            Default::default()
        }
    }

    pub fn on_canvas_edit(&self, callback: impl Fn(f32, f32, bool) + 'static) {
        self.0.callbacks.borrow_mut().canvas_edit = Some(Rc::new(callback));
    }

    pub fn invoke_canvas_edit(&self, a0: f32, a1: f32, a2: bool) {
        let callback = self.0.callbacks.borrow().canvas_edit.clone();
        if let Some(callback) = callback {
            callback(a0, a1, a2)
        } else {
            Default::default()
        }
    }

    pub fn on_layout_change(&self, callback: impl Fn(f32, f32) + 'static) {
        self.0.callbacks.borrow_mut().layout_change = Some(Rc::new(callback));
    }

    pub fn invoke_layout_change(&self, lane_height: f32, inspector_width: f32) {
        let callback = self.0.callbacks.borrow().layout_change.clone();
        if let Some(callback) = callback {
            callback(lane_height, inspector_width)
        }
    }

    pub fn on_preview_click(&self, callback: impl Fn(f32, f32) + 'static) {
        self.0.callbacks.borrow_mut().preview_click = Some(Rc::new(callback));
    }

    pub fn invoke_preview_click(&self, a0: f32, a1: f32) {
        let callback = self.0.callbacks.borrow().preview_click.clone();
        if let Some(callback) = callback {
            callback(a0, a1)
        } else {
            Default::default()
        }
    }

    pub fn on_keyboard(&self, callback: impl Fn(String, bool, bool, bool) -> bool + 'static) {
        self.0.callbacks.borrow_mut().keyboard = Some(Rc::new(callback));
    }

    pub fn invoke_keyboard(&self, a0: String, a1: bool, a2: bool, a3: bool) -> bool {
        let callback = self.0.callbacks.borrow().keyboard.clone();
        if let Some(callback) = callback {
            callback(a0, a1, a2, a3)
        } else {
            Default::default()
        }
    }

    pub fn on_translate(&self, callback: impl Fn(String, String) -> String + 'static) {
        self.0.callbacks.borrow_mut().translate = Some(Rc::new(callback));
    }

    pub fn invoke_translate(&self, a0: String, a1: String) -> String {
        let callback = self.0.callbacks.borrow().translate.clone();
        if let Some(callback) = callback {
            callback(a0, a1)
        } else {
            Default::default()
        }
    }

    pub fn on_option(&self, callback: impl Fn(String, String) + 'static) {
        self.0.callbacks.borrow_mut().option = Some(Rc::new(callback));
    }

    pub fn invoke_option(&self, a0: String, a1: String) {
        let callback = self.0.callbacks.borrow().option.clone();
        if let Some(callback) = callback {
            callback(a0, a1)
        } else {
            Default::default()
        }
    }
}

macro_rules! surface {
    ($name:ident, $variant:ident, $kind:expr) => {
        #[derive(Clone)]
        pub struct $name(pub(crate) UiHandle);
        impl std::ops::Deref for $name {
            type Target = UiHandle;
            fn deref(&self) -> &UiHandle {
                &self.0
            }
        }

        impl $name {
            pub fn new() -> anyhow::Result<Self> {
                let window = Window::new($kind);
                // The bar's window is the bar, so it takes the bar's size
                // before it first opens.
                if $kind == ui_runtime::WindowKind::Launcher {
                    window.set_size(ui_runtime::LogicalSize::new(
                        subtake_theme::Theme::RECORDER_WIDTH,
                        subtake_theme::Theme::RECORDER_HEIGHT,
                    ));
                }
                let mut props = Properties::default();
                if $kind == ui_runtime::WindowKind::Editor {
                    props.camera_names =
                        ModelRc::new(VecModel::from(vec!["System default".into()]));
                    props.microphone_names =
                        ModelRc::new(VecModel::from(vec!["System default".into()]));
                } else {
                    // Recorder surfaces begin with their options closed; the
                    // controller supplies device models when synchronizing them.
                    props.panel.clear();
                }
                let value = Self(UiHandle(Rc::new(UiData {
                    props: RefCell::new(props),
                    callbacks: RefCell::new(Callbacks::default()),
                    window,
                })));
                ui_runtime::register(crate::ui::Surface::$variant(value.clone()));
                Ok(value)
            }

            pub fn as_weak(&self) -> ui_runtime::Weak<Self> {
                ui_runtime::Weak::new(&self.0.0)
            }
        }

        impl ui_runtime::FromUiData for $name {
            fn from_data(data: Rc<UiData>) -> Self {
                Self(UiHandle(data))
            }
        }
    };
}
surface!(EditorWindow, Editor, ui_runtime::WindowKind::Editor);
surface!(
    RecordingLauncher,
    Launcher,
    ui_runtime::WindowKind::Launcher
);
surface!(RecordingOptions, Options, ui_runtime::WindowKind::Options);
surface!(
    RecordingCountdown,
    Countdown,
    ui_runtime::WindowKind::Countdown
);
/// The macOS status item is retained by AppKit and dispatches existing controller actions.
#[derive(Clone, Default)]
pub struct AppTray {
    callback: Rc<RefCell<Option<Rc<dyn Fn(String)>>>>,
}

impl AppTray {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self::default())
    }

    pub fn on_action(&self, f: impl Fn(String) + 'static) {
        *self.callback.borrow_mut() = Some(Rc::new(f));
    }

    pub fn show(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
