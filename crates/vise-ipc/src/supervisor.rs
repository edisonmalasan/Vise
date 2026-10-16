//! Worker process lifecycle: start, handshake, monitoring, and restart.
//!
//! The supervisor is the parent-side half of the worker contract. It spawns the
//! worker binary with a fresh token and a loopback port, waits for the handshake,
//! and then hands out a [`WorkerClient`]. If the worker dies — for any reason,
//! including a crash inside an encoder subprocess — [`WorkerSupervisor::ensure_running`]
//! notices and starts a replacement, so a failed job never takes the desktop
//! application down with it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::api;
use crate::client::{ClientError, WorkerClient};
use crate::portable::PortableSettings;
use crate::token::generate_token;

/// Environment variable carrying the loopback port to the worker.
pub const ENV_PORT: &str = "VISE_WORKER_PORT";
/// Environment variable carrying the session token to the worker.
pub const ENV_TOKEN: &str = "VISE_WORKER_TOKEN";
/// Environment variable carrying the parent process id to the worker.
pub const ENV_PARENT_PID: &str = "VISE_WORKER_PARENT_PID";
/// Environment variable carrying the handshake file path to the worker.
pub const ENV_HANDSHAKE: &str = "VISE_WORKER_HANDSHAKE_FILE";

/// How many consecutive ports after the requested one the worker may try
/// before falling back to an ephemeral port.
pub const PORT_SEARCH_SPAN: u16 = 100;

/// Default time allowed for the worker to answer its first request.
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Why the supervisor could not provide a worker.
#[derive(Debug)]
pub enum SupervisorError {
    /// Portable settings disabled process isolation.
    IsolationDisabled(String),
    /// The worker executable is missing or not executable.
    WorkerMissing(PathBuf),
    /// The worker process could not be spawned.
    Spawn(String),
    /// The worker never completed its handshake.
    HandshakeTimeout { port: u16, detail: String },
    /// The worker exited during startup.
    WorkerExited { code: Option<i32> },
    /// The worker could not be shut down.
    Shutdown(String),
    /// A client call failed.
    Client(ClientError),
}

impl std::fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupervisorError::IsolationDisabled(reason) => {
                write!(f, "worker process isolation is unavailable: {reason}")
            }
            SupervisorError::WorkerMissing(path) => {
                write!(f, "worker executable not found: {}", path.display())
            }
            SupervisorError::Spawn(message) => write!(f, "worker could not be started: {message}"),
            SupervisorError::HandshakeTimeout { port, detail } => write!(
                f,
                "worker did not complete its handshake on port {port}: {detail}"
            ),
            SupervisorError::WorkerExited { code } => {
                write!(f, "worker exited during startup with code {code:?}")
            }
            SupervisorError::Shutdown(message) => write!(f, "worker shutdown failed: {message}"),
            SupervisorError::Client(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for SupervisorError {}

impl From<ClientError> for SupervisorError {
    fn from(err: ClientError) -> Self {
        SupervisorError::Client(err)
    }
}

/// What the worker writes to the handshake file once it is listening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Handshake {
    pub name: String,
    pub protocol_version: u32,
    pub pid: u32,
    /// Port the worker actually bound, which may differ from the request.
    pub port: u16,
    pub token: String,
}

/// Live connection details for one worker process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConnection {
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub restarts: u32,
}

impl WorkerConnection {
    /// Client bound to this connection.
    pub fn client(&self) -> WorkerClient {
        WorkerClient::new(self.port, self.token.clone())
    }
}

/// How to start a worker.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Path to the worker binary.
    pub executable: PathBuf,
    /// Port to request; `0` lets the worker choose.
    pub port: u16,
    /// Token to hand to the worker; `None` generates one per start.
    pub token: Option<String>,
    /// Directory for the handshake file; a temporary file is used when absent.
    pub handshake_dir: Option<PathBuf>,
    /// How long to wait for the handshake.
    pub handshake_timeout: Duration,
    /// Whether process isolation is permitted at all.
    pub isolation_enabled: bool,
    /// Reason recorded when isolation is refused.
    pub isolation_disabled_reason: Option<String>,
}

impl WorkerConfig {
    /// Configuration for `executable` with defaults for everything else.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        WorkerConfig {
            executable: executable.into(),
            port: 0,
            token: None,
            handshake_dir: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            isolation_enabled: true,
            isolation_disabled_reason: None,
        }
    }

    /// Request a specific port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Use a caller-supplied token instead of generating one.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Honour a portable-mode decision about process isolation.
    pub fn with_portable_settings(mut self, settings: &PortableSettings) -> Self {
        self.isolation_enabled = settings.should_isolate();
        if !settings.should_isolate() {
            self.isolation_disabled_reason = Some(settings.isolation_disabled_reason());
        }
        self
    }

    /// Place the handshake file in `dir`.
    pub fn with_handshake_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.handshake_dir = Some(dir.into());
        self
    }

    /// Override the handshake timeout.
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }
}

struct RunningWorker {
    child: Child,
    /// Held open for the lifetime of the worker: closing it signals the worker
    /// that its parent has gone.
    stdin: Option<ChildStdin>,
    handshake_file: PathBuf,
    connection: WorkerConnection,
}

/// Owns at most one worker process and knows how to replace it.
pub struct WorkerSupervisor {
    config: WorkerConfig,
    worker: Option<RunningWorker>,
    restarts: u32,
    last_failure: Option<String>,
}

impl std::fmt::Debug for WorkerSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerSupervisor")
            .field("executable", &self.config.executable)
            .field("running", &self.worker.is_some())
            .field("restarts", &self.restarts)
            .field("last_failure", &self.last_failure)
            .finish()
    }
}

impl WorkerSupervisor {
    /// Create a supervisor without starting anything.
    pub fn new(config: WorkerConfig) -> Self {
        WorkerSupervisor {
            config,
            worker: None,
            restarts: 0,
            last_failure: None,
        }
    }

    /// Number of workers started after the first one.
    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// Reason the previous worker stopped, when it did.
    pub fn last_failure(&self) -> Option<&str> {
        self.last_failure.as_deref()
    }

    /// Connection details for the running worker, if any.
    pub fn connection(&self) -> Option<WorkerConnection> {
        self.worker.as_ref().map(|w| w.connection.clone())
    }

    /// Client for the running worker.
    pub fn client(&self) -> Option<WorkerClient> {
        self.worker.as_ref().map(|w| w.connection.client())
    }

    /// Whether a worker process is currently running.
    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }

    /// Whether the worker answered its most recent request.
    pub fn is_alive(&mut self) -> bool {
        match self.client() {
            Some(client) => !client.ping().is_err(),
            None => false,
        }
    }

    /// Start a worker if one is not already running.
    pub fn start(&mut self) -> Result<WorkerConnection, SupervisorError> {
        if let Some(existing) = self.connection() {
            return Ok(existing);
        }
        self.spawn_worker(false)
    }

    /// Return a client for a healthy worker, restarting it if necessary.
    ///
    /// This is the crash-recovery entry point: call it before any operation
    /// that must not fail because the worker died underneath the UI.
    pub fn ensure_running(&mut self) -> Result<WorkerConnection, SupervisorError> {
        if let Some(worker) = self.worker.as_mut() {
            match worker.child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code();
                    self.last_failure = Some(format!("worker exited with code {code:?}"));
                }
                Ok(None) => {
                    // Process is alive; confirm it still answers requests.
                    if api::wait_until_ready(
                        &worker.connection.client(),
                        Duration::from_millis(250),
                    ) {
                        return Ok(worker.connection.clone());
                    }
                    self.last_failure = Some("worker stopped answering requests".to_string());
                }
                Err(e) => {
                    self.last_failure = Some(format!("worker status unavailable: {e}"));
                }
            }
            self.worker = None;
            self.restarts += 1;
        }
        self.spawn_worker(true)
    }

    /// Force-terminate the worker without a graceful shutdown.
    ///
    /// Used to recover from an unresponsive worker; the next
    /// [`WorkerSupervisor::ensure_running`] starts a replacement.
    pub fn terminate_worker(&mut self) -> Result<(), SupervisorError> {
        if let Some(mut worker) = self.worker.take() {
            drop(worker.stdin.take());
            let _ = worker.child.kill();
            let _ = worker.child.wait();
            let _ = std::fs::remove_file(&worker.handshake_file);
            self.restarts += 1;
        }
        Ok(())
    }

    /// Ask the worker to shut down and wait for the process to exit.
    pub fn shutdown(&mut self) -> Result<(), SupervisorError> {
        let worker = match self.worker.take() {
            Some(worker) => worker,
            None => return Ok(()),
        };

        let client = worker.connection.client();
        let graceful = client
            .request("GET", "/Shutdown", b"")
            .map(|reply| reply.status == 200)
            .unwrap_or(false);

        // Closing the parent's pipe handle is itself a shutdown signal.
        drop(worker.stdin);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut child = worker.child;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let _ = std::fs::remove_file(&worker.handshake_file);
                    return Ok(());
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&worker.handshake_file);
                    if graceful {
                        return Ok(());
                    }
                    return Err(SupervisorError::Shutdown(format!(
                        "worker on port {} did not exit after a shutdown request",
                        worker.connection.port
                    )));
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&worker.handshake_file);
                    return Err(SupervisorError::Shutdown(e.to_string()));
                }
            }
        }
    }

    fn spawn_worker(&mut self, is_restart: bool) -> Result<WorkerConnection, SupervisorError> {
        if !self.config.isolation_enabled {
            let reason = self
                .config
                .isolation_disabled_reason
                .clone()
                .unwrap_or_else(|| "process isolation is switched off".to_string());
            return Err(SupervisorError::IsolationDisabled(reason));
        }
        if !self.config.executable.is_file() {
            return Err(SupervisorError::WorkerMissing(
                self.config.executable.clone(),
            ));
        }

        let token = self.config.token.clone().unwrap_or_else(generate_token);
        let handshake_file = self.handshake_path();
        let _ = std::fs::remove_file(&handshake_file);

        let mut child = Command::new(&self.config.executable)
            .env(ENV_PORT, self.config.port.to_string())
            .env(ENV_TOKEN, &token)
            .env(ENV_PARENT_PID, std::process::id().to_string())
            .env(ENV_HANDSHAKE, &handshake_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| SupervisorError::Spawn(e.to_string()))?;

        let stdin = child.stdin.take();
        let pid = child.id();

        let handshake = self.await_handshake(&mut child, &handshake_file);
        let handshake = match handshake {
            Ok(handshake) => handshake,
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&handshake_file);
                return Err(err);
            }
        };

        let connection = WorkerConnection {
            port: handshake.port,
            token: handshake.token,
            pid: handshake.pid.max(pid),
            restarts: self.restarts,
        };

        // The handshake proves the socket is open, but confirm the worker
        // actually answers an authenticated request before declaring success.
        if !api::wait_until_ready(&connection.client(), Duration::from_secs(5)) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&handshake_file);
            return Err(SupervisorError::HandshakeTimeout {
                port: connection.port,
                detail: "worker accepted the connection but rejected requests".into(),
            });
        }

        if is_restart {
            self.last_failure = None;
        }
        self.worker = Some(RunningWorker {
            child,
            stdin,
            handshake_file,
            connection: connection.clone(),
        });
        Ok(connection)
    }

    fn handshake_path(&self) -> PathBuf {
        let name = format!(
            "vise-worker-handshake-{}-{}.json",
            std::process::id(),
            self.restarts
        );
        match &self.config.handshake_dir {
            Some(dir) => dir.join(name),
            None => std::env::temp_dir().join(name),
        }
    }

    fn await_handshake(
        &self,
        child: &mut Child,
        handshake_file: &Path,
    ) -> Result<Handshake, SupervisorError> {
        let deadline = Instant::now() + self.config.handshake_timeout;
        loop {
            if let Ok(text) = std::fs::read_to_string(handshake_file) {
                if let Ok(handshake) = serde_json::from_str::<Handshake>(&text) {
                    return Ok(handshake);
                }
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(SupervisorError::WorkerExited {
                    code: status.code(),
                });
            }
            if Instant::now() >= deadline {
                return Err(SupervisorError::HandshakeTimeout {
                    port: self.config.port,
                    detail: "no handshake file was written".into(),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for WorkerSupervisor {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            drop(worker.stdin);
            let mut child = worker.child;
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    Err(_) => {
                        let _ = child.kill();
                        break;
                    }
                }
            }
            let _ = std::fs::remove_file(&worker.handshake_file);
        }
    }
}

/// Read a handshake file, for callers that start the worker themselves.
pub fn read_handshake(path: impl AsRef<Path>) -> Option<Handshake> {
    let text = std::fs::read_to_string(path.as_ref()).ok()?;
    serde_json::from_str(&text).ok()
}

/// Parent-liveness probe used by the worker's own monitor and by tests.
///
/// On Windows this opens the process with query rights and reads its exit code;
/// elsewhere it relies on the `kill(pid, 0)` existence check.
pub fn parent_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
            fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
            fn CloseHandle(handle: *mut c_void) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // The process is gone (or was never ours to query).
                return false;
            }
            let mut code: u32 = 0;
            let ok = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            ok != 0 && code == STILL_ACTIVE
        }
    }
    #[cfg(not(windows))]
    {
        extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        const SIGNAL_NONE: i32 = 0;
        unsafe { kill(pid as i32, SIGNAL_NONE) == 0 }
    }
}

/// Placeholder helper kept for symmetry with the worker's stdin pipe protocol.
pub fn pipe_is_closed(read: &mut dyn Read) -> bool {
    let mut buffer = [0u8; 64];
    match read.read(&mut buffer) {
        Ok(0) => true,
        Ok(_) => false,
        Err(_) => true,
    }
}

/// Write the handshake file from the worker side.
pub fn write_handshake(path: impl AsRef<Path>, handshake: &Handshake) -> std::io::Result<()> {
    let body = serde_json::to_vec_pretty(handshake)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut file = std::fs::File::create(path.as_ref())?;
    file.write_all(&body)?;
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-ipc-supervisor-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn refuses_to_start_when_isolation_is_disabled() {
        let config =
            WorkerConfig::new("vise-worker.exe").with_portable_settings(&PortableSettings {
                process_isolation_enabled: false,
                portable_mode_active: true,
                storage_directory: None,
                entries: Default::default(),
            });
        let mut supervisor = WorkerSupervisor::new(config);
        match supervisor.start() {
            Err(SupervisorError::IsolationDisabled(reason)) => {
                assert!(reason.contains("portable"));
            }
            other => panic!("expected isolation refusal, got {other:?}"),
        }
        assert!(!supervisor.is_running());
    }

    #[test]
    fn reports_a_missing_worker_executable() {
        let dir = temp_dir("missing");
        let mut supervisor = WorkerSupervisor::new(WorkerConfig::new(dir.join("absent.exe")));
        assert!(matches!(
            supervisor.start(),
            Err(SupervisorError::WorkerMissing(_))
        ));
    }

    #[test]
    fn detects_the_live_parent_process() {
        assert!(parent_is_alive(std::process::id()));
    }

    #[test]
    fn handshake_files_round_trip() {
        let dir = temp_dir("handshake");
        let path = dir.join("handshake.json");
        let handshake = Handshake {
            name: "vise-worker".into(),
            protocol_version: crate::PROTOCOL_VERSION,
            pid: 4242,
            port: 51234,
            token: "abc".into(),
        };
        write_handshake(&path, &handshake).expect("write");
        assert_eq!(read_handshake(&path), Some(handshake));
    }

    #[test]
    fn starting_twice_reuses_the_running_worker() {
        let dir = temp_dir("reuse");
        let config = WorkerConfig::new(dir.join("absent.exe"));
        let mut supervisor = WorkerSupervisor::new(config);
        assert!(supervisor.start().is_err());
        assert!(!supervisor.is_running());
        assert_eq!(supervisor.client(), None);
        assert_eq!(supervisor.restarts(), 0);
    }

    #[test]
    fn shutdown_is_a_no_op_without_a_worker() {
        let mut supervisor = WorkerSupervisor::new(WorkerConfig::new("vise-worker"));
        supervisor.shutdown().expect("no-op shutdown");
    }
}
