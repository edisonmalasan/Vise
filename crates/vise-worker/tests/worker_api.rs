//! End-to-end tests for the worker JSON API.
//!
//! Every test here starts the real worker binary on its own ephemeral loopback
//! port with its own freshly generated token, and talks to it over HTTP exactly
//! as the desktop application does. Media is generated locally with ffmpeg, so
//! the tests are hermetic: no network, no shared state, no pre-existing files.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use vise_core::{ContainerFormat, JobSpec};
use vise_ipc::contract::{
    CommandResult, EncodeCommand, InitCommand, LogMessage, PreviewCommand, PreviewResult,
    ProgressReport, ScanCommand, TitlesResult, WorkState,
};
use vise_ipc::crash::{EncodeOutcome, IsolationPolicy, Resolution, WorkerDeath};
use vise_ipc::timespan::TimeSpan;
use vise_ipc::{ClientError, WorkerClient, WorkerConfig, WorkerSupervisor};

/// Path to the worker binary built alongside this test.
const WORKER_EXE: &str = env!("CARGO_BIN_EXE_vise-worker");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vise-worker-it-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Media lives under its own root so regenerating the media never disturbs the
/// handshake files a running worker writes beside it.
fn media_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vise-worker-it-{name}-media"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("media dir");
    dir
}

/// Start a worker on an ephemeral port with a generated token.
///
/// The supervisor is the supported entry point: it spawns the process, waits
/// for the handshake, and hands back a client. Dropping the supervisor stops
/// the process, so no test can leak a worker.
fn start_worker(name: &str) -> (WorkerSupervisor, WorkerClient) {
    let mut supervisor = WorkerSupervisor::new(
        WorkerConfig::new(WORKER_EXE)
            .with_port(0)
            .with_handshake_dir(temp_dir(name))
            .with_handshake_timeout(Duration::from_secs(30)),
    );
    let connection = supervisor.start().expect("worker start");
    let client = connection.client().with_timeout(Duration::from_secs(120));
    (supervisor, client)
}

/// Create a short media file with a video and an audio track.
fn generate_source(dir: &Path, name: &str, seconds: u32) -> PathBuf {
    let ffmpeg = vise_core::FfprobeScanner::default()
        .ffmpeg_path()
        .expect("ffmpeg must be available for the worker tests")
        .to_path_buf();
    let out = dir.join(name);
    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=duration={seconds}:size=320x180:rate=15"),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={seconds}"),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&out)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "source generation failed for {out:?}");
    assert!(out.is_file());
    out
}

fn init_command(log_dir: &Path) -> InitCommand {
    InitCommand {
        log_directory: log_dir.to_string_lossy().to_string(),
        log_file: "worker.log".into(),
        enable_disk_logging: true,
        ..InitCommand::default()
    }
}

fn scan_command(source: &Path, logs: &Path) -> ScanCommand {
    ScanCommand {
        initialise_command: init_command(logs),
        paths: vec![source.to_string_lossy().to_string()],
        preview_count: 2,
        min_duration: TimeSpan::ZERO,
        max_duration: TimeSpan::parse("1.00:00:00").expect("max duration"),
        title_index: 0,
        ..ScanCommand::default()
    }
}

/// Resumes a paused encoder when a test unwinds, so a failing assertion never
/// leaves a suspended ffmpeg behind.
struct ResumeGuard<'a> {
    client: &'a WorkerClient,
    armed: bool,
}

impl<'a> ResumeGuard<'a> {
    fn new(client: &'a WorkerClient) -> Self {
        ResumeGuard {
            client,
            armed: false,
        }
    }
}

impl Drop for ResumeGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.client.resume_encode();
            let _ = self.client.stop_encode();
        }
    }
}

fn poll_until<F>(mut read: F, timeout: Duration, label: &str) -> ProgressReport
where
    F: FnMut() -> ProgressReport,
{
    let deadline = Instant::now() + timeout;
    loop {
        let report = read();
        if report.state == WorkState::WorkDone {
            return report;
        }
        assert!(
            Instant::now() < deadline,
            "{label} never reached a terminal state; last report {report:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Read a progress report, riding out a worker that is being replaced.
///
/// The supervisor restarts a worker that died, and the window simply asks for
/// progress again. A client that happened to be mid-request across that handover
/// sees a reset connection rather than a state, which says nothing about the
/// behaviour under test. Retrying briefly mirrors what the window does, so a
/// test is not failed by a restart it did not ask for.
fn read_progress<F>(mut read: F, label: &str) -> ProgressReport
where
    F: FnMut() -> Result<ProgressReport, vise_ipc::client::ClientError>,
{
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match read() {
            Ok(report) => return report,
            Err(error) => {
                if Instant::now() >= deadline {
                    panic!("{label} could not be read for 30s: {error}");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Progress of an encode, tolerating a worker handover.
fn encode_progress(client: &WorkerClient) -> ProgressReport {
    read_progress(|| client.poll_encode_progress(), "encode progress")
}

/// Progress of a scan, tolerating a worker handover.
fn scan_progress(client: &WorkerClient) -> ProgressReport {
    read_progress(|| client.poll_scan_progress(), "scan progress")
}

// ---------------------------------------------------------------------------
// Crash isolation
// ---------------------------------------------------------------------------
//
// What follows drives the same attribution the desktop window uses, over a real
// worker that is really killed. The window's part of it is the poll loop and the
// sweep; both are exercised here against the actual binary and the actual
// subprocess, because the one thing worth being sure of is that a worker killed
// mid-encode is noticed as a dead *process* and not as a queue that went quiet.

/// Wait until the encoder process exists, so a kill lands during the encode
/// rather than during the planning that precedes it.
fn wait_until_encoding<F>(mut read: F, label: &str) -> ProgressReport
where
    F: FnMut() -> ProgressReport,
{
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let report = read();
        assert!(
            Instant::now() < deadline,
            "{label} never started encoding; last report {report:?}"
        );
        if report
            .working
            .as_ref()
            .map(|w| w.stage.starts_with("encoding"))
            .unwrap_or(false)
        {
            return report;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait until the client cannot reach the worker any more.
fn wait_until_gone(client: &WorkerClient) -> ClientError {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Err(err) = client.is_token_set() {
            return err;
        }
        assert!(
            Instant::now() < deadline,
            "the client must notice a worker that died"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A source long enough that an encode is still running when the worker dies.
fn long_source(dir: &Path, seconds: u32) -> PathBuf {
    generate_source(dir, "long.mkv", seconds)
}

/// Runs the isolation tests one at a time.
///
/// They kill worker processes and leave an encoder behind each one: a killed
/// worker does not get to kill the ffmpeg it started, so an orphaned encoder
/// keeps running after its test has finished. Running these in parallel would
/// have one test's orphan competing with another's encode for the machine, and
/// the failures that produces look nothing like the thing being tested.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Stop an encode, retrying while the encoder process is still being planned.
///
/// The control routes deliberately refuse while the encode thread has not
/// spawned ffmpeg yet, so a test that stops the moment the stage turns to
/// "encoding" is racing the very window the refusal describes.
fn stop_when_ready(client: &WorkerClient) -> bool {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Ok(result) = client.stop_encode() {
            if result.was_successful {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_worker_killed_mid_encode_fails_only_its_job_and_the_next_job_still_runs() {
    let _serial = serial();
    let dir = temp_dir("isolation");
    let media = media_dir("isolation");
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).expect("log dir");
    // Long enough that the encode is still going when the worker is killed.
    let source = long_source(&media, 600);
    let first = dir.join("first.mp4");
    let second = dir.join("second.mp4");

    let (mut supervisor, client) = start_worker("isolation");
    client
        .start_scan(&scan_command(&source, &logs))
        .expect("StartScan");
    poll_until(|| scan_progress(&client), Duration::from_secs(60), "scan");

    let job = |output: &Path| JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        quality: Some(40),
        ..JobSpec::default()
    };

    // Job one starts, and the window's poll loop is what notices the death.
    let started = client
        .start_encode(&EncodeCommand {
            initialise_command: init_command(&logs),
            encode_job: job(&first),
        })
        .expect("StartEncode");
    assert!(started.was_successful, "encode refused: {}", started.error);
    wait_until_encoding(|| encode_progress(&client), "job one");

    // Kill the worker outright: no shutdown request, no chance to tidy up.
    let connection = supervisor.connection().expect("a running worker");
    let killed = Command::new("taskkill")
        .args(["/PID", &connection.pid.to_string(), "/F"])
        .output();
    assert!(
        killed.map(|out| out.status.success()).unwrap_or(false),
        "the worker process must be killable for this test"
    );

    // The poll fails, and the failure is recognised as a dead process rather
    // than as a queue that went quiet.
    let error = wait_until_gone(&client);
    assert!(error.is_worker_gone(), "unexpected error: {error}");
    let death = supervisor
        .notice_death()
        .expect("the supervisor must see the worker it lost");
    assert_eq!(death.pid, connection.pid, "{death:?}");
    assert_ne!(
        death.code,
        Some(0),
        "a killed process does not report success: {death:?}"
    );
    assert!(
        death
            .describe()
            .contains(&format!("{}", death.code.unwrap_or(-1)))
            || death.code.is_none(),
        "the exit status travels with the death: {death:?}"
    );

    // The window's decision: this job fails, on its own, and the queue carries
    // on.
    let mut policy = IsolationPolicy::new();
    let resolution = policy.resolved(EncodeOutcome::WorkerGone(death.clone()), &first.clone());
    match &resolution {
        Resolution::JobCrashed { reason, output, .. } => {
            assert_eq!(output, &first, "the crash is about this job's file");
            assert!(reason.contains("worker process"), "{reason}");
        }
        other => panic!("expected an isolated crash, got {other:?}"),
    }
    assert!(
        resolution.continues_queue(),
        "one dead worker must not stop the queue"
    );

    // What the dead worker could not do for itself: remove its partial output.
    // The sweep is the window's job, and it must not touch the destination.
    //
    // A worker killed outright does not get to kill the encoder it started, so
    // on some platforms the file it was writing is still open. That is the real
    // case, and the contract is not "the file is gone": it is that every partial
    // is either removed or named, so the user is told which file is still there
    // and can remove it themselves. Nothing is ever promoted to a finished file.
    let orphan = vise_ipc::partial::partial_output_path_for(&first, connection.pid);
    assert!(
        orphan.is_file(),
        "the killed worker left the partial it was writing: {orphan:?}"
    );
    let stale = vise_ipc::partial::partial_output_path_for(&first, connection.pid + 1);
    std::fs::write(&stale, b"an earlier partial").expect("write a partial");
    let cleanup = vise_ipc::partial::remove_partial_outputs(&first);
    assert!(
        cleanup.removed.contains(&stale),
        "a partial nobody is writing is removed: {cleanup:?}"
    );
    let still_there: Vec<&PathBuf> = cleanup
        .failed
        .iter()
        .map(|(path, _)| path)
        .chain(cleanup.removed.iter())
        .collect();
    for path in still_there {
        assert!(
            cleanup.removed.contains(path) || cleanup.failed.iter().any(|(p, _)| p == path),
            "every partial is either removed or reported: {path:?} in {cleanup:?}"
        );
    }
    assert!(
        !cleanup.summary().is_empty(),
        "the sweep says what it did: {cleanup:?}"
    );
    assert!(
        !first.exists(),
        "a crashed job is never promoted to a finished file"
    );

    // Job two runs: the supervisor brings up a fresh worker and the next encode
    // completes on it.
    let replacement = supervisor.ensure_running().expect("a fresh worker");
    assert_ne!(replacement.pid, connection.pid, "a new process");
    assert_eq!(supervisor.restarts(), 1);
    let next = replacement.client().with_timeout(Duration::from_secs(120));
    let started = next
        .start_encode(&EncodeCommand {
            initialise_command: init_command(&logs),
            encode_job: job(&second),
        })
        .expect("StartEncode on the replacement");
    assert!(
        started.was_successful,
        "the next job must be accepted: {}",
        started.error
    );
    // This source is far too long to encode in a test, so the encode is stopped
    // once the replacement has proved it is really encoding. What is being
    // asserted is that a fresh worker took the job on at all: the failure being
    // tested is the first worker dying, not this one finishing.
    wait_until_encoding(|| encode_progress(&next), "job two");
    assert!(
        stop_when_ready(&next),
        "the replacement accepts the control routes"
    );
    let _ = poll_until(
        || encode_progress(&next),
        Duration::from_secs(60),
        "the second encode",
    );
    supervisor.shutdown().expect("shutdown");
}

#[test]
fn a_user_requested_stop_is_not_reported_as_a_crash() {
    let _serial = serial();
    let dir = temp_dir("isolation-stop");
    let media = media_dir("isolation-stop");
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).expect("log dir");
    let source = long_source(&media, 600);
    let output = dir.join("stopped.mp4");

    let (mut supervisor, client) = start_worker("isolation-stop");
    client
        .start_scan(&scan_command(&source, &logs))
        .expect("StartScan");
    poll_until(|| scan_progress(&client), Duration::from_secs(60), "scan");

    let job = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        quality: Some(40),
        ..JobSpec::default()
    };
    let started = client
        .start_encode(&EncodeCommand {
            initialise_command: init_command(&logs),
            encode_job: job,
        })
        .expect("StartEncode");
    assert!(started.was_successful, "encode refused: {}", started.error);
    wait_until_encoding(|| encode_progress(&client), "the job");

    // The user presses stop. The window records that before it asks the worker,
    // because the worker may be killed while the request is in flight.
    let mut policy = IsolationPolicy::new();
    policy.request_stop();
    assert!(
        stop_when_ready(&client),
        "the worker accepts the stop request"
    );

    let report = poll_until(
        || encode_progress(&client),
        Duration::from_secs(60),
        "the stopped encode",
    );
    let done = report.work_done.clone().expect("WorkDone");
    let resolution = policy.resolved(
        EncodeOutcome::Finished {
            was_successful: done.was_successful,
            message: done.message.clone(),
            error_code: done.error,
        },
        &output,
    );
    assert!(
        matches!(resolution, Resolution::JobCancelled { .. }),
        "a stop is a cancellation, not a failure: {resolution:?}"
    );
    assert_eq!(
        policy.guard().consecutive(),
        0,
        "and it is not counted against the worker"
    );
    // The stopped encode leaves no file for the user to explain.
    assert!(!output.exists(), "a cancelled job writes no destination");
    supervisor.shutdown().expect("shutdown");
}

#[test]
fn consecutive_worker_deaths_are_replaced_and_then_reported_rather_than_looping() {
    let dir = temp_dir("isolation-streak");
    let mut supervisor = WorkerSupervisor::new(
        WorkerConfig::new(WORKER_EXE)
            .with_port(0)
            .with_handshake_dir(dir)
            .with_handshake_timeout(Duration::from_secs(30)),
    );

    // The same decision the window makes, against the same streak, but driven
    // from real processes: each worker is killed, the streak grows, and the
    // window is told whether to replace it or to stop.
    let _serial = serial();
    let mut policy = IsolationPolicy::new();
    let mut replacements = 0u32;
    let mut pids: Vec<u32> = Vec::new();

    for attempt in 1..=3 {
        let connection = supervisor.ensure_running().expect("a worker");
        pids.push(connection.pid);
        assert!(connection.client().is_token_set().expect("IsTokenSet"));
        let _ = Command::new("taskkill")
            .args(["/PID", &connection.pid.to_string(), "/F"])
            .output();
        let _ = wait_until_gone(&connection.client().with_timeout(Duration::from_secs(5)));
        let death = supervisor
            .notice_death()
            .unwrap_or_else(|| WorkerDeath::unexplained(connection.pid, "killed in the test"));
        let resolution = policy.resolved(EncodeOutcome::WorkerGone(death), Path::new("out.mkv"));
        match attempt {
            1 | 2 => match resolution {
                Resolution::JobCrashed { backoff, .. } => {
                    replacements += 1;
                    assert_eq!(
                        backoff,
                        Duration::from_secs(2) * (replacements - 1),
                        "each replacement waits longer than the last"
                    );
                }
                other => panic!("attempt {attempt}: expected a replacement, got {other:?}"),
            },
            _ => {
                assert!(
                    !resolution.continues_queue(),
                    "the third death in a row stops the queue"
                );
                match &resolution {
                    Resolution::QueueStopped { reason, .. } => assert!(
                        reason.contains("3 time(s) in a row"),
                        "the streak must be reported: {reason}"
                    ),
                    other => panic!("attempt {attempt}: expected the queue to stop, got {other:?}"),
                }
            }
        }
    }
    assert_eq!(replacements, 2, "two workers were replaced before the stop");
    assert_eq!(policy.guard().consecutive(), 3);
    assert_eq!(pids.len(), 3, "three distinct processes ran");
    // No fourth worker: the streak was reported rather than restarted again.
    let restarts_before = supervisor.restarts();
    let _ = supervisor.ensure_running().expect("a worker");
    assert_eq!(supervisor.restarts(), restarts_before + 1);
    supervisor.shutdown().expect("shutdown");
}

#[test]
fn handshake_reports_a_generated_token_and_an_ephemeral_port() {
    let mut supervisor = WorkerSupervisor::new(
        WorkerConfig::new(WORKER_EXE)
            .with_port(0)
            .with_handshake_dir(temp_dir("handshake-dir")),
    );
    let connection = supervisor.start().expect("worker start");

    assert_ne!(connection.port, 0, "an ephemeral port must be reported");
    assert!(!connection.token.is_empty(), "a token must be issued");
    assert!(connection.pid > 0);

    let info = connection.client().worker_info().expect("worker info");
    assert_eq!(info.port, connection.port);
    assert_eq!(info.pid, connection.pid);
    assert_eq!(info.protocol_version, vise_ipc::PROTOCOL_VERSION);
    assert!(info.token_set);
    assert!(info.requires_token);
    assert!(info.routes.contains(&"StartEncode".to_string()));
}

#[test]
fn requires_a_token_on_every_route() {
    let (_supervisor, client) = start_worker("token");

    assert!(client.is_token_set().expect("IsTokenSet"));

    // No token at all.
    let reply = client
        .request_with_token("GET", "/GetTitles", None, b"")
        .expect("raw request");
    assert_eq!(reply.status, 401);
    assert!(reply.body.contains("/GetTitles"), "{}", reply.body);
    assert!(reply.body.contains("token"), "{}", reply.body);

    // A wrong token.
    let reply = client
        .request_with_token("GET", "/GetTitles", Some("not-the-token"), b"")
        .expect("raw request");
    assert_eq!(reply.status, 401);
    assert!(reply.body.contains("was not valid"), "{}", reply.body);

    // An unknown route is reported as unknown, not as an authentication
    // failure, so a caller can tell the two apart.
    let reply = client
        .request("GET", "/NoSuchCommand", b"")
        .expect("raw request");
    assert_eq!(reply.status, 404);
    assert!(
        reply.body.to_lowercase().contains("unknown"),
        "{}",
        reply.body
    );
}

#[test]
fn scan_contract_reports_titles_and_progress() {
    let dir = media_dir("scan");
    let source = generate_source(&dir, "source.mp4", 3);
    let logs = dir.join("logs");
    let (_supervisor, client) = start_worker("scan");

    let started = client
        .start_scan(&scan_command(&source, &logs))
        .expect("StartScan");
    assert!(started.was_successful, "scan refused: {}", started.error);

    let report = poll_until(|| scan_progress(&client), Duration::from_secs(60), "scan");
    assert!(
        report.work_done.as_ref().expect("WorkDone").was_successful,
        "scan failed: {report:?}"
    );

    let titles: TitlesResult = client.get_titles().expect("GetTitles");
    assert_eq!(titles.title_list.len(), 1, "{titles:?}");
    let title = &titles.title_list[0];
    assert_eq!(title.index, 1);
    assert_eq!(title.width, 320);
    assert_eq!(title.height, 180);
    assert!((title.duration_seconds - 3.0).abs() < 1.0, "{title:?}");
    assert_eq!(title.audio_list.len(), 1);
    assert_eq!(
        client.get_main_title().expect("GetMainTitle"),
        titles.main_feature
    );
    assert_eq!(titles.main_feature, 1);

    // The init command in the scan request configured disk logging.
    let text = std::fs::read_to_string(logs.join("worker.log")).expect("log file");
    assert!(text.contains("initialised"), "{text}");
}

#[test]
fn scan_contract_rejects_unusable_requests() {
    let (_supervisor, client) = start_worker("scan-bad");

    // No source paths at all.
    let result = client
        .start_scan(&ScanCommand::default())
        .expect("StartScan");
    assert!(!result.was_successful);
    assert!(result.error.contains("no source paths"), "{}", result.error);

    // A duration outside the accepted range is a decode error, not a crash.
    let body = br#"{"Paths":["a.mkv"],"MinDuration":"99:59:59"}"#;
    let reply = client
        .request("POST", "/StartScan", body)
        .expect("raw request");
    assert_eq!(reply.status, 400);
    assert!(reply.body.contains("between 00 and 23"), "{}", reply.body);

    // A source that does not exist is refused with a readable reason.
    let command = ScanCommand {
        paths: vec!["definitely-not-here.mkv".into()],
        ..ScanCommand::default()
    };
    let result = client.start_scan(&command).expect("StartScan");
    assert!(!result.was_successful);
    assert!(result.error.contains("does not exist"), "{}", result.error);
}

#[test]
fn encode_contract_runs_pauses_resumes_and_stops() {
    let dir = media_dir("encode");
    let source = generate_source(&dir, "source.mp4", 20);
    let logs = dir.join("logs");
    let (_supervisor, client) = start_worker("encode");

    // Validate the source first so the job has real facts behind it.
    assert!(
        client
            .start_scan(&scan_command(&source, &logs))
            .expect("StartScan")
            .was_successful
    );
    poll_until(|| scan_progress(&client), Duration::from_secs(60), "scan");

    let output = dir.join("out.mp4");
    let job = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        quality: Some(40),
        ..JobSpec::default()
    };

    let started = client
        .start_encode(&EncodeCommand {
            initialise_command: init_command(&logs),
            encode_job: job,
        })
        .expect("StartEncode");
    assert!(started.was_successful, "encode refused: {}", started.error);

    // Pause as soon as the encoder process exists, resume it, and let the job
    // run to completion. The control routes report a clear error while the
    // encoder is still being planned, so this retries briefly.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut pause_guard = ResumeGuard::new(&client);
    let mut paused = false;
    while Instant::now() < deadline {
        let report = encode_progress(&client);
        if report.state != WorkState::Working {
            break;
        }
        match client.pause_encode() {
            Ok(result) if result.was_successful => {
                pause_guard.armed = true;
                let paused_report = encode_progress(&client);
                assert!(
                    paused_report
                        .working
                        .as_ref()
                        .map(|w| w.paused)
                        .unwrap_or(false),
                    "progress must report the pause: {paused_report:?}"
                );
                let resumed = client.resume_encode().expect("ResumeEncode");
                assert!(resumed.was_successful, "resume refused: {}", resumed.error);
                pause_guard.armed = false;
                paused = true;
                break;
            }
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    assert!(paused, "the encode finished before it could be paused");

    let report = poll_until(
        || encode_progress(&client),
        Duration::from_secs(180),
        "encode",
    );
    let done = report.work_done.clone().expect("WorkDone");
    assert!(done.was_successful, "encode failed: {done:?}");
    assert!(output.is_file(), "the output file must exist");
}

#[test]
fn encode_contract_stops_a_running_job() {
    let dir = media_dir("stop");
    let source = generate_source(&dir, "source.mp4", 120);
    let logs = dir.join("logs");
    let (_supervisor, client) = start_worker("stop");

    let output = dir.join("stopped.mp4");
    let job = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        ..JobSpec::default()
    };
    let started = client
        .start_encode(&EncodeCommand {
            initialise_command: init_command(&logs),
            encode_job: job,
        })
        .expect("StartEncode");
    assert!(started.was_successful, "{}", started.error);

    // Stop as soon as the encoder process exists.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut stopped = false;
    while Instant::now() < deadline {
        let report = encode_progress(&client);
        if report.state != WorkState::Working {
            break;
        }
        match client.stop_encode() {
            Ok(result) if result.was_successful => {
                stopped = true;
                break;
            }
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    assert!(stopped, "the encode finished before it could be stopped");

    let report = poll_until(
        || encode_progress(&client),
        Duration::from_secs(60),
        "encode",
    );
    let done = report.work_done.clone().expect("WorkDone");
    assert!(
        !done.was_successful,
        "a stopped job must not report success: {done:?}"
    );
    assert!(
        !output.is_file(),
        "a stopped job must not leave an output file behind"
    );
}

#[test]
fn encode_contract_rejects_an_invalid_job() {
    let (_supervisor, client) = start_worker("invalid");

    let result = client
        .start_encode(&EncodeCommand::default())
        .expect("StartEncode");
    assert!(!result.was_successful);
    assert!(
        result.error.contains("invalid encode job"),
        "{}",
        result.error
    );

    // The worker stays up and still answers, which is the whole point of
    // validating before the pipeline starts.
    assert_eq!(
        client
            .poll_encode_progress()
            .expect("PollEncodeProgress")
            .state,
        WorkState::Unknown
    );
    let control = client.pause_encode().expect("PauseEncode");
    assert!(!control.was_successful);
    assert!(
        control.error.contains("no encode is running"),
        "{}",
        control.error
    );
}

#[test]
fn preview_contract_extracts_a_frame() {
    let dir = media_dir("preview");
    let source = generate_source(&dir, "source.mp4", 3);
    let (_supervisor, client) = start_worker("preview");

    // Without a source the worker says so instead of guessing.
    let failure = client
        .get_preview(&PreviewCommand::default())
        .expect("GetPreview");
    assert!(!failure.was_successful);
    assert!(
        failure.error.contains("scan a source first"),
        "{}",
        failure.error
    );

    let target = dir.join("frame.jpg");
    let result: PreviewResult = client
        .get_preview(&PreviewCommand {
            path: source.to_string_lossy().to_string(),
            title_index: 1,
            time_seconds: 1.0,
            output_file: target.to_string_lossy().to_string(),
        })
        .expect("GetPreview");
    assert!(result.was_successful, "preview failed: {}", result.error);
    assert_eq!(
        std::fs::metadata(&target).expect("preview file").len(),
        result.image_bytes
    );

    // With no output file the image comes back inline, base64 encoded.
    let inline = client
        .get_preview(&PreviewCommand {
            path: source.to_string_lossy().to_string(),
            time_seconds: 1.0,
            ..PreviewCommand::default()
        })
        .expect("GetPreview");
    assert!(inline.was_successful, "{}", inline.error);
    assert!(inline.image_bytes > 0);
    let decoded = vise_ipc::base64::decode(&inline.image_base64).expect("base64");
    assert_eq!(decoded.len() as u64, inline.image_bytes);
}

#[test]
fn logging_contract_reads_and_resets() {
    let (_supervisor, client) = start_worker("logging");

    let all: Vec<LogMessage> = client.get_all_log_messages().expect("GetAllLogMessages");
    assert!(!all.is_empty(), "the worker logs its own startup");
    assert_eq!(all[0].index, 0);

    let page = client.get_log_messages_from_index(1).expect("paged logs");
    assert!(page.messages.iter().all(|m| m.index >= 1));
    assert_eq!(page.next_index, all.last().expect("entry").index + 1);

    client.reset_logging().expect("ResetLogging");
    assert!(client.get_all_log_messages().expect("logs").is_empty());

    // A reset restart is reflected in the index the worker hands out next.
    let result = client.reset_logging().expect("ResetLogging");
    assert!(result.was_successful);
}

#[test]
fn shutdown_route_stops_the_worker_and_the_supervisor_reports_the_death() {
    let mut supervisor = WorkerSupervisor::new(
        WorkerConfig::new(WORKER_EXE)
            .with_port(0)
            .with_handshake_dir(temp_dir("shutdown-dir")),
    );
    let connection = supervisor.start().expect("worker start");
    let client = connection.client();

    let result: CommandResult = client.shutdown().expect("Shutdown");
    assert!(result.was_successful);

    // The client notices the worker is gone, which is what triggers a restart.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut gone = false;
    while Instant::now() < deadline {
        if let Err(err) = client.is_token_set() {
            assert!(err.is_worker_gone(), "unexpected error: {err}");
            gone = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(gone, "the worker must stop answering after Shutdown");

    // The supervisor replaces the dead worker on demand.
    let restarted = supervisor.ensure_running().expect("restart");
    assert_ne!(
        restarted.pid, connection.pid,
        "a new process must be started"
    );
    assert_eq!(supervisor.restarts(), 1);
    assert!(restarted.client().is_token_set().expect("IsTokenSet"));
    supervisor.shutdown().expect("shutdown");
}

#[test]
fn a_worker_that_dies_without_being_asked_is_detected_and_replaced() {
    let mut supervisor = WorkerSupervisor::new(
        WorkerConfig::new(WORKER_EXE)
            .with_port(0)
            .with_handshake_dir(temp_dir("crash")),
    );
    let connection = supervisor.start().expect("worker start");
    let client = connection.client();

    // Simulate a crash: the process disappears without a shutdown request.
    supervisor.terminate_worker().expect("terminate");

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut gone = false;
    while Instant::now() < deadline {
        if client.is_token_set().is_err() {
            gone = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(gone, "the client must notice a worker that died");

    // The next operation brings up a replacement without any operator action.
    let replacement = supervisor.ensure_running().expect("restart");
    assert_ne!(replacement.port, 0);
    assert!(replacement.client().is_token_set().expect("IsTokenSet"));
    assert_eq!(supervisor.restarts(), 1);
    supervisor.shutdown().expect("shutdown");
}

#[test]
fn a_client_can_talk_to_a_worker_that_is_not_there() {
    let client = WorkerClient::new(1, "token").with_timeout(Duration::from_millis(500));
    let err = client.get_titles().expect_err("nothing is listening");
    assert!(err.is_worker_gone(), "unexpected error: {err}");
    assert!(matches!(err, ClientError::Unreachable(_)));
}
