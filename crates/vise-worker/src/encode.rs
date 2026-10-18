//! Encode state machine backing `StartEncode`, `PauseEncode`, `ResumeEncode`,
//! `StopEncode`, and `PollEncodeProgress`.
//!
//! The encoder runs as a separate ffmpeg process so that pause, resume, and
//! cancel are real process-level operations rather than best-effort flags.
//! Output is written to a temporary sibling file and moved into place only after
//! a successful run, so a cancelled or failed encode never leaves a corrupt
//! destination file behind.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use vise_core::{JobSpec, TranscodePlan, Transcoder};
use vise_ipc::contract::error_codes;
use vise_ipc::contract::{EncodeProgress, WorkDone, WorkState};
use vise_ipc::power::{self, PowerRequest, PowerScope};

use crate::log::LogService;
use crate::platform;

/// Whether this process is currently holding the machine awake.
///
/// Held in one place rather than in the encode state, because the hold outlives
/// any single encode thread: the flag is what makes the release idempotent, so a
/// failed encode, a cancelled one, and a worker shutting down can all let go
/// without one of them releasing somebody else's hold.
static POWER_HELD: AtomicBool = AtomicBool::new(false);

/// Hold the machine awake for the duration of an encode.
///
/// The scope comes from the preferences rather than being fixed here, and a
/// platform that cannot do it says so in the log once rather than on every
/// encode: a warning nobody can act on is how a log stops being read.
fn hold_power(scope: Option<PowerScope>, logs: &LogService) {
    let Some(scope) = scope else {
        return;
    };
    match power::request(PowerRequest::Hold(scope)) {
        Ok(()) => {
            POWER_HELD.store(true, Ordering::SeqCst);
            logs.info(format!(
                "the machine is being held awake: {}",
                scope.effect()
            ));
        }
        Err(e) => logs.warn(format!("power management: {e}")),
    }
}

/// Let the machine sleep again, if this process is the one holding it.
fn release_power(logs: &LogService) {
    if !POWER_HELD.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = power::request(PowerRequest::Release) {
        logs.warn(format!("power management: {e}"));
    }
}

/// Lock a mutex, recovering from poisoning.
///
/// A panic on an encode thread must not turn every later request into a
/// failure, so the inner value is reused even when the lock was poisoned.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Live handle on the running encoder process.
#[derive(Debug)]
struct EncodeControl {
    pid: u32,
    child: Mutex<Option<Child>>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl EncodeControl {
    fn kill(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
        }
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

/// Encode state exposed to the router.
#[derive(Debug, Clone)]
pub struct EncodeSlot {
    state: WorkState,
    progress: EncodeProgress,
    done: WorkDone,
    started_at: Option<Instant>,
    sequence: u64,
}

impl Default for EncodeSlot {
    fn default() -> Self {
        EncodeSlot {
            state: WorkState::Unknown,
            progress: EncodeProgress::default(),
            done: WorkDone {
                error: error_codes::NONE,
                was_successful: true,
                message: String::new(),
            },
            started_at: None,
            sequence: 0,
        }
    }
}

/// Shared encode state and process control.
#[derive(Debug, Clone)]
pub struct EncodeService {
    inner: Arc<Mutex<EncodeSlot>>,
    control: Arc<Mutex<Option<Arc<EncodeControl>>>>,
    logs: Arc<LogService>,
    /// Source facts cached by the scan service, used for validation.
    scanner: Arc<Mutex<Option<vise_core::ScanResult>>>,
    /// What the machine should keep doing while an encode runs, when it should
    /// keep doing anything at all.
    power: Arc<Mutex<Option<PowerScope>>>,
}

impl EncodeService {
    /// Create an encode service that logs through `logs`.
    pub fn new(logs: Arc<LogService>) -> Self {
        EncodeService {
            inner: Arc::new(Mutex::new(EncodeSlot::default())),
            control: Arc::new(Mutex::new(None)),
            logs,
            scanner: Arc::new(Mutex::new(None)),
            power: Arc::new(Mutex::new(None)),
        }
    }

    /// Record what the preferences asked for while an encode runs.
    ///
    /// `None` means the machine may sleep, which is not the same as not knowing:
    /// the preference is stored either way, so turning it back on takes effect on
    /// the next encode rather than needing a restart.
    pub fn set_power(&self, scope: Option<PowerScope>) {
        *lock(&self.power) = scope;
    }

    /// The scope the preferences asked for.
    pub fn power_scope(&self) -> Option<PowerScope> {
        *lock(&self.power)
    }

    /// Record the scan result most recently produced by the scan service.
    pub fn set_scanned_source(&self, result: vise_core::ScanResult) {
        *lock(&self.scanner) = Some(result);
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, EncodeSlot> {
        lock(&self.inner)
    }

    /// Current state.
    pub fn state(&self) -> WorkState {
        self.slot().state
    }

    /// Copy of the current progress.
    pub fn progress(&self) -> EncodeProgress {
        self.slot().progress.clone()
    }

    /// Terminal status of the last encode.
    pub fn done(&self) -> WorkDone {
        self.slot().done.clone()
    }

    /// Whether an encode is running.
    pub fn is_running(&self) -> bool {
        self.slot().state == WorkState::Working
    }

    /// Suspend the encoder.
    pub fn pause(&self) -> Result<(), String> {
        let control = self.current_control()?;
        if control.is_paused() {
            return Ok(());
        }
        platform::suspend(control.pid).map_err(|e| e.to_string())?;
        control.paused.store(true, Ordering::SeqCst);
        self.slot().progress.paused = true;
        self.logs
            .info(format!("encode paused (pid {})", control.pid));
        Ok(())
    }

    /// Resume a suspended encoder.
    pub fn resume(&self) -> Result<(), String> {
        let control = self.current_control()?;
        if !control.is_paused() {
            return Ok(());
        }
        platform::resume(control.pid).map_err(|e| e.to_string())?;
        control.paused.store(false, Ordering::SeqCst);
        self.slot().progress.paused = false;
        self.logs
            .info(format!("encode resumed (pid {})", control.pid));
        Ok(())
    }

    /// Cancel the encoder and clean up its partial output.
    pub fn stop(&self) -> Result<(), String> {
        let control = self.current_control()?;
        self.logs.warn(format!(
            "encode cancelled by the caller (pid {})",
            control.pid
        ));
        control.kill();
        Ok(())
    }

    /// Describe why a control route has nothing to act on.
    fn current_control(&self) -> Result<Arc<EncodeControl>, String> {
        match lock(&self.control).clone() {
            Some(control) => Ok(control),
            None if self.is_running() => Err(
                "the encoder is still starting; retry once the job reports progress".to_string(),
            ),
            None => Err("no encode is running".to_string()),
        }
    }

    /// Validate and start an encode.
    ///
    /// Everything that can be checked synchronously is checked here so a bad
    /// job is refused immediately; the remaining source-dependent checks run on
    /// the encode thread before the encoder is spawned.
    pub fn start(&self, job: &JobSpec) -> Result<(), String> {
        if self.is_running() {
            return Err("an encode is already running".to_string());
        }

        let cached = lock(&self.scanner).clone();
        let report = vise_ipc::validation::validate_job(job, cached.as_ref());
        if !report.is_valid() {
            self.logs.error(format!("encode rejected: {report}"));
            return Err(format!("invalid encode job: {report}"));
        }

        {
            let mut slot = self.slot();
            slot.state = WorkState::Working;
            slot.sequence += 1;
            slot.started_at = Some(Instant::now());
            slot.progress = EncodeProgress {
                progress: 0.0,
                encoded_seconds: 0.0,
                eta_seconds: None,
                paused: false,
                output_path: job.output_path.clone(),
                stage: "validating".to_string(),
            };
            slot.done = WorkDone {
                error: error_codes::NONE,
                was_successful: true,
                message: String::new(),
            };
        }

        self.logs
            .info(format!("encode started for {}", job.input_path));
        // The per-encode log is opened here, before any work happens, so that
        // the reason a job was *refused* is in the same file as the reason it
        // failed. A log that starts when the encoder starts cannot answer "why
        // did this one not run".
        if let Some(path) = self.logs.begin_encode(&encode_log_label(job)) {
            self.logs.info(format!("encoding log: {}", path.display()));
        }
        // The hold is taken here, after the job has been accepted and before any
        // work happens, and released on every path out of the encode thread. An
        // encode that never started cannot need a machine held awake for it.
        hold_power(self.power_scope(), &self.logs);

        let inner = self.inner.clone();
        let control_slot = self.control.clone();
        let logs = self.logs.clone();
        let scanner = self.scanner.clone();
        let job = job.clone();
        let outcome = std::thread::Builder::new()
            .name("vise-encode".into())
            .spawn({
                let inner = inner.clone();
                move || run_encode(&inner, &control_slot, &logs, &scanner, &job)
            });

        match outcome {
            Ok(_) => Ok(()),
            Err(e) => {
                let message = format!("encode worker could not start: {e}");
                let mut slot = lock(&inner);
                slot.state = WorkState::WorkDone;
                slot.done = WorkDone {
                    error: error_codes::FAILED,
                    was_successful: false,
                    message: message.clone(),
                };
                Err(message)
            }
        }
    }

    /// Kill a running encoder so a worker shutdown leaves no orphan behind.
    ///
    /// A worker that exits while ffmpeg is still running would otherwise leave
    /// the encoder writing into a file nobody is watching.
    pub fn shutdown(&self) {
        if let Some(control) = lock(&self.control).clone() {
            control.kill();
        }
        // A worker that exits while holding the machine awake would leave it
        // awake for whoever is using it, so the hold goes with the process.
        release_power(&self.logs);
    }
}

fn run_encode(
    inner: &Arc<Mutex<EncodeSlot>>,
    control_slot: &Arc<Mutex<Option<Arc<EncodeControl>>>>,
    logs: &Arc<LogService>,
    scanner: &Arc<Mutex<Option<vise_core::ScanResult>>>,
    job: &JobSpec,
) {
    // A target-size job needs the real source duration, so scan when the scan
    // service has not already described this exact source.
    let mut source_facts = lock(scanner).clone();
    if source_facts.is_none() || job.resolved_encoding_mode().is_target_size() {
        let scanner_impl = vise_core::FfprobeScanner::default();
        match scanner_impl.scan_path(Path::new(&job.input_path)) {
            Ok(result) => {
                *lock(scanner) = Some(result.clone());
                source_facts = Some(result);
            }
            Err(e) => {
                logs.warn(format!("source could not be probed for validation: {e}"));
            }
        }
    }

    let report = vise_ipc::validation::validate_job(job, source_facts.as_ref());
    if !report.is_valid() {
        finish(
            inner,
            WorkDone {
                error: error_codes::INVALID,
                was_successful: false,
                message: format!("invalid encode job: {report}"),
            },
            logs,
            "encode rejected",
        );
        return;
    }

    set_stage(inner, "planning");
    let transcoder = Transcoder::default();
    let plan = match transcoder.plan(job) {
        Ok(plan) => plan,
        Err(e) => {
            finish(
                inner,
                WorkDone {
                    error: error_codes::INVALID,
                    was_successful: false,
                    message: format!("encode could not be planned: {e}"),
                },
                logs,
                "encode rejected",
            );
            return;
        }
    };

    // The encoder writes to a partial file that is promoted onto the requested
    // destination only after a successful run, so a cancelled or failed job
    // never leaves a half-written destination behind. The destination is kept
    // aside here because the plan's own output path is redirected.
    let destination = plan.output_path.clone();
    let partial = partial_output_path(&destination);
    let plan = TranscodePlan {
        args: rewrite_output(&plan.args, &partial),
        first_pass_args: plan
            .first_pass_args
            .as_ref()
            .map(|args| rewrite_output(args, &partial)),
        output_path: partial.clone(),
        ..plan
    };

    if let Some(parent) = plan.output_path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                finish(
                    inner,
                    WorkDone {
                        error: error_codes::FAILED,
                        was_successful: false,
                        message: format!("output directory could not be created: {e}"),
                    },
                    logs,
                    "encode failed",
                );
                return;
            }
        }
    }

    set_stage(inner, "encoding");
    logs.info(format!("encoder command: {}", plan.command_line()));

    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    if let Some(first_pass) = &plan.first_pass_args {
        let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let _ = Command::new(&plan.program)
            .args(first_pass)
            .arg(null_device)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            finish(
                inner,
                WorkDone {
                    error: error_codes::FAILED,
                    was_successful: false,
                    message: format!("encoder could not be started: {e}"),
                },
                logs,
                "encode failed",
            );
            return;
        }
    };

    let pid = child.id();
    let cancel = Arc::new(AtomicBool::new(false));
    // Take the progress pipe before the handle moves into the control block so
    // the encoder's status lines can be read while it runs.
    let stderr = child.stderr.take();
    let control = Arc::new(EncodeControl {
        pid,
        child: Mutex::new(Some(child)),
        cancel: cancel.clone(),
        paused: Arc::new(AtomicBool::new(false)),
    });
    *lock(control_slot) = Some(control.clone());

    let duration = plan.duration_seconds.unwrap_or(0.0);
    let started = Instant::now();
    let mut tail = String::new();

    if let Some(stderr) = stderr {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(|line| line.ok()) {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            if let Some((seconds, frame)) = parse_progress(&line) {
                let fraction = if duration > 0.0 {
                    (seconds / duration).clamp(0.0, 0.999)
                } else {
                    0.0
                };
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let eta = if duration > 0.0 {
                    let remaining = (duration - seconds).max(0.0);
                    Some(remaining / fraction.max(0.001) * elapsed - elapsed)
                } else {
                    None
                };
                let mut slot = lock(inner);
                slot.progress.progress = fraction;
                slot.progress.encoded_seconds = seconds;
                slot.progress.eta_seconds = eta;
                slot.progress.stage = format!("encoding frame {frame}");
            }
            if line.len() > 400 {
                tail = format!("{tail}\n{}", line);
                if tail.len() > 8_000 {
                    tail = tail[tail.len() - 8_000..].to_string();
                }
            }
        }
    }

    let status = {
        let mut guard = lock(&control.child);
        match guard.as_mut() {
            Some(child) => child.wait().ok(),
            None => None,
        }
    };
    *lock(control_slot) = None;

    if cancel.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&partial);
        finish(
            inner,
            WorkDone {
                error: error_codes::CANCELLED,
                was_successful: false,
                message: "encode cancelled before completion".to_string(),
            },
            logs,
            "encode cancelled",
        );
        return;
    }

    match status {
        Some(status) if status.success() && partial.is_file() => {
            match promote(&partial, &destination) {
                Ok(()) => {
                    {
                        let mut slot = lock(inner);
                        slot.progress.progress = 1.0;
                        slot.progress.stage = "complete".to_string();
                    }
                    // `finish` takes the same lock, so the guard above must be
                    // released first.
                    finish(
                        inner,
                        WorkDone {
                            error: error_codes::NONE,
                            was_successful: true,
                            message: String::new(),
                        },
                        logs,
                        "encode completed",
                    );
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&partial);
                    finish(
                        inner,
                        WorkDone {
                            error: error_codes::FAILED,
                            was_successful: false,
                            message: format!("output could not be published: {e}"),
                        },
                        logs,
                        "encode failed",
                    );
                }
            }
        }
        Some(status) => {
            let _ = std::fs::remove_file(&partial);
            let detail = if tail.trim().is_empty() {
                format!("encoder exited with status {status}")
            } else {
                tail.trim().to_string()
            };
            finish(
                inner,
                WorkDone {
                    error: error_codes::FAILED,
                    was_successful: false,
                    message: detail,
                },
                logs,
                "encode failed",
            );
        }
        None => {
            let _ = std::fs::remove_file(&partial);
            finish(
                inner,
                WorkDone {
                    error: error_codes::FAILED,
                    was_successful: false,
                    message: "encoder process could not be waited on".to_string(),
                },
                logs,
                "encode failed",
            );
        }
    }
}

fn set_stage(inner: &Arc<Mutex<EncodeSlot>>, stage: &str) {
    let mut slot = lock(inner);
    slot.progress.stage = stage.to_string();
}

fn finish(inner: &Arc<Mutex<EncodeSlot>>, done: WorkDone, logs: &LogService, message: &str) {
    // Every way out of an encode goes through here — finished, failed, cancelled,
    // rejected, or never startable — so this is the one place the machine is let
    // go of, and the flag makes a second call harmless.
    release_power(logs);
    let level = if done.was_successful {
        vise_ipc::contract::LogLevel::Info
    } else {
        vise_ipc::contract::LogLevel::Warning
    };
    logs.log(level, message.to_string());
    // Closing the encode log and trimming the folder happen here rather than on
    // a timer, so the file is never rotated while something is being written to
    // it, and a worker that is not encoding never spends time on housekeeping.
    // It runs before the level is chosen so that a log about the log lands in
    // the session file only, after the encode file is closed.
    close_encode_log(logs);
    let mut slot = lock(inner);
    slot.state = WorkState::WorkDone;
    slot.progress.paused = false;
    slot.done = done;
}

/// Close the running encode's log and record what the folder looks like now.
fn close_encode_log(logs: &LogService) {
    let report = logs.end_encode();
    if let Some(problem) = report.problem {
        logs.warn(format!("encode logs could not be trimmed: {problem}"));
    }
    if !report.removed.is_empty() {
        logs.info(format!(
            "trimmed {} older encode log(s), {} file(s) and {} byte(s) remain",
            report.removed.len(),
            report.kept,
            report.bytes_kept
        ));
    }
}

/// The name one encode's log file is written under.
///
/// The output file is preferred because that is what the person queueing the
/// job recognises; the input is the fallback for a job whose output has not been
/// decided yet, and the last resort keeps the file name usable rather than
/// refusing to log.
pub fn encode_log_label(job: &JobSpec) -> String {
    let stem = |path: &str| {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string())
    };
    stem(&job.output_path)
        .or_else(|| stem(&job.input_path))
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| "job".to_string())
}

/// Temporary output path used while an encode runs.
pub fn partial_output_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let extension = output.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    parent.join(format!(
        "{stem}.vise-part-{}.{extension}",
        std::process::id()
    ))
}

/// Replace the destination argument in a resolved command line.
fn rewrite_output(args: &[String], partial: &Path) -> Vec<String> {
    let mut out = args.to_vec();
    if let Some(last) = out.last_mut() {
        *last = partial.to_string_lossy().to_string();
    }
    out
}

/// Move a completed partial output onto the requested destination.
fn promote(partial: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    match std::fs::rename(partial, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            // A cross-device move is the usual reason a rename fails here.
            std::fs::copy(partial, destination)?;
            let _ = std::fs::remove_file(partial);
            Ok(())
        }
    }
}

/// Extract encoded seconds and frame number from an encoder status line.
///
/// The encoder right-aligns some values, so a field can be split across two
/// whitespace-separated tokens (`frame=  120`).
pub fn parse_progress(line: &str) -> Option<(f64, u64)> {
    let mut seconds = None;
    let mut frame = 0u64;
    let mut tokens = line.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        let (key, mut value) = match token.split_once('=') {
            Some((key, value)) => (key, value),
            None => continue,
        };
        if value.is_empty() {
            // The number is in the next token when the encoder padded the field.
            value = tokens.next().unwrap_or_default();
        }
        match key {
            "time" => seconds = parse_clock(value),
            "frame" => frame = value.trim().parse().unwrap_or(frame),
            _ => {}
        }
    }
    seconds.map(|s| (s, frame))
}

fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let mut parts = text.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next().unwrap_or("0").parse().ok()?;
    let seconds: f64 = parts.next().unwrap_or("0").parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_progress_lines() {
        assert_eq!(
            parse_progress(
                "frame=  120 fps=25 q=28.0 size=  10240kB time=00:00:04.80 bitrate=17.0kbits/s"
            ),
            Some((4.8, 120))
        );
        assert_eq!(parse_progress("time=00:01:00.00"), Some((60.0, 0)));
        assert_eq!(parse_progress("no progress here"), None);
    }

    #[test]
    fn partial_output_sits_next_to_the_destination() {
        let partial = partial_output_path(Path::new("/tmp/out/movie.mkv"));
        assert_eq!(partial.parent().unwrap(), Path::new("/tmp/out"));
        assert!(partial.to_string_lossy().contains("movie.vise-part-"));
        assert!(partial.to_string_lossy().ends_with(".mkv"));
    }

    #[test]
    fn rewrites_only_the_destination_argument() {
        let args = vec![
            "-y".to_string(),
            "-i".to_string(),
            "in.mkv".to_string(),
            "out.mp4".to_string(),
        ];
        let rewritten = rewrite_output(&args, Path::new("out.part.mp4"));
        assert_eq!(rewritten[..3], args[..3]);
        assert_eq!(rewritten[3], "out.part.mp4");
    }

    #[test]
    fn an_encode_log_is_named_after_the_output_the_user_recognises() {
        let job = JobSpec {
            input_path: "C:\\media\\raw.mkv".into(),
            output_path: "D:\\out\\Episode 1.mkv".into(),
            ..JobSpec::default()
        };
        assert_eq!(encode_log_label(&job), "Episode 1");

        // A job with no destination yet still gets a usable name rather than an
        // empty one, because the log is opened before the job is refused.
        let unnamed = JobSpec {
            input_path: "C:\\media\\raw.mkv".into(),
            ..JobSpec::default()
        };
        assert_eq!(encode_log_label(&unnamed), "raw");
        assert_eq!(encode_log_label(&JobSpec::default()), "job");
    }

    #[test]
    fn control_routes_require_a_running_encode() {
        let logs = Arc::new(LogService::default());
        let service = EncodeService::new(logs);
        assert!(service.pause().unwrap_err().contains("no encode"));
        assert!(service.resume().unwrap_err().contains("no encode"));
        assert!(service.stop().unwrap_err().contains("no encode"));
        assert!(!service.is_running());
    }

    #[test]
    fn rejects_an_invalid_job_before_starting() {
        let logs = Arc::new(LogService::default());
        let service = EncodeService::new(logs);
        let err = service.start(&JobSpec::default()).unwrap_err();
        assert!(err.contains("invalid encode job"));
        assert!(!service.is_running());
        assert_eq!(service.state(), WorkState::Unknown);
    }

    #[test]
    fn refuses_a_second_concurrent_encode() {
        let logs = Arc::new(LogService::default());
        let service = EncodeService::new(logs);
        let mut slot = lock(&service.inner);
        slot.state = WorkState::Working;
        drop(slot);
        let err = service.start(&JobSpec::default()).unwrap_err();
        assert!(err.contains("already running"));
    }

    #[test]
    fn promotes_completed_output() {
        let dir = std::env::temp_dir().join("vise-worker-promote");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let partial = dir.join("movie.vise-part.mp4");
        let destination = dir.join("movie.mp4");
        std::fs::write(&partial, b"data").expect("write partial");
        promote(&partial, &destination).expect("promote");
        assert_eq!(std::fs::read(&destination).unwrap(), b"data");
        assert!(!partial.exists());

        // An existing destination is replaced.
        std::fs::write(&partial, b"new").expect("write partial");
        promote(&partial, &destination).expect("replace");
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
    }
}
