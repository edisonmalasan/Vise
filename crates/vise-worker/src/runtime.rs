//! Worker runtime: the shared state every route reads and writes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use vise_core::VISE_VERSION;
use vise_ipc::contract::{InitCommand, LogLevel, ServerBanner, WorkerInfo, LOG_BUFFER_LIMIT};
use vise_ipc::route::{Route, TOKEN_HEADER};
use vise_ipc::token::TokenGuard;
use vise_ipc::WORKER_NAME;

use crate::encode::EncodeService;
use crate::log::LogService;
use crate::platform::{self, Priority};
use crate::scan::ScanService;

/// Everything a request handler needs, shared across connection threads.
#[derive(Debug)]
pub struct WorkerRuntime {
    pub settings: WorkerSettings,
    token: TokenGuard,
    logs: Arc<LogService>,
    scan: ScanService,
    encode: EncodeService,
    init: Mutex<InitCommand>,
    shutdown: Arc<AtomicBool>,
}

/// How this worker process was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSettings {
    /// Port the parent asked for; `0` means the worker picks one.
    pub requested_port: u16,
    /// Port actually bound.
    pub port: u16,
    /// Session token supplied by the parent.
    pub token: String,
    /// Parent process id, when supplied.
    pub parent_pid: Option<u32>,
    /// File the worker writes its handshake to.
    pub handshake_file: Option<PathBuf>,
    /// Whether the worker exits when its parent does.
    pub monitor_parent: bool,
}

/// Why the worker could not start.
#[derive(Debug)]
pub enum StartupError {
    /// The worker was launched without the environment the parent sets.
    MissingEnvironment(&'static str),
    /// A supplied value could not be parsed.
    InvalidEnvironment { key: &'static str, value: String },
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupError::MissingEnvironment(key) => write!(
                f,
                "{key} is not set; the worker is started by the Vise desktop application and is not meant to run standalone"
            ),
            StartupError::InvalidEnvironment { key, value } => {
                write!(f, "{key} has an unusable value: {value}")
            }
        }
    }
}

impl std::error::Error for StartupError {}

impl WorkerSettings {
    /// Read the settings the parent process placed in the environment.
    pub fn from_env() -> Result<WorkerSettings, StartupError> {
        let port_raw = std::env::var(vise_ipc::supervisor::ENV_PORT)
            .map_err(|_| StartupError::MissingEnvironment(vise_ipc::supervisor::ENV_PORT))?;
        let requested_port =
            port_raw
                .parse::<u16>()
                .map_err(|_| StartupError::InvalidEnvironment {
                    key: vise_ipc::supervisor::ENV_PORT,
                    value: port_raw.clone(),
                })?;
        let token = std::env::var(vise_ipc::supervisor::ENV_TOKEN)
            .map_err(|_| StartupError::MissingEnvironment(vise_ipc::supervisor::ENV_TOKEN))?;
        let parent_pid = match std::env::var(vise_ipc::supervisor::ENV_PARENT_PID) {
            Ok(raw) => Some(
                raw.parse::<u32>()
                    .map_err(|_| StartupError::InvalidEnvironment {
                        key: vise_ipc::supervisor::ENV_PARENT_PID,
                        value: raw.clone(),
                    })?,
            ),
            Err(_) => None,
        };
        let handshake_file = std::env::var(vise_ipc::supervisor::ENV_HANDSHAKE)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        let monitor_parent = parent_pid.is_some();

        Ok(WorkerSettings {
            requested_port,
            port: requested_port,
            token,
            parent_pid,
            handshake_file,
            monitor_parent,
        })
    }

    /// Settings for a worker started directly from command-line arguments.
    ///
    /// A manually launched worker still needs a port and a token: an API without
    /// a token would be reachable by any local process.
    pub fn from_arguments(
        port: u16,
        token: impl Into<String>,
    ) -> Result<WorkerSettings, StartupError> {
        let token = token.into();
        if token.trim().is_empty() {
            return Err(StartupError::InvalidEnvironment {
                key: vise_ipc::supervisor::ENV_TOKEN,
                value: token,
            });
        }
        Ok(WorkerSettings {
            requested_port: port,
            port,
            token,
            parent_pid: None,
            handshake_file: None,
            monitor_parent: false,
        })
    }

    /// Settings for a directly launched worker, used by tests and by tooling.
    pub fn for_tests(port: u16, token: impl Into<String>) -> WorkerSettings {
        WorkerSettings {
            requested_port: port,
            port,
            token: token.into(),
            parent_pid: None,
            handshake_file: None,
            monitor_parent: false,
        }
    }
}

impl WorkerRuntime {
    /// Build a runtime around `settings`.
    pub fn new(settings: WorkerSettings) -> WorkerRuntime {
        let logs = Arc::new(LogService::with_limit(LOG_BUFFER_LIMIT));
        let scan = ScanService::new(logs.clone());
        let encode = EncodeService::new(logs.clone());
        let token = TokenGuard::new(settings.token.clone());
        logs.log(
            LogLevel::Info,
            format!(
                "{WORKER_NAME} {VISE_VERSION} starting on 127.0.0.1:{}",
                settings.port
            ),
        );
        logs.log(
            LogLevel::Info,
            format!("authentication uses the '{TOKEN_HEADER}' request header on every route"),
        );
        if settings.monitor_parent {
            logs.log(
                LogLevel::Info,
                format!(
                    "watching parent process {}; the worker exits when it disappears",
                    settings.parent_pid.unwrap_or(0)
                ),
            );
        }

        WorkerRuntime {
            settings,
            token,
            logs,
            scan,
            encode,
            init: Mutex::new(InitCommand::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Log service.
    pub fn logs(&self) -> &Arc<LogService> {
        &self.logs
    }

    /// Scan service.
    pub fn scan(&self) -> &ScanService {
        &self.scan
    }

    /// Encode service.
    pub fn encode(&self) -> &EncodeService {
        &self.encode
    }

    /// Token guard.
    pub fn token(&self) -> &TokenGuard {
        &self.token
    }

    /// Whether shutdown has been requested.
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Flag consulted by the accept loop.
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Request shutdown.
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Apply an init command to the log service and remember it.
    ///
    /// The scheduling priority is asked for on this process rather than being
    /// recorded and forgotten: a process cannot be created above the priority of
    /// whoever created it, so the worker is the only place the answer can be
    /// acted on. A platform that will not do it says so in the log instead of
    /// quietly leaving the inherited priority in place.
    pub fn apply_init(&self, init: &InitCommand) {
        self.logs.set_verbosity(init.log_verbosity);
        self.logs.set_disk_target(init.log_file_path());
        for note in init.unsupported_notes() {
            self.logs.warn(note);
        }
        if init.process_priority != 0 {
            let priority = Priority::of(init.priority());
            match platform::set_priority(std::process::id(), priority) {
                Ok(()) => self.logs.info(format!(
                    "the worker is running at {} priority",
                    priority.label()
                )),
                Err(e) => self.logs.warn(format!(
                    "the worker could not change its own priority to {}: {e}",
                    priority.label()
                )),
            }
        }
        // Where the worker runs is decided here, for the same reason the priority
        // is: the worker is the process that has to be placed, and a process can
        // restrict itself where nothing else can restrict it reliably. The
        // decision is logged as a sentence rather than as a number, because "it is
        // pinned" and "it was left alone" are the two things that can happen and
        // both of them are correct answers.
        let topology = vise_ipc::numa::topology();
        let placement = vise_ipc::numa::placement_for(
            &topology,
            init.numa(),
            vise_ipc::numa::WorkerPriority::of(init.priority()),
        );
        match vise_ipc::numa::apply(&placement.placement) {
            Ok(()) => self.logs.info(placement.reason.clone()),
            Err(e) => self.logs.warn(format!("{}: {e}", placement.reason)),
        }
        // The power scope is remembered rather than acted on: the hold belongs to
        // an encode, and the encode service is what knows when one starts and
        // stops.
        self.encode
            .set_power(init.prevent_sleep.then(|| init.power()));
        self.logs.info(format!(
            "initialised (log verbosity {}, hardware acceleration {}, parent monitoring {})",
            init.log_verbosity, init.enable_hardware_acceleration, self.settings.monitor_parent
        ));
        if let Ok(mut guard) = self.init.lock() {
            *guard = init.clone();
        }
    }

    /// The init command most recently applied.
    pub fn init(&self) -> InitCommand {
        self.init
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Loopback banner reply.
    pub fn banner(&self) -> ServerBanner {
        ServerBanner {
            name: WORKER_NAME.to_string(),
            version: VISE_VERSION.to_string(),
            protocol_version: vise_ipc::PROTOCOL_VERSION,
            pid: std::process::id(),
            message: "Vise worker is running and reachable on the loopback interface only."
                .to_string(),
            notice: format!(
                "Requests must carry a valid '{TOKEN_HEADER}' header. The worker never listens on a routable address."
            ),
        }
    }

    /// Runtime facts for `GetWorkerInfo`.
    pub fn info(&self) -> WorkerInfo {
        WorkerInfo {
            name: WORKER_NAME.to_string(),
            version: VISE_VERSION.to_string(),
            protocol_version: vise_ipc::PROTOCOL_VERSION,
            pid: std::process::id(),
            port: self.settings.port,
            token_set: self.token.is_set(),
            monitors_parent: self.settings.monitor_parent,
            routes: Route::ALL
                .iter()
                .map(|route| route.name().to_string())
                .filter(|name| !name.is_empty())
                .collect(),
            requires_token: true,
        }
    }
}

impl Drop for WorkerRuntime {
    /// Stop the encoder when the last worker handle goes away.
    ///
    /// A worker process must never leave ffmpeg running behind it, whether it
    /// is shutting down or being replaced after a crash.
    fn drop(&mut self) {
        self.encode.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_reports_facts_about_itself() {
        let runtime = WorkerRuntime::new(WorkerSettings::for_tests(0, "token"));
        let info = runtime.info();
        assert_eq!(info.protocol_version, vise_ipc::PROTOCOL_VERSION);
        assert!(info.token_set);
        assert!(info.requires_token);
        assert!(info.routes.contains(&"StartEncode".to_string()));
        assert!(!info.routes.contains(&String::new()));
    }

    #[test]
    fn shutdown_flag_round_trips() {
        let runtime = WorkerRuntime::new(WorkerSettings::for_tests(0, "token"));
        assert!(!runtime.shutdown_requested());
        runtime.request_shutdown();
        assert!(runtime.shutdown_requested());
        assert!(runtime.shutdown_flag().load(Ordering::SeqCst));
    }

    #[test]
    fn init_command_is_recorded_and_logged() {
        let runtime = WorkerRuntime::new(WorkerSettings::for_tests(0, "token"));
        let init = InitCommand {
            log_verbosity: 3,
            ..InitCommand::default()
        };
        runtime.apply_init(&init);
        assert_eq!(runtime.init().log_verbosity, 3);
        assert!(runtime
            .logs()
            .all()
            .iter()
            .any(|entry| entry.message.contains("initialised")));

        // A quiet verbosity is applied to the log service as well, so the
        // initialisation line itself is dropped once the caller asks for it.
        runtime.logs().reset();
        let quiet = InitCommand {
            log_verbosity: 0,
            ..InitCommand::default()
        };
        runtime.apply_init(&quiet);
        assert!(runtime
            .logs()
            .all()
            .iter()
            .all(|entry| entry.log_level == LogLevel::Error));
    }

    #[test]
    fn unsupported_init_switches_are_warned_about() {
        let runtime = WorkerRuntime::new(WorkerSettings::for_tests(0, "token"));
        runtime.logs().reset();
        let init = InitCommand {
            enable_lib_dvd_nav: true,
            ..InitCommand::default()
        };
        runtime.apply_init(&init);
        assert!(runtime
            .logs()
            .all()
            .iter()
            .any(|entry| entry.log_level == LogLevel::Warning));
    }

    #[test]
    fn banner_reports_the_loopback_contract() {
        let runtime = WorkerRuntime::new(WorkerSettings::for_tests(0, "token"));
        let banner = runtime.banner();
        assert!(banner.notice.contains(TOKEN_HEADER));
        assert!(banner.message.contains("loopback"));
    }
}
