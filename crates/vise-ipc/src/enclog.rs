//! Per-encode log files: how they are named, and which of them are kept.
//!
//! The worker keeps one log for the whole session and one log per encode. The
//! per-encode file is what a person attaches to a bug report: a queue of forty
//! jobs has forty questions attached to it, and "the log" cannot answer any of
//! them on its own.
//!
//! # A long queue cannot fill the disk
//!
//! One file per job is one more way for a machine that is left encoding over a
//! weekend to run out of room, so the files are trimmed. Both bounds — the
//! number of files kept and the total number of bytes kept — are enforced by
//! [`decide_rotation`], which is a **pure function of the policy, the files,
//! and their measured sizes and ages**. Nothing in it touches a filesystem,
//! reads a clock, or decides anything about which file is open: the caller
//! measures, and the function decides. That is what makes the awkward cases —
//! a file being written right now, a file somebody has open in an editor —
//! testable rather than hopeful.
//!
//! # Nothing deletes a log somebody is reading
//!
//! A file carries a [`LogRole`]. [`LogRole::InProgress`] is the encode log the
//! worker is appending to, and [`LogRole::BeingRead`] is one the diagnostics
//! surface has handed to a viewer. Neither is ever in the removal list, and
//! neither is counted as disposable even when it is the oldest and largest
//! thing in the folder. The cost is that a run of very large logs can exceed
//! the byte bound; the alternative is a support bundle that references a file
//! which no longer exists.
//!
//! # Where the files go
//!
//! Under the resolved storage root, so a portable install keeps its logs beside
//! the executable and an installed one keeps them in the configuration
//! directory. The root is resolved by [`crate::portable`] as
//! [`crate::portable::StorageKind::Logs`]; this module only names the files
//! inside that folder.

use std::path::Path;

/// Prefix of one encode's log file.
pub const ENCODE_LOG_PREFIX: &str = "encode-";

/// Suffix of one encode's log file.
pub const ENCODE_LOG_SUFFIX: &str = ".log";

/// Longest source label kept in a log file name.
///
/// Long enough for a title and a quality value, short enough that the name
/// still fits on one line of a file list with a window behind it.
pub const MAX_LABEL_LENGTH: usize = 48;

/// Name used when a label has nothing usable left in it.
pub const FALLBACK_LABEL: &str = "job";

/// Reduce a path or title to something safe to put in a file name.
///
/// Everything outside `[A-Za-z0-9._-]` becomes a hyphen, runs of hyphens
/// collapse, and the result is trimmed and capped. A title called
/// `Episode 1: "The Return", Part 2` becomes `Episode-1-The-Return-Part-2`,
/// which is what a person would have typed, rather than a name no file system
/// accepts and no person reads.
pub fn sanitize_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_LABEL_LENGTH));
    let mut pending_separator = false;
    for character in raw.chars() {
        if out.chars().count() >= MAX_LABEL_LENGTH {
            break;
        }
        let keep = character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
        if keep {
            if pending_separator && !out.is_empty() {
                out.push('-');
            }
            pending_separator = false;
            out.push(character);
        } else if !out.is_empty() {
            pending_separator = true;
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        FALLBACK_LABEL.to_string()
    } else {
        trimmed
    }
}

/// The log file name one encode writes to.
pub fn encode_log_name(label: &str) -> String {
    format!(
        "{ENCODE_LOG_PREFIX}{}{ENCODE_LOG_SUFFIX}",
        sanitize_label(label)
    )
}

/// Whether a name is one of this build's encode logs.
///
/// The folder belongs to the application, so anything that is not one of these
/// is left exactly as it was found rather than being tidied away.
pub fn is_encode_log_name(name: &str) -> bool {
    name.starts_with(ENCODE_LOG_PREFIX) && name.ends_with(ENCODE_LOG_SUFFIX)
}

/// Why a log file must not be removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRole {
    /// A finished log, waiting to be trimmed if the folder is over its bounds.
    Retained,
    /// The log the worker is appending to right now.
    InProgress,
    /// A log a viewer has open, or the surface has shown.
    BeingRead,
}

impl LogRole {
    /// Whether the policy may remove a file in this role.
    pub fn is_removable(self) -> bool {
        self == LogRole::Retained
    }
}

/// One log file, as the policy sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFile {
    /// File name inside the log folder.
    pub name: String,
    /// Size on disk.
    pub bytes: u64,
    /// Seconds since it was last written.
    ///
    /// Measured by the caller rather than read from a clock here, so that the
    /// order two files are removed in is a fact the test states rather than a
    /// fact about how long the test took to run.
    pub age_seconds: u64,
    /// Why it must not be removed, when it must not.
    pub role: LogRole,
}

impl LogFile {
    /// A retained file of `bytes`, last written `age_seconds` ago.
    pub fn retained(name: &str, bytes: u64, age_seconds: u64) -> Self {
        LogFile {
            name: name.to_string(),
            bytes,
            age_seconds,
            role: LogRole::Retained,
        }
    }

    /// The same file in another role.
    pub fn with_role(mut self, role: LogRole) -> Self {
        self.role = role;
        self
    }
}

/// How much of the log folder is allowed to exist at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPolicy {
    /// Total bytes the folder may hold, including the file being written.
    pub max_bytes: u64,
    /// Total files the folder may hold, including the file being written.
    pub max_files: usize,
}

impl Default for RotationPolicy {
    /// Four megabytes across twenty files.
    ///
    /// Twenty encodes of history is enough to answer "what happened last
    /// night", and four megabytes is small enough that nobody has noticed it
    /// missing on a machine with a nearly full system drive.
    fn default() -> Self {
        RotationPolicy {
            max_bytes: 4 * 1024 * 1024,
            max_files: 20,
        }
    }
}

/// What the policy decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationPlan {
    /// Whether the file being written has reached the size bound.
    ///
    /// The caller acts on this by closing the current file and letting the next
    /// encode open a new one. Nothing is moved or truncated underneath a write,
    /// which is the only way a half-written line survives in a log somebody is
    /// going to read.
    pub rotate: bool,
    /// Files to remove, oldest first.
    pub remove: Vec<String>,
    /// Files to keep, by name.
    pub keep: Vec<String>,
    /// Bytes the folder will hold once the removals have happened.
    pub bytes_kept: u64,
}

impl RotationPlan {
    /// A plan that changes nothing.
    pub fn none() -> Self {
        RotationPlan {
            rotate: false,
            remove: Vec::new(),
            keep: Vec::new(),
            bytes_kept: 0,
        }
    }

    /// Whether the plan would delete anything.
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty()
    }
}

/// Decide what the log folder should look like after this encode.
///
/// The decision is total and pure: every file either appears in `keep` or in
/// `remove`, protected files are never in `remove`, and the two lists are in a
/// fixed order so two runs over the same folder produce the same plan.
///
/// Removal runs oldest first and stops as soon as the folder is inside both
/// bounds, so a folder that is only over the byte bound loses its oldest
/// large logs and not its newest small ones.
pub fn decide_rotation(policy: &RotationPolicy, files: &[LogFile]) -> RotationPlan {
    // Oldest first. A tie is broken by name so that two files written in the
    // same second still have a defined order.
    let mut ordered: Vec<&LogFile> = files.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .age_seconds
            .cmp(&left.age_seconds)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut kept: Vec<&LogFile> = ordered.clone();
    let mut bytes_kept: u64 = ordered.iter().map(|file| file.bytes).sum();
    let mut remove: Vec<String> = Vec::new();

    for file in &ordered {
        if kept.len() <= policy.max_files && bytes_kept <= policy.max_bytes {
            break;
        }
        if !file.role.is_removable() {
            continue;
        }
        kept.retain(|candidate| candidate.name != file.name);
        bytes_kept = bytes_kept.saturating_sub(file.bytes);
        remove.push(file.name.clone());
    }

    kept.sort_by(|left, right| left.name.cmp(&right.name));
    RotationPlan {
        // The size bound is judged on the file being written, because that is
        // the one that grows. A folder that is over its byte bound only because
        // of twenty finished logs is handled by the removals below.
        rotate: ordered
            .iter()
            .find(|file| file.role == LogRole::InProgress)
            .is_some_and(|file| file.bytes >= policy.max_bytes),
        keep: kept.iter().map(|file| file.name.clone()).collect(),
        remove,
        bytes_kept,
    }
}

/// The files in `directory` that this build's encode logs, measured now.
///
/// A folder that cannot be listed is an empty list rather than an error: a
/// machine with no logs yet and a machine with an unreadable log folder are the
/// same problem to the person waiting for an encode to start, and neither
/// should stop one from starting.
pub fn list_encode_logs(directory: &Path, now_seconds: u64) -> Vec<LogFile> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<LogFile> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_encode_log_name(&name) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_secs())
                .unwrap_or(now_seconds);
            Some(LogFile::retained(
                &name,
                metadata.len(),
                now_seconds.saturating_sub(modified),
            ))
        })
        .collect();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    files
}

/// Seconds since the Unix epoch, for the caller that measures a folder.
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(max_bytes: u64, max_files: usize) -> RotationPolicy {
        RotationPolicy {
            max_bytes,
            max_files,
        }
    }

    #[test]
    fn a_title_becomes_a_name_a_file_system_and_a_person_both_accept() {
        assert_eq!(
            encode_log_name("Episode 1: \"The Return\", Part 2"),
            "encode-Episode-1-The-Return-Part-2.log"
        );
        assert_eq!(sanitize_label("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_label("   "), FALLBACK_LABEL);
        assert_eq!(sanitize_label("///"), FALLBACK_LABEL);
    }

    #[test]
    fn a_very_long_title_is_capped_rather_than_rejected() {
        let long = "a".repeat(200);
        let name = sanitize_label(&long);
        assert_eq!(name.len(), MAX_LABEL_LENGTH);
        assert!(encode_log_name(&name).starts_with(ENCODE_LOG_PREFIX));
    }

    #[test]
    fn only_this_build_s_own_files_are_recognised() {
        assert!(is_encode_log_name("encode-a.log"));
        assert!(!is_encode_log_name("vise-worker-12.log"));
        assert!(!is_encode_log_name("encode-a.txt"));
        assert!(!is_encode_log_name("notes.log"));
    }

    #[test]
    fn a_folder_inside_both_bounds_is_left_alone() {
        let files = vec![
            LogFile::retained("encode-a.log", 100, 10),
            LogFile::retained("encode-b.log", 100, 5),
        ];
        let plan = decide_rotation(&policy(1000, 5), &files);
        assert!(plan.is_empty());
        assert!(!plan.rotate);
        assert_eq!(plan.keep, vec!["encode-a.log", "encode-b.log"]);
        assert_eq!(plan.bytes_kept, 200);
    }

    #[test]
    fn the_oldest_file_goes_first_when_the_folder_is_over_its_byte_bound() {
        let files = vec![
            LogFile::retained("encode-old.log", 600, 900),
            LogFile::retained("encode-new.log", 600, 10),
        ];
        let plan = decide_rotation(&policy(1000, 20), &files);
        assert_eq!(plan.remove, vec!["encode-old.log"]);
        assert_eq!(plan.keep, vec!["encode-new.log"]);
        assert_eq!(plan.bytes_kept, 600);
    }

    #[test]
    fn the_oldest_file_goes_first_when_the_folder_is_over_its_file_bound() {
        let files = vec![
            LogFile::retained("encode-a.log", 1, 30),
            LogFile::retained("encode-b.log", 1, 20),
            LogFile::retained("encode-c.log", 1, 10),
        ];
        let plan = decide_rotation(&policy(1_000_000, 2), &files);
        assert_eq!(plan.remove, vec!["encode-a.log"]);
        assert_eq!(plan.keep, vec!["encode-b.log", "encode-c.log"]);
    }

    #[test]
    fn a_log_that_is_being_written_is_never_removed_and_never_moved() {
        // The open file is both the oldest and the largest, which is exactly
        // what happens when one encode writes for a day.
        let files = vec![
            LogFile::retained("encode-open.log", 9000, 10_000).with_role(LogRole::InProgress),
            LogFile::retained("encode-finished.log", 9000, 1),
        ];
        let plan = decide_rotation(&policy(1000, 1), &files);
        assert!(
            plan.keep.contains(&"encode-open.log".to_string()),
            "the file being written survives: {plan:?}"
        );
        assert_eq!(plan.remove, vec!["encode-finished.log"]);
        assert!(
            plan.rotate,
            "and the caller is told to close it for the next encode"
        );
    }

    #[test]
    fn a_log_the_user_is_reading_is_never_removed() {
        let files = vec![
            LogFile::retained("encode-open.log", 100, 5).with_role(LogRole::InProgress),
            LogFile::retained("encode-reading.log", 100, 500).with_role(LogRole::BeingRead),
        ];
        let plan = decide_rotation(&policy(1, 1), &files);
        assert!(plan.is_empty(), "nothing else was disposable: {plan:?}");
        assert_eq!(plan.keep.len(), 2);
        assert_eq!(plan.bytes_kept, 200);
    }

    #[test]
    fn an_encounter_of_protected_files_leaves_the_bound_unmet_rather_than_deleting_one() {
        let files = vec![
            LogFile::retained("encode-a.log", 10, 30).with_role(LogRole::BeingRead),
            LogFile::retained("encode-b.log", 10, 20).with_role(LogRole::BeingRead),
            LogFile::retained("encode-c.log", 10, 10).with_role(LogRole::BeingRead),
        ];
        let plan = decide_rotation(&policy(1, 1), &files);
        assert!(plan.is_empty());
        assert_eq!(plan.keep.len(), 3);
        assert_eq!(
            plan.bytes_kept, 30,
            "and it says so rather than claiming to have met the bound"
        );
    }

    #[test]
    fn rotation_is_only_asked_for_once_the_open_file_reaches_the_bound() {
        let small = vec![LogFile::retained("encode-a.log", 99, 1).with_role(LogRole::InProgress)];
        assert!(!decide_rotation(&policy(100, 5), &small).rotate);
        let full = vec![LogFile::retained("encode-a.log", 100, 1).with_role(LogRole::InProgress)];
        assert!(decide_rotation(&policy(100, 5), &full).rotate);
        // With nothing open there is nothing to rotate, however full the folder is.
        let closed = vec![LogFile::retained("encode-a.log", 500, 1)];
        assert!(!decide_rotation(&policy(100, 5), &closed).rotate);
    }

    #[test]
    fn every_file_lands_in_exactly_one_list_and_the_lists_are_stable() {
        let files = vec![
            LogFile::retained("encode-b.log", 700, 20),
            LogFile::retained("encode-a.log", 700, 40),
            LogFile::retained("encode-c.log", 700, 30).with_role(LogRole::BeingRead),
        ];
        let first = decide_rotation(&policy(1500, 10), &files);
        let second = decide_rotation(&policy(1500, 10), &files);
        assert_eq!(first, second, "the same folder produces the same plan");
        assert_eq!(first.remove, vec!["encode-a.log"]);
        assert_eq!(first.keep, vec!["encode-b.log", "encode-c.log"]);
        assert!(
            first.keep.iter().all(|name| !first.remove.contains(name)),
            "no file is in both lists"
        );
        assert_eq!(first.keep.len() + first.remove.len(), files.len());
    }

    #[test]
    fn a_folder_that_cannot_be_listed_is_empty_rather_than_an_error() {
        let missing = std::env::temp_dir().join("vise-enclog-absent-folder");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(list_encode_logs(&missing, 0).is_empty());
    }

    #[test]
    fn listing_measures_the_files_that_are_there_and_ignores_the_rest() {
        let dir = std::env::temp_dir().join("vise-enclog-listing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("encode-a.log"), b"0123456789").expect("write");
        std::fs::write(dir.join("vise-worker-1.log"), b"session").expect("write");
        std::fs::create_dir_all(dir.join("encode-folder.log")).expect("dir");

        let files = list_encode_logs(&dir, 0);
        assert_eq!(files.len(), 1, "only this build's own files");
        assert_eq!(files[0].name, "encode-a.log");
        assert_eq!(files[0].bytes, 10);
        assert_eq!(files[0].role, LogRole::Retained);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
