use crate::{
    AppTray, CaptureArea, CaptureSource, EditorWindow, Field, Recent, RecordingCountdown,
    RecordingLauncher, RecordingOptions, Region, Wallpaper,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use subtake_native::ui_runtime;
use subtake_native::{
    export::{self, ExportSettings},
    media::{self, MediaInfo},
    platform::{self, Recording},
    project::{History, Project, parse_srt},
    render::Scene,
    timeline::{self, n},
};
use ui_runtime::{ModelRc, SharedString, Timer, TimerMode, VecModel};

thread_local! {static STATE:RefCell<Option<(Rc<RefCell<App>>,ui_runtime::Weak<EditorWindow>)>>=const{RefCell::new(None)};}
fn with_app(f: impl FnOnce(&mut App, &EditorWindow)) {
    STATE.with(|slot| {
        if let Some((state, weak)) = slot.borrow().as_ref()
            && let Some(ui) = weak.upgrade()
        {
            let mut app = state.borrow_mut();
            let began = std::time::Instant::now();
            f(&mut app, &ui);
            subtake_ui::perf::log_took("with_app callback", began, 1.0);
            let began = std::time::Instant::now();
            app.sync_launcher(&ui);
            subtake_ui::perf::log_took("with_app sync_launcher", began, 1.0);
        }
    });
}
// Native modal dialogs pump timers while their caller still holds App's borrow.
// Background refreshes may skip a tick; user actions must not be silently dropped.
fn when_idle<T>(state: &RefCell<T>, refresh: impl FnOnce(&T)) {
    if let Ok(state) = state.try_borrow() {
        refresh(&state);
    }
}

fn post(f: impl FnOnce(&mut App, &EditorWindow) + Send + 'static) {
    let _ = ui_runtime::invoke_from_event_loop(move || with_app(f));
}

fn report(ui: &EditorWindow, result: Result<()>) {
    if let Err(error) = result {
        ui.set_status(format!("{error:#}"));
    }
}

mod actions;
mod documents;
mod fields;
mod playback;
mod presets;
mod recorder;
mod run;
mod smoke;
mod walkthrough;

pub use run::run;

use documents::id_to_null;
use playback::{Preview, aspect_ratio};
use smoke::launcher_smoke_step;

pub struct App {
    preferences: subtake_native::preferences::Preferences,
    history: Option<History>,
    document: Option<PathBuf>,
    source: Option<PathBuf>,
    /// The source's preview proxy once it is made (`media::proxy`), and the
    /// stop for the one being made.
    proxy: Option<PathBuf>,
    proxy_cancel: Arc<AtomicBool>,
    info: Option<MediaInfo>,
    selected: Option<(String, String)>,
    /// The annotations on the last preview frame, bottom first, with their
    /// normalized bounds (`Scene::annotation_bounds`).
    shown_annotations: Vec<(String, [f32; 4])>,
    clipboard: Vec<(String, Value, Option<Value>)>,
    extra_selection: Vec<(String, String)>,
    recovery: subtake_native::recovery::Store,
    recovery_timer: Timer,
    recovery_origin: Option<PathBuf>,
    recoveries: Vec<PathBuf>,
    wallpapers: Vec<PathBuf>,
    presets: Vec<PathBuf>,
    library: Vec<PathBuf>,
    library_query: String,
    /// Each library entry's card still and running time once asked for
    /// (`media::library_still`): `None` while it is being made or when it
    /// cannot be.
    stills: std::collections::HashMap<PathBuf, Option<(ui_runtime::Image, f64)>>,
    fresh_recording: Option<PathBuf>,
    preview: Preview,
    epoch: u64,
    /// The playhead has just been moved: frames come from the proxy, as
    /// playback's do, until `scrub_settle` finds it at rest and draws from
    /// the take.
    scrubbing: bool,
    scrub_settle: Timer,
    source_time: f64,
    source_revision: u64,
    playback: Timer,
    started: Option<(Arc<AtomicU64>, f64)>,
    audio_cancel: Arc<AtomicBool>,
    job_cancel: Arc<AtomicBool>,
    /// The running export's own stop, apart from `job_cancel`, because an
    /// export no longer holds the editor and can run beside another job.
    export_cancel: Arc<AtomicBool>,
    sources: Vec<Value>,
    devices: Value,
    launcher: Option<RecordingLauncher>,
    launcher_options: Option<RecordingOptions>,
    /// The count drawn over the display being recorded, beside the bar's.
    countdown_overlay: Option<RecordingCountdown>,
    tray: Option<AppTray>,
    editor_shown: bool,
    capture_started: Option<std::time::Instant>,
    /// Seconds left on the countdown before a capture, 0 when none runs.
    counting: u32,
    /// Capture has ended and the recording is being written out.
    stopping: bool,
    /// The Source card's area overlay is up: the bar and the card are
    /// hidden under it and the global shortcuts wait until it closes.
    drawing_area: bool,
    /// The sources are being listed again behind the Source card
    /// (`refresh_sources_quietly`).
    refreshing_sources: bool,
    pause_started: Option<std::time::Instant>,
    paused_total: Duration,
    recording_watch: Timer,
    recording: Option<Recording>,
    hotkeys: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey_ids: Vec<(u32, String)>,
    /// Esc, held as a global shortcut only while the countdown runs: the
    /// count is over whatever app is in front, so the bar has no focus.
    escape_hotkey: Option<global_hotkey::hotkey::HotKey>,
    last_export: Option<PathBuf>,
    /// The panel a click on a region replaced with Selection, to go back to
    /// when nothing is selected any more.
    panel_before_selection: Option<String>,
    /// The Export panel's Frame format: a still of the current frame. Not
    /// stored in the project — a still is a one-off, not a setting.
    export_frame: bool,
    /// The file Change… picked; `None` writes to the Movies folder.
    export_path: Option<PathBuf>,
}

impl App {
    pub(super) fn new() -> Self {
        Self {
            preferences: subtake_native::preferences::Preferences::load().unwrap_or_else(|e| {
                eprintln!("Preferences: {e}");
                Default::default()
            }),
            history: None,
            document: None,
            source: None,
            proxy: None,
            proxy_cancel: Arc::new(AtomicBool::new(false)),
            info: None,
            selected: None,
            shown_annotations: vec![],
            clipboard: vec![],
            extra_selection: vec![],
            recovery: subtake_native::recovery::Store::new(|message| {
                post(move |_, ui| ui.set_status(message))
            }),
            recovery_timer: Timer::default(),
            recovery_origin: None,
            recoveries: subtake_native::recovery::list().unwrap_or_default(),
            wallpapers: media::wallpapers(),
            presets: subtake_native::presets::list(),
            library: vec![],
            library_query: String::new(),
            stills: Default::default(),
            fresh_recording: None,
            preview: Preview::new(),
            epoch: 0,
            scrubbing: false,
            scrub_settle: Timer::default(),
            source_time: 0.,
            source_revision: 0,
            playback: Timer::default(),
            started: None,
            audio_cancel: Arc::new(AtomicBool::new(false)),
            job_cancel: Arc::new(AtomicBool::new(false)),
            export_cancel: Arc::new(AtomicBool::new(false)),
            sources: vec![],
            devices: Value::Null,
            launcher: None,
            launcher_options: None,
            countdown_overlay: None,
            tray: None,
            editor_shown: false,
            capture_started: None,
            counting: 0,
            stopping: false,
            drawing_area: false,
            refreshing_sources: false,
            pause_started: None,
            paused_total: Duration::ZERO,
            recording_watch: Timer::default(),
            recording: None,
            hotkeys: None,
            hotkey_ids: vec![],
            escape_hotkey: None,
            last_export: None,
            panel_before_selection: None,
            export_frame: false,
            export_path: None,
        }
    }

    pub(super) fn selected_keys(&self) -> Vec<(String, String)> {
        let mut keys = self.extra_selection.clone();
        if let Some(key) = &self.selected
            && !keys.contains(key)
        {
            keys.push(key.clone());
        }
        keys.retain(|(kind, id)| {
            self.history
                .as_ref()
                .is_some_and(|h| h.project.regions(kind).iter().any(|r| r["id"] == *id))
        });
        keys
    }

    pub(super) fn project(&self) -> Result<&Project> {
        self.history
            .as_ref()
            .map(|h| &h.project)
            .context("Open a video first")
    }

    pub(super) fn edit(
        &mut self,
        ui: &EditorWindow,
        f: impl FnOnce(&mut Project) -> Result<()>,
    ) -> Result<()> {
        self.stop(ui);
        self.history
            .as_mut()
            .context("Open a video first")?
            .edit(f)?;
        self.epoch += 1;
        self.schedule_recovery();
        self.refresh(ui);
        self.request();
        Ok(())
    }
}
