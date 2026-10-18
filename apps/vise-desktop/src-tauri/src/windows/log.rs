//! The activity window.
//!
//! Two things write to this log: the window itself, which records what the user
//! asked for and what it decided, and the worker, which records what the media
//! tools did. Both arrive here as one list so the user reads a single history
//! rather than two interleaved ones they have to reconcile.
//!
//! The worker is paged rather than re-read: `GetLogMessagesFromIndex` is asked
//! for everything after the last index it gave us, and the reply says whether
//! older entries were dropped along the way. That is what makes the tail cheap
//! to follow on a long encode, and the "some lines were dropped" note is shown
//! rather than hidden when it happens, because a log that silently loses lines
//! is worse than no log.
//!
//! # Two ways the stream can stop being a stream
//!
//! Both are handled here rather than left to the page, and both are *shown*:
//!
//! - **The worker is replaced.** A crashed worker is restarted, and the new one
//!   numbers its entries from zero again. A cursor held across that would ask
//!   for index nine hundred of a buffer that has six entries in it and receive
//!   nothing, forever. The reply carries the worker and buffer it came from, so
//!   a change is noticed; the window's own lines are kept, the worker's are
//!   dropped, and a line says why.
//! - **The window cannot keep up.** The held history is bounded, because a
//!   window that grows a list without limit is a window that eventually stops
//!   painting. When the bound is reached the oldest lines go, the count of what
//!   went is kept, and the window says it is no longer showing everything.
//!   Back-pressure is explicit: the alternative is a window that looks complete
//!   and is not.
//!
//! Filtering, following, copying, and clearing are all decided here. The page
//! only paints the list and moves the scroll position.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use vise_ipc::contract::{LogLevel, LogMessage, LogMessagesFromIndex};

/// Identifier of the activity surface.
pub const SURFACE: &str = "activity";

/// Which half of the application wrote a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    /// The desktop window.
    Desktop,
    /// The background worker.
    Worker,
}

impl LogSource {
    /// Stable identifier the page uses as a row class and a filter key.
    pub fn as_str(self) -> &'static str {
        match self {
            LogSource::Desktop => "desktop",
            LogSource::Worker => "worker",
        }
    }
}

/// One recorded line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    /// Monotonic number, so two lines can never be confused for one row.
    pub index: u64,
    /// How severe the line is.
    pub level: LogLevel,
    /// Which half wrote it.
    pub source: LogSource,
    /// The line itself.
    pub message: String,
    /// The worker's own index, for the lines that came from the worker.
    ///
    /// Carried rather than looked up because the window's numbers and the
    /// worker's are different sequences, and the only place that needs to relate
    /// them is the one that trims the history and has to forget the worker
    /// indices of the lines it dropped.
    #[serde(skip)]
    worker: Option<usize>,
}

impl LogLine {
    /// A line as the window records it.
    pub fn new(index: u64, level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        Self {
            index,
            level,
            source,
            message: message.into(),
            worker: None,
        }
    }

    /// A worker line, with the index it had in the worker's buffer.
    fn from_worker(index: u64, level: LogLevel, worker_index: usize, message: String) -> Self {
        LogLine {
            index,
            level,
            source: LogSource::Worker,
            message,
            worker: Some(worker_index),
        }
    }

    /// The worker's index for this line, when it came from the worker.
    pub fn worker_index(&self) -> Option<usize> {
        self.worker
    }
}

/// Largest number of lines the window keeps.
///
/// Ten thousand lines is a very long scroll on a very tall window, and it is
/// bounded because the alternative is a list that grows for as long as the
/// application is open. What is dropped is counted and shown.
pub const MAX_HELD_LINES: usize = 10_000;

/// Every level, least to most verbose, for the filter control.
///
/// The order and the names come from the contract so the filter, the row labels,
/// and the preference that chooses a level are all reading one table.
pub fn levels() -> [LogLevel; 4] {
    LogLevel::ALL
}

/// The name of a level, as the filter and each row show it.
pub fn level_label(level: LogLevel) -> &'static str {
    level.label()
}

/// One level in the filter control, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelOptionViewModel {
    /// Stable identifier sent back on a filter change.
    pub id: String,
    /// Label on the option.
    pub label: String,
    /// How many recorded lines are at this level or more severe.
    pub count: usize,
    /// Whether this is the chosen floor.
    pub selected: bool,
}

/// A line the worker wrote more than once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatedLineViewModel {
    /// The line itself.
    pub message: String,
    /// How many times it was written.
    pub count: usize,
}

/// One recorded line, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLineViewModel {
    /// Monotonic number.
    pub index: u64,
    /// Name of the level.
    pub level: String,
    /// Tone the page paints the row with.
    pub tone: String,
    /// Which half wrote the line.
    pub source: String,
    /// The line itself.
    pub message: String,
}

/// One control in the activity window, with its availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogControlViewModel {
    /// Element identifier.
    pub id: String,
    /// Label on the control.
    pub label: String,
    /// Whether it can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl LogControlViewModel {
    /// A control that is on offer.
    pub fn open(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: true,
            reason: String::new(),
        }
    }

    /// The control, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// The activity window, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityViewModel {
    /// The lines that pass the filter, oldest first.
    pub lines: Vec<LogLineViewModel>,
    /// Worker lines that were written more than once, with their counts.
    pub repeated: Vec<RepeatedLineViewModel>,
    /// How many lines were recorded in all.
    pub total: usize,
    /// How many the filter is hiding.
    pub hidden: usize,
    /// The chosen floor, as the filter control shows it.
    pub selected_level: String,
    /// Every level with its count, least to most verbose.
    pub level_options: Vec<LevelOptionViewModel>,
    /// Whether the page should keep the newest line in view.
    pub follow: bool,
    /// Whether older worker lines were dropped before the ones held here.
    pub truncated: bool,
    /// Whether the window dropped lines of its own to stay inside its bound.
    ///
    /// A window that has to choose between showing a very long history and
    /// staying usable chooses to stay usable, and then says which it did.
    pub over_capacity: bool,
    /// How many lines the window has dropped.
    pub dropped: usize,
    /// How many lines the window will hold.
    pub limit: usize,
    /// How many times the worker behind this window was replaced or cleared.
    pub resets: usize,
    /// Whether the follow switch can be used.
    pub follow_available: bool,
    /// Why following is unavailable, when it is.
    pub follow_reason: String,
    /// The copy control.
    pub copy: LogControlViewModel,
    /// The clear control.
    pub clear: LogControlViewModel,
    /// One line summarising the log.
    pub summary: String,
    /// Whether there is nothing to show.
    pub empty: bool,
    /// The whole visible log as plain text.
    pub copy_text: String,
}

/// The activity history, with its filter and paging cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActivityLog {
    lines: Vec<LogLine>,
    /// Index to pass to the worker for the next page.
    cursor: usize,
    /// Whether the worker dropped lines before the ones held here.
    truncated: bool,
    /// Worker indices already folded in, so a page that repeats itself does not
    /// double the history.
    ///
    /// Bounded by the same bound as the history: an index whose line has been
    /// dropped is forgotten with it, so this cannot outgrow what it describes.
    worker_seen: BTreeSet<usize>,
    /// Whether the page should keep the newest line in view.
    follow: bool,
    /// The filter floor.
    min_level: Option<LogLevel>,
    next_index: u64,
    /// Which worker and buffer the last page came from.
    session: String,
    /// How many times the worker under this window was replaced or cleared.
    resets: usize,
    /// Lines the window dropped to stay inside its bound.
    dropped: usize,
    /// How many lines this window may hold.
    limit: usize,
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            cursor: 0,
            truncated: false,
            worker_seen: BTreeSet::new(),
            follow: true,
            min_level: Some(LogLevel::Info),
            next_index: 0,
            session: String::new(),
            resets: 0,
            dropped: 0,
            limit: MAX_HELD_LINES,
        }
    }
}

impl ActivityLog {
    /// Record a line the window itself wrote.
    pub fn push(&mut self, level: LogLevel, message: impl Into<String>) -> u64 {
        self.next_index += 1;
        let index = self.next_index;
        self.lines
            .push(LogLine::new(index, level, LogSource::Desktop, message));
        index
    }

    /// Record a line at the info level, which is what most window events are.
    pub fn record(&mut self, message: impl Into<String>) -> u64 {
        self.push(LogLevel::Info, message)
    }

    /// Take the newest worker page and fold it in.
    ///
    /// The reply's `next_index` becomes the next cursor, and its `truncated`
    /// flag sticks: once the worker has dropped lines, the note stays so the
    /// reader knows the history in front of them is not the whole history.
    ///
    /// A page from a *different* worker or buffer starts a new stream: the
    /// cursor goes back to zero, the worker lines held here are dropped, and a
    /// line is recorded saying so. See [`ActivityLog::take_reset`].
    pub fn take_page(&mut self, page: LogMessagesFromIndex) -> usize {
        let reset = self.note_reset(&page.session);
        let taken = page.messages.len();
        for message in &page.messages {
            self.append_worker(message);
        }
        // After a reset the cursor is the page's own answer when the page carried
        // lines — it then reflects the new worker's real numbering — and zero
        // when it did not. A page with nothing in it echoes back whatever index
        // was asked for, and a worker that was replaced starts numbering at
        // zero, so keeping the asked-for index there would be a cursor that
        // waits for ever.
        self.cursor = if reset {
            if page.messages.is_empty() {
                0
            } else {
                page.next_index
            }
        } else {
            page.next_index.max(self.cursor)
        };
        self.truncated = if reset {
            page.truncated
        } else {
            self.truncated || page.truncated
        };
        self.enforce_limit();
        taken
    }

    /// Notice a new worker or a cleared buffer, and report whether there was one.
    ///
    /// The window's own lines are kept: they happened on this machine and they
    /// did not stop being true. The worker's lines did — they belonged to a
    /// buffer that is gone, and keeping them would put a gap in the history
    /// without saying there was one.
    fn note_reset(&mut self, session: &str) -> bool {
        if session.is_empty() || session == self.session {
            return false;
        }
        let first = self.session.is_empty();
        self.session = session.to_string();
        if first {
            // The first page from a worker only records who it came from: there
            // was no previous stream to break.
            return false;
        }
        self.cursor = 0;
        self.truncated = false;
        self.worker_seen.clear();
        self.lines.retain(|line| line.source == LogSource::Desktop);
        self.resets += 1;
        self.record(
            "the worker behind this window was replaced, so its log starts again from the beginning",
        );
        true
    }

    /// The index the next worker page starts at.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Which worker and buffer the last page came from.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// How many times the worker was replaced or cleared.
    pub fn resets(&self) -> usize {
        self.resets
    }

    /// How many lines the window dropped to stay inside its bound.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Add one worker line.
    ///
    /// The worker numbers its own entries, and a page that repeats itself would
    /// otherwise double the history, so an index already folded in is dropped.
    fn append_worker(&mut self, message: &LogMessage) {
        if !self.worker_seen.insert(message.index) {
            return;
        }
        self.next_index += 1;
        self.lines.push(LogLine::from_worker(
            self.next_index,
            message.log_level,
            message.index,
            message.message.clone(),
        ));
    }

    /// Drop the oldest lines until the history is inside its bound.
    ///
    /// The worker indices of the dropped lines are forgotten with them, so the
    /// set of indices this window is tracking cannot grow past the history it
    /// describes. A line that arrives again after being dropped is added again,
    /// which is why the count of dropped lines is kept and shown: the window is
    /// not pretending its history is whole.
    fn enforce_limit(&mut self) {
        while self.lines.len() > self.limit {
            let removed = self.lines.remove(0);
            self.dropped += 1;
            if let Some(worker) = removed.worker_index() {
                self.worker_seen.remove(&worker);
            }
        }
    }

    /// Set the filter floor.
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = Some(level);
    }

    /// Set the filter floor by name.
    pub fn set_min_level_name(&mut self, text: &str) -> Result<(), String> {
        let wanted = text.trim().to_ascii_lowercase();
        let level = levels()
            .into_iter()
            .find(|level| level_label(*level).to_ascii_lowercase() == wanted)
            .ok_or_else(|| format!("'{text}' is not a log level this build shows."))?;
        self.set_min_level(level);
        Ok(())
    }

    /// The chosen floor.
    pub fn min_level(&self) -> LogLevel {
        self.min_level.unwrap_or_default()
    }

    /// Turn the follow switch on or off.
    pub fn set_follow(&mut self, follow: bool) {
        self.follow = follow;
    }

    /// Forget every line and start the paging again.
    pub fn clear(&mut self) -> usize {
        let count = self.lines.len();
        self.lines.clear();
        self.cursor = 0;
        self.truncated = false;
        self.worker_seen.clear();
        count
    }

    /// The lines that pass the filter, oldest first.
    pub fn visible(&self) -> Vec<&LogLine> {
        let floor = self.min_level();
        self.lines
            .iter()
            .filter(|line| line.level.rank() <= floor.rank())
            .collect()
    }

    /// How many recorded lines are at `level` or more severe.
    pub fn count_at(&self, level: LogLevel) -> usize {
        self.lines
            .iter()
            .filter(|line| line.level.rank() <= level.rank())
            .count()
    }

    /// The visible log as plain text, for the clipboard.
    pub fn copy_text(&self) -> String {
        self.visible()
            .into_iter()
            .map(|line| {
                format!(
                    "[{} {}] {}",
                    line.source.as_str(),
                    level_label(line.level),
                    line.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Everything the activity window shows.
    pub fn view(&self) -> ActivityViewModel {
        let visible = self.visible();
        let lines: Vec<LogLineViewModel> = visible
            .iter()
            .map(|line| LogLineViewModel {
                index: line.index,
                level: level_label(line.level).to_string(),
                tone: match line.level {
                    LogLevel::Error => "error",
                    LogLevel::Warning => "warn",
                    LogLevel::Info | LogLevel::Debug => "info",
                }
                .to_string(),
                source: line.source.as_str().to_string(),
                message: line.message.clone(),
            })
            .collect();
        let selected = self.min_level();
        let level_options = levels()
            .into_iter()
            .map(|level| LevelOptionViewModel {
                id: level_label(level).to_ascii_lowercase(),
                label: level_label(level).to_string(),
                count: self.count_at(level),
                selected: level == selected,
            })
            .collect();
        let total = self.lines.len();
        let hidden = total - lines.len();
        let copy_text = self.copy_text();
        let copy = if copy_text.trim().is_empty() {
            LogControlViewModel::open("activityCopy", "Copy")
                .blocked("There is nothing to copy yet.")
        } else {
            LogControlViewModel::open("activityCopy", "Copy")
        };
        let clear = if total == 0 {
            LogControlViewModel::open("activityClear", "Clear").blocked("The log is already empty.")
        } else {
            LogControlViewModel::open("activityClear", "Clear")
        };
        ActivityViewModel {
            empty: lines.is_empty(),
            repeated: repeated_lines(&self.lines),
            follow: self.follow,
            follow_available: total > 0,
            follow_reason: if total == 0 {
                "There is nothing to follow yet.".to_string()
            } else {
                String::new()
            },
            truncated: self.truncated,
            over_capacity: self.dropped > 0,
            dropped: self.dropped,
            limit: self.limit,
            resets: self.resets,
            summary: self.summary_text(total, hidden, lines.len()),
            copy,
            clear,
            copy_text,
            total,
            hidden,
            selected_level: level_label(selected).to_ascii_lowercase(),
            level_options,
            lines,
        }
    }

    /// One line describing what the window is holding, including what it is not.
    ///
    /// Every clause that would change the reader's understanding of the list is
    /// in here, because a line that is quietly missing is the one failure mode
    /// this surface cannot have.
    fn summary_text(&self, total: usize, hidden: usize, shown: usize) -> String {
        if total == 0 {
            return "Nothing has been recorded yet.".to_string();
        }
        let mut parts = Vec::new();
        if hidden == 0 {
            parts.push(format!("{total} line(s)"));
        } else {
            parts.push(format!("{total} line(s), showing {shown}"));
        }
        if self.dropped > 0 {
            parts.push(format!(
                "{} older line(s) were dropped to stay inside the {} line limit",
                self.dropped, self.limit
            ));
        }
        if self.truncated {
            parts.push("the worker dropped older lines".to_string());
        }
        if self.resets > 0 {
            parts.push(format!("the worker was replaced {} time(s)", self.resets));
        }
        parts.join("; ")
    }
}

/// Count the worker lines that were written more than once.
///
/// A long encode writes the same line thousands of times; the list still shows
/// every one of them, because a log that quietly collapses lines is worse than
/// no log, but the summary names the repeats so the reader knows what they are
/// looking at.
pub fn repeated_lines(lines: &[LogLine]) -> Vec<RepeatedLineViewModel> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for line in lines {
        if line.source == LogSource::Worker {
            *counts.entry(line.message.clone()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(message, count)| RepeatedLineViewModel { message, count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_page(
        messages: Vec<LogMessage>,
        next_index: usize,
        truncated: bool,
    ) -> LogMessagesFromIndex {
        LogMessagesFromIndex {
            messages,
            next_index,
            truncated,
            session: "worker-1:0".to_string(),
        }
    }

    fn page_from(
        session: &str,
        messages: Vec<LogMessage>,
        next_index: usize,
    ) -> LogMessagesFromIndex {
        LogMessagesFromIndex {
            messages,
            next_index,
            truncated: false,
            session: session.to_string(),
        }
    }

    fn message(level: LogLevel, text: &str, index: usize) -> LogMessage {
        LogMessage {
            log_level: level,
            message: text.to_string(),
            index,
        }
    }

    #[test]
    fn lines_from_both_halves_are_one_history() {
        let mut log = ActivityLog::default();
        log.record("scan finished");
        log.take_page(worker_page(
            vec![
                message(LogLevel::Info, "ffmpeg started", 0),
                message(LogLevel::Error, "the encoder exited", 1),
            ],
            2,
            false,
        ));
        log.record("job 1 failed");
        let view = log.view();
        assert_eq!(view.total, 4);
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.lines[0].source, "desktop");
        assert_eq!(view.lines[1].source, "worker");
        assert_eq!(view.lines[2].level, "Error");
        assert_eq!(view.lines[2].tone, "error");
        assert_eq!(view.hidden, 0);
        assert!(!view.truncated);
        assert!(view.copy.available);
        assert!(view.clear.available);
        assert!(view.copy_text.contains("[worker Error] the encoder exited"));
    }

    #[test]
    fn the_filter_hides_the_quieter_lines_and_counts_each_level() {
        let mut log = ActivityLog::default();
        log.push(LogLevel::Debug, "a detail");
        log.push(LogLevel::Info, "a fact");
        log.push(LogLevel::Warning, "a worry");
        log.push(LogLevel::Error, "a failure");
        assert_eq!(log.view().lines.len(), 3, "debug is hidden by default");

        log.set_min_level_name("debug").expect("debug");
        let view = log.view();
        assert_eq!(view.selected_level, "debug");
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.hidden, 0);

        log.set_min_level_name("error").expect("error");
        let view = log.view();
        assert_eq!(view.lines.len(), 1);
        assert_eq!(view.hidden, 3);
        assert_eq!(view.lines[0].message, "a failure");
        assert!(view.summary.contains("showing 1"), "{}", view.summary);

        let counts: Vec<usize> = view.level_options.iter().map(|o| o.count).collect();
        assert_eq!(counts, vec![1, 2, 3, 4]);
        assert!(view
            .level_options
            .iter()
            .all(|option| !option.id.is_empty()));
        assert!(
            view.level_options
                .iter()
                .any(|option| option.id == "error" && option.selected),
            "the floor is marked in the picker"
        );
    }

    #[test]
    fn a_filter_level_this_build_does_not_name_is_refused() {
        let mut log = ActivityLog::default();
        assert!(log
            .set_min_level_name("verbose")
            .expect_err("unknown level")
            .contains("verbose"));
        assert_eq!(log.min_level(), LogLevel::Info, "the old floor stands");
    }

    #[test]
    fn the_next_page_starts_where_the_last_one_ended() {
        let mut log = ActivityLog::default();
        assert_eq!(log.cursor(), 0);
        log.take_page(worker_page(
            vec![
                message(LogLevel::Info, "first", 0),
                message(LogLevel::Info, "second", 1),
            ],
            2,
            false,
        ));
        assert_eq!(log.cursor(), 2);
        log.take_page(worker_page(
            vec![message(LogLevel::Info, "third", 2)],
            3,
            false,
        ));
        assert_eq!(log.cursor(), 3);
        assert_eq!(log.view().total, 3);
    }

    #[test]
    fn a_worker_that_dropped_older_lines_says_so_and_keeps_saying_so() {
        let mut log = ActivityLog::default();
        log.take_page(worker_page(
            vec![message(LogLevel::Info, "recent", 40)],
            41,
            true,
        ));
        let view = log.view();
        assert!(view.truncated);
        assert!(view.truncated, "the note sticks once lines have been lost");

        log.take_page(worker_page(
            vec![message(LogLevel::Info, "newer", 41)],
            42,
            false,
        ));
        assert!(log.view().truncated, "a clean page does not un-truncate it");
    }

    #[test]
    fn the_same_worker_index_is_not_stored_twice() {
        let mut log = ActivityLog::default();
        log.take_page(worker_page(
            vec![message(LogLevel::Info, "once", 7)],
            8,
            false,
        ));
        log.take_page(worker_page(
            vec![message(LogLevel::Info, "once", 7)],
            8,
            false,
        ));
        assert_eq!(log.view().total, 1);
    }

    #[test]
    fn following_can_be_turned_off_so_a_read_line_stays_put() {
        let mut log = ActivityLog::default();
        assert!(log.view().follow);
        log.set_follow(false);
        assert!(!log.view().follow);
        log.set_follow(true);
        assert!(log.view().follow);
    }

    #[test]
    fn an_empty_log_explains_itself_and_closes_its_controls() {
        let view = ActivityLog::default().view();
        assert!(view.empty);
        assert_eq!(view.total, 0);
        assert_eq!(view.summary, "Nothing has been recorded yet.");
        assert!(!view.copy.available);
        assert!(view.copy.reason.contains("nothing to copy"));
        assert!(!view.clear.available);
        assert!(!view.follow_available);
        assert!(view.follow_reason.contains("nothing to follow"));
        assert_eq!(view.copy_text, "");
    }

    #[test]
    fn clearing_empties_the_log_and_restarts_the_worker_cursor() {
        let mut log = ActivityLog::default();
        log.record("something");
        log.take_page(worker_page(
            vec![message(LogLevel::Info, "worker", 0)],
            1,
            true,
        ));
        assert_eq!(log.clear(), 2);
        let view = log.view();
        assert_eq!(view.total, 0);
        assert!(!view.truncated);
        assert_eq!(log.cursor(), 0);
        assert!(log.view().empty);
    }

    #[test]
    fn copied_lines_carry_their_level_and_which_half_wrote_them() {
        let mut log = ActivityLog::default();
        log.push(LogLevel::Warning, "disk is nearly full");
        let text = log.copy_text();
        assert_eq!(text, "[desktop Warning] disk is nearly full");
    }

    #[test]
    fn repeated_worker_lines_are_counted_rather_than_hidden() {
        let mut log = ActivityLog::default();
        log.record("a window line");
        log.take_page(worker_page(
            vec![
                message(LogLevel::Info, "frame", 0),
                message(LogLevel::Info, "frame", 1),
                message(LogLevel::Error, "gone", 2),
            ],
            3,
            false,
        ));
        let view = log.view();
        assert_eq!(view.lines.len(), 4, "every line is still listed");
        assert_eq!(view.repeated.len(), 1);
        assert_eq!(view.repeated[0].message, "frame");
        assert_eq!(view.repeated[0].count, 2);
        assert!(ActivityLog::default().view().repeated.is_empty());
    }

    #[test]
    fn each_half_of_the_application_names_its_own_lines() {
        assert_eq!(LogSource::Desktop.as_str(), "desktop");
        assert_eq!(LogSource::Worker.as_str(), "worker");
    }

    #[test]
    fn a_stream_keeps_the_order_the_worker_sent_and_asks_only_for_what_is_new() {
        let mut log = ActivityLog::default();
        log.take_page(page_from(
            "worker-1:0",
            vec![
                message(LogLevel::Info, "one", 0),
                message(LogLevel::Warning, "two", 1),
            ],
            2,
        ));
        assert_eq!(log.cursor(), 2, "the next page starts after the last line");
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "three", 2)],
            3,
        ));
        let view = log.view();
        assert_eq!(
            view.lines
                .iter()
                .map(|l| l.message.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two", "three"],
            "order is the worker's order"
        );
        assert_eq!(
            view.lines.iter().map(|l| l.index).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "and the window's own numbers never repeat"
        );
        assert_eq!(log.resets(), 0);
        assert_eq!(log.dropped(), 0);
    }

    #[test]
    fn a_worker_that_starts_numbering_again_is_not_waited_on_forever() {
        let mut log = ActivityLog::default();
        log.record("queued job 1");
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "old worker line", 0)],
            1,
        ));
        assert_eq!(log.cursor(), 1);

        // The worker crashed and was replaced. Its new buffer numbers from zero
        // again, so the old cursor would ask for an entry that is not there and
        // be answered with silence, for ever.
        let taken = log.take_page(page_from(
            "worker-2:0",
            vec![
                message(LogLevel::Info, "new worker line", 0),
                message(LogLevel::Error, "the encoder exited", 1),
            ],
            2,
        ));
        assert_eq!(taken, 2, "the new worker's lines are read, not dropped");
        assert_eq!(log.cursor(), 2);
        assert_eq!(log.session(), "worker-2:0");
        assert_eq!(log.resets(), 1);

        let view = log.view();
        let messages: Vec<&str> = view.lines.iter().map(|l| l.message.as_str()).collect();
        assert!(
            !messages.contains(&"old worker line"),
            "the old worker's lines are dropped: {messages:?}"
        );
        assert!(
            messages.contains(&"queued job 1"),
            "and the window's are kept"
        );
        assert!(messages.contains(&"new worker line"));
        assert!(
            view.lines
                .iter()
                .any(|l| l.message.contains("was replaced")),
            "and the reader is told: {messages:?}"
        );
        assert!(
            view.summary.contains("worker was replaced"),
            "{}",
            view.summary
        );
    }

    #[test]
    fn a_cleared_worker_buffer_is_a_reset_too() {
        let mut log = ActivityLog::default();
        log.take_page(page_from(
            "worker-1:0",
            vec![
                message(LogLevel::Info, "a", 0),
                message(LogLevel::Info, "b", 1),
            ],
            2,
        ));
        assert_eq!(log.cursor(), 2);

        // `ResetLogging` clears the buffer without replacing the process. The
        // session counter is what makes it visible, because the indices keep
        // counting and a page from here looks exactly like a quiet worker.
        log.take_page(page_from("worker-1:1", Vec::new(), 2));
        assert_eq!(log.resets(), 1);
        assert_eq!(log.cursor(), 0, "reading starts again from the beginning");

        // The next line the worker writes is read rather than waited past.
        let taken = log.take_page(page_from(
            "worker-1:1",
            vec![message(LogLevel::Info, "after the clear", 2)],
            3,
        ));
        assert_eq!(taken, 1);
        assert_eq!(log.cursor(), 3);
        assert!(log
            .view()
            .lines
            .iter()
            .any(|l| l.message == "after the clear"));
        assert!(!log.view().lines.iter().any(|l| l.message == "a"));
    }

    #[test]
    fn the_first_page_from_a_worker_is_not_announced_as_a_reset() {
        let mut log = ActivityLog::default();
        log.record("window opened");
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "a", 0)],
            1,
        ));
        assert_eq!(log.resets(), 0);
        assert!(log.view().total == 2, "and nothing extra was recorded");
    }

    #[test]
    fn a_page_with_no_session_is_treated_as_no_information_rather_than_as_a_reset() {
        // An older worker build answers without the field. Reading that as a
        // reset would clear the history every poll.
        let mut log = ActivityLog::default();
        log.take_page(LogMessagesFromIndex {
            messages: vec![message(LogLevel::Info, "a", 0)],
            next_index: 1,
            truncated: false,
            session: String::new(),
        });
        assert_eq!(log.resets(), 0);
        log.take_page(LogMessagesFromIndex {
            messages: vec![message(LogLevel::Info, "b", 1)],
            next_index: 2,
            truncated: false,
            session: String::new(),
        });
        assert_eq!(log.cursor(), 2);
        assert_eq!(log.view().total, 2);
    }

    #[test]
    fn a_window_that_cannot_keep_every_line_says_so_instead_of_growing() {
        let mut log = ActivityLog::default();
        log.limit = 3;
        for index in 0..6usize {
            log.take_page(page_from(
                "worker-1:0",
                vec![message(LogLevel::Info, &format!("line {index}"), index)],
                index + 1,
            ));
        }
        let view = log.view();
        assert_eq!(view.total, 3, "the bound is the bound");
        assert_eq!(view.over_capacity, true);
        assert_eq!(view.dropped, 3);
        assert_eq!(view.limit, 3);
        assert!(
            view.summary.contains("3 older line(s) were dropped"),
            "{}",
            view.summary
        );
        assert_eq!(
            view.lines.last().map(|line| line.message.clone()),
            Some("line 5".to_string()),
            "the newest lines are the ones kept"
        );
    }

    #[test]
    fn the_indices_being_tracked_do_not_outlive_the_lines_they_describe() {
        let mut log = ActivityLog::default();
        log.limit = 2;
        for index in 0..5usize {
            log.take_page(page_from(
                "worker-1:0",
                vec![message(LogLevel::Info, &format!("line {index}"), index)],
                index + 1,
            ));
        }
        assert_eq!(
            log.worker_seen.len(),
            2,
            "the set is bounded by the history"
        );
        assert!(log.worker_seen.contains(&3) && log.worker_seen.contains(&4));
        assert!(
            !log.worker_seen.contains(&0),
            "the forgotten line's index went with it"
        );
        // An index whose line was dropped is not swallowed as a duplicate of a
        // line that is no longer on screen.
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "line 0 again", 0)],
            5,
        ));
        let view = log.view();
        assert!(
            view.lines.iter().any(|line| line.message == "line 0 again"),
            "the line is read rather than deduplicated away"
        );
        assert_eq!(view.total, 2, "and the cap is applied again afterwards");
        assert_eq!(log.dropped(), 4);
    }

    #[test]
    fn dropping_worker_lines_does_not_cost_the_window_its_own() {
        let mut log = ActivityLog::default();
        log.limit = 2;
        log.record("the user pressed something");
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "a", 0)],
            1,
        ));
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "b", 1)],
            2,
        ));
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "c", 2)],
            3,
        ));
        let messages: Vec<String> = log.view().lines.iter().map(|l| l.message.clone()).collect();
        assert_eq!(messages, vec!["b".to_string(), "c".to_string()]);
        assert_eq!(log.dropped(), 2);
    }

    #[test]
    fn the_window_says_when_a_drop_happened_beside_the_workers_own_note() {
        // Both sources of a gap are reported, and neither is reported as the
        // other: a worker that dropped lines did not fill this window.
        let mut log = ActivityLog::default();
        log.limit = 1;
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "old", 0)],
            1,
        ));
        log.take_page(page_from(
            "worker-1:0",
            vec![message(LogLevel::Info, "new", 1)],
            2,
        ));
        let view = log.view();
        assert!(view.over_capacity);
        assert!(!view.truncated, "this window dropped them, not the worker");
        assert!(view.summary.contains("dropped"), "{}", view.summary);
    }
}
