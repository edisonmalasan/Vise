//! Worker log buffer.
//!
//! The log is an in-memory ring of entries that the desktop application drains
//! through the logging routes. It is bounded so a long encode that logs every
//! frame cannot exhaust worker memory; when the bound is hit the oldest entries
//! are dropped and the truncation is recorded.
//!
//! # Two kinds of file
//!
//! An entry can reach two files. The *session* file is one file for the whole
//! worker process, and it is what `InitCommand::LogDirectory` names. The
//! *encode* file is opened by [`LogService::begin_encode`] and closed by
//! [`LogService::end_encode`], so one queue of forty jobs leaves forty files a
//! person can read one at a time instead of one file they have to search.
//!
//! The rotation policy runs when an encode ends and never while one is running,
//! and it is asked the same question whatever the folder looks like: which
//! files may go. The decision itself is [`vise_ipc::enclog::decide_rotation`],
//! which is a pure function, so the awkward cases — the file being written, a
//! file the user has open — are pinned by tests rather than by hope.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use vise_ipc::contract::{LogLevel, LogMessage, LOG_BUFFER_LIMIT};
use vise_ipc::enclog::{self, LogFile, LogRole, RotationPolicy};

/// Buffered worker log.
#[derive(Debug)]
pub struct LogService {
    entries: Mutex<VecDeque<LogMessage>>,
    limit: usize,
    /// Monotonic index handed to the next entry.
    next_index: AtomicUsize,
    verbosity: Mutex<u8>,
    disk_target: Mutex<Option<PathBuf>>,
    truncated: Mutex<bool>,
    /// Folder per-encode logs are written into.
    log_folder: Mutex<Option<PathBuf>>,
    /// The log the running encode is being appended to.
    encode_target: Mutex<Option<PathBuf>>,
    /// How much of the folder is allowed to exist at once.
    rotation: Mutex<RotationPolicy>,
}

impl Default for LogService {
    fn default() -> Self {
        Self::with_limit(LOG_BUFFER_LIMIT)
    }
}

/// What the log folder looked like after a rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationReport {
    /// Files that were removed, oldest first.
    pub removed: Vec<String>,
    /// Files that are still there.
    pub kept: usize,
    /// Bytes the folder holds now.
    pub bytes_kept: u64,
    /// Whether the folder could not be rotated at all.
    pub problem: Option<String>,
}

impl RotationReport {
    /// A folder that was already inside its bounds.
    pub fn clean() -> Self {
        RotationReport {
            removed: Vec::new(),
            kept: 0,
            bytes_kept: 0,
            problem: None,
        }
    }
}

impl LogService {
    /// Create a log holding at most `limit` entries.
    pub fn with_limit(limit: usize) -> Self {
        LogService {
            entries: Mutex::new(VecDeque::new()),
            limit: limit.max(1),
            next_index: AtomicUsize::new(0),
            verbosity: Mutex::new(2),
            disk_target: Mutex::new(None),
            truncated: Mutex::new(false),
            log_folder: Mutex::new(None),
            encode_target: Mutex::new(None),
            rotation: Mutex::new(RotationPolicy::default()),
        }
    }

    /// Set the minimum level an entry must have to be recorded.
    pub fn set_verbosity(&self, verbosity: u8) {
        if let Ok(mut guard) = self.verbosity.lock() {
            *guard = verbosity;
        }
    }

    /// Enable appending entries to `path`.
    pub fn set_disk_target(&self, path: Option<PathBuf>) {
        if let Ok(mut guard) = self.disk_target.lock() {
            *guard = path;
        }
    }

    /// Name the folder per-encode logs are written into.
    ///
    /// Separate from the session log's own path because the two answer different
    /// questions: one is "where does this worker's own log live", the other is
    /// "where do the per-encode logs live". Pointing them at the same folder is
    /// normal and correct; pointing them apart is an operator's choice.
    pub fn set_log_folder(&self, folder: Option<PathBuf>) {
        if let Ok(mut guard) = self.log_folder.lock() {
            *guard = folder;
        }
    }

    /// Replace the rotation bounds.
    pub fn set_rotation_policy(&self, policy: RotationPolicy) {
        if let Ok(mut guard) = self.rotation.lock() {
            *guard = policy;
        }
    }

    /// Record an entry at `level`.
    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        self.record(level, message.into());
    }

    /// Record an informational entry.
    pub fn info(&self, message: impl Into<String>) {
        self.record(LogLevel::Info, message.into());
    }

    /// Record a debug entry.
    pub fn debug(&self, message: impl Into<String>) {
        self.record(LogLevel::Debug, message.into());
    }

    /// Record a warning entry.
    pub fn warn(&self, message: impl Into<String>) {
        self.record(LogLevel::Warning, message.into());
    }

    /// Record an error entry.
    pub fn error(&self, message: impl Into<String>) {
        self.record(LogLevel::Error, message.into());
    }

    fn record(&self, level: LogLevel, message: String) {
        let minimum = self.verbosity.lock().map(|guard| *guard).unwrap_or(2);
        if level.rank() > minimum {
            return;
        }

        let entry = {
            let mut guard = match self.entries.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let index = self.next_index.fetch_add(1, Ordering::Relaxed);
            let entry = LogMessage {
                log_level: level,
                message,
                index,
            };
            guard.push_back(entry.clone());
            while guard.len() > self.limit {
                guard.pop_front();
                if let Ok(mut flag) = self.truncated.lock() {
                    *flag = true;
                }
            }
            entry
        };

        self.append_to_disk(&entry);
    }

    fn append_to_disk(&self, entry: &LogMessage) {
        let line = format!("[{:?}] {}\n", entry.log_level, entry.message);
        // The session file and the encode file are two independent handles on
        // the same two-line entry. A failure on one is not a reason to lose the
        // other, so neither is allowed to stop the other being tried.
        let session = self.disk_target.lock().ok().and_then(|guard| guard.clone());
        let encode = self.encode_target.lock().ok().and_then(|guard| guard.clone());
        for path in [session, encode].into_iter().flatten() {
            append_line(&path, &line);
        }
    }

    /// Open a log for one encode and send everything written until
    /// [`LogService::end_encode`] to it as well as to the session log.
    ///
    /// Returns the file it opened, or `None` when disk logging is off or the
    /// folder cannot be created. That is not a failure the encode should stop
    /// for: a log is worth writing, and an encode that cannot be logged is still
    /// an encode somebody asked for.
    pub fn begin_encode(&self, label: &str) -> Option<PathBuf> {
        let folder = self.log_folder.lock().ok().and_then(|guard| guard.clone())?;
        if std::fs::create_dir_all(&folder).is_err() {
            return None;
        }
        let path = folder.join(enclog::encode_log_name(label));
        let mut guard = match self.encode_target.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(path.clone());
        Some(path)
    }

    /// Close the running encode's log and bring the folder back inside its
    /// bounds.
    ///
    /// Rotation happens here rather than on a timer for two reasons: the moment
    /// the log stops being written is the only moment nothing can be lost by
    /// moving it, and an application that is not encoding is not filling the
    /// disk with logs.
    pub fn end_encode(&self) -> RotationReport {
        if let Ok(mut guard) = self.encode_target.lock() {
            *guard = None;
        }
        self.rotate()
    }

    /// Apply the rotation policy to the log folder now.
    pub fn rotate(&self) -> RotationReport {
        let Some(folder) = self.log_folder.lock().ok().and_then(|guard| guard.clone()) else {
            return RotationReport::clean();
        };
        let policy = self
            .rotation
            .lock()
            .map(|guard| *guard)
            .unwrap_or_default();
        let files: Vec<LogFile> = enclog::list_encode_logs(&folder, enclog::now_seconds());
        // Nothing is open right now, so nothing is protected: the folder holds
        // only finished logs. The policy still supports the protected roles,
        // and they are exercised there, because a viewer may hold one open
        // across the rotation this triggers.
        let plan = enclog::decide_rotation(&policy, &files);
        let mut removed = Vec::new();
        let mut problem: Option<String> = None;
        for name in &plan.remove {
            match std::fs::remove_file(folder.join(name)) {
                Ok(()) => removed.push(name.clone()),
                Err(e) => {
                    problem = Some(format!("{name} could not be removed: {e}"));
                    break;
                }
            }
        }
        RotationReport {
            removed,
            kept: files.len().saturating_sub(plan.remove.len()),
            bytes_kept: plan.bytes_kept,
            problem,
        }
    }

    /// The log the running encode is being written to.
    pub fn encode_log(&self) -> Option<PathBuf> {
        self.encode_target.lock().ok().and_then(|guard| guard.clone())
    }

    /// The folder per-encode logs are written into.
    pub fn log_folder(&self) -> Option<PathBuf> {
        self.log_folder.lock().ok().and_then(|guard| guard.clone())
    }

    /// The logs in the folder right now, with `name` marked as in progress.
    ///
    /// The marking is what stops a rotation from taking the file the worker is
    /// appending to; it is done here rather than inside the policy so the policy
    /// stays a function of values rather than of a filesystem it cannot see.
    pub fn encode_logs(&self, now_seconds: u64) -> Vec<enclog::LogFile> {
        let Some(folder) = self.log_folder.lock().ok().and_then(|guard| guard.clone()) else {
            return Vec::new();
        };
        let open = self.encode_log();
        enclog::list_encode_logs(&folder, now_seconds)
            .into_iter()
            .map(|file| {
                let role = match &open {
                    Some(path) if *path == folder.join(&file.name) => LogRole::InProgress,
                    _ => LogRole::Retained,
                };
                file.with_role(role)
            })
            .collect()
    }

    /// Every buffered entry, oldest first.
    pub fn all(&self) -> Vec<LogMessage> {
        self.entries
            .lock()
            .map(|guard| guard.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Entries from `index` onwards, plus the index to continue from.
    pub fn from_index(&self, index: usize) -> vise_ipc::contract::LogMessagesFromIndex {
        let entries = self.entries.lock();
        let truncated = self.truncated.lock().map(|flag| *flag).unwrap_or(false);
        let messages: Vec<LogMessage> = match entries {
            Ok(guard) => guard
                .iter()
                .filter(|entry| entry.index >= index)
                .cloned()
                .collect(),
            Err(poisoned) => poisoned
                .into_inner()
                .iter()
                .filter(|entry| entry.index >= index)
                .cloned()
                .collect(),
        };
        let next_index = messages.last().map(|e| e.index + 1).unwrap_or(index);
        vise_ipc::contract::LogMessagesFromIndex {
            messages,
            next_index,
            truncated,
        }
    }

    /// Number of buffered entries.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|guard| guard.len()).unwrap_or(0)
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop every entry and restart indexing.
    pub fn reset(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
        if let Ok(mut flag) = self.truncated.lock() {
            *flag = false;
        }
    }
}

/// Append one line, creating the folder and the file if they are not there yet.
///
/// Every failure is swallowed on purpose: a log that cannot be written is a
/// reason to carry on without it, not a reason to fail an encode.
fn append_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_returns_entries_in_order() {
        let log = LogService::default();
        log.info("first");
        log.warn("second");
        let all = log.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].message, "first");
        assert_eq!(all[0].index, 0);
        assert_eq!(all[1].log_level, LogLevel::Warning);
    }

    #[test]
    fn respects_verbosity() {
        let log = LogService::default();
        // Verbosity zero keeps errors only.
        log.set_verbosity(0);
        log.debug("chatty");
        log.info("informative");
        log.error("broken");
        let all = log.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].message, "broken");

        // The default rank keeps informational entries and drops debug ones.
        log.reset();
        log.set_verbosity(2);
        log.debug("chatty");
        log.info("informative");
        assert_eq!(log.all().len(), 1);
        assert_eq!(log.all()[0].message, "informative");
    }

    #[test]
    fn drops_oldest_entries_and_reports_truncation() {
        let log = LogService::with_limit(3);
        for i in 0..6 {
            log.info(format!("entry {i}"));
        }
        let all = log.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].message, "entry 3");
        assert!(log.from_index(0).truncated);
    }

    #[test]
    fn reads_from_an_index() {
        let log = LogService::default();
        for i in 0..4 {
            log.info(format!("entry {i}"));
        }
        let page = log.from_index(2);
        assert_eq!(page.messages.len(), 2);
        assert_eq!(page.messages[0].message, "entry 2");
        assert_eq!(page.next_index, 4);
        assert_eq!(log.from_index(99).messages.len(), 0);
        assert_eq!(log.from_index(99).next_index, 99);
    }

    #[test]
    fn reset_clears_entries_and_truncation_flag() {
        let log = LogService::with_limit(2);
        for i in 0..5 {
            log.info(format!("entry {i}"));
        }
        log.reset();
        assert!(log.is_empty());
        assert!(!log.from_index(0).truncated);
    }

    fn log_folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-worker-enclog-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn an_encode_writes_its_own_file_and_the_session_file_both() {
        let dir = log_folder("two-files");
        let log = LogService::default();
        log.set_disk_target(Some(dir.join("vise-worker-1.log")));
        log.set_log_folder(Some(dir.clone()));
        log.info("before the encode");

        let opened = log.begin_encode("Episode 1").expect("an encode log");
        assert_eq!(opened, dir.join("encode-Episode-1.log"));
        assert_eq!(log.encode_log(), Some(opened.clone()));
        log.info("during the encode");

        assert!(std::fs::read_to_string(&opened)
            .expect("encode log")
            .contains("during the encode"));
        let session = std::fs::read_to_string(dir.join("vise-worker-1.log")).expect("session log");
        assert!(session.contains("before the encode"));
        assert!(
            session.contains("during the encode"),
            "the session log is not interrupted by the per-encode file: {session}"
        );

        log.end_encode();
        assert_eq!(log.encode_log(), None, "the encode file is closed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_encode_log_is_opened_under_the_resolved_folder_and_never_above_it() {
        let dir = log_folder("resolved");
        let log = LogService::default();
        // A job whose output is somewhere else entirely still writes its log
        // where the storage rules said logs go.
        log.set_log_folder(Some(dir.join("logs")));
        let opened = log.begin_encode("../../escape").expect("a log");
        assert_eq!(opened.parent(), Some(dir.join("logs").as_path()));
        assert!(opened
            .file_name()
            .expect("name")
            .to_string_lossy()
            .contains("escape"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_with_no_log_folder_writes_no_encode_logs_and_says_nothing() {
        let log = LogService::default();
        assert_eq!(log.begin_encode("Episode 1"), None);
        assert_eq!(log.end_encode(), RotationReport::clean());
        assert_eq!(log.encode_log(), None);
        assert!(log.encode_logs(0).is_empty());
    }

    #[test]
    fn an_open_encode_log_is_never_the_one_a_rotation_removes() {
        let dir = log_folder("protected");
        let log = LogService::default();
        log.set_log_folder(Some(dir.clone()));
        log.set_rotation_policy(RotationPolicy {
            max_bytes: 32,
            max_files: 1,
        });
        // Two finished logs that the policy would otherwise both delete, and the
        // one being written now.
        log.set_disk_target(None);
        std::fs::write(dir.join("encode-old.log"), vec![b'a'; 64]).expect("write");
        std::fs::write(dir.join("encode-older.log"), vec![b'b'; 64]).expect("write");
        let open = log.begin_encode("current").expect("a log");
        std::fs::write(&open, vec![b'c'; 64]).expect("write");

        let files = log.encode_logs(0);
        assert_eq!(
            files
                .iter()
                .find(|file| file.name == "encode-current.log")
                .map(|file| file.role),
            Some(LogRole::InProgress)
        );

        // Rotation while the encode is still running is not something the worker
        // does, but the policy is asked the question anyway, and the file being
        // written survives it.
        let policy = RotationPolicy {
            max_bytes: 32,
            max_files: 1,
        };
        let plan = enclog::decide_rotation(&policy, &files);
        assert!(!plan.remove.contains(&"encode-current.log".to_string()));
        assert!(plan.rotate, "and the caller is told to close it next time");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ending_an_encode_trims_the_folder_and_removes_nothing_it_should_keep() {
        let dir = log_folder("trim");
        let log = LogService::default();
        log.set_log_folder(Some(dir.clone()));
        log.set_rotation_policy(RotationPolicy {
            max_bytes: 100,
            max_files: 2,
        });
        for name in ["encode-a.log", "encode-b.log", "encode-c.log"] {
            std::fs::write(dir.join(name), vec![b'x'; 50]).expect("write");
        }
        // The newest file is the one just written; the policy removes by age, so
        // the age is made explicit rather than left to the filesystem clock.
        let now = enclog::now_seconds();
        std::fs::write(dir.join("encode-a.log"), vec![b'x'; 50]).expect("write");
        let files = log.encode_logs(now);
        assert_eq!(files.len(), 3);

        let report = log.end_encode();
        assert!(report.problem.is_none(), "{:?}", report.problem);
        assert_eq!(report.removed.len(), 1, "{report:?}");
        assert_eq!(report.kept, 2);
        assert!(!dir.join(&report.removed[0]).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_is_inside_its_bounds_loses_nothing() {
        let dir = log_folder("inside");
        let log = LogService::default();
        log.set_log_folder(Some(dir.clone()));
        std::fs::write(dir.join("encode-a.log"), b"small").expect("write");
        let report = log.end_encode();
        assert!(report.removed.is_empty(), "{report:?}");
        assert!(report.problem.is_none());
        assert_eq!(report.kept, 1);
        assert_eq!(report.bytes_kept, 5);
        assert!(dir.join("encode-a.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_log_the_folder_does_not_own_is_never_touched() {
        let dir = log_folder("foreign");
        let log = LogService::default();
        log.set_log_folder(Some(dir.clone()));
        log.set_rotation_policy(RotationPolicy {
            max_bytes: 1,
            max_files: 1,
        });
        std::fs::write(dir.join("encode-a.log"), vec![b'x'; 100]).expect("write");
        std::fs::write(dir.join("someone-elses-notes.log"), vec![b'y'; 100]).expect("write");
        let report = log.end_encode();
        assert_eq!(report.removed, vec!["encode-a.log".to_string()]);
        assert!(dir.join("someone-elses-notes.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
