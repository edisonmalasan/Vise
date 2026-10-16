//! Worker log buffer.
//!
//! The log is an in-memory ring of entries that the desktop application drains
//! through the logging routes. It is bounded so a long encode that logs every
//! frame cannot exhaust worker memory; when the bound is hit the oldest entries
//! are dropped and the truncation is recorded.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use vise_ipc::contract::{LogLevel, LogMessage, LOG_BUFFER_LIMIT};

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
}

impl Default for LogService {
    fn default() -> Self {
        Self::with_limit(LOG_BUFFER_LIMIT)
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
        let target = self.disk_target.lock().ok().and_then(|guard| guard.clone());
        if let Some(path) = target {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(file, "[{:?}] {}", entry.log_level, entry.message);
            }
        }
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
}
