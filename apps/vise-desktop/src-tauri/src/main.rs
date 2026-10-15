#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::thread;

use tauri::State;
use vise_core::{
    FfprobeScanner, Job, JobId, JobSpec, JobState, Progress, Session, Transcoder,
};

#[derive(Default)]
struct DesktopState {
    session: Session,
    logs: Vec<String>,
}

type SharedState = Arc<Mutex<DesktopState>>;

fn push_log(state: &SharedState, message: impl Into<String>) {
    if let Ok(mut guard) = state.lock() {
        guard.logs.push(message.into());
    }
}

#[tauri::command]
fn scan_source(path: String) -> Result<vise_core::ScanResult, String> {
    FfprobeScanner::default()
        .scan_path(std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

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

#[tauri::command]
fn run_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let spec = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let job = guard.session.job(id).ok_or_else(|| format!("job {} not found", id))?;
        if job.status.state != JobState::Queued && job.status.state != JobState::Paused {
            return Err(format!("job {} is not runnable from state {:?}", id, job.status.state));
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
        guard.session.job(id).map(|j| j.spec.clone()).ok_or_else(|| format!("job {} missing", id))?
    };

    let shared: SharedState = state.inner().clone();
    thread::spawn(move || {
        push_log(&shared, format!("encoding job {}", id));
        let result = Transcoder::default().transcode(&spec);
        if let Ok(mut guard) = shared.lock() {
            match result {
                Ok(res) => {
                    let _ = guard.session.update_status(
                        id,
                        vise_core::JobStatus {
                            state: JobState::Completed,
                            progress: Progress::new(1.0),
                            message: Some(res.output_path.display().to_string()),
                        },
                    );
                    guard.logs.push(format!("completed job {}", id));
                }
                Err(err) => {
                    let _ = guard.session.update_status(
                        id,
                        vise_core::JobStatus {
                            state: JobState::Failed,
                            progress: Progress::new(0.0),
                            message: Some(err.to_string()),
                        },
                    );
                    guard.logs.push(format!("job {} failed: {}", id, err));
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn pause_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = guard
        .session
        .job(id)
        .map(|j| j.status.progress.clone())
        .unwrap_or_default();
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Paused,
                progress,
                message: Some("pause requested; running encoder process control unavailable".into()),
            },
        )
        .map_err(|e| e.to_string())?;
    guard
        .logs
        .push(format!("pause requested for job {} (process control unavailable)", id));
    Ok(())
}

#[tauri::command]
fn resume_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = guard
        .session
        .job(id)
        .map(|j| j.status.progress.clone())
        .unwrap_or_default();
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Queued,
                progress,
                message: Some("resume requested; re-run to continue".into()),
            },
        )
        .map_err(|e| e.to_string())?;
    guard
        .logs
        .push(format!("resume requested for job {} (re-run required)", id));
    Ok(())
}

#[tauri::command]
fn cancel_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = guard
        .session
        .job(id)
        .map(|j| j.status.progress.clone())
        .unwrap_or_default();
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Cancelled,
                progress,
                message: Some("cancel requested; running encoder process control unavailable".into()),
            },
        )
        .map_err(|e| e.to_string())?;
    guard
        .logs
        .push(format!("cancel requested for job {} (process control unavailable)", id));
    Ok(())
}

#[tauri::command]
fn logs(state: State<'_, SharedState>) -> Result<Vec<String>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.logs.clone())
}

#[tauri::command]
fn clear_logs(state: State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.logs.clear();
    Ok(())
}

fn main() {
    let state: SharedState = Arc::new(Mutex::new(DesktopState::default()));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            scan_source,
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
