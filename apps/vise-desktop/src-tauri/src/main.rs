#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Vise desktop shell.
//!
//! The window never runs the transcode pipeline itself. Scanning and encoding
//! happen in the background worker process, reached over its loopback,
//! token-authenticated JSON API, so an encoder crash costs a job rather than
//! the application. Process isolation can be switched off in the portable
//! settings file; only then does the shell fall back to calling the core
//! library in-process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;
use vise_core::{Job, JobId, JobSpec, JobState, Progress, Session};
use vise_ipc::contract::{CommandResult, EncodeCommand, InitCommand, ScanCommand, WorkState};
use vise_ipc::portable::PortableSettings;
use vise_ipc::timespan::TimeSpan;
use vise_ipc::{WorkerClient, WorkerConfig, WorkerSupervisor};

mod scanflow;
mod sources;
mod tabs;
mod viewmodel;
mod windows;

use scanflow::{scan_in_process, ScanFlow};
use sources::{RecentSources, SourceDropReport, SourceViewModel};
use tabs::{
    AudioDefaultsInput, AudioTrackInput, ChaptersInput, DimensionsInput, FilterInput, MarkerInput,
    MetadataInput, SubtitleTrackInput, SubtitlesInput, SummaryInput, TabSetViewModel, TabSettings,
    TitleFacts, VideoInput,
};
use viewmodel::{
    RangeSelection, ScanPhase, ScanProgressViewModel, SelectionViewModel, TrackSelectionInput,
};
use windows::dialogs::{self, AboutViewModel, AlertAction, AlertState, ErrorState, ErrorViewModel};
use windows::log::ActivityLog;
use windows::presets::{PresetAction, PresetInput, PresetState};
use windows::preview::{CropOverlay, PreviewState, PreviewViewModel, StillImage, StillPosition};
use windows::queue::{QueueSelectionInput, QueueState};
use windows::SurfaceState;

/// How the shell reaches the transcode pipeline.
enum Backend {
    /// A background worker process.
    Worker(Box<WorkerSupervisor>),
    /// In-process execution, only when portable settings disable isolation.
    InProcess,
}

impl Backend {
    /// A client for the running worker, if isolation is enabled.
    fn client(&mut self) -> Option<WorkerClient> {
        match self {
            Backend::Worker(supervisor) => {
                // A worker that died underneath the window is replaced here, so
                // a failed job never becomes an application failure.
                match supervisor.ensure_running() {
                    Ok(connection) => Some(connection.client()),
                    Err(e) => {
                        eprintln!("worker restart failed: {e}");
                        None
                    }
                }
            }
            Backend::InProcess => None,
        }
    }

    /// One-line description used in the activity log.
    fn describe(&self) -> String {
        match self {
            Backend::Worker(supervisor) => match supervisor.connection() {
                Some(connection) => format!(
                    "worker process {} on 127.0.0.1:{} ({} restart(s))",
                    connection.pid,
                    connection.port,
                    supervisor.restarts()
                ),
                None => "worker process is not running".to_string(),
            },
            Backend::InProcess => {
                "running in process: process isolation is switched off".to_string()
            }
        }
    }
}

#[derive(Default)]
struct DesktopState {
    session: Session,
    /// The activity history, from this window and from the worker.
    activity: ActivityLog,
    backend: Option<Backend>,
    /// The source surface: source, scan, titles, and the current selection.
    flow: ScanFlow,
    /// The configuration tab set.
    tabs: TabSettings,
    /// Sources the user opened recently.
    recents: RecentSources,
    /// Where the recent source list is persisted.
    settings_path: PathBuf,
    /// Monotonic stamp that orders the recent source list.
    recents_stamp: u64,
    /// Which secondary windows and dialogs are open.
    surfaces: SurfaceState,
    /// The queue's own order and priorities.
    queue: QueueState,
    /// What the queue selection sheet is holding.
    selection: QueueSelectionInput,
    /// The still frames the preview window is looking at.
    preview: PreviewState,
    /// Everything that went wrong.
    errors: ErrorState,
    /// The action waiting for its countdown.
    alerts: AlertState,
    /// The action the user has confirmed but not yet acted on.
    confirmed: Option<AlertAction>,
    /// The preset library.
    presets: PresetState,
}

type SharedState = Arc<Mutex<DesktopState>>;

fn push_log(state: &SharedState, message: impl Into<String>) {
    if let Ok(mut guard) = state.lock() {
        guard.activity.record(message);
    }
}

/// Record a problem and hand back its number.
///
/// Everything that fails for a reason the user can act on goes through here, so
/// the error view is a record of what happened rather than a place the page
/// composes strings on its own.
fn record_problem(
    state: &SharedState,
    source: &str,
    context: impl Into<String>,
    detail: impl Into<String>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.errors.record(source, &context.into(), detail);
    }
}

/// Record a warning.
fn record_warning(
    state: &SharedState,
    source: &str,
    context: impl Into<String>,
    detail: impl Into<String>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.errors.push_warning(source, &context.into(), detail);
    }
}

/// Bring the queue's order in line with the session.
fn sync_queue(guard: &mut DesktopState) {
    let jobs = guard.session.jobs().to_vec();
    guard.queue.sync(&jobs);
}

/// The queue, as the page renders it, for a state that is already locked.
fn queue_view(guard: &DesktopState) -> windows::queue::QueueViewModel {
    windows::queue::queue_view(&guard.queue, guard.session.jobs())
}

/// Whether the user has confirmed `action` and has not acted on it yet.
///
/// The countdown alert is only worth having if it is load-bearing: an action
/// that destroys work asks first, and the answer has to come from the alert
/// rather than from the same control that asked for it.
fn is_confirmed(state: &SharedState, action: AlertAction) -> bool {
    state
        .lock()
        .ok()
        .map(|guard| guard.confirmed == Some(action))
        .unwrap_or(false)
}

/// Refuse `action` until it has been confirmed, with the reason to show.
fn require_confirmation(state: &SharedState, action: AlertAction) -> Result<(), String> {
    if is_confirmed(state, action) {
        return Ok(());
    }
    Err(format!(
        "'{}' cannot be undone, so it asks for confirmation first.",
        action.verb()
    ))
}

/// Spend a confirmation, so one answer cannot drive two actions.
fn take_confirmation(state: &SharedState, action: AlertAction) {
    if let Ok(mut guard) = state.lock() {
        if guard.confirmed == Some(action) {
            guard.confirmed = None;
        }
    }
}

/// Build the backend the application will use.
///
/// Isolation is on unless a portable settings file switches it off, and the
/// worker executable is looked for next to the application binary first so a
/// packaged build uses its own copy.
fn build_backend() -> Backend {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let settings = match vise_ipc::portable::load_for(&executable_dir) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("portable settings ignored: {e}");
            PortableSettings::default()
        }
    };
    if !settings.should_isolate() {
        let reason = settings.isolation_disabled_reason();
        eprintln!("{reason}");
        return Backend::InProcess;
    }

    let name = if cfg!(windows) {
        "vise-worker.exe"
    } else {
        "vise-worker"
    };
    let candidates = [
        executable_dir.join(name),
        PathBuf::from("target/debug").join(name),
        PathBuf::from("target/release").join(name),
    ];
    let Some(executable) = candidates.into_iter().find(|path| path.is_file()) else {
        eprintln!(
            "the worker executable '{name}' was not found next to the application; \
             process isolation is unavailable"
        );
        return Backend::InProcess;
    };

    let mut supervisor =
        WorkerSupervisor::new(WorkerConfig::new(executable).with_portable_settings(&settings));
    match supervisor.start() {
        Ok(connection) => {
            eprintln!(
                "worker process {} listening on 127.0.0.1:{}",
                connection.pid, connection.port
            );
            Backend::Worker(Box::new(supervisor))
        }
        Err(e) => {
            eprintln!("worker process could not be started: {e}");
            Backend::InProcess
        }
    }
}

/// Whether a worker is available for the next operation.
fn worker_client(state: &SharedState) -> Result<Option<WorkerClient>, String> {
    let mut guard = state
        .lock()
        .map_err(|_| "desktop state is unavailable".to_string())?;
    if guard.backend.is_none() {
        guard.backend = Some(build_backend());
    }
    Ok(guard.backend.as_mut().and_then(Backend::client))
}

/// Logging preferences sent with every scan and encode command.
fn init_command() -> InitCommand {
    InitCommand::default()
}

// ---------------------------------------------------------------------------
// Source surface
// ---------------------------------------------------------------------------

/// Scan options as the interface layer sends them.
///
/// The window owns validation: an unusable option is refused with a reason
/// instead of being forwarded to the worker and failing there.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ScanOptions {
    /// Preview images to extract per title. Zero asks for none, which is what
    /// the window wants until the preview surface is in use.
    preview_count: Option<u32>,
    /// Shortest acceptable title duration, as `h:mm:ss`.
    min_duration: Option<String>,
    /// Longest acceptable title duration, as `h:mm:ss`.
    max_duration: Option<String>,
    /// Keep duplicate title entries instead of collapsing them.
    keep_duplicate_titles: bool,
    /// File names skipped during the scan.
    file_exclusions: Vec<String>,
}

impl ScanOptions {
    /// Build the worker command for `paths`.
    fn to_command(&self, paths: Vec<String>) -> Result<ScanCommand, String> {
        Ok(ScanCommand {
            initialise_command: InitCommand {
                keep_duplicate_titles: self.keep_duplicate_titles,
                ..init_command()
            },
            paths,
            preview_count: self.preview_count.unwrap_or(0).min(32),
            min_duration: parse_duration(self.min_duration.as_deref(), "shortest")?,
            max_duration: parse_duration(self.max_duration.as_deref(), "longest")?,
            file_exclusion_list: self
                .file_exclusions
                .iter()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect(),
            ..ScanCommand::default()
        })
    }
}

/// Parse a duration option, treating an empty value as "no limit".
fn parse_duration(text: Option<&str>, label: &str) -> Result<TimeSpan, String> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(TimeSpan::ZERO);
    };
    TimeSpan::parse(text)
        .map_err(|e| format!("The {label} title duration '{text}' is not valid: {e}"))
}

/// Progress of the current scan, as the interface polls it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanStatusViewModel {
    phase: ScanPhase,
    phase_name: String,
    running: bool,
    progress: Option<ScanProgressViewModel>,
    title_count: usize,
    problem: String,
}

impl ScanStatusViewModel {
    fn of(flow: &ScanFlow) -> Self {
        let view = flow.view_model();
        Self {
            phase: flow.phase(),
            phase_name: flow.phase().as_str().to_string(),
            running: flow.is_running(),
            progress: view.progress,
            title_count: view.titles.len(),
            problem: view.problem,
        }
    }
}

/// Validate and load a source the user picked.
#[tauri::command]
fn select_source(
    state: State<'_, SharedState>,
    path: String,
) -> Result<SelectionViewModel, String> {
    let source = sources::resolve_source(&path).map_err(|e| e.message().to_string())?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.recents_stamp = sources::next_stamp(guard.recents_stamp);
    let stamp = guard.recents_stamp;
    guard.recents.record(&source, stamp);
    guard.flow.set_source(Some(source));
    let _ = guard.recents.save(&guard.settings_path);
    Ok(guard.flow.view_model())
}

/// Check paths that arrived from a drag-and-drop gesture or a paste.
///
/// Every path is answered with a reason, so a gesture that mixes a folder and
/// an unreadable file explains itself instead of silently doing half the work.
#[tauri::command]
fn inspect_sources(paths: Vec<String>) -> Result<SourceDropReport, String> {
    Ok(sources::inspect_paths(&paths))
}

/// Forget the loaded source.
#[tauri::command]
fn clear_source(state: State<'_, SharedState>) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.set_source(None);
    Ok(guard.flow.view_model())
}

/// Sources opened recently, newest first.
#[tauri::command]
fn recent_sources(state: State<'_, SharedState>) -> Result<Vec<SourceViewModel>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let before = guard.recents.len();
    let views = guard.recents.view_models();
    if guard.recents.len() != before {
        let _ = guard.recents.save(&guard.settings_path);
    }
    Ok(views)
}

/// Drop one recent source.
#[tauri::command]
fn forget_recent_source(state: State<'_, SharedState>, path: String) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.recents.forget(&path);
    guard.recents.save(&guard.settings_path)
}

/// Forget every recent source.
#[tauri::command]
fn clear_recent_sources(state: State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.recents.clear();
    guard.recents.save(&guard.settings_path)
}

/// The whole source surface, as the interface renders it.
#[tauri::command]
fn source_selection(state: State<'_, SharedState>) -> Result<SelectionViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.flow.view_model())
}

/// Start scanning the loaded source.
///
/// The command returns as soon as the worker accepted the work, and the
/// interface polls [`scan_status`] from there, so a long scan never blocks the
/// window.
#[tauri::command]
fn begin_scan(
    state: State<'_, SharedState>,
    options: Option<ScanOptions>,
) -> Result<ScanStatusViewModel, String> {
    let worker = worker_client(&state)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let Some(source) = guard.flow.source().cloned() else {
        return Err("Choose a file or folder to scan.".to_string());
    };
    let mut options = options.unwrap_or_default();
    // The preview window owns how many stills a scan extracts, so the page does
    // not have to remember it.
    if options.preview_count.is_none() {
        options.preview_count = Some(guard.preview.still_count());
    }
    let command = options.to_command(vec![source.path.clone()])?;
    match worker {
        Some(client) => {
            let started = client.start_scan(&command).map_err(|e| e.to_string())?;
            if !started.was_successful {
                guard.flow.fail(started.error);
                return Err(ScanStatusViewModel::of(&guard.flow).problem);
            }
            guard.flow.begin();
        }
        None => {
            // Portable settings switched process isolation off; the same scan
            // runs here so the window behaves the same either way.
            match scan_in_process(std::slice::from_ref(&source)) {
                Ok(titles) => guard.flow.accept_scan(titles),
                Err(e) => guard.flow.fail(e),
            }
        }
    }
    Ok(ScanStatusViewModel::of(&guard.flow))
}

/// Poll the running scan once.
#[tauri::command]
fn scan_status(state: State<'_, SharedState>) -> Result<ScanStatusViewModel, String> {
    let worker = worker_client(&state)?;
    // The worker round trip happens before the state is locked, so a slow or
    // hung worker cannot freeze the rest of the window.
    let polled = match &worker {
        Some(client) => {
            let report = client.poll_scan_progress().map_err(|e| e.to_string())?;
            let finished = matches!(report.state, WorkState::ScanDone | WorkState::WorkDone);
            let successful = report
                .work_done
                .as_ref()
                .map(|done| done.was_successful)
                .unwrap_or(true);
            let titles = if finished && successful {
                Some(client.get_titles().map_err(|e| e.to_string())?)
            } else {
                None
            };
            Some((report, titles))
        }
        None => None,
    };
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some((report, titles)) = polled {
        guard.flow.apply_progress(&report);
        if let Some(titles) = titles {
            guard.flow.apply_titles(titles);
        }
    }
    Ok(ScanStatusViewModel::of(&guard.flow))
}

/// Choose a title, which replaces the track and range selection with that
/// title's own.
#[tauri::command]
fn choose_title(state: State<'_, SharedState>, index: u32) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.select_title(index)?;
    Ok(guard.flow.view_model())
}

/// Range choice as the interface layer sends it.
///
/// The mode arrives as text and is validated here, so an unknown mode is
/// refused with a readable reason rather than a decoding failure. An empty mode
/// keeps the mode already in force, which lets the point pickers work without
/// the page having to track the mode itself.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RangeChoice {
    mode: String,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    angle: Option<u32>,
}

/// Apply a range and angle choice for the chosen title.
#[tauri::command]
fn choose_range(
    state: State<'_, SharedState>,
    choice: RangeChoice,
) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let current_mode = guard
        .flow
        .view_model()
        .range
        .map(|range| range.mode)
        .unwrap_or_default();
    let mut selection = RangeSelection {
        mode: current_mode,
        start_seconds: choice.start_seconds,
        end_seconds: choice.end_seconds,
        angle: choice.angle,
    };
    if !choice.mode.trim().is_empty() {
        selection = selection.with_mode_name(&choice.mode)?;
    }
    guard.flow.set_range(selection)?;
    Ok(guard.flow.view_model())
}

/// Apply the track choice for the chosen title.
#[tauri::command]
fn choose_tracks(
    state: State<'_, SharedState>,
    input: TrackSelectionInput,
) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.set_tracks(input.clone())?;
    // The tab set keeps the same choice, so the audio and subtitle tabs start
    // with the tracks the source area has already chosen.
    guard.tabs.choose_tracks(&input.audio, &input.subtitle);
    Ok(guard.flow.view_model())
}

/// The preview window, as the page renders it.
///
/// The view is built from a locked state rather than by calling the command
/// again, so a command that already holds the lock cannot ask for it twice.
fn preview_view(guard: &mut DesktopState) -> PreviewViewModel {
    point_preview_at_title(guard);
    let selection = guard.flow.view_model();
    let source_label = guard
        .flow
        .source()
        .map(|source| source.name.clone())
        .unwrap_or_default();
    let title_label = selection
        .titles
        .iter()
        .find(|title| title.index == selection.selected_title_index)
        .map(|title| title.label.clone())
        .unwrap_or_default();
    guard
        .preview
        .view(guard.flow.source().is_some(), &source_label, &title_label)
}

/// The preview window, as the page renders it.
#[tauri::command]
fn preview_window(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    Ok(preview_view(&mut guard))
}

/// Keep the preview window pointed at whatever title the source area has
/// chosen, so the stills on screen and the crop rectangle always describe the
/// title the rest of the window is working on.
fn point_preview_at_title(guard: &mut DesktopState) {
    let title = guard.flow.current_title().cloned();
    let wanted = match title {
        Some(title) => (title.duration_seconds, (title.width, title.height)),
        None => (0.0, (0, 0)),
    };
    if guard.preview.describes() != wanted {
        guard.preview.use_title(wanted.0, wanted.1);
    }
}

/// Ask the worker for the still the preview window is showing.
#[tauri::command]
fn fetch_preview_still(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let (command, number) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        preview_view(&mut guard);
        let path = guard
            .flow
            .source()
            .map(|source| source.path.clone())
            .ok_or_else(|| "Choose a file or folder to scan first.".to_string())?;
        let title_index = guard
            .flow
            .current_title()
            .map(|title| title.index)
            .ok_or_else(|| "Choose a title before asking for a still.".to_string())?;
        let number = guard.preview.still_number();
        (guard.preview.command(&path, title_index), number)
    };
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err("preview stills require the worker process".to_string());
    };
    let result = client.get_preview(&command).map_err(|e| e.to_string())?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !result.was_successful {
        let reason = if result.error.trim().is_empty() {
            "the worker produced no image for that point in the title".to_string()
        } else {
            result.error.clone()
        };
        guard
            .errors
            .record("worker", "extracting a preview still", &reason);
        guard.preview.fail_image(&reason);
    } else if result.image_base64.is_empty() {
        guard
            .preview
            .fail_image("the worker returned no image for that point in the title");
    } else {
        guard.preview.accept_image(StillImage {
            number,
            byte_length: result.image_bytes,
            data_base64: result.image_base64,
        });
    }
    Ok(preview_view(&mut guard))
}

/// Choose how many stills the next scan extracts.
#[tauri::command]
fn set_preview_still_count(
    state: State<'_, SharedState>,
    count: u32,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.set_still_count(count)?;
    Ok(preview_view(&mut guard))
}

/// Move along the still list.
#[tauri::command]
fn select_preview_still(
    state: State<'_, SharedState>,
    number: Option<u32>,
    delta: Option<i32>,
    position: Option<StillPosition>,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    preview_view(&mut guard);
    if let Some(position) = position {
        guard.preview.scrub(position)?;
    } else if let Some(delta) = delta {
        guard.preview.step(delta);
    } else if let Some(number) = number {
        guard.preview.select(number)?;
    } else {
        return Err("Ask for a still by number, by step, or by position.".to_string());
    }
    Ok(preview_view(&mut guard))
}

/// Move the crop rectangle drawn over the frame.
#[tauri::command]
fn set_preview_crop(
    state: State<'_, SharedState>,
    crop: CropOverlay,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.set_overlay(crop)?;
    Ok(preview_view(&mut guard))
}

/// Put the rectangle drawn over the frame into the dimensions tab.
///
/// The preview window and the dimensions tab are the same rectangle: the tab's
/// crop fields are written from the overlay, so what the user drew and what the
/// encoder is asked for cannot drift apart.
#[tauri::command]
fn apply_preview_crop(state: State<'_, SharedState>) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let Some(rect) = guard.preview.crop_rect()? else {
        guard.tabs.dimensions.crop_mode = "none".to_string();
        guard
            .activity
            .record("preview crop cleared; the whole frame is kept");
        return Ok(guard.tabs.view(&facts));
    };
    guard.tabs.dimensions.crop_mode = "manual".to_string();
    guard.tabs.dimensions.crop_x = tabs::common::NumField::of(rect.0);
    guard.tabs.dimensions.crop_y = tabs::common::NumField::of(rect.1);
    guard.tabs.dimensions.crop_width = tabs::common::NumField::of(rect.2);
    guard.tabs.dimensions.crop_height = tabs::common::NumField::of(rect.3);
    guard.activity.record(format!(
        "preview crop applied: {}, {} · {} by {}",
        rect.0, rect.1, rect.2, rect.3
    ));
    Ok(guard.tabs.view(&facts))
}

/// Forget the crop rectangle and keep the whole frame.
#[tauri::command]
fn reset_preview_crop(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.reset_overlay();
    Ok(preview_view(&mut guard))
}

// ---------------------------------------------------------------------------
// Configuration tabs
// ---------------------------------------------------------------------------

/// The facts the tabs read for the chosen title.
fn title_facts(guard: &DesktopState) -> TitleFacts {
    TitleFacts::of(guard.flow.current_title())
}

/// Every tab, as the page renders it.
///
/// The tab set is brought in line with the chosen title first, so a title with
/// different tracks never leaves the previous title's per-track rows behind.
#[tauri::command]
fn tab_set(state: State<'_, SharedState>) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one tab and hand back every tab.
///
/// Each command does the same three things: read the change, let the window
/// check it, and return the whole set. Returning the whole set is what keeps the
/// page honest — it never has to work out which other tabs the change affected.
macro_rules! tab_command {
    ($name:ident, $input:ty, |$tabs:ident, $change:ident| $body:expr) => {
        #[tauri::command]
        fn $name(state: State<'_, SharedState>, change: $input) -> Result<TabSetViewModel, String> {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            let facts = title_facts(&guard);
            guard.tabs.sync_with_title(&facts);
            {
                let $tabs = &mut guard.tabs;
                let $change = change;
                $body?;
            }
            Ok(guard.tabs.view(&facts))
        }
    };
}

/// Apply a change to the summary tab.
///
/// The metadata tab owns the passthrough flag, so this switch writes into it
/// rather than keeping a second copy that could drift out of step.
#[tauri::command]
fn set_summary(
    state: State<'_, SharedState>,
    change: SummaryInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let passthrough = change.passthrough_metadata;
    change.apply(&mut guard.tabs.summary)?;
    if passthrough.is_some() {
        let wanted = guard.tabs.summary.passthrough_metadata;
        guard.tabs.metadata.set_passthrough(wanted);
    }
    Ok(guard.tabs.view(&facts))
}

tab_command!(set_dimensions, DimensionsInput, |tabs, change| {
    change.apply(&mut tabs.dimensions)
});

tab_command!(set_filter, FilterInput, |tabs, change| {
    change.apply(&mut tabs.filters)
});

tab_command!(set_video, VideoInput, |tabs, change| {
    change.apply(&mut tabs.video)
});

tab_command!(set_subtitles, SubtitlesInput, |tabs, change| {
    change.apply(&mut tabs.subtitles)
});

tab_command!(set_chapters, ChaptersInput, |tabs, change| {
    change.apply(&mut tabs.chapters)
});

tab_command!(change_marker, MarkerInput, |tabs, change| {
    change.apply(&mut tabs.chapters)
});

/// Apply a change to the metadata tab.
///
/// The tab reads the chosen title's tags, so it needs those facts rather than
/// the generic macro the other tabs use.
#[tauri::command]
fn set_metadata(
    state: State<'_, SharedState>,
    change: MetadataInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    change.apply(&mut guard.tabs.metadata, &facts)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to the audio rules every track starts from.
#[tauri::command]
fn set_audio_defaults(
    state: State<'_, SharedState>,
    change: AudioDefaultsInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let container = guard.tabs.summary.container.clone();
    change.apply(&mut guard.tabs.audio.defaults, &container)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one audio track.
#[tauri::command]
fn set_audio_track(
    state: State<'_, SharedState>,
    change: AudioTrackInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let container = guard.tabs.summary.container.clone();
    change.apply(&mut guard.tabs.audio, &container)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one subtitle track.
#[tauri::command]
fn set_subtitle_track(
    state: State<'_, SharedState>,
    change: SubtitleTrackInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    change.apply(&mut guard.tabs.subtitles)?;
    Ok(guard.tabs.view(&facts))
}

/// Show one tab.
///
/// The page keeps its own notion of which panel is open, but the window records
/// it so the next launch opens where the user left off.
#[tauri::command]
fn set_active_tab(state: State<'_, SharedState>, tab: String) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !guard.tabs.is_tab(&tab) {
        return Err(format!("'{}' is not a tab this build offers.", tab.trim()));
    }
    guard.tabs.active = tab.trim().to_ascii_lowercase();
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Add an external subtitle file to the output.
///
/// An empty path clears the imported file, which is how the page offers a way out
/// without a second command.
#[tauri::command]
fn import_subtitles(
    state: State<'_, SharedState>,
    path: String,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        guard.tabs.subtitles.external_path.clear();
        guard.tabs.subtitles.external_name.clear();
    } else {
        let source = sources::resolve_source(trimmed)
            .map_err(|e| format!("That subtitle file could not be used: {}", e.message()))?;
        if source.kind != sources::SourceKind::File {
            return Err("Choose a subtitle file rather than a folder.".to_string());
        }
        if !subtitle_extension(&source.name) {
            return Err(format!(
                "'{}' does not look like a subtitle file.",
                source.name
            ));
        }
        guard.tabs.subtitles.external_path = source.path.clone();
        guard.tabs.subtitles.external_name = source.name.clone();
    }
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Whether a file name looks like a subtitle file this build can read.
fn subtitle_extension(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension.trim().to_ascii_lowercase().as_str(),
        "srt" | "ass" | "ssa" | "vtt" | "webvtt" | "sub" | "smi" | "txt" | "ttml" | "dfxp" | "mpl2"
    )
}

// ---------------------------------------------------------------------------
// Queue
// ---------------------------------------------------------------------------

#[tauri::command]
fn add_job(state: State<'_, SharedState>, spec: JobSpec) -> Result<JobId, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let id = guard.session.add_job(spec);
    guard.queue.push(id);
    guard.activity.record(format!("queued job {id}"));
    Ok(id)
}

/// Queue the current source selection.
///
/// The page names the output file and nothing else: the window builds the job
/// from the tab settings it already validated, so a queued job cannot disagree
/// with what the tabs showed, and it cannot be queued against a title the user
/// did not select.
#[tauri::command]
fn queue_selection(state: State<'_, SharedState>, output_path: String) -> Result<JobId, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let selection = guard.flow.view_model();
    if !selection.is_usable() {
        return Err("Scan a source and choose a title before queueing a job.".to_string());
    }
    let output_path = output_path.trim();
    if output_path.is_empty() {
        return Err("Choose where the finished file should be written.".to_string());
    }
    let source = guard
        .flow
        .source()
        .map(|source| source.path.clone())
        .ok_or_else(|| "Choose a file or folder to scan.".to_string())?;
    let title_index = guard
        .flow
        .current_title()
        .map(|title| title.index)
        .ok_or_else(|| "Choose a title before queueing a job.".to_string())?;

    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let tabs = guard.tabs.view(&facts);
    if !tabs.can_queue {
        return Err(format!(
            "Fix these before queueing: {}",
            tabs.problems.join(" ")
        ));
    }
    let mut spec = guard.tabs.to_job_spec(&source, output_path, &facts)?;
    spec.title_index = Some(selection.selected_title_index);

    let id = guard.session.add_job(spec);
    guard.queue.push(id);
    guard
        .activity
        .record(format!("queued job {id} for title {title_index}"));
    Ok(id)
}

/// The titles the queue selection sheet may offer, with the facts it needs.
fn selection_titles(guard: &DesktopState) -> Vec<(u32, String, String)> {
    guard
        .flow
        .view_model()
        .titles
        .iter()
        .map(|title| (title.index, title.name.clone(), title.facts.clone()))
        .collect()
}

/// The output extension the chosen container implies.
fn output_extension(guard: &DesktopState) -> String {
    guard
        .tabs
        .summary
        .container
        .clone()
        .trim()
        .to_ascii_lowercase()
}

/// The queue selection sheet, as the page renders it.
#[tauri::command]
fn queue_selection_window(
    state: State<'_, SharedState>,
) -> Result<windows::queue::QueueSelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let titles = selection_titles(&guard);
    let extension = output_extension(&guard);
    // An empty sheet starts with the chosen title picked, because that is the
    // one title a user almost always wants.
    if guard.selection.titles.is_empty() {
        let chosen = guard.flow.view_model().selected_title_index;
        if chosen > 0 {
            guard.selection.titles = vec![chosen];
        }
    }
    let directory = guard.selection.output_directory.clone();
    let (valid, problem) = check_output_directory(&directory);
    Ok(windows::queue::selection_view(
        &guard.selection,
        &titles,
        &extension,
        valid,
        &problem,
    ))
}

/// Whether a folder can be written to, and why not when it cannot.
fn check_output_directory(directory: &str) -> (bool, String) {
    let directory = directory.trim();
    if directory.is_empty() {
        return (
            false,
            "Choose a folder to write the finished files into.".to_string(),
        );
    }
    let path = PathBuf::from(directory);
    if !path.is_dir() {
        return (false, format!("{directory} is not a folder that exists."));
    }
    (true, String::new())
}

/// Record what the selection sheet is holding.
#[tauri::command]
fn set_queue_selection(
    state: State<'_, SharedState>,
    change: QueueSelectionInput,
) -> Result<windows::queue::QueueSelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let titles = selection_titles(&guard);
    let extension = output_extension(&guard);
    let directory = change.output_directory.trim().to_string();
    let (valid, problem) = check_output_directory(&directory);
    guard.selection.output_directory = directory;
    if !change.name_pattern.trim().is_empty() {
        guard.selection.name_pattern = change.name_pattern.trim().to_string();
    }
    if change.titles.is_empty() {
        guard.selection.titles.clear();
    } else {
        let known: Vec<u32> = titles.iter().map(|(index, _, _)| *index).collect();
        let wanted: Vec<u32> = change
            .titles
            .iter()
            .copied()
            .filter(|index| known.contains(index))
            .collect();
        if wanted.is_empty() {
            return Err("None of those titles is in the scanned source.".to_string());
        }
        guard.selection.titles = wanted;
    }
    for (index, name) in &change.names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            guard.selection.names.remove(index);
        } else {
            guard
                .selection
                .names
                .insert(index.clone(), trimmed.to_string());
        }
    }
    Ok(windows::queue::selection_view(
        &guard.selection,
        &titles,
        &extension,
        valid,
        &problem,
    ))
}

/// Queue every title the selection sheet has chosen.
///
/// The whole batch is planned before anything is queued, so two titles that
/// would write the same file are refused together rather than the second one
/// quietly overwriting the first.
#[tauri::command]
fn queue_titles(state: State<'_, SharedState>) -> Result<Vec<JobId>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let selection = guard.flow.view_model();
    if !selection.is_usable() {
        return Err("Scan a source and choose a title before queueing.".to_string());
    }
    let source = guard
        .flow
        .source()
        .map(|source| source.path.clone())
        .ok_or_else(|| "Choose a file or folder to scan.".to_string())?;
    let titles = selection_titles(&guard);
    let names: Vec<(u32, String)> = titles
        .iter()
        .map(|(index, name, _)| (*index, name.clone()))
        .collect();
    let extension = output_extension(&guard);
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let tabs = guard.tabs.view(&facts);
    if !tabs.can_queue {
        return Err(format!(
            "Fix these before queueing: {}",
            tabs.problems.join(" ")
        ));
    }
    let planned = windows::queue::plan(&guard.selection, &names, &extension)?;
    let mut queued = Vec::new();
    for (index, output) in planned {
        let base = guard.tabs.to_job_spec(&source, &output, &facts)?;
        let spec = windows::queue::titled_spec(&base, index, &output);
        let id = guard.session.add_job(spec);
        guard.queue.push(id);
        queued.push(id);
    }
    guard.activity.record(format!(
        "queued {} title(s) from the selection sheet",
        queued.len()
    ));
    Ok(queued)
}

/// The queue, as the page renders it.
#[tauri::command]
fn queue_window(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    Ok(queue_view(&guard))
}

/// Move a job through the queue's order.
#[tauri::command]
fn move_job(
    state: State<'_, SharedState>,
    id: JobId,
    delta: i32,
) -> Result<windows::queue::QueueViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    guard.queue.move_by(id, delta)?;
    guard.activity.record(format!("moved job {id}"));
    Ok(windows::queue::queue_view(
        &guard.queue,
        guard.session.jobs(),
    ))
}

/// Set what a job is worth in the run order.
#[tauri::command]
fn set_job_priority(
    state: State<'_, SharedState>,
    id: JobId,
    priority: String,
) -> Result<windows::queue::QueueViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    let set = guard.queue.set_priority_name(id, &priority)?;
    guard.activity.record(format!(
        "job {id} is now {} priority",
        set.label().to_ascii_lowercase()
    ));
    Ok(windows::queue::queue_view(
        &guard.queue,
        guard.session.jobs(),
    ))
}

#[tauri::command]
fn list_jobs(state: State<'_, SharedState>) -> Result<Vec<Job>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.session.jobs().to_vec())
}

#[tauri::command]
fn remove_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    require_confirmation(&state, AlertAction::RemoveJob)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.session.remove_job(id).map_err(|e| e.to_string())?;
    guard.queue.forget(id);
    guard.activity.record(format!("removed job {id}"));
    drop(guard);
    take_confirmation(&state, AlertAction::RemoveJob);
    Ok(())
}

fn set_job_state(
    state: &SharedState,
    id: JobId,
    job_state: JobState,
    progress: Progress,
    message: Option<String>,
) {
    if let Ok(mut guard) = state.lock() {
        let _ = guard.session.update_status(
            id,
            vise_core::JobStatus {
                state: job_state,
                progress,
                message,
            },
        );
    }
}

#[tauri::command]
fn run_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let spec = claim_job(&state, id)?;
    let shared: SharedState = state.inner().clone();
    let worker = worker_client(&state)?;
    thread::spawn(move || match worker {
        Some(client) => run_through_worker(shared, client, id, spec),
        None => run_in_process(shared, id, spec),
    });
    Ok(())
}

/// Mark a queued or paused job as running and hand back its specification.
///
/// The state change happens under the lock and the work happens after it, so two
/// clicks cannot both pick up the same job.
fn claim_job(state: &SharedState, id: JobId) -> Result<JobSpec, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let jobs = guard.session.jobs().to_vec();
    let Some(job) = jobs.iter().find(|job| job.id == id) else {
        return Err(format!("job {id} not found"));
    };
    let spec = job.spec.clone();
    let job_state = job.status.state;
    if !windows::queue::is_runnable(job_state) {
        return Err(format!(
            "job {id} is not runnable from state {:?}",
            job_state
        ));
    }
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Running,
                progress: Progress::new(0.0),
                message: Some("running".into()),
            },
        )
        .map_err(|e| e.to_string())?;
    let fresh = guard.session.jobs().to_vec();
    guard.queue.sync(&fresh);
    guard.activity.record(format!(
        "started job {id} from {}",
        windows::queue::state_label(job_state).to_ascii_lowercase()
    ));
    Ok(spec)
}

/// Run every waiting job, in the queue's run order.
///
/// The encoder takes one job at a time, so the queue runs them one after
/// another rather than starting a thread per job; a job that fails does not
/// stop the ones behind it.
#[tauri::command]
fn start_queue(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::StartQueue)?;
    let waiting = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        guard
            .queue
            .pending(guard.session.jobs())
            .into_iter()
            .collect::<Vec<_>>()
    };
    if waiting.is_empty() {
        return Err("Nothing in the queue is waiting for the encoder.".to_string());
    }
    let shared: SharedState = state.inner().clone();
    take_confirmation(&shared, AlertAction::StartQueue);
    {
        let mut guard = shared.lock().map_err(|e| e.to_string())?;
        guard
            .activity
            .record(format!("starting the queue with {} job(s)", waiting.len()));
    }
    thread::spawn(move || {
        for id in waiting {
            let spec = match claim_job(&shared, id) {
                Ok(spec) => spec,
                Err(reason) => {
                    record_problem(&shared, "queue", format!("starting job {id}"), &reason);
                    continue;
                }
            };
            // A client per job, because a worker that dies mid-queue is
            // replaced by the supervisor on the next connection.
            let client = worker_client(&shared).ok().flatten();
            match client {
                Some(client) => run_through_worker(shared.clone(), client, id, spec),
                None => run_in_process(shared.clone(), id, spec),
            }
        }
    });
    queue_window(state)
}

/// Forget every job that has finished.
#[tauri::command]
fn clear_finished_jobs(
    state: State<'_, SharedState>,
) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::ClearFinished)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    let jobs = guard.session.jobs().to_vec();
    let done = guard.queue.clear_finished(&jobs);
    for id in &done {
        let _ = guard.session.remove_job(*id);
    }
    guard
        .activity
        .record(format!("cleared {} finished job(s)", done.len()));
    let view = queue_view(&guard);
    drop(guard);
    take_confirmation(&state, AlertAction::ClearFinished);
    Ok(view)
}

/// Empty the queue of everything that is not being encoded.
#[tauri::command]
fn empty_queue(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::ClearQueue)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    let jobs = guard.session.jobs().to_vec();
    let running = guard.queue.active(&jobs);
    if !running.is_empty() {
        return Err(format!(
            "{} job(s) are being encoded; cancel them first.",
            running.len()
        ));
    }
    let total = guard.session.jobs().len();
    let ids: Vec<JobId> = guard
        .queue
        .run_order(&jobs)
        .iter()
        .map(|job| job.id)
        .collect();
    for id in ids {
        let _ = guard.session.remove_job(id);
        guard.queue.forget(id);
    }
    guard
        .activity
        .record(format!("emptied the queue of {total} job(s)"));
    let view = queue_view(&guard);
    drop(guard);
    take_confirmation(&state, AlertAction::ClearQueue);
    Ok(view)
}

/// Ask the worker to stop whatever it is encoding.
#[tauri::command]
fn cancel_all(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    let shared: SharedState = state.inner().clone();
    require_confirmation(&shared, AlertAction::CancelQueue)?;
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err("cancelling an encode requires the worker process".to_string());
    };
    let running = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        guard.queue.active(guard.session.jobs())
    };
    for id in &running {
        let result = client.stop_encode().map_err(|e| e.to_string())?;
        if !result.was_successful {
            let reason = result.error.clone();
            record_problem(&state, "worker", format!("cancelling job {id}"), &reason);
            continue;
        }
        set_job_state(
            &state,
            *id,
            JobState::Cancelled,
            current_progress(&state, *id),
            Some("cancelled".into()),
        );
    }
    let view = queue_window(state)?;
    take_confirmation(&shared, AlertAction::CancelQueue);
    Ok(view)
}

/// Ask the worker to stop what it is encoding for one job.
///
/// Cancelling an encode always asks first, even for a single job, because the
/// work already spent is lost along with it.
#[tauri::command]
fn cancel_job_confirmed(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let shared: SharedState = state.inner().clone();
    require_confirmation(&shared, AlertAction::CancelJob)?;
    cancel_one(&shared, id)?;
    take_confirmation(&shared, AlertAction::CancelJob);
    Ok(())
}

/// Drive one job through the worker and mirror its progress into the queue.
fn run_through_worker(shared: SharedState, client: WorkerClient, id: JobId, spec: JobSpec) {
    push_log(&shared, format!("encoding job {}", id));
    let command = EncodeCommand {
        initialise_command: init_command(),
        encode_job: spec,
    };
    match client.start_encode(&command) {
        Ok(result) if result.was_successful => {}
        Ok(CommandResult { error, .. }) => {
            push_log(&shared, format!("job {id} refused: {error}"));
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(error),
            );
            return;
        }
        Err(e) => {
            // A worker that died mid-job is a job failure, not an application
            // failure; the supervisor replaces it on the next operation.
            push_log(&shared, format!("job {id} lost its worker: {e}"));
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(e.to_string()),
            );
            return;
        }
    }

    loop {
        match client.poll_encode_progress() {
            Ok(report) => {
                if let Some(working) = &report.working {
                    let mut progress = Progress::new(working.progress);
                    progress.eta_ms = working.eta_seconds.map(|seconds| (seconds * 1000.0) as u64);
                    let paused = working.paused;
                    set_job_state(
                        &shared,
                        id,
                        if paused {
                            JobState::Paused
                        } else {
                            JobState::Running
                        },
                        progress,
                        Some(working.stage.clone()),
                    );
                }
                if report.state == vise_ipc::WorkState::WorkDone {
                    let done = report.work_done.unwrap_or_default();
                    set_job_state(
                        &shared,
                        id,
                        if done.was_successful {
                            JobState::Completed
                        } else {
                            JobState::Failed
                        },
                        Progress::new(if done.was_successful { 1.0 } else { 0.0 }),
                        Some(done.message.clone()),
                    );
                    push_log(
                        &shared,
                        if done.was_successful {
                            format!("completed job {id}")
                        } else {
                            format!("job {id} failed: {}", done.message)
                        },
                    );
                    return;
                }
            }
            Err(e) => {
                set_job_state(
                    &shared,
                    id,
                    JobState::Failed,
                    Progress::new(0.0),
                    Some(e.to_string()),
                );
                push_log(&shared, format!("job {id} lost its worker: {e}"));
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// Fallback path used only when process isolation is switched off.
fn run_in_process(shared: SharedState, id: JobId, spec: JobSpec) {
    push_log(&shared, format!("encoding job {} in process", id));
    match vise_core::Transcoder::default().transcode(&spec) {
        Ok(result) => {
            set_job_state(
                &shared,
                id,
                JobState::Completed,
                Progress::new(1.0),
                Some(result.output_path.display().to_string()),
            );
            push_log(&shared, format!("completed job {id}"));
        }
        Err(e) => {
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(e.to_string()),
            );
            push_log(&shared, format!("job {id} failed: {e}"));
        }
    }
}

#[tauri::command]
fn pause_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let result = client.pause_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                &state,
                id,
                JobState::Paused,
                current_progress(&state, id),
                Some("paused".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = current_progress(&state, id);
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Paused,
                progress,
                message: Some(
                    "pause requested; running encoder process control unavailable".into(),
                ),
            },
        )
        .map_err(|e| e.to_string())?;
    guard.activity.record(format!(
        "pause requested for job {id} (process control unavailable)"
    ));
    Ok(())
}

#[tauri::command]
fn resume_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let result = client.resume_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                &state,
                id,
                JobState::Running,
                current_progress(&state, id),
                Some("resumed".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }
    Err("resume requires the worker process".to_string())
}

#[tauri::command]
fn cancel_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let shared: SharedState = state.inner().clone();
    cancel_one(&shared, id)
}

/// Stop what the worker is encoding for one job.
fn cancel_one(state: &SharedState, id: JobId) -> Result<(), String> {
    let worker = worker_client(state)?;
    if let Some(client) = worker {
        let result = client.stop_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                state,
                id,
                JobState::Cancelled,
                current_progress(state, id),
                Some("cancelled".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = current_progress(state, id);
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Cancelled,
                progress,
                message: Some(
                    "cancel requested; running encoder process control unavailable".into(),
                ),
            },
        )
        .map_err(|e| e.to_string())?;
    guard.activity.record(format!(
        "cancel requested for job {id} (process control unavailable)"
    ));
    Ok(())
}

fn current_progress(state: &SharedState, id: JobId) -> Progress {
    state
        .lock()
        .ok()
        .and_then(|guard| guard.session.job(id).map(|job| job.status.progress.clone()))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Activity window
// ---------------------------------------------------------------------------

/// The activity window, as the page renders it.
///
/// The worker is read by index rather than in full: only the lines this window
/// has not seen are fetched, so following the tail of a long encode costs the
/// same whether it has written ten lines or ten thousand.
#[tauri::command]
fn activity_window(
    state: State<'_, SharedState>,
) -> Result<windows::log::ActivityViewModel, String> {
    let cursor = state
        .lock()
        .map(|guard| guard.activity.cursor())
        .map_err(|e| e.to_string())?;
    let worker = worker_client(&state)?;
    let mut page = None;
    if let Some(client) = worker {
        match client.get_log_messages_from_index(cursor) {
            Ok(reply) => page = Some(reply),
            Err(e) => record_warning(
                &state,
                "worker",
                "reading the activity log",
                format!("the worker's log could not be read this time: {e}"),
            ),
        }
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(page) = page {
        guard.activity.take_page(page);
    }
    Ok(guard.activity.view())
}

/// Set the level the activity window shows from.
#[tauri::command]
fn set_log_level(
    state: State<'_, SharedState>,
    level: String,
) -> Result<windows::log::ActivityViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.activity.set_min_level_name(&level)?;
    Ok(guard.activity.view())
}

/// Turn the follow switch on or off.
#[tauri::command]
fn set_follow_logs(
    state: State<'_, SharedState>,
    follow: bool,
) -> Result<windows::log::ActivityViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.activity.set_follow(follow);
    Ok(guard.activity.view())
}

/// Forget every line, here and in the worker.
#[tauri::command]
fn clear_logs(state: State<'_, SharedState>) -> Result<windows::log::ActivityViewModel, String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        match client.reset_logging() {
            Ok(result) if !result.was_successful => record_warning(
                &state,
                "worker",
                "clearing the activity log",
                result.error.clone(),
            ),
            Ok(_) => {}
            Err(e) => record_warning(
                &state,
                "worker",
                "clearing the activity log",
                format!("the worker did not confirm the reset: {e}"),
            ),
        }
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let cleared = guard.activity.clear();
    guard.activity.record(format!(
        "cleared {cleared} line(s) from the activity window"
    ));
    Ok(guard.activity.view())
}

// ---------------------------------------------------------------------------
// Problems, alerts, and the about view
// ---------------------------------------------------------------------------

/// Everything that went wrong, as the page renders it.
#[tauri::command]
fn error_view(state: State<'_, SharedState>) -> Result<ErrorViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.errors.view())
}

/// The whole problem list as plain text, for the clipboard.
#[tauri::command]
fn error_report(state: State<'_, SharedState>) -> Result<String, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(dialogs::report_text(guard.errors.entries()))
}

/// Forget every recorded problem.
#[tauri::command]
fn clear_errors(state: State<'_, SharedState>) -> Result<ErrorViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.errors.clear();
    Ok(guard.errors.view())
}

/// Ask for confirmation before an action that cannot be undone.
///
/// A destructive action also opens the alert as its own surface, so the
/// question is where the user is looking rather than tucked behind the control
/// they just pressed. The headline arrives from the page when it has something
/// specific to name; otherwise the window writes one, so the question never
/// appears as a bare verb.
#[tauri::command]
fn ask_for_alert(
    state: State<'_, SharedState>,
    action: AlertAction,
    headline: String,
    job_id: Option<u64>,
    preset_id: Option<String>,
) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let destructive = action.is_destructive();
    let headline = alert_headline(action, headline.trim(), job_id, preset_id.as_deref());
    guard.alerts.request(action, headline, job_id, preset_id);
    if destructive {
        guard.surfaces.open(dialogs::ALERT_SURFACE)?;
    }
    Ok(guard.alerts.view())
}

/// The question a confirmation asks.
///
/// An empty headline from the page becomes a sentence that still says what is
/// about to happen, so the alert is never "Are you sure? Confirm."
fn alert_headline(
    action: AlertAction,
    headline: &str,
    job_id: Option<u64>,
    preset_id: Option<&str>,
) -> String {
    if !headline.is_empty() {
        return headline.to_string();
    }
    match action {
        AlertAction::CancelJob | AlertAction::RemoveJob => match job_id {
            Some(id) => format!("{} job {id}?", action.verb()),
            None => format!("{}?", action.verb()),
        },
        AlertAction::DeletePreset => match preset_id {
            Some(id) if !id.trim().is_empty() => format!("{} '{id}'?", action.verb()),
            _ => format!("{}?", action.verb()),
        },
        _ => format!("{}?", action.verb()),
    }
}

/// Count one second off a pending alert.
#[tauri::command]
fn tick_alert(state: State<'_, SharedState>) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.alerts.tick();
    Ok(guard.alerts.view())
}

/// Confirm a pending action, once its countdown is over.
///
/// The action itself is carried back rather than performed here: only the
/// commands that own an action can carry it out, so a stale or replayed
/// confirmation cannot cancel something the caller never asked to cancel.
#[tauri::command]
fn confirm_alert(state: State<'_, SharedState>) -> Result<Option<dialogs::CountdownAlert>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let confirmed = guard.alerts.take_confirmed();
    if let Some(alert) = &confirmed {
        guard
            .activity
            .record(format!("confirmed: {}", alert.headline));
        guard.confirmed = Some(alert.action);
    }
    guard.surfaces.close(dialogs::ALERT_SURFACE);
    Ok(confirmed)
}

/// Back out of a pending action.
#[tauri::command]
fn dismiss_alert(
    state: State<'_, SharedState>,
) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let dropped = guard
        .alerts
        .pending()
        .map(|pending| pending.headline.clone());
    guard.confirmed = None;
    if let Some(headline) = dropped {
        guard.activity.record(format!("backed out of: {headline}"));
    }
    guard.alerts.cancel();
    guard.surfaces.close(dialogs::ALERT_SURFACE);
    Ok(guard.alerts.view())
}

/// The about view.
#[tauri::command]
fn about_view(state: State<'_, SharedState>) -> Result<AboutViewModel, String> {
    let engine = state
        .lock()
        .ok()
        .and_then(|guard| guard.backend.as_ref().map(|backend| backend.describe()))
        .unwrap_or_default();
    Ok(dialogs::about(&engine))
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// The stored presets, or none when the file cannot be read.
///
/// A library that will not read is treated as empty rather than as a failure:
/// the window must still open so the problem can be read out of it.
fn stored_presets(path: &Path) -> Vec<vise_core::UserPreset> {
    vise_core::load_user_presets(path).unwrap_or_default()
}

/// The preset manager, as the page renders it.
#[tauri::command]
fn preset_window(
    state: State<'_, SharedState>,
    change: Option<PresetInput>,
) -> Result<windows::presets::PresetView, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let stored = stored_presets(&guard.presets.path);
    let built_in = vise_core::builtin_presets();
    let input = change.unwrap_or_default();
    let names: Vec<String> = stored.iter().map(|preset| preset.name.clone()).collect();
    let found = stored.iter().find(|preset| preset.id == input.id);
    Ok(guard.presets.view(
        &stored,
        &built_in,
        windows::presets::form_view(&input, &names, found),
    ))
}

/// Act on a preset: create, edit, copy, delete, or apply.
///
/// Everything is written through `vise_core`'s preset API, so the file on disk
/// is exactly what the core expects to read back on the next launch.
#[tauri::command]
fn apply_preset_action(
    state: State<'_, SharedState>,
    change: PresetInput,
) -> Result<windows::presets::PresetView, String> {
    let action = change.action()?;
    let path = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.presets.path.clone()
    };
    {
        let names: Vec<String> = stored_presets(&path)
            .into_iter()
            .map(|preset| preset.name)
            .collect();
        let blocking: Vec<String> = windows::presets::form_view(&change, &names, None)
            .problems
            .into_iter()
            .filter(|problem| !problem.starts_with("Another preset"))
            .collect();
        for problem in &blocking {
            record_problem(&state, "presets", action.as_str(), problem);
        }
        if !blocking.is_empty() {
            return Err(format!(
                "Fix this before storing the preset: {}",
                blocking.join(" ")
            ));
        }
    }

    // An action that destroys work says so in the log before it happens, so a
    // bug report that starts after the fact still shows what was asked for.
    if action.is_destructive() {
        push_log(
            &state,
            format!(
                "'{}' cannot be undone and asks for confirmation first",
                action.as_str()
            ),
        );
    }

    match action {
        PresetAction::Create | PresetAction::Clone => {
            let spec = preset_from_tabs(&state, &change)?;
            let created = vise_core::create_user_preset(&path, change.name.trim(), spec)
                .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&created.id, category);
            if change.default {
                guard.presets.set_default(&created.id);
            }
            guard
                .activity
                .record(format!("stored the preset '{}'", created.name));
        }
        PresetAction::Edit => {
            let name = change.name.trim().to_string();
            let stored = vise_core::update_user_preset(
                &path,
                change.id.trim(),
                Some(&name),
                Some(preset_from_tabs(&state, &change)?),
            )
            .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&stored.id, category);
            guard
                .activity
                .record(format!("renamed the preset '{}'", stored.name));
        }
        PresetAction::Delete => {
            require_confirmation(&state, AlertAction::DeletePreset)?;
            vise_core::delete_user_preset(&path, change.id.trim()).map_err(|e| e.to_string())?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.forget(change.id.trim());
            guard.activity.record("deleted a preset");
            drop(guard);
            take_confirmation(&state, AlertAction::DeletePreset);
        }
        PresetAction::SetDefault => {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_default(change.id.trim());
            let name = guard
                .presets
                .default_name(&stored_presets(&path), &vise_core::builtin_presets());
            guard
                .activity
                .record(format!("'{name}' now starts every new job"));
        }
        PresetAction::ClearDefault => {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.clear_default();
            guard
                .activity
                .record("no preset starts every new job any more");
        }
        PresetAction::Apply => {
            let stored = vise_core::load_user_presets(&path).map_err(|e| e.to_string())?;
            let preset = match stored.into_iter().find(|preset| preset.id == change.id) {
                Some(found) => found.preset,
                None => vise_core::builtin_preset(change.id.trim())
                    .map(|preset| preset.preset)
                    .ok_or_else(|| "There is no preset with that name.".to_string())?,
            };
            apply_preset_to_tabs(&state, &preset)?;
            push_log(&state, "applied a preset to the output settings");
        }
        PresetAction::Import => {
            if let Some(problem) = windows::presets::import_path_problem(&change.path) {
                return Err(problem);
            }
            let imported = vise_core::import_user_preset(std::path::Path::new(change.path.trim()))
                .map_err(|e| format!("That file could not be read as a preset: {e}"))?;
            let name = if imported.name.trim().is_empty() {
                return Err(
                    "A preset file needs a name; rename it in the file and try again.".to_string(),
                );
            } else {
                imported.name.trim().to_string()
            };
            let created = vise_core::create_user_preset(&path, &name, imported.preset)
                .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&created.id, category);
            if change.default {
                guard.presets.set_default(&created.id);
            }
            guard
                .activity
                .record(format!("imported the preset '{}'", created.name));
        }
        PresetAction::Export => {
            if let Some(problem) = windows::presets::export_path_problem(&change.path) {
                return Err(problem);
            }
            let stored = vise_core::load_user_presets(&path).map_err(|e| e.to_string())?;
            let wanted = change.id.trim();
            if wanted.is_empty() {
                return Err("Choose which preset to export.".to_string());
            }
            // A preset that ships with the build is exported as a stored preset
            // with the same name and description, so the file the user gets back
            // is one this window could import again.
            let preset = match stored.into_iter().find(|preset| preset.id == wanted) {
                Some(found) => found,
                None => match vise_core::builtin_preset(wanted) {
                    Some(found) => vise_core::UserPreset {
                        id: found.id.clone(),
                        name: found.name.clone(),
                        description: found.description.clone(),
                        preset: found.preset.clone(),
                    },
                    None => return Err("There is no preset with that name.".to_string()),
                },
            };
            let name = preset.name.clone();
            vise_core::export_user_preset(std::path::Path::new(change.path.trim()), &preset)
                .map_err(|e| format!("That preset could not be written: {e}"))?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard
                .activity
                .record(format!("exported the preset '{name}'"));
        }
    }

    preset_window(state, None)
}

/// Build the preset the current tab settings describe.
///
/// A preset carries the choices that shape the output rather than the whole job,
/// so it stays meaningful against a different title: container, codecs, the
/// crop and size the user picked, and the quality. Everything else stays on the
/// tabs.
fn preset_from_tabs(
    state: &SharedState,
    change: &PresetInput,
) -> Result<vise_core::PresetSpec, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let (crop, scale) =
        tabs::dimensions::to_job_geometry(&guard.tabs.dimensions, &facts).unwrap_or((None, None));
    Ok(vise_core::PresetSpec {
        container: tabs::compatibility::container_of(&guard.tabs.summary.container),
        format: None,
        video_codec: tabs::compatibility::video_codec_of(&guard.tabs.summary.video_codec),
        video: None,
        audio_codec: tabs::compatibility::audio_codec_of(&guard.tabs.summary.audio_codec),
        audio: None,
        subtitle_mode: None,
        subtitles: None,
        title_index: None,
        title: None,
        crop,
        scale_to: scale,
        quality: guard.tabs.video.quality.whole("Quality").ok().flatten(),
        name: Some(change.name.trim().to_string()),
        description: non_empty(change.description.trim()),
    })
}

/// A description, or nothing rather than an empty string.
fn non_empty(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Write a preset back into the tab settings.
fn apply_preset_to_tabs(state: &SharedState, preset: &vise_core::PresetSpec) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(container) = preset.container.or(preset.format) {
        guard.tabs.summary.container = format!("{container:?}").to_ascii_lowercase();
    }
    if let Some(codec) = preset.video_codec.or(preset.video) {
        guard.tabs.summary.video_codec = format!("{codec:?}").to_ascii_lowercase();
    }
    if let Some(codec) = preset.audio_codec.or(preset.audio) {
        guard.tabs.summary.audio_codec = format!("{codec:?}").to_ascii_lowercase();
    }
    if let Some(quality) = preset.quality {
        guard.tabs.video.quality = tabs::common::NumField::of(quality);
        guard.tabs.video.rate_control = "quality".to_string();
    }
    if let Some((_, _, width, height)) = preset.crop {
        guard.tabs.dimensions.crop_mode = "manual".to_string();
        guard.tabs.dimensions.crop_width = tabs::common::NumField::of(width);
        guard.tabs.dimensions.crop_height = tabs::common::NumField::of(height);
    }
    if let Some((width, height)) = preset.scale_to {
        guard.tabs.dimensions.scale_mode = "exact".to_string();
        guard.tabs.dimensions.scale_width = tabs::common::NumField::of(width);
        guard.tabs.dimensions.scale_height = tabs::common::NumField::of(height);
    }
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let problems = guard.tabs.view(&facts).problems;
    drop(guard);
    if !problems.is_empty() {
        return Err(format!(
            "That preset does not fit this title: {}",
            problems.join(" ")
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Secondary windows
// ---------------------------------------------------------------------------

/// Which secondary windows and dialogs exist, and which are open.
#[tauri::command]
fn window_catalog(
    state: State<'_, SharedState>,
) -> Result<windows::SurfaceCatalogViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.surfaces.catalog())
}

/// Open a secondary window or dialog.
#[tauri::command]
fn open_window(state: State<'_, SharedState>, id: String) -> Result<windows::SurfaceSpec, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.surfaces.open(&id)
}

/// Close a secondary window or dialog.
///
/// An empty name, or `all`, closes every one of them; anything else has to
/// name a surface this build offers, so a stale page cannot quietly close a
/// surface the user opened on purpose.
#[tauri::command]
fn close_window(
    state: State<'_, SharedState>,
    id: String,
) -> Result<windows::SurfaceStateViewModel, String> {
    let wanted = id.trim();
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if wanted.is_empty() || wanted == "all" {
        guard.surfaces.close_all();
        return Ok(guard.surfaces.view());
    }
    if !SurfaceState::is_surface(wanted) {
        return Err(format!("'{wanted}' is not a window this build offers."));
    }
    guard.surfaces.close(wanted);
    Ok(guard.surfaces.view())
}

fn main() {
    let settings_path = sources::default_settings_path();
    let recents = RecentSources::load(&settings_path);
    let preset_path = preset_file(&settings_path);
    let state: SharedState = Arc::new(Mutex::new(DesktopState {
        // Build the backend before the window opens so the first scan does not
        // pay for process start-up.
        backend: Some(build_backend()),
        recents,
        settings_path,
        presets: PresetState::new(preset_path),
        tabs: TabSettings::new(),
        ..DesktopState::default()
    }));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            select_source,
            inspect_sources,
            clear_source,
            recent_sources,
            forget_recent_source,
            clear_recent_sources,
            source_selection,
            begin_scan,
            scan_status,
            choose_title,
            choose_range,
            choose_tracks,
            tab_set,
            set_active_tab,
            set_summary,
            set_dimensions,
            set_filter,
            set_video,
            set_audio_defaults,
            set_audio_track,
            set_subtitles,
            set_subtitle_track,
            set_chapters,
            change_marker,
            set_metadata,
            import_subtitles,
            preview_window,
            fetch_preview_still,
            set_preview_still_count,
            select_preview_still,
            set_preview_crop,
            reset_preview_crop,
            apply_preview_crop,
            add_job,
            queue_selection,
            queue_selection_window,
            set_queue_selection,
            queue_titles,
            queue_window,
            move_job,
            set_job_priority,
            list_jobs,
            remove_job,
            run_job,
            start_queue,
            cancel_all,
            clear_finished_jobs,
            empty_queue,
            pause_job,
            resume_job,
            cancel_job,
            cancel_job_confirmed,
            activity_window,
            set_log_level,
            set_follow_logs,
            clear_logs,
            error_view,
            error_report,
            clear_errors,
            ask_for_alert,
            tick_alert,
            confirm_alert,
            dismiss_alert,
            about_view,
            preset_window,
            apply_preset_action,
            window_catalog,
            open_window,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Where the stored presets live, beside the other settings.
fn preset_file(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .map(|folder| folder.join("presets.json"))
        .unwrap_or_else(|| PathBuf::from("presets.json"))
}

#[cfg(test)]
mod tests {
    //! The contract between the page and these commands.
    //!
    //! The page sends plain JSON and reads plain JSON. These tests pin both
    //! directions so a rename on either side fails here rather than in the
    //! window.

    use super::*;
    use tabs::{SummarySettings, VideoSettings};

    fn idle_flow() -> ScanFlow {
        ScanFlow::default()
    }

    #[test]
    fn duration_options_are_validated_before_they_reach_the_worker() {
        assert_eq!(
            parse_duration(None, "shortest").expect("none"),
            TimeSpan::ZERO
        );
        assert_eq!(
            parse_duration(Some("  "), "shortest").expect("blank"),
            TimeSpan::ZERO
        );
        assert_eq!(
            parse_duration(Some("00:00:30"), "longest")
                .expect("valid")
                .to_string(),
            "00:00:30"
        );
        let error = parse_duration(Some("99:99:99"), "shortest").expect_err("invalid");
        assert!(error.contains("shortest title duration"), "{error}");
        assert!(error.contains("99:99:99"), "{error}");
    }

    #[test]
    fn scan_options_build_the_worker_command() {
        let options: ScanOptions = serde_json::from_str(
            r#"{"previewCount":99,"minDuration":"","maxDuration":"00:10:00",
                "keepDuplicateTitles":true,"fileExclusions":[" sample.mkv ","  "]}"#,
        )
        .expect("options");
        let command = options
            .to_command(vec!["C:\\media\\clips".to_string()])
            .expect("command");
        assert_eq!(command.paths, vec!["C:\\media\\clips".to_string()]);
        assert_eq!(command.preview_count, 32, "clamped to something sane");
        assert_eq!(
            command.min_duration,
            TimeSpan::ZERO,
            "an empty value means no limit"
        );
        assert_eq!(command.max_duration.to_string(), "00:10:00");
        assert!(command.initialise_command.keep_duplicate_titles);
        assert_eq!(command.file_exclusion_list, vec!["sample.mkv".to_string()]);
    }

    #[test]
    fn scan_options_default_to_no_previews_and_no_limits() {
        let options: ScanOptions = serde_json::from_str("{}").expect("empty options");
        let command = options
            .to_command(vec!["in.mkv".to_string()])
            .expect("command");
        assert_eq!(command.preview_count, 0);
        assert!(command.file_exclusion_list.is_empty());
        assert_eq!(command.title_index, 0, "zero asks for every title");
    }

    #[test]
    fn the_page_sends_scan_options_the_command_accepts() {
        let payload = r#"{"previewCount":0,"minDuration":"","maxDuration":"",
            "keepDuplicateTitles":false,"fileExclusions":[]}"#;
        let options: ScanOptions = serde_json::from_str(payload).expect("camel case options");
        assert_eq!(options.preview_count, Some(0));
        assert!(!options.keep_duplicate_titles);
        assert!(options.file_exclusions.is_empty());
    }

    #[test]
    fn a_range_choice_arrives_as_point_to_point_with_its_points() {
        let payload = r#"{"mode":"point_to_point","startSeconds":90.0,
            "endSeconds":120.0,"angle":2}"#;
        let choice: RangeChoice = serde_json::from_str(payload).expect("choice");
        assert_eq!(choice.mode, "point_to_point");
        assert_eq!(choice.start_seconds, Some(90.0));
        assert_eq!(choice.end_seconds, Some(120.0));
        assert_eq!(choice.angle, Some(2));

        let selection = RangeSelection::default()
            .with_mode_name(&choice.mode)
            .expect("mode");
        assert_eq!(selection.mode, viewmodel::RangeMode::PointToPoint);

        // An empty mode keeps the mode already in force, which is how the point
        // pickers work without the page tracking the mode.
        let blank: RangeChoice = serde_json::from_str(r#"{"mode":""}"#).expect("blank");
        assert!(blank.mode.trim().is_empty());
    }

    #[test]
    fn an_unknown_range_mode_is_refused_with_a_reason() {
        let error = RangeSelection::default()
            .with_mode_name("sideways")
            .expect_err("unknown mode");
        assert!(error.contains("sideways"), "{error}");
    }

    #[test]
    fn a_track_choice_arrives_as_track_numbers() {
        let payload = r#"{"audio":[2,1],"subtitle":[1]}"#;
        let input: TrackSelectionInput = serde_json::from_str(payload).expect("tracks");
        assert_eq!(input.audio, vec![2, 1]);
        assert_eq!(input.subtitle, vec![1]);
        // Numbers outside the title are dropped before they are stored.
        let reconciled = input.reconcile(&vise_ipc::contract::ScanTitle {
            audio_list: vec![vise_ipc::contract::ScanAudioTrack {
                track_number: 1,
                ..Default::default()
            }],
            subtitle_list: vec![vise_ipc::contract::ScanSubtitleTrack {
                track_number: 1,
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(reconciled.audio, vec![1]);
        assert_eq!(reconciled.subtitle, vec![1]);
    }

    #[test]
    fn scan_status_serialises_for_the_page() {
        let status = ScanStatusViewModel::of(&idle_flow());
        let json = serde_json::to_value(&status).expect("status");
        assert_eq!(json["phase"], "idle");
        assert_eq!(json["phaseName"], "idle");
        assert_eq!(json["running"], false);
        assert_eq!(json["titleCount"], 0);
        assert!(json["progress"]["percent"].is_number());
    }

    #[test]
    fn a_dropped_path_list_is_answered_with_accepted_and_refused_entries() {
        let dir = std::env::temp_dir().join("vise-desktop-drop");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("movie.mkv"), b"x").expect("file");

        let report = inspect_sources(vec![
            String::new(),
            dir.join("movie.mkv").to_string_lossy().to_string(),
            dir.join("gone.mkv").to_string_lossy().to_string(),
        ])
        .expect("report");
        assert_eq!(report.accepted.len(), 1);
        assert_eq!(report.rejected.len(), 2);
        let json = serde_json::to_value(&report).expect("serialise");
        assert_eq!(json["accepted"][0]["kind"], "file");
        assert!(!json["rejected"][0]["reason"].as_str().unwrap().is_empty());
    }

    #[test]
    fn the_tab_set_arrives_as_one_object_the_page_can_render() {
        let settings = TabSettings::new();
        let view = settings.view(&TitleFacts::default());
        let json = serde_json::to_value(&view).expect("serialise");
        assert_eq!(json["tabs"].as_array().map(|tabs| tabs.len()), Some(8));
        assert_eq!(json["tabs"][0]["id"], "summary");
        assert_eq!(json["active"], "summary");
        assert_eq!(json["source"]["chosen"], false);
        assert!(json["summary"]["container"]["options"].is_array());
        assert!(json["dimensions"]["cropMode"]["selected"].is_string());
        assert!(json["filters"]["rows"].is_array());
        assert_eq!(json["video"]["rateControl"]["selected"], "quality");
        assert!(json["audio"]["tracks"].is_array());
        assert!(json["subtitles"]["behaviour"]["options"].is_array());
        assert!(json["chapters"]["mode"]["options"].is_array());
        assert!(json["metadata"]["mode"]["options"].is_array());
        assert!(json["metadata"]["tags"].is_array());
        assert_eq!(json["metadata"]["mode"]["selected"], "copy");
        assert!(json["problems"].is_array());
    }

    #[test]
    fn a_tab_change_arrives_as_camel_case_the_command_accepts() {
        let payload = r#"{"container":"mkv","videoCodec":"vp9","audioCodec":"opus",
            "webOptimized":false,"ipodCompatible":false,"alignAudioVideo":true,
            "passthroughMetadata":false}"#;
        let change: SummaryInput = serde_json::from_str(payload).expect("camel case change");
        let mut settings = SummarySettings::default();
        change.apply(&mut settings).expect("apply");
        assert_eq!(settings.container, "mkv");
        assert_eq!(settings.video_codec, "vp9");
        assert!(!settings.web_optimized);
        assert!(settings.align_audio_video);

        let video = r#"{"encoder":"av1","rateControl":"bitrate","bitrate":"4000",
            "twoPass":true,"preset":"slow","colourRange":"full"}"#;
        let change: VideoInput = serde_json::from_str(video).expect("video change");
        let mut video_settings = VideoSettings::default();
        change.apply(&mut video_settings).expect("video");
        assert_eq!(video_settings.encoder, "av1");
        assert_eq!(video_settings.bitrate.text, "4000");
        assert!(video_settings.two_pass);
        assert_eq!(video_settings.colour_range, "full");

        let filter = r#"{"filter":"denoise","enabled":true,"choice":"custom","value":"7"}"#;
        let change: tabs::FilterInput = serde_json::from_str(filter).expect("filter change");
        let mut filters = tabs::FilterSettings::default();
        change.apply(&mut filters).expect("filter");
        assert!(filters.denoise);
        assert_eq!(filters.denoise_value(), Some(7));

        let marker = r#"{"action":"add","name":"Extra","start":"10","end":"20"}"#;
        let change: MarkerInput = serde_json::from_str(marker).expect("marker change");
        let mut chapters = tabs::ChapterSettings::default();
        change.apply(&mut chapters).expect("marker");
        assert_eq!(chapters.markers.len(), 1);
        assert_eq!(chapters.markers[0].name, "Extra");
    }

    #[test]
    fn only_subtitle_files_are_accepted_for_import() {
        assert!(subtitle_extension("extra.srt"));
        assert!(subtitle_extension("captions.ASS"));
        assert!(subtitle_extension("subs.webvtt"));
        assert!(!subtitle_extension("movie.mkv"));
        assert!(!subtitle_extension("subtitles"));
    }

    #[test]
    fn a_tab_the_build_does_not_offer_is_refused_by_name() {
        let settings = TabSettings::new();
        assert!(settings.is_tab("chapters"));
        assert!(settings.is_tab("metadata"));
        assert!(!settings.is_tab("advanced"));
    }

    #[test]
    fn a_queue_selection_arrives_as_the_names_and_choices_it_needs() {
        let payload = r#"{"outputDirectory":"C:\\out","namePattern":"{title} ({n})",
            "names":{"2":"Second"},"titles":[1,2]}"#;
        let input: windows::queue::QueueSelectionInput =
            serde_json::from_str(payload).expect("selection");
        assert_eq!(input.output_directory, "C:\\out");
        assert_eq!(input.name_pattern, "{title} ({n})");
        assert_eq!(input.titles, vec![1, 2]);
        assert_eq!(input.names.get("2").map(String::as_str), Some("Second"));

        let titles = vec![(1u32, "One".to_string()), (2, "One".to_string())];
        let plan = windows::queue::plan(&input, &titles, "mp4").expect("plan");
        assert_eq!(
            plan,
            vec![
                (1, "C:\\out\\One (1).mp4".to_string()),
                (2, "C:\\out\\Second.mp4".to_string())
            ]
        );
    }

    #[test]
    fn a_metadata_change_arrives_as_camel_case_the_tab_accepts() {
        let payload = r#"{"action":"add","key":"comment","value":"mine","checked":false}"#;
        let change: MetadataInput = serde_json::from_str(payload).expect("metadata change");
        let mut settings = tabs::MetadataSettings::default();
        change
            .apply(&mut settings, &TitleFacts::default())
            .expect("add");
        assert!(settings.entries.iter().any(|entry| entry.key == "comment"));

        let mode = r#"{"action":"mode","mode":"edit"}"#;
        let change: MetadataInput = serde_json::from_str(mode).expect("mode change");
        change
            .apply(&mut settings, &TitleFacts::default())
            .expect("mode");
        assert!(settings.applies_edits());
    }

    #[test]
    fn a_preset_change_arrives_as_the_action_and_the_preset_it_names() {
        let payload = r#"{"action":"clone","id":"fast-1080p","name":"Mine",
            "description":"for the shelf","category":"web","default":true}"#;
        let input: PresetInput = serde_json::from_str(payload).expect("preset change");
        assert_eq!(input.action().expect("clone"), PresetAction::Clone);
        assert_eq!(input.id, "fast-1080p");
        assert_eq!(input.category, "web");
        assert!(input.default);
        assert!(PresetAction::Delete.is_destructive());
        assert!(PresetAction::Edit.needs_name());
        assert!(!PresetAction::SetDefault.needs_name());

        let view = windows::presets::form_view(&input, &["Fast 1080p".to_string()], None);
        assert!(view.can_submit, "{:?}", view.problems);
        assert_eq!(view.selected_category, "web");
        assert!(view.default);
    }

    #[test]
    fn the_output_folder_is_only_accepted_when_it_exists() {
        let (valid, problem) = check_output_directory("   ");
        assert!(!valid);
        assert!(problem.contains("folder"));

        let (valid, problem) = check_output_directory("C:\\definitely\\not\\here");
        assert!(!valid);
        assert!(problem.contains("definitely"), "{problem}");

        let dir = std::env::temp_dir().join("vise-desktop-output");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(
            check_output_directory(dir.to_str().unwrap()),
            (true, String::new())
        );
    }

    #[test]
    fn a_window_this_build_does_not_offer_is_refused_and_a_whole_close_is_not() {
        let mut surfaces = SurfaceState::default();
        assert!(surfaces.open("queue").is_ok());
        assert!(surfaces.open("registry editor").is_err());
        assert_eq!(surfaces.open_surfaces().len(), 1);
        assert!(!surfaces.open_surfaces().is_empty());
        surfaces.close_all();
        assert!(surfaces.open_surfaces().is_empty());

        let catalog = surfaces.catalog();
        assert_eq!(catalog.surfaces.len(), 8);
        assert!(catalog
            .surfaces
            .iter()
            .all(|surface| surface.role == "dialog" && surface.modal));
    }

    #[test]
    fn the_alert_asks_a_question_even_when_the_page_names_nothing() {
        assert_eq!(
            alert_headline(AlertAction::RemoveJob, "", Some(7), None),
            "Remove from the queue job 7?"
        );
        assert_eq!(
            alert_headline(AlertAction::CancelJob, "", None, None),
            "Cancel the encode?"
        );
        assert_eq!(
            alert_headline(AlertAction::DeletePreset, "", None, Some("  ")),
            "Delete the preset?"
        );
        assert_eq!(
            alert_headline(AlertAction::DeletePreset, "", None, Some("user-1234")),
            "Delete the preset 'user-1234'?"
        );
        assert_eq!(
            alert_headline(AlertAction::ClearQueue, "", None, None),
            "Empty the queue?"
        );
        // A headline the page did write is kept, word for word.
        assert_eq!(
            alert_headline(
                AlertAction::CancelQueue,
                "Cancel 3 encodes and lose their progress?",
                None,
                None
            ),
            "Cancel 3 encodes and lose their progress?"
        );
        assert_eq!(
            alert_headline(AlertAction::StartQueue, "", None, None),
            "Start the queue?"
        );
    }

    #[test]
    fn a_preset_exchange_change_carries_the_file_it_reads_or_writes() {
        let payload = r#"{"action":"import","path":"C:\\out\\mine.json","category":"web"}"#;
        let change: PresetInput = serde_json::from_str(payload).expect("import change");
        let action = change.action().expect("import");
        assert_eq!(action, PresetAction::Import);
        assert_eq!(change.path, "C:\\out\\mine.json");
        assert!(action.needs_path(), "an import reads a file");

        let export = r#"{"action":"export","id":"fast-1080p","path":"C:\\out\\fast.json"}"#;
        let change: PresetInput = serde_json::from_str(export).expect("export change");
        let action = change.action().expect("export");
        assert_eq!(action, PresetAction::Export);
        assert_eq!(change.id, "fast-1080p");
        assert!(action.needs_path(), "an export writes a file");
        assert!(!action.needs_name(), "the exported preset is not renamed");
    }

    #[test]
    fn a_preset_file_that_is_not_there_is_refused_before_anything_is_written() {
        assert!(windows::presets::import_path_problem("C:\\nowhere\\mine.json").is_some());
        assert!(windows::presets::export_path_problem("C:\\nowhere\\mine.txt").is_some());
    }

    #[test]
    fn the_preset_file_lives_beside_the_other_settings() {
        let path = preset_file(std::path::Path::new("C:/app/settings.json"));
        assert_eq!(path, std::path::PathBuf::from("C:/app/presets.json"));
        assert_eq!(
            preset_file(std::path::Path::new("settings.json")),
            std::path::PathBuf::from("presets.json")
        );
    }
}
