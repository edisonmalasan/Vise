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

use serde::Serialize;
use tauri::State;
use vise_core::{Job, JobId, JobSpec, JobState, Progress, Session};
use vise_ipc::contract::{
    CommandResult, EncodeCommand, InitCommand, PreviewCommand, PreviewResult, ScanCommand,
    TitlesResult,
};
use vise_ipc::portable::PortableSettings;
use vise_ipc::{WorkerClient, WorkerConfig, WorkerSupervisor};

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
// Scan
// ---------------------------------------------------------------------------

/// Scan result shape consumed by the user interface.
///
/// Field names match the core scan result the window already expects, so the
/// web layer needed no changes when scanning moved into the worker.
#[derive(Debug, Clone, Serialize)]
struct UiScanResult {
    format: UiFormat,
    streams: Vec<UiStream>,
    titles: Vec<UiTitle>,
    chapters: Vec<UiChapter>,
}

#[derive(Debug, Clone, Serialize)]
struct UiFormat {
    format_name: Option<String>,
    duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct UiStream {
    index: usize,
    kind: String,
    codec: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    channels: Option<u32>,
    language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct UiTitle {
    index: u32,
    name: String,
    duration_seconds: f64,
    width: u32,
    height: u32,
    chapters: Vec<UiChapter>,
}

#[derive(Debug, Clone, Serialize)]
struct UiChapter {
    index: u32,
    start_seconds: f64,
    end_seconds: f64,
    title: String,
}

fn chapters_to_ui(chapters: &[vise_ipc::ScanChapter]) -> Vec<UiChapter> {
    chapters
        .iter()
        .map(|chapter| UiChapter {
            index: chapter.index,
            start_seconds: chapter.start_seconds,
            end_seconds: chapter.end_seconds,
            title: chapter.title.clone(),
        })
        .collect()
}

/// Map a worker title list onto the shape the window renders.
fn ui_titles(titles: &TitlesResult) -> UiScanResult {
    UiScanResult {
        format: UiFormat {
            format_name: titles.title_list.first().map(|t| t.container.clone()),
            duration_seconds: titles.title_list.first().map(|t| t.duration_seconds),
        },
        streams: Vec::new(),
        titles: titles
            .title_list
            .iter()
            .map(|title| UiTitle {
                index: title.index,
                name: title.name.clone(),
                duration_seconds: title.duration_seconds,
                width: title.width,
                height: title.height,
                chapters: chapters_to_ui(&title.chapter_list),
            })
            .collect(),
        chapters: titles
            .title_list
            .iter()
            .flat_map(|title| chapters_to_ui(&title.chapter_list))
            .collect(),
    }
}

/// Convert a core scan result for the isolated fallback path.
fn ui_core_scan(result: &vise_core::ScanResult) -> UiScanResult {
    UiScanResult {
        format: UiFormat {
            format_name: result.format.format_name.clone(),
            duration_seconds: result.format.duration_seconds,
        },
        streams: result
            .streams
            .iter()
            .map(|stream| UiStream {
                index: stream.index,
                kind: format!("{:?}", stream.kind).to_lowercase(),
                codec: stream.codec.clone(),
                width: stream.width,
                height: stream.height,
                channels: stream.channels,
                language: stream.language.clone(),
            })
            .collect(),
        titles: result
            .titles
            .iter()
            .map(|title| UiTitle {
                index: title.index as u32,
                name: result
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "title".to_string()),
                duration_seconds: title.duration_seconds.unwrap_or(0.0),
                width: result.streams.iter().find_map(|s| s.width).unwrap_or(0),
                height: result.streams.iter().find_map(|s| s.height).unwrap_or(0),
                chapters: title
                    .chapters
                    .iter()
                    .map(|chapter| UiChapter {
                        index: chapter.index as u32,
                        start_seconds: chapter.start_seconds.unwrap_or(0.0),
                        end_seconds: chapter.end_seconds.unwrap_or(0.0),
                        title: chapter.title.clone().unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect(),
        chapters: result
            .chapters
            .iter()
            .map(|chapter| UiChapter {
                index: chapter.index as u32,
                start_seconds: chapter.start_seconds.unwrap_or(0.0),
                end_seconds: chapter.end_seconds.unwrap_or(0.0),
                title: chapter.title.clone().unwrap_or_default(),
            })
            .collect(),
    }
}

#[tauri::command]
fn scan_source(state: State<'_, SharedState>, path: String) -> Result<UiScanResult, String> {
    let worker = worker_client(&state)?;
    match worker {
        Some(client) => {
            let command = ScanCommand {
                initialise_command: init_command(),
                paths: vec![path.clone()],
                ..ScanCommand::default()
            };
            let started = client.start_scan(&command).map_err(|e| e.to_string())?;
            if !started.was_successful {
                return Err(started.error);
            }
            let report = vise_ipc::api::wait_for_scan(&client, Duration::from_secs(600))
                .map_err(|e| e.to_string())?;
            if let Some(done) = &report.work_done {
                if !done.was_successful {
                    return Err(done.message.clone());
                }
            }
            let titles = client.get_titles().map_err(|e| e.to_string())?;
            Ok(ui_titles(&titles))
        }
        None => {
            let result = vise_core::FfprobeScanner::default()
                .scan_path(std::path::Path::new(&path))
                .map_err(|e| e.to_string())?;
            Ok(ui_core_scan(&result))
        }
    }
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
    let state: SharedState = Arc::new(Mutex::new(DesktopState {
        // Build the backend before the window opens so the first scan does not
        // pay for process start-up.
        backend: Some(build_backend()),
        ..DesktopState::default()
    }));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            scan_source,
            extract_preview,
            add_job,
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
