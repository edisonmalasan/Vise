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
}

impl LogLine {
    /// A line as the window records it.
    pub fn new(index: u64, level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        Self {
            index,
            level,
            source,
            message: message.into(),
        }
    }
}

/// Every level, least to most verbose, for the filter control.
pub fn levels() -> [LogLevel; 4] {
    [
        LogLevel::Error,
        LogLevel::Warning,
        LogLevel::Info,
        LogLevel::Debug,
    ]
}

/// The name of a level, as the filter and each row show it.
pub fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "Debug",
        LogLevel::Info => "Info",
        LogLevel::Warning => "Warning",
        LogLevel::Error => "Error",
    }
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
    worker_seen: BTreeSet<usize>,
    /// Whether the page should keep the newest line in view.
    follow: bool,
    /// The filter floor.
    min_level: Option<LogLevel>,
    next_index: u64,
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
    pub fn take_page(&mut self, page: LogMessagesFromIndex) -> usize {
        let taken = page.messages.len();
        for message in &page.messages {
            self.append_worker(message);
        }
        self.cursor = page.next_index.max(self.cursor);
        self.truncated = self.truncated || page.truncated;
        taken
    }

    /// The index the next worker page starts at.
    pub fn cursor(&self) -> usize {
        self.cursor
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
        self.lines.push(LogLine::new(
            self.next_index,
            message.log_level,
            LogSource::Worker,
            message.message.clone(),
        ));
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
            summary: if total == 0 {
                "Nothing has been recorded yet.".to_string()
            } else if hidden == 0 {
                format!("{total} line(s).")
            } else {
                format!("{total} line(s), showing {}.", lines.len())
            },
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
}
