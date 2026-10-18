//! Request and response payloads for the worker JSON API.
//!
//! Field names are PascalCase so that a payload can be read on its own without
//! a schema lookup, and so the shape matches the JSON-first control plane the
//! desktop application already speaks. Optional members are omitted when empty
//! rather than serialised as `null`, so a response only carries the section that
//! actually applies.

use std::fmt;

use serde::{Deserialize, Serialize};
use vise_core::JobSpec;

use crate::timespan::TimeSpan;

/// Maximum number of log entries retained in memory before the oldest are
/// dropped. Bounds worker memory for callers that poll logs aggressively.
pub const LOG_BUFFER_LIMIT: usize = 20_000;

// ---------------------------------------------------------------------------
// Shared shapes
// ---------------------------------------------------------------------------

/// Result of a mutating worker command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct CommandResult {
    /// Whether the command was accepted and completed as requested.
    pub was_successful: bool,
    /// Human-readable failure reason; empty on success.
    pub error: String,
}

impl Default for CommandResult {
    fn default() -> Self {
        CommandResult {
            was_successful: true,
            error: String::new(),
        }
    }
}

impl CommandResult {
    /// Successful result with no error text.
    pub fn ok() -> Self {
        Self::default()
    }

    /// Failed result carrying `error`.
    pub fn failed(error: impl Into<String>) -> Self {
        CommandResult {
            was_successful: false,
            error: error.into(),
        }
    }

    /// Failed result describing an operation this build does not implement.
    pub fn unsupported(what: impl fmt::Display) -> Self {
        CommandResult::failed(format!("{what} is not supported by this worker build"))
    }
}

/// Severity attached to a worker log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warning,
    Error,
}

impl LogLevel {
    /// Numeric verbosity rank, matching the `LogVerbosity` ordering where a
    /// higher number means more detail.
    pub fn rank(self) -> u8 {
        match self {
            LogLevel::Error => 0,
            LogLevel::Warning => 1,
            LogLevel::Info => 2,
            LogLevel::Debug => 3,
        }
    }

    /// Every level, least to most verbose.
    ///
    /// One list, so the filter control, the row labels, and the preference that
    /// chooses a level are all built from the same order and cannot disagree
    /// about which way round it is.
    pub const ALL: [LogLevel; 4] = [
        LogLevel::Error,
        LogLevel::Warning,
        LogLevel::Info,
        LogLevel::Debug,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "error",
        }
    }

    /// The name shown on a filter option and beside a row.
    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warning => "Warning",
            LogLevel::Error => "Error",
        }
    }

    /// Read a level the interface layer sent.
    ///
    /// The wire spelling is the one the contract serialises, and the lower-case
    /// identifier is accepted as well because that is what the preference and the
    /// page both send.
    pub fn parse(raw: &str) -> Option<Self> {
        let wanted = raw.trim().to_ascii_lowercase();
        LogLevel::ALL.into_iter().find(|level| level.id() == wanted)
    }
}

/// One worker log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LogMessage {
    pub log_level: LogLevel,
    pub message: String,
    /// Monotonic index assigned when the entry was recorded.
    pub index: usize,
}

/// Progress states reported by the polling routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkState {
    /// Nothing has been started, or the worker cannot tell.
    #[default]
    Unknown,
    /// A scan finished.
    ScanDone,
    /// A scan or encode is in flight.
    Working,
    /// The operation reached a terminal state.
    WorkDone,
}

/// Terminal status of a finished operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct WorkDone {
    /// Zero on success; a non-zero code identifies the failure class.
    pub error: i32,
    /// Whether the operation produced its intended result.
    pub was_successful: bool,
    /// Failure detail, when the operation did not succeed.
    pub message: String,
}

/// Error codes reported in [`WorkDone::error`].
pub mod error_codes {
    /// The operation finished successfully.
    pub const NONE: i32 = 0;
    /// The caller cancelled the operation.
    pub const CANCELLED: i32 = 1;
    /// The operation failed for a media or tool reason.
    pub const FAILED: i32 = 2;
    /// The worker rejected the request before starting.
    pub const INVALID: i32 = 3;
}

/// Scan section of a progress reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanProgress {
    /// Fraction complete in the range 0.0–1.0.
    pub progress: f64,
    /// Paths already scanned.
    pub completed: usize,
    /// Paths submitted to the scan.
    pub total: usize,
    /// Source currently being probed.
    pub current_path: String,
    /// Titles found so far.
    pub title_count: usize,
}

/// Encode section of a progress reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct EncodeProgress {
    /// Fraction complete in the range 0.0–1.0.
    pub progress: f64,
    /// Seconds of encoded media produced so far.
    pub encoded_seconds: f64,
    /// Estimated seconds remaining, when a duration is known.
    pub eta_seconds: Option<f64>,
    /// Whether the encode is currently suspended.
    pub paused: bool,
    /// Output path being written.
    pub output_path: String,
    /// Current pipeline stage description.
    pub stage: String,
}

/// Reply from `PollScanProgress` and `PollEncodeProgress`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProgressReport {
    pub state: WorkState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan: Option<ScanProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working: Option<EncodeProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_done: Option<WorkDone>,
}

impl ProgressReport {
    /// Report with no activity recorded yet.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Build a `WORKDONE` report.
    pub fn done(done: WorkDone) -> Self {
        ProgressReport {
            state: WorkState::WorkDone,
            work_done: Some(done),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Init contract
// ---------------------------------------------------------------------------

/// Runtime configuration sent with the first scan or encode command.
///
/// This is the worker's initialise step: it records logging preferences and
/// feature switches for the current session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct InitCommand {
    /// Verbosity rank; entries above this level are discarded.
    pub log_verbosity: u8,
    /// Directory for on-disk logs; empty disables disk logging.
    pub log_directory: String,
    /// Log file name inside `LogDirectory`.
    pub log_file: String,
    pub enable_disk_logging: bool,
    pub enable_hardware_acceleration: bool,
    /// Reserved for optical-disc navigation; not implemented in this build.
    pub enable_lib_dvd_nav: bool,
    /// Keep the worker alive after the parent process exits. Intended for
    /// attach-and-debug sessions; the parent monitor ignores this only when
    /// explicitly requested.
    pub allow_disconnected_worker: bool,
    /// File extensions skipped when scanning directories.
    pub exclude_extension_list: Vec<String>,
    /// Source handling mode; zero selects single-source scanning.
    pub mode: i32,
    /// Hardware decode preference; zero selects automatic software fallback.
    pub hw_decode: i32,
    /// Name of the hardware device to decode with when `hw_decode` names one.
    pub hw_decode_device: String,
    /// Scheduling priority the worker asks for on itself: zero is inherited,
    /// negative is below normal, positive is above it.
    ///
    /// Sent rather than applied by the parent because the worker is the process
    /// that needs the priority: it is the one competing with the machine for
    /// the encoder, and a parent cannot raise a priority it does not hold.
    pub process_priority: i32,
    /// Keep the machine awake for as long as an encode is running.
    ///
    /// Sent rather than decided by the window because the worker is the process
    /// that outlives the encode thread: a hold requested by the window would be
    /// released by whichever thread finished last, and a hold that outlived its
    /// encode is a machine that will not sleep tonight.
    pub prevent_sleep: bool,
    /// What the machine should keep doing while it is held awake: zero keeps the
    /// machine running and lets the screen go dark, one keeps both.
    pub power_scope: i32,
    /// Where the worker should run on a machine with several processor groups:
    /// negative leaves the placement to the operating system, zero lets the
    /// worker decide from the layout, and a positive number names a group.
    pub numa_mode: i32,
    /// Keep duplicate title entries instead of collapsing them.
    pub keep_duplicate_titles: bool,
}

impl Default for InitCommand {
    fn default() -> Self {
        InitCommand {
            log_verbosity: 2,
            log_directory: String::new(),
            log_file: String::new(),
            enable_disk_logging: false,
            enable_hardware_acceleration: false,
            enable_lib_dvd_nav: false,
            allow_disconnected_worker: false,
            exclude_extension_list: Vec::new(),
            mode: 0,
            hw_decode: 0,
            hw_decode_device: String::new(),
            process_priority: 0,
            prevent_sleep: true,
            power_scope: 0,
            numa_mode: 0,
            keep_duplicate_titles: false,
        }
    }
}

impl InitCommand {
    /// Capabilities this build cannot honour, as human-readable notes.
    pub fn unsupported_notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.enable_lib_dvd_nav {
            notes.push(
                "optical-disc navigation (EnableLibDvdNav) is not implemented in this build"
                    .to_string(),
            );
        }
        if self.mode != 0 {
            notes.push(format!(
                "source mode {} is not implemented; scanning single files",
                self.mode
            ));
        }
        if !self.hw_decode_device.trim().is_empty() && self.hw_decode == 0 {
            notes.push(
                "a hardware decode device was named while hardware decoding is off; decoding in software"
                    .to_string(),
            );
        }
        if !(-2..=2).contains(&self.process_priority) {
            notes.push(format!(
                "process priority {} is outside the range this build applies; keeping the inherited priority",
                self.process_priority
            ));
        }
        notes
    }

    /// Whether a named hardware decode device should be used.
    ///
    /// Hardware decoding has to be asked for as well as named: a device name on
    /// its own says which one, not whether any is wanted, and the two are
    /// separate answers in the interface.
    pub fn uses_hardware_decode(&self) -> bool {
        self.hw_decode != 0
    }

    /// The decode device to name, which is nothing when none was asked for.
    pub fn decode_device(&self) -> Option<&str> {
        let device = self.hw_decode_device.trim();
        if !self.uses_hardware_decode() || device.is_empty() {
            return None;
        }
        Some(device)
    }

    /// The scheduling priority to ask for, clamped to what the worker applies.
    pub fn priority(&self) -> i32 {
        self.process_priority.clamp(-2, 2)
    }

    /// What the machine should keep doing while it is held awake.
    ///
    /// Read through [`crate::power::PowerScope`] rather than from a second table,
    /// so the scope the preferences stored and the scope the worker holds are one
    /// value with one set of names.
    pub fn power(&self) -> crate::power::PowerScope {
        if self.power_scope <= 0 {
            crate::power::PowerScope::System
        } else {
            crate::power::PowerScope::SystemAndDisplay
        }
    }

    /// Where the worker should run, as the preferences asked.
    pub fn numa(&self) -> crate::numa::NumaMode {
        crate::numa::NumaMode::read(self.numa_mode)
    }

    /// Resolve the log file path when disk logging is enabled.
    pub fn log_file_path(&self) -> Option<std::path::PathBuf> {
        if !self.enable_disk_logging || self.log_directory.trim().is_empty() {
            return None;
        }
        let dir = std::path::Path::new(self.log_directory.trim());
        let name = if self.log_file.trim().is_empty() {
            format!("vise-worker-{}.log", std::process::id())
        } else {
            self.log_file.trim().to_string()
        };
        Some(dir.join(name))
    }

    /// Resolve the folder per-encode logs are written into.
    ///
    /// The session log and the per-encode logs share one folder, which is what
    /// makes "open the folder and look at the last thing that happened" a thing
    /// a person can do. The folder is resolved even when disk logging is
    /// switched off, because a session log file name and a per-encode log
    /// folder are both asked for by the same preference and a window that
    /// resolved one and not the other would produce two answers to one question.
    pub fn log_folder(&self) -> Option<std::path::PathBuf> {
        let dir = self.log_directory.trim();
        if dir.is_empty() {
            return None;
        }
        Some(std::path::PathBuf::from(dir))
    }
}

// ---------------------------------------------------------------------------
// Scan contract
// ---------------------------------------------------------------------------

/// Body of `StartScan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanCommand {
    pub initialise_command: InitCommand,
    /// Files or directories to scan.
    pub paths: Vec<String>,
    /// Preview images to extract per title.
    pub preview_count: u32,
    /// Shortest acceptable title duration.
    pub min_duration: TimeSpan,
    /// Longest acceptable title duration.
    pub max_duration: TimeSpan,
    /// Restrict the scan to one reported title index; zero asks for every
    /// title, which is what a source selection normally wants.
    pub title_index: i32,
    /// File names skipped during a directory scan.
    pub file_exclusion_list: Vec<String>,
    /// Hardware decode preference for probing.
    pub hw_decode: i32,
}

impl Default for ScanCommand {
    fn default() -> Self {
        ScanCommand {
            initialise_command: InitCommand::default(),
            paths: Vec::new(),
            preview_count: 10,
            min_duration: TimeSpan::ZERO,
            max_duration: TimeSpan::ZERO,
            title_index: 0,
            file_exclusion_list: Vec::new(),
            hw_decode: 0,
        }
    }
}

/// A title as reported by `GetTitles`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanTitle {
    pub index: u32,
    pub name: String,
    pub container: String,
    pub duration_seconds: f64,
    pub video_codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub angle_count: u32,
    pub crop: [u32; 4],
    pub audio_list: Vec<ScanAudioTrack>,
    pub subtitle_list: Vec<ScanSubtitleTrack>,
    pub chapter_list: Vec<ScanChapter>,
    pub meta_data: std::collections::BTreeMap<String, String>,
    /// What kind of source this title came from: `file`, `folder`, `dvd`,
    /// `bluray`, or `iso`. Empty means a plain file.
    pub source_kind: String,
    /// Dolby Vision profile reported in the stream side data; zero when none.
    pub dolby_vision_profile: u32,
    /// Whether the stream carries a Dolby Vision RPU.
    pub dolby_vision_rpu: bool,
    /// Whether the stream carries HDR10+ dynamic metadata.
    pub hdr10_plus: bool,
}

/// One audio track inside a [`ScanTitle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanAudioTrack {
    pub track_number: u32,
    pub codec_name: String,
    pub channel_count: u32,
    pub channel_layout: String,
    pub sample_rate: u32,
    pub bit_rate: u64,
    pub language: String,
    pub language_code: String,
    pub description: String,
    /// Whether this track's language differs from the source's primary one,
    /// which is what a foreign-audio scan looks for.
    pub foreign_language: bool,
}

/// One subtitle track inside a [`ScanTitle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanSubtitleTrack {
    pub track_number: u32,
    pub codec_name: String,
    pub language: String,
    pub language_code: String,
    pub description: String,
}

/// One chapter inside a [`ScanTitle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScanChapter {
    pub index: u32,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub title: String,
}

/// Reply from `GetTitles`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct TitlesResult {
    /// Index of the main feature title.
    pub main_feature: u32,
    pub title_list: Vec<ScanTitle>,
}

// ---------------------------------------------------------------------------
// Encode contract
// ---------------------------------------------------------------------------

/// Body of `StartEncode`.
///
/// `EncodeJob` is the Vise job model itself, so its members keep the snake_case
/// names the desktop application already sends over the UI bridge. The command
/// envelope around it is PascalCase like every other route payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct EncodeCommand {
    pub initialise_command: InitCommand,
    /// The job to run. Validated by the worker before the pipeline starts.
    pub encode_job: JobSpec,
}

/// Reply from `GetWorkerInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorkerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: u32,
    pub pid: u32,
    /// Port the worker is actually listening on.
    pub port: u16,
    /// Whether a token is registered.
    pub token_set: bool,
    /// Whether the worker will exit when its parent does.
    pub monitors_parent: bool,
    /// Routes this worker implements.
    pub routes: Vec<String>,
    /// Whether the worker refuses to leave its parent process.
    pub requires_token: bool,
}

// ---------------------------------------------------------------------------
// Preview contract
// ---------------------------------------------------------------------------

/// Body of `GetPreview`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PreviewCommand {
    /// Source to sample; empty means the most recently scanned source.
    pub path: String,
    /// Title index within the source.
    pub title_index: u32,
    /// Sample position in seconds.
    pub time_seconds: f64,
    /// Output file for the extracted image. When empty the image is returned
    /// inline instead of being written.
    pub output_file: String,
}

impl Default for PreviewCommand {
    fn default() -> Self {
        PreviewCommand {
            path: String::new(),
            title_index: 0,
            time_seconds: 0.0,
            output_file: String::new(),
        }
    }
}

/// Reply from `GetPreview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct PreviewResult {
    pub was_successful: bool,
    pub error: String,
    /// Path of the written image, when one was requested.
    pub output_file: String,
    /// Base64-encoded image bytes, when the image was returned inline.
    pub image_base64: String,
    /// Byte length of `ImageBase64` before encoding.
    pub image_bytes: u64,
}

// ---------------------------------------------------------------------------
// Logging contract
// ---------------------------------------------------------------------------

/// Reply from `GetLogMessagesFromIndex`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct LogMessagesFromIndex {
    pub messages: Vec<LogMessage>,
    /// Index to pass to the next call.
    pub next_index: usize,
    /// Whether older entries were dropped before `next_index`.
    pub truncated: bool,
    /// Which worker this page came from, and which buffer it is.
    ///
    /// A caller that has been paging one worker cannot page another by index
    /// alone: a worker that was replaced starts numbering at zero again, so its
    /// first page looks exactly like a worker that simply has nothing new to
    /// say. This is what tells the two apart. It is `pid:resets`, so it changes
    /// when the process changes and again whenever the buffer is cleared.
    pub session: String,
}

/// Reply from the loopback banner route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServerBanner {
    pub name: String,
    pub version: String,
    pub protocol_version: u32,
    pub pid: u32,
    pub message: String,
    /// Loopback-only notice included for operators.
    pub notice: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_result_omits_empty_error() {
        let json = serde_json::to_string(&CommandResult::ok()).unwrap();
        assert_eq!(json, "{\"WasSuccessful\":true,\"Error\":\"\"}");

        let json = serde_json::to_string(&CommandResult::failed("bad job")).unwrap();
        assert_eq!(json, "{\"WasSuccessful\":false,\"Error\":\"bad job\"}");
    }

    #[test]
    fn work_state_serializes_uppercase() {
        assert_eq!(
            serde_json::to_string(&WorkState::ScanDone).unwrap(),
            "\"SCANDONE\""
        );
        assert_eq!(
            serde_json::to_string(&WorkState::WorkDone).unwrap(),
            "\"WORKDONE\""
        );
        assert_eq!(
            serde_json::to_string(&WorkState::Unknown).unwrap(),
            "\"UNKNOWN\""
        );
    }

    #[test]
    fn progress_report_omits_absent_sections() {
        let report = ProgressReport::done(WorkDone {
            error: error_codes::NONE,
            was_successful: true,
            message: String::new(),
        });
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"State\":\"WORKDONE\""));
        assert!(json.contains("\"WasSuccessful\":true"));
        assert!(!json.contains("\"Scan\""));
        assert!(!json.contains("\"Working\""));

        let empty = serde_json::to_string(&ProgressReport::unknown()).unwrap();
        assert_eq!(empty, "{\"State\":\"UNKNOWN\"}");
    }

    #[test]
    fn scan_command_round_trips_with_time_spans() {
        let cmd = ScanCommand {
            paths: vec!["a.mkv".into()],
            min_duration: TimeSpan::parse("00:00:10").unwrap(),
            max_duration: TimeSpan::parse("2.00:00:00").unwrap(),
            preview_count: 5,
            ..ScanCommand::default()
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"MinDuration\":\"00:00:10\""));
        assert!(json.contains("\"MaxDuration\":\"2.00:00:00\""));
        let back: ScanCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn scan_command_rejects_malformed_duration_with_clear_error() {
        let payload = r#"{"Paths":["a.mkv"],"MinDuration":"99:59:59"}"#;
        let err = serde_json::from_str::<ScanCommand>(payload).unwrap_err();
        assert!(
            err.to_string().contains("between 00 and 23"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn encode_command_accepts_partial_job_and_leaves_it_to_validation() {
        // A partial job decodes; rejecting it is the worker's validation step,
        // not a deserialization failure.
        let payload = r#"{"EncodeJob":{"input_path":"in.mkv"}}"#;
        let cmd: EncodeCommand = serde_json::from_str(payload).expect("decode");
        assert_eq!(cmd.encode_job.input_path, "in.mkv");
        assert_eq!(cmd.encode_job.container, vise_core::ContainerFormat::Mp4);
        assert_eq!(cmd.initialise_command, InitCommand::default());
        let report = crate::validation::validate_job(
            &cmd.encode_job,
            &crate::validation::MachineFacts::unknown(),
        );
        assert!(!report.is_valid(), "the job has no output destination yet");
        assert!(
            report.summary().contains("OutputPath"),
            "unexpected issues: {report}"
        );
    }

    #[test]
    fn encode_command_keeps_the_core_job_field_names() {
        // The job model is shared with the desktop application, so its members
        // are snake_case inside the PascalCase command envelope.
        let json = serde_json::to_string(&EncodeCommand::default()).expect("encode");
        assert!(json.contains("\"InitialiseCommand\""), "{json}");
        assert!(json.contains("\"EncodeJob\""), "{json}");
        assert!(json.contains("\"input_path\""), "{json}");
    }

    #[test]
    fn init_command_reports_unsupported_switches() {
        let mut init = InitCommand::default();
        assert!(init.unsupported_notes().is_empty());
        init.enable_lib_dvd_nav = true;
        init.mode = 4;
        let notes = init.unsupported_notes();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn init_command_resolves_disk_log_path() {
        let mut init = InitCommand::default();
        assert!(init.log_file_path().is_none());
        init.enable_disk_logging = true;
        assert!(init.log_file_path().is_none(), "directory still required");
        init.log_directory = "/tmp/vise-logs".into();
        let path = init.log_file_path().expect("path");
        assert!(path.to_string_lossy().contains("vise-logs"));
        assert_eq!(
            init.log_folder(),
            Some(std::path::PathBuf::from("/tmp/vise-logs"))
        );
    }

    #[test]
    fn init_command_resolves_the_per_encode_log_folder() {
        let mut init = InitCommand::default();
        assert!(init.log_folder().is_none(), "no directory names no folder");
        init.log_directory = "  /tmp/vise-logs  ".into();
        assert_eq!(
            init.log_folder(),
            Some(std::path::PathBuf::from("/tmp/vise-logs")),
            "the folder does not depend on whether the session log is enabled"
        );
    }

    #[test]
    fn a_log_page_names_the_worker_it_came_from() {
        // The field is what lets a caller notice that the worker under it was
        // replaced, so it has to be present and has to survive the round trip.
        let page = LogMessagesFromIndex {
            messages: vec![LogMessage {
                log_level: LogLevel::Info,
                message: "a line".into(),
                index: 0,
            }],
            next_index: 1,
            truncated: false,
            session: "4242:0".into(),
        };
        let json = serde_json::to_string(&page).expect("page");
        assert!(json.contains("\"Session\":\"4242:0\""), "{json}");
        let back: LogMessagesFromIndex = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, page);
    }
}
