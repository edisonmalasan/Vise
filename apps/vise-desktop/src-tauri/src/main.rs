#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Vise desktop shell.
//!
//! The window never runs the transcode pipeline itself. Scanning and encoding
//! happen in the background worker process, reached over its loopback,
//! token-authenticated JSON API, so an encoder crash costs a job rather than
//! the application. Process isolation can be switched off in the portable
//! settings file; only then does the shell fall back to calling the core
//! library in-process.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;
use vise_core::{Job, JobId, JobSpec, JobState, Progress, Session};
use vise_ipc::contract::{
    CommandResult, EncodeCommand, InitCommand, PreviewCommand, PreviewResult, ScanCommand,
    WorkState,
};
use vise_ipc::portable::PortableSettings;
use vise_ipc::timespan::TimeSpan;
use vise_ipc::{WorkerClient, WorkerConfig, WorkerSupervisor};

mod scanflow;
mod sources;
mod viewmodel;

use scanflow::{scan_in_process, ScanFlow};
use sources::{RecentSources, SourceDropReport, SourceViewModel};
use viewmodel::{
    RangeSelection, ScanPhase, ScanProgressViewModel, SelectionViewModel, TrackSelectionInput,
};

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
    logs: Vec<String>,
    backend: Option<Backend>,
    /// The source surface: source, scan, titles, and the current selection.
    flow: ScanFlow,
    /// Sources the user opened recently.
    recents: RecentSources,
    /// Where the recent source list is persisted.
    settings_path: PathBuf,
    /// Monotonic stamp that orders the recent source list.
    recents_stamp: u64,
}

type SharedState = Arc<Mutex<DesktopState>>;

fn push_log(state: &SharedState, message: impl Into<String>) {
    if let Ok(mut guard) = state.lock() {
        guard.logs.push(message.into());
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
    let command = options
        .unwrap_or_default()
        .to_command(vec![source.path.clone()])?;
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
    guard.flow.set_tracks(input)?;
    Ok(guard.flow.view_model())
}

#[tauri::command]
fn extract_preview(
    state: State<'_, SharedState>,
    path: String,
    seconds: f64,
) -> Result<PreviewResult, String> {
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err("preview extraction requires the worker process".to_string());
    };
    client
        .get_preview(&PreviewCommand {
            path,
            time_seconds: seconds,
            ..PreviewCommand::default()
        })
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Queue
// ---------------------------------------------------------------------------

#[tauri::command]
fn add_job(state: State<'_, SharedState>, spec: JobSpec) -> Result<JobId, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let id = guard.session.add_job(spec);
    guard.logs.push(format!("queued job {}", id));
    Ok(id)
}

/// Queue the current source selection.
///
/// The page names the output and the encoders; the window applies the title and
/// track choice it already validated, so a job cannot be queued against a title
/// the user did not select.
#[tauri::command]
fn queue_selection(state: State<'_, SharedState>, spec: JobSpec) -> Result<JobId, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let selection = guard.flow.view_model();
    if !selection.is_usable() {
        return Err("Scan a source and choose a title before queueing a job.".to_string());
    }
    let source = guard
        .flow
        .source()
        .map(|source| source.path.clone())
        .ok_or_else(|| "Choose a file or folder to scan.".to_string())?;
    let tracks = guard.flow.tracks();
    let title_index = guard
        .flow
        .current_title()
        .map(|title| title.index)
        .ok_or_else(|| "Choose a title before queueing a job.".to_string())?;

    let mut spec = spec;
    spec.input_path = source;
    spec.title_index = Some(selection.selected_title_index);
    spec.audio_tracks = tracks
        .audio
        .iter()
        .map(|number| vise_core::AudioTrackConfig {
            source_track_id: Some(*number),
            ..Default::default()
        })
        .collect();
    spec.subtitle_tracks = tracks
        .subtitle
        .iter()
        .map(|number| vise_core::SubtitleTrackConfig {
            source_track_id: Some(*number),
            ..Default::default()
        })
        .collect();

    let id = guard.session.add_job(spec);
    guard
        .logs
        .push(format!("queued job {id} for title {title_index}"));
    Ok(id)
}

#[tauri::command]
fn list_jobs(state: State<'_, SharedState>) -> Result<Vec<Job>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.session.jobs().to_vec())
}

#[tauri::command]
fn remove_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.session.remove_job(id).map_err(|e| e.to_string())?;
    guard.logs.push(format!("removed job {}", id));
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
    let spec = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let job = guard
            .session
            .job(id)
            .ok_or_else(|| format!("job {id} not found"))?;
        let spec = job.spec.clone();
        if job.status.state != JobState::Queued && job.status.state != JobState::Paused {
            return Err(format!(
                "job {id} is not runnable from state {:?}",
                job.status.state
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
        guard.logs.push(format!("started job {}", id));
        spec
    };

    let shared: SharedState = state.inner().clone();
    let worker = worker_client(&state)?;
    thread::spawn(move || match worker {
        Some(client) => run_through_worker(shared, client, id, spec),
        None => run_in_process(shared, id, spec),
    });
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
    guard.logs.push(format!(
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
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let result = client.stop_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                &state,
                id,
                JobState::Cancelled,
                current_progress(&state, id),
                Some("cancelled".into()),
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
                state: JobState::Cancelled,
                progress,
                message: Some(
                    "cancel requested; running encoder process control unavailable".into(),
                ),
            },
        )
        .map_err(|e| e.to_string())?;
    guard.logs.push(format!(
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
// Logs
// ---------------------------------------------------------------------------

#[tauri::command]
fn logs(state: State<'_, SharedState>) -> Result<Vec<String>, String> {
    let local = state
        .lock()
        .map(|guard| guard.logs.clone())
        .map_err(|e| e.to_string())?;
    let worker = worker_client(&state)?;
    let mut lines = Vec::new();
    if let Some(backend) = state
        .lock()
        .ok()
        .and_then(|guard| guard.backend.as_ref().map(|backend| backend.describe()))
    {
        lines.push(format!("[desktop] {backend}"));
    }
    if let Some(client) = worker {
        if let Ok(messages) = client.get_all_log_messages() {
            lines.extend(
                messages
                    .iter()
                    .map(|entry| format!("[worker {:?}] {}", entry.log_level, entry.message)),
            );
        }
    }
    lines.extend(local);
    Ok(lines)
}

#[tauri::command]
fn clear_logs(state: State<'_, SharedState>) -> Result<(), String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let _ = client.reset_logging();
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.logs.clear();
    Ok(())
}

fn main() {
    let settings_path = sources::default_settings_path();
    let recents = RecentSources::load(&settings_path);
    let state: SharedState = Arc::new(Mutex::new(DesktopState {
        // Build the backend before the window opens so the first scan does not
        // pay for process start-up.
        backend: Some(build_backend()),
        recents,
        settings_path,
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
            extract_preview,
            add_job,
            queue_selection,
            list_jobs,
            remove_job,
            run_job,
            pause_job,
            resume_job,
            cancel_job,
            logs,
            clear_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    //! The contract between the page and these commands.
    //!
    //! The page sends plain JSON and reads plain JSON. These tests pin both
    //! directions so a rename on either side fails here rather than in the
    //! window.

    use super::*;

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
}
