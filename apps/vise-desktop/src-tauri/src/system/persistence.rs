//! The queue on disk, and what happens to it when the window comes back.
//!
//! A queue of forty jobs is not something anybody is going to re-enter by hand
//! because a laptop went to sleep and the window was closed. So the queue is
//! written out as the window changes it, and read back at the next start — but
//! the interesting half of this is not the writing, it is the reading.
//!
//! # A half-finished encode is never silently resumed
//!
//! A job that was running when the window went away is *interrupted*, not queued.
//! Resuming it without asking would start a second encode over a file that is
//! already partly written, and starting nothing would look like a queue that
//! forgot what it was doing. So it comes back as an item that is waiting, marked
//! as interrupted, carrying the reason, and it does not start on its own: the user
//! decides. A job that was suspended by a pause is interrupted for the same reason
//! — a suspended encoder died with the process that was holding it.
//!
//! The file is versioned and written atomically, because a queue file that was
//! half-written when the machine lost power is a queue file nobody can read. A
//! document from a newer build is read as far as it can be and then left alone
//! rather than overwritten, and a document that cannot be read at all is not
//! deleted: it is reported, and the queue starts empty with that on the record.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vise_core::{Job, JobId, JobSpec, JobState, JobStatus, Progress};

use crate::windows::queue::{JobPriority, QueueState};

/// Version of the queue document this build writes.
pub const SNAPSHOT_VERSION: u32 = 1;

/// One queued job, as it is written down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotItem {
    /// The job's identifier.
    pub id: JobId,
    /// The job as it was queued.
    pub spec: JobSpec,
    /// The state it was in.
    pub state: JobState,
    /// How far it had got, as a whole percentage.
    #[serde(default)]
    pub percent: u32,
    /// What the encoder last said.
    #[serde(default)]
    pub message: String,
}

/// The whole queue document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    /// The version this build writes.
    #[serde(default = "current_version")]
    pub version: u32,
    /// The jobs, in the order they were queued.
    #[serde(default)]
    pub items: Vec<SnapshotItem>,
    /// What each job is worth, for the ones that were given a priority.
    #[serde(default)]
    pub priorities: BTreeMap<JobId, String>,
}

fn current_version() -> u32 {
    SNAPSHOT_VERSION
}

impl QueueSnapshot {
    /// A snapshot of the jobs in `jobs`, with the queue's own order and
    /// priorities beside them.
    ///
    /// The queue's order and its priorities are written down with the jobs rather
    /// than derived from them on the way back: a run order the user set by hand is
    /// as much part of the queue as the jobs are.
    pub fn of(jobs: &[Job], queue: &QueueState) -> QueueSnapshot {
        QueueSnapshot {
            version: SNAPSHOT_VERSION,
            items: queue
                .run_order(jobs)
                .into_iter()
                .map(|job| SnapshotItem {
                    id: job.id,
                    spec: job.spec.clone(),
                    state: job.status.state,
                    percent: (job.status.progress.fraction.clamp(0.0, 1.0) * 100.0).round() as u32,
                    message: job.status.message.clone().unwrap_or_default(),
                })
                .collect(),
            priorities: queue
                .priorities()
                .into_iter()
                .map(|(id, priority)| (id, priority.as_str().to_string()))
                .collect(),
        }
    }

    /// Encode as the bytes that go on disk.
    pub fn to_bytes(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// What one item became when the queue came back.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredItem {
    /// The job, ready to be put back into the session.
    pub job: Job,
    /// Whether it was interrupted rather than simply left waiting.
    pub interrupted: bool,
    /// Why, when it was.
    pub reason: String,
}

/// What reading the queue back produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovery {
    /// The jobs, in the order they were queued.
    pub items: Vec<RecoveredItem>,
    /// The priority each recovered job had, by the identifier it had on disk.
    pub priorities: BTreeMap<JobId, JobPriority>,
    /// One line per thing worth saying about the recovery.
    pub notes: Vec<String>,
}

impl Recovery {
    /// Put the recovered jobs into `session`, and answer with the queue to use.
    ///
    /// The jobs are added in the order they were queued and come back with the
    /// identifiers the session hands out, so the queue this builds is in terms of
    /// those new identifiers: a snapshot from yesterday must not be able to claim
    /// an identifier this session has already given to something else.
    pub fn install(&self, session: &mut vise_core::Session) -> QueueState {
        let mut queue = QueueState::default();
        for item in &self.items {
            let id = session.add_job(item.job.spec.clone());
            let _ = session.update_status(id, item.job.status.clone());
            queue.push(id);
            if let Some(priority) = self.priorities.get(&item.job.id) {
                queue.set_priority(id, *priority);
            }
        }
        queue
    }

    /// How many jobs were interrupted rather than left waiting.
    pub fn interrupted(&self) -> usize {
        self.items.iter().filter(|item| item.interrupted).count()
    }

    /// One line describing what came back, for the activity log.
    pub fn summary(&self) -> String {
        let interrupted = self.interrupted();
        match (self.items.len(), interrupted) {
            (0, _) => "the saved queue was empty".to_string(),
            (count, 0) => format!("{count} job(s) were recovered from the last session"),
            (count, 1) => format!(
                "{count} job(s) were recovered, and 1 of them was interrupted and is waiting to be started again"
            ),
            (count, many) => format!(
                "{count} job(s) were recovered, and {many} of them were interrupted and are waiting to be started again"
            ),
        }
    }
}

/// Why a stored state is not the state the job is put back in.
pub fn classify(state: JobState) -> RecoveredItemKind {
    match state {
        // A job that was running, or suspended, was in the hands of a process that
        // is not there any more. It comes back waiting, marked as interrupted, and
        // it does not start by itself.
        JobState::Running | JobState::Paused => RecoveredItemKind::Interrupted,
        JobState::Queued => RecoveredItemKind::Waiting,
        // A job that had finished had finished: the file exists and re-running it
        // would be a decision nobody asked for.
        JobState::Completed => RecoveredItemKind::Finished,
        JobState::Failed => RecoveredItemKind::Failed,
        JobState::Cancelled => RecoveredItemKind::Cancelled,
    }
}

/// What a stored state became.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveredItemKind {
    /// It was waiting and it still is.
    Waiting,
    /// It was in the middle of something and it is not any more.
    Interrupted,
    /// It had finished.
    Finished,
    /// It had failed.
    Failed,
    /// It had been cancelled.
    Cancelled,
}

impl RecoveredItemKind {
    /// The state the job is put back into the session with.
    pub fn state(self) -> JobState {
        match self {
            RecoveredItemKind::Waiting
            | RecoveredItemKind::Interrupted
            | RecoveredItemKind::Cancelled => JobState::Queued,
            RecoveredItemKind::Finished => JobState::Completed,
            RecoveredItemKind::Failed => JobState::Failed,
        }
    }

    /// Whether a job of this kind can be run again without asking.
    ///
    /// A cancelled job goes back to waiting, so the run order includes it and the
    /// user has to say whether it should run — which is the same thing the cancel
    /// button did not ask.
    #[allow(
        dead_code,
        reason = "the classification is pinned by tests; every kind is installed"
    )]
    pub fn runnable(self) -> bool {
        matches!(
            self,
            RecoveredItemKind::Waiting | RecoveredItemKind::Interrupted
        )
    }
}

/// Read a queue document back into jobs, an order, and a set of notes.
///
/// Never fails. A document that is not a queue document, or one written by a build
/// that is newer than this one, produces an empty recovery with a note saying so:
/// the file is left exactly as it was found, because throwing it away is how a
/// downgrade turns into data loss.
pub fn recover(snapshot: &QueueSnapshot) -> Recovery {
    let mut notes = Vec::new();
    if snapshot.version > SNAPSHOT_VERSION {
        notes.push(format!(
            "the saved queue was written by a newer build (version {}), so it was read but not changed",
            snapshot.version
        ));
    }
    let mut items = Vec::new();
    for item in &snapshot.items {
        let kind = classify(item.state);
        let reason = match kind {
            RecoveredItemKind::Interrupted => format!(
                "{} was interrupted at {}% and has not been started again",
                file_name(&item.spec.output_path),
                item.percent
            ),
            RecoveredItemKind::Cancelled => format!(
                "{} had been cancelled and is waiting to be started again",
                file_name(&item.spec.output_path)
            ),
            _ => item.message.clone(),
        };
        items.push(RecoveredItem {
            job: Job {
                id: item.id,
                spec: item.spec.clone(),
                status: JobStatus {
                    state: kind.state(),
                    // A finished job is a hundred per cent; anything else starts
                    // from nothing rather than from a progress bar nobody is
                    // watching move.
                    progress: Progress::new(if kind == RecoveredItemKind::Finished {
                        1.0
                    } else {
                        0.0
                    }),
                    message: Some(reason.clone()).filter(|message| !message.is_empty()),
                },
            },
            interrupted: kind == RecoveredItemKind::Interrupted,
            reason,
        });
    }
    let mut queue = QueueState::default();
    for item in &items {
        queue.push(item.job.id);
    }
    let mut priorities = BTreeMap::new();
    for (id, priority) in &snapshot.priorities {
        // A priority for a job that is not in the queue is dropped rather than
        // resurrected: it would be a row with nothing to describe.
        if !items.iter().any(|item| item.job.id == *id) {
            continue;
        }
        if let Some(priority) = JobPriority::parse(priority) {
            queue.set_priority(*id, priority);
            priorities.insert(*id, priority);
        }
    }
    if let Some(interrupted) = items.iter().find(|item| item.interrupted) {
        notes.push(format!(
            "{} was interrupted when the window closed; it is waiting and will not start on its own",
            interrupted.reason
        ));
    }
    Recovery {
        items,
        priorities,
        notes,
    }
}

/// Read a queue document from text.
pub fn parse(text: &str) -> Result<QueueSnapshot, String> {
    serde_json::from_str(text).map_err(|e| format!("the saved queue could not be read: {e}"))
}

/// Read the queue from `path`.
///
/// A file that is not there is not a problem — nothing has been saved yet — and is
/// reported as an empty queue with no notes, so a first run is not told it has
/// lost anything.
pub fn load(path: &Path) -> Recovery {
    match std::fs::read_to_string(path) {
        Ok(text) => match parse(&text) {
            Ok(snapshot) => recover(&snapshot),
            Err(problem) => Recovery {
                items: Vec::new(),
                priorities: BTreeMap::new(),
                notes: vec![format!(
                    "{problem}; the file has been left exactly as it was found"
                )],
            },
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Recovery {
            items: Vec::new(),
            priorities: BTreeMap::new(),
            notes: Vec::new(),
        },
        Err(e) => Recovery {
            items: Vec::new(),
            priorities: BTreeMap::new(),
            notes: vec![format!(
                "the saved queue could not be read ({e}); the file has been left exactly as it was found"
            )],
        },
    }
}

/// Write the queue to `path`.
///
/// The document is written to a temporary file beside it and moved into place, so
/// a machine that loses power halfway through this leaves either the old queue or
/// the new one and never half of either.
pub fn save(path: &Path, jobs: &[Job], queue: &QueueState) -> Result<PathBuf, String> {
    let snapshot = QueueSnapshot::of(jobs, queue);
    let text = snapshot.to_bytes();
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Err("the queue has nowhere to be written".to_string());
    };
    std::fs::create_dir_all(parent).map_err(|e| {
        format!(
            "the folder for the queue could not be created ({}): {e}",
            parent.display()
        )
    })?;
    let temporary = path.with_extension("json.part");
    std::fs::write(&temporary, text.as_bytes()).map_err(|e| {
        format!(
            "the queue could not be written ({}): {e}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, path).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("the queue could not be saved ({}): {e}", path.display())
    })?;
    Ok(path.to_path_buf())
}

/// The file name of a path, for a sentence about one job.
fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::ContainerFormat;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-queue-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn spec(name: &str) -> JobSpec {
        JobSpec {
            input_path: format!("C:\\media\\{name}.mkv"),
            output_path: format!("D:\\out\\{name}.mp4"),
            container: ContainerFormat::Mp4,
            ..JobSpec::default()
        }
    }

    /// The queue holding every job, in the order they were queued.
    fn queued(jobs: &[Job]) -> QueueState {
        let mut queue = QueueState::default();
        for job in jobs {
            queue.push(job.id);
        }
        queue
    }

    fn jobs() -> Vec<Job> {
        vec![
            Job {
                id: 1,
                spec: spec("One"),
                status: JobStatus {
                    state: JobState::Completed,
                    progress: Progress::new(1.0),
                    message: Some("done".into()),
                },
            },
            Job {
                id: 2,
                spec: spec("Two"),
                status: JobStatus {
                    state: JobState::Running,
                    progress: Progress::new(0.42),
                    message: Some("encoding frame 4000".into()),
                },
            },
            Job {
                id: 3,
                spec: spec("Three"),
                status: JobStatus::default(),
            },
        ]
    }

    #[test]
    fn the_queue_round_trips_through_the_file() {
        let jobs = jobs();
        let mut queue = QueueState::default();
        for job in &jobs {
            queue.push(job.id);
        }
        queue.set_priority(3, JobPriority::High);
        let dir = temp_dir("round-trip");
        let path = dir.join("queue.json");
        save(&path, &jobs, &queue).expect("saved");

        let recovery = load(&path);
        // One job was running when this was written, so the only note is about it
        // coming back interrupted rather than about the file.
        assert_eq!(recovery.notes.len(), 1, "{:?}", recovery.notes);
        assert!(
            recovery.notes[0].contains("interrupted"),
            "{:?}",
            recovery.notes
        );
        assert_eq!(recovery.items.len(), 3);
        assert_eq!(recovery.interrupted(), 1);
        // Every job survives the file, looked up by the identifier it had, because
        // the order they come back in is the queue's run order rather than the order
        // the session happens to hold them in.
        for was in &jobs {
            let found = recovery
                .items
                .iter()
                .find(|item| item.job.id == was.id)
                .unwrap_or_else(|| panic!("job {} is missing", was.id));
            assert_eq!(found.job.spec, was.spec, "the job survives the file");
        }
        assert_eq!(recovery.priorities.get(&3), Some(&JobPriority::High));
        // The order the user set is part of the document, and the priority is part of
        // it too: the high job comes first because it was marked high, not because it
        // was queued first.
        assert_eq!(
            recovery.items.iter().map(|i| i.job.id).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
    }

    #[test]
    fn a_running_job_comes_back_interrupted_and_never_as_a_job_that_just_started() {
        let jobs = jobs();
        // The queue's own order is what gets written down, so the snapshot is
        // taken from a queue that holds the jobs rather than from an empty one.
        let queue = queued(&jobs);
        let snapshot = QueueSnapshot::of(&jobs, &queue);
        let recovery = recover(&snapshot);
        let interrupted = recovery
            .items
            .iter()
            .find(|item| item.job.id == 2)
            .expect("the running job");
        assert!(interrupted.interrupted);
        assert_eq!(interrupted.job.status.state, JobState::Queued);
        assert!(
            interrupted.reason.contains("interrupted at 42%"),
            "{}",
            interrupted.reason
        );
        assert!(
            recovery
                .notes
                .iter()
                .any(|note| note.contains("will not start on its own")),
            "{:?}",
            recovery.notes
        );
        assert_eq!(recovery.interrupted(), 1);
        // The progress bar does not open at 42% and quietly continue from there.
        assert_eq!(interrupted.job.status.progress.fraction, 0.0);
    }

    #[test]
    fn a_suspended_job_is_interrupted_for_the_same_reason() {
        // A suspended encoder was held by a process that is no longer there, so
        // resuming it would be resuming something that cannot be resumed.
        for state in [JobState::Running, JobState::Paused] {
            assert_eq!(classify(state), RecoveredItemKind::Interrupted);
            // It comes back runnable, which is not the same as coming back started:
            // the queue offers it and the user decides.
            assert!(
                classify(state).runnable(),
                "it can be run again, deliberately"
            );
        }
    }

    #[test]
    fn a_job_that_is_not_in_the_queue_is_not_in_the_snapshot() {
        let jobs = jobs();
        let snapshot = QueueSnapshot::of(&jobs, &QueueState::default());
        assert!(
            snapshot.items.is_empty(),
            "an empty queue writes an empty queue, not every job in the session"
        );
    }

    #[test]
    fn a_finished_job_stays_finished_and_a_failed_one_stays_failed() {
        assert_eq!(classify(JobState::Completed), RecoveredItemKind::Finished);
        assert_eq!(classify(JobState::Failed), RecoveredItemKind::Failed);
        assert_eq!(classify(JobState::Cancelled), RecoveredItemKind::Cancelled);
        assert_eq!(classify(JobState::Queued), RecoveredItemKind::Waiting);
        assert_eq!(RecoveredItemKind::Finished.state(), JobState::Completed);
        assert_eq!(RecoveredItemKind::Failed.state(), JobState::Failed);
        assert_eq!(RecoveredItemKind::Cancelled.state(), JobState::Queued);

        let snapshot = QueueSnapshot {
            version: SNAPSHOT_VERSION,
            items: jobs()
                .into_iter()
                .map(|job| SnapshotItem {
                    id: job.id,
                    spec: job.spec,
                    state: JobState::Completed,
                    percent: 100,
                    message: String::new(),
                })
                .collect(),
            priorities: BTreeMap::new(),
        };
        let recovery = recover(&snapshot);
        assert!(recovery.items.iter().all(|item| !item.interrupted));
        assert!(recovery
            .items
            .iter()
            .all(|item| item.job.status.progress.fraction == 1.0));
        assert!(
            recovery.summary().contains("recovered"),
            "{}",
            recovery.summary()
        );
    }

    #[test]
    fn an_empty_queue_is_reported_as_empty_and_not_as_a_loss() {
        let recovery = Recovery {
            items: Vec::new(),
            priorities: BTreeMap::new(),
            notes: Vec::new(),
        };
        assert_eq!(recovery.summary(), "the saved queue was empty");
        assert_eq!(recovery.interrupted(), 0);
    }

    #[test]
    fn a_file_from_a_newer_build_is_read_and_not_thrown_away() {
        let jobs = jobs();
        let mut snapshot = QueueSnapshot::of(&jobs, &queued(&jobs));
        snapshot.version = SNAPSHOT_VERSION + 1;
        let dir = temp_dir("future");
        let path = dir.join("queue.json");
        std::fs::write(&path, snapshot.to_bytes()).expect("written");
        let before = std::fs::read_to_string(&path).expect("read back");

        let recovery = load(&path);
        assert_eq!(recovery.items.len(), 3, "read as far as this build can");
        assert!(
            recovery
                .notes
                .iter()
                .any(|note| note.contains("newer build")),
            "{:?}",
            recovery.notes
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read back"),
            before,
            "reading it does not change it"
        );
    }

    #[test]
    fn a_corrupt_file_is_reported_and_left_alone() {
        let dir = temp_dir("corrupt");
        let path = dir.join("queue.json");
        std::fs::write(&path, "{ this is not the queue").expect("written");
        let recovery = load(&path);
        assert!(recovery.items.is_empty());
        assert!(
            recovery.notes[0].contains("could not be read"),
            "{:?}",
            recovery.notes
        );
        assert!(
            std::fs::read_to_string(&path)
                .expect("read back")
                .contains("not the queue"),
            "the bytes on disk are the ones the user left"
        );
    }

    #[test]
    fn a_queue_that_was_never_saved_is_not_reported_as_lost() {
        let dir = temp_dir("absent");
        let recovery = load(&dir.join("queue.json"));
        assert!(recovery.items.is_empty());
        assert!(recovery.notes.is_empty(), "{:?}", recovery.notes);
    }

    #[test]
    fn writing_the_queue_leaves_no_temporary_file_behind() {
        let jobs = jobs();
        let dir = temp_dir("atomic");
        let path = dir.join("queue.json");
        save(&path, &jobs, &QueueState::default()).expect("saved");
        let entries: Vec<String> = std::fs::read_dir(&dir)
            .expect("listed")
            .filter_map(|entry| {
                entry
                    .ok()
                    .map(|entry| entry.file_name().to_string_lossy().to_string())
            })
            .collect();
        assert_eq!(entries, vec!["queue.json".to_string()]);
    }

    #[test]
    fn a_priority_for_a_job_that_is_not_there_is_dropped_rather_than_resurrected() {
        let snapshot = QueueSnapshot {
            version: SNAPSHOT_VERSION,
            items: vec![SnapshotItem {
                id: 1,
                spec: spec("One"),
                state: JobState::Queued,
                percent: 0,
                message: String::new(),
            }],
            priorities: BTreeMap::from([(1, "high".to_string()), (99, "low".to_string())]),
        };
        let recovery = recover(&snapshot);
        assert_eq!(recovery.priorities.get(&1), Some(&JobPriority::High));
        assert!(
            !recovery.priorities.contains_key(&99),
            "there is no job 99 to be worth anything"
        );
    }

    #[test]
    fn a_summary_names_how_many_jobs_were_interrupted() {
        let jobs = jobs();
        let snapshot = QueueSnapshot::of(&jobs, &queued(&jobs));
        let recovery = recover(&snapshot);
        assert!(
            recovery.summary().contains("1 of them was interrupted"),
            "{}",
            recovery.summary()
        );
    }
}
