//! Failure attribution for a worker that died mid-job.
//!
//! A worker process is the thing that crashes, not the application and not the
//! rest of the queue. The distinction matters because the three outcomes look
//! alike from the queue's side — nothing is answering, nothing is progressing —
//! while meaning three different things:
//!
//! - the encode finished, and the worker said how;
//! - the user asked for the encode to stop;
//! - the worker died, and the job it was running died with it.
//!
//! This module decides which of the three happened and what the queue should do
//! about it, as a pure function of the outcome and of what has been seen before.
//! Nothing here touches a process or a file: the supervisor supplies what a dead
//! worker looked like, and the window performs the tidying. That is what lets
//! every rule below be exercised without starting an encoder.
//!
//! The rules are:
//!
//! - a job whose worker died fails **on its own**, with the exit status, and the
//!   queue carries on with the next job on a fresh worker;
//! - the same goes for an ordinary encode failure: the worker answered, so it is
//!   healthy and the streak is cleared;
//! - a stop the user asked for is a cancellation, never a crash, whichever side of
//!   the worker dying it lands on;
//! - a worker that dies on consecutive jobs is not replaced forever. The
//!   replacements get further apart, and once the streak reaches the threshold the
//!   queue stops and says why rather than spinning.

use std::path::{Path, PathBuf};
use std::time::Duration;

use vise_core::JobState;

use crate::contract::error_codes;

/// How many worker deaths in a row the window will replace before it stops.
pub const DEFAULT_DEATH_THRESHOLD: u32 = 3;

/// How long to wait before the second death's replacement, doubled per death.
pub const DEFAULT_DEATH_BACKOFF: Duration = Duration::from_secs(2);

/// What a dead worker process looked like when the parent noticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerDeath {
    /// The process that died.
    pub pid: u32,
    /// Its exit code, where the platform reported one.
    pub code: Option<i32>,
    /// The signal that ended it, where the platform has signals.
    pub signal: Option<i32>,
    /// What was observed, in a sentence.
    pub reason: String,
}

impl WorkerDeath {
    /// A death whose cause was a failure to answer rather than an exit.
    pub fn unexplained(pid: u32, reason: impl Into<String>) -> Self {
        WorkerDeath {
            pid,
            code: None,
            signal: None,
            reason: reason.into(),
        }
    }

    /// The death as one line for a queue row and the log.
    pub fn describe(&self) -> String {
        let outcome = match (self.code, self.signal) {
            (Some(code), _) => format!("exited with code {code}"),
            (None, Some(signal)) => format!("was killed by signal {signal}"),
            (None, None) => "gave no exit status".to_string(),
        };
        format!("the worker process {} {outcome}: {}", self.pid, self.reason)
    }
}

/// What to do about a worker that died.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDecision {
    /// Bring a replacement up, after waiting this long first.
    Restart { delay: Duration },
    /// Do not replace it again on this streak; report instead.
    Backoff { reason: String },
}

/// How the window has been getting on with the worker process.
///
/// The streak is the whole point: one worker dying is an event, and a worker
/// dying on every job in a row is a machine that cannot run this queue. A
/// counter is what tells the two apart, and it is cleared by anything that shows
/// the worker is alive again.
#[derive(Debug, Clone)]
pub struct CrashGuard {
    threshold: u32,
    backoff: Duration,
    consecutive: u32,
    last: Option<WorkerDeath>,
}

impl Default for CrashGuard {
    fn default() -> Self {
        CrashGuard::new()
    }
}

impl CrashGuard {
    /// A guard with the shipped threshold and backoff.
    pub fn new() -> Self {
        CrashGuard::with_policy(DEFAULT_DEATH_THRESHOLD, DEFAULT_DEATH_BACKOFF)
    }

    /// A guard with `threshold` deaths allowed and `backoff` between replacements.
    ///
    /// A threshold of one stops at the first death, which is what a caller that
    /// wants no automatic replacement at all asks for.
    pub fn with_policy(threshold: u32, backoff: Duration) -> Self {
        CrashGuard {
            threshold: threshold.max(1),
            backoff,
            consecutive: 0,
            last: None,
        }
    }

    /// Record a death and say what to do about it.
    pub fn note(&mut self, death: &WorkerDeath) -> CrashDecision {
        self.consecutive += 1;
        self.last = Some(death.clone());
        if self.consecutive >= self.threshold {
            return CrashDecision::Backoff {
                reason: format!(
                    "the worker has died {} time(s) in a row and will not be replaced again: {}",
                    self.consecutive,
                    death.describe()
                ),
            };
        }
        // The first death is replaced at once, because a single crash during a
        // long encode is ordinary. Each later one waits longer, so a queue that
        // keeps killing its worker cannot turn into a restart loop.
        CrashDecision::Restart {
            delay: self.backoff * (self.consecutive - 1),
        }
    }

    /// Clear the streak: a worker that answered is a worker that works.
    pub fn reset(&mut self) {
        self.consecutive = 0;
        self.last = None;
    }

    /// How many deaths have happened without a worker answering in between.
    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }

    /// The most recent death, while the streak stands.
    pub fn last_death(&self) -> Option<&WorkerDeath> {
        self.last.as_ref()
    }

    /// How many deaths are allowed before the queue stops.
    pub fn threshold(&self) -> u32 {
        self.threshold
    }
}

/// What came back from one attempt to run a job.
#[derive(Debug, Clone, PartialEq)]
pub enum EncodeOutcome {
    /// The worker answered, and the encode finished one way or another.
    Finished {
        was_successful: bool,
        message: String,
        error_code: i32,
    },
    /// The worker is not answering any more.
    WorkerGone(WorkerDeath),
}

impl EncodeOutcome {
    /// The worker finished the job.
    pub fn completed(message: impl Into<String>) -> Self {
        EncodeOutcome::Finished {
            was_successful: true,
            message: message.into(),
            error_code: error_codes::NONE,
        }
    }

    /// The worker answered, and the job failed in the encoder.
    pub fn failed(message: impl Into<String>) -> Self {
        EncodeOutcome::Finished {
            was_successful: false,
            message: message.into(),
            error_code: error_codes::FAILED,
        }
    }

    /// The job was refused before it started, which is not an encode failure.
    pub fn refused(message: impl Into<String>) -> Self {
        EncodeOutcome::Finished {
            was_successful: false,
            message: message.into(),
            error_code: error_codes::INVALID,
        }
    }

    /// The job was stopped on request.
    pub fn cancelled(message: impl Into<String>) -> Self {
        EncodeOutcome::Finished {
            was_successful: false,
            message: message.into(),
            error_code: error_codes::CANCELLED,
        }
    }
}

/// What the window does about one attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// The job is finished; record this state on it.
    JobFinished { state: JobState, message: String },
    /// The user asked for this. Not a failure, and never counted as one.
    JobCancelled { message: String },
    /// The worker died holding this job. Fail this job, tidy what it left, and
    /// carry on with the queue.
    JobCrashed {
        reason: String,
        /// The destination whose partial files the worker could not tidy itself.
        output: PathBuf,
        /// How long to wait before the next job starts a fresh worker.
        backoff: Duration,
    },
    /// The worker keeps dying. Stop here and report rather than loop.
    QueueStopped {
        reason: String,
        /// The destination whose partial files the worker could not tidy itself.
        output: PathBuf,
    },
}

impl Resolution {
    /// Whether the queue may go on to the next job.
    pub fn continues_queue(&self) -> bool {
        !matches!(self, Resolution::QueueStopped { .. })
    }

    /// How long to wait before the next job starts, when there is a wait.
    pub fn backoff(&self) -> Duration {
        match self {
            Resolution::JobCrashed { backoff, .. } => *backoff,
            _ => Duration::ZERO,
        }
    }

    /// One line for the activity log.
    pub fn describe(&self) -> String {
        match self {
            Resolution::JobFinished { state, message } => match message.trim() {
                "" => format!("job finished as {}", state_name(*state)),
                other => format!("job {}: {other}", state_name(*state)),
            },
            Resolution::JobCancelled { message } => {
                format!("job cancelled ({})", message.trim())
            }
            Resolution::JobCrashed { reason, .. } => reason.clone(),
            Resolution::QueueStopped { reason, .. } => reason.clone(),
        }
    }
}

/// The state name a job is recorded under, as the queue shows it.
fn state_name(state: JobState) -> String {
    match state {
        JobState::Queued => "queued".to_string(),
        JobState::Running => "running".to_string(),
        JobState::Paused => "paused".to_string(),
        JobState::Completed => "finished".to_string(),
        JobState::Failed => "failed".to_string(),
        JobState::Cancelled => "cancelled".to_string(),
    }
}

/// Attributes one attempt, for one job, against what has been seen before.
///
/// One policy object per running job: the streak it carries is a property of the
/// worker, but the stop it remembers is a property of the job, and mixing the two
/// would let one job's cancellation excuse another job's crash.
#[derive(Debug, Clone, Default)]
pub struct IsolationPolicy {
    guard: CrashGuard,
    stop_requested: bool,
}

impl IsolationPolicy {
    /// A policy with the shipped death threshold and backoff.
    pub fn new() -> Self {
        IsolationPolicy {
            guard: CrashGuard::new(),
            stop_requested: false,
        }
    }

    /// A policy with `threshold` deaths allowed and `backoff` between replacements.
    pub fn with_policy(threshold: u32, backoff: Duration) -> Self {
        IsolationPolicy::with_guard(CrashGuard::with_policy(threshold, backoff))
    }

    /// A policy with an explicit guard.
    pub fn with_guard(guard: CrashGuard) -> Self {
        IsolationPolicy {
            guard,
            stop_requested: false,
        }
    }

    /// Note that the user asked for this job to stop.
    ///
    /// Recorded before the stop is sent, because the worker may be killed while
    /// the request is still in flight and the answer to "was this a crash?" has to
    /// be the same either way.
    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    /// Whether a stop was asked for.
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// The death streak this policy is carrying.
    pub fn guard(&self) -> &CrashGuard {
        &self.guard
    }

    /// Clear the death streak, for a worker that answered.
    pub fn worker_healthy(&mut self) {
        self.guard.reset();
    }

    /// Decide what one attempt means for this job.
    pub fn resolved(&mut self, outcome: EncodeOutcome, output: &Path) -> Resolution {
        match outcome {
            // A worker that answers at all is a worker that works, whether or not
            // the encode it was running succeeded. One failed encode says nothing
            // about the next one's chances, and counting it would stop a queue
            // over an encoder error that has nothing to do with the worker.
            EncodeOutcome::Finished {
                was_successful,
                message,
                error_code,
            } => {
                let stopped = self.stop_requested || error_code == error_codes::CANCELLED;
                if stopped {
                    let message = if message.trim().is_empty() {
                        "cancelled on request".to_string()
                    } else {
                        message
                    };
                    return Resolution::JobCancelled { message };
                }
                self.guard.reset();
                Resolution::JobFinished {
                    state: if was_successful {
                        JobState::Completed
                    } else {
                        JobState::Failed
                    },
                    message,
                }
            }
            // The worker went away. If the user asked for this job to stop, that
            // is what happened, and a stop is not a crash however the worker
            // ended.
            EncodeOutcome::WorkerGone(death) if self.stop_requested => Resolution::JobCancelled {
                message: format!("cancelled on request; {}", death.describe()),
            },
            EncodeOutcome::WorkerGone(death) => match self.guard.note(&death) {
                CrashDecision::Restart { delay } => Resolution::JobCrashed {
                    reason: death.describe(),
                    output: output.to_path_buf(),
                    backoff: delay,
                },
                CrashDecision::Backoff { reason } => Resolution::QueueStopped {
                    reason,
                    output: output.to_path_buf(),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn death(code: i32) -> WorkerDeath {
        WorkerDeath {
            pid: 4321,
            code: Some(code),
            signal: None,
            reason: "the encoder subprocess failed".to_string(),
        }
    }

    fn output() -> PathBuf {
        PathBuf::from("C:\\out\\movie.mkv")
    }

    #[test]
    fn a_death_says_what_the_process_did() {
        assert!(WorkerDeath {
            pid: 7,
            code: Some(3),
            signal: None,
            reason: "a panic in the encode thread".into()
        }
        .describe()
        .contains("exited with code 3"));
        assert!(WorkerDeath {
            pid: 7,
            code: None,
            signal: Some(9),
            reason: "killed".into()
        }
        .describe()
        .contains("killed by signal 9"));
        assert!(WorkerDeath::unexplained(7, "stopped answering requests")
            .describe()
            .contains("gave no exit status"));
    }

    #[test]
    fn a_worker_that_answered_is_recorded_as_finished() {
        let mut policy = IsolationPolicy::new();
        let resolution = policy.resolved(EncodeOutcome::completed(""), &output());
        assert_eq!(
            resolution,
            Resolution::JobFinished {
                state: JobState::Completed,
                message: String::new()
            }
        );
        assert!(resolution.continues_queue());
        assert_eq!(policy.guard().consecutive(), 0);
    }

    #[test]
    fn an_encode_failure_is_a_failed_job_and_not_a_crash() {
        let mut policy = IsolationPolicy::new();
        let resolution = policy.resolved(EncodeOutcome::failed("encoder exited 1"), &output());
        assert_eq!(
            resolution,
            Resolution::JobFinished {
                state: JobState::Failed,
                message: "encoder exited 1".to_string()
            }
        );
        assert_eq!(
            policy.guard().consecutive(),
            0,
            "a worker that answered is not on a death streak"
        );
    }

    #[test]
    fn a_worker_killed_mid_job_fails_only_that_job_and_names_the_exit_status() {
        let mut policy = IsolationPolicy::new();
        let resolution = policy.resolved(EncodeOutcome::WorkerGone(death(3)), &output());
        match &resolution {
            Resolution::JobCrashed {
                reason,
                output: destination,
                backoff,
            } => {
                assert!(reason.contains("exited with code 3"), "{reason}");
                assert_eq!(destination, &output());
                assert_eq!(*backoff, Duration::ZERO, "the first death waits not");
            }
            other => panic!("expected an isolated crash, got {other:?}"),
        }
        assert!(
            resolution.continues_queue(),
            "the queue carries on with the next job"
        );
    }

    #[test]
    fn a_user_requested_stop_is_not_a_crash() {
        let mut policy = IsolationPolicy::new();
        policy.request_stop();
        assert!(policy.stop_requested());

        // The worker answering with a cancellation is a cancellation.
        let resolution = policy.resolved(EncodeOutcome::cancelled(""), &output());
        assert!(matches!(resolution, Resolution::JobCancelled { .. }));
        assert_eq!(policy.guard().consecutive(), 0);

        // So is the worker going away while the stop was in flight.
        let resolution = policy.resolved(EncodeOutcome::WorkerGone(death(1)), &output());
        assert!(matches!(resolution, Resolution::JobCancelled { .. }));
        assert_eq!(
            policy.guard().consecutive(),
            0,
            "a stop the user asked for is not counted against the worker"
        );
    }

    #[test]
    fn a_job_that_was_not_stopped_is_not_cancelled_by_one_that_was() {
        let mut stopped = IsolationPolicy::new();
        stopped.request_stop();
        assert!(stopped.stop_requested());
        let mut running = IsolationPolicy::new();
        assert!(!running.stop_requested());
        // The two jobs are separate policies, so one cancellation does not leak
        // into the other job's crash.
        let resolution = running.resolved(EncodeOutcome::WorkerGone(death(4)), &output());
        assert!(matches!(resolution, Resolution::JobCrashed { .. }));
    }

    #[test]
    fn consecutive_worker_deaths_back_off_and_then_report_instead_of_looping() {
        let mut policy = IsolationPolicy::with_policy(3, Duration::from_secs(2));
        let mut delays = Vec::new();
        // The first two deaths are replaced, each one further apart than the last.
        for _ in 0..2 {
            match policy.resolved(EncodeOutcome::WorkerGone(death(5)), &output()) {
                Resolution::JobCrashed { backoff, .. } => delays.push(backoff),
                other => panic!("expected an isolated crash, got {other:?}"),
            }
        }
        assert_eq!(
            delays,
            vec![Duration::ZERO, Duration::from_secs(2)],
            "the replacements get further apart"
        );

        // The third death in a row is where the window stops and says so: three
        // workers dying on three consecutive jobs is a machine, not bad luck.
        let resolution = policy.resolved(EncodeOutcome::WorkerGone(death(5)), &output());
        match &resolution {
            Resolution::QueueStopped { reason, .. } => {
                assert!(reason.contains("3 time(s) in a row"), "{reason}");
                assert!(reason.contains("exited with code 5"), "{reason}");
            }
            other => panic!("expected the queue to stop, got {other:?}"),
        }
        assert!(!resolution.continues_queue());
        assert_eq!(resolution.backoff(), Duration::ZERO);

        // And it stays stopped: a fourth death is still a stop, not a restart.
        let again = policy.resolved(EncodeOutcome::WorkerGone(death(5)), &output());
        assert!(matches!(again, Resolution::QueueStopped { .. }));
    }

    #[test]
    fn a_worker_that_answers_again_clears_the_death_streak() {
        let mut policy = IsolationPolicy::with_policy(3, Duration::from_secs(1));
        policy.resolved(EncodeOutcome::WorkerGone(death(2)), &output());
        assert_eq!(policy.guard().consecutive(), 1);
        policy.resolved(EncodeOutcome::completed(""), &output());
        assert_eq!(policy.guard().consecutive(), 0);

        // So the streak starts again from the beginning rather than carrying on.
        for _ in 0..2 {
            assert!(matches!(
                policy.resolved(EncodeOutcome::WorkerGone(death(2)), &output()),
                Resolution::JobCrashed { .. }
            ));
        }
        assert!(matches!(
            policy.resolved(EncodeOutcome::WorkerGone(death(2)), &output()),
            Resolution::QueueStopped { .. }
        ));
    }

    #[test]
    fn a_threshold_of_one_stops_at_the_first_death() {
        let mut policy = IsolationPolicy::with_guard(CrashGuard::with_policy(1, Duration::ZERO));
        assert_eq!(policy.guard().threshold(), 1);
        assert!(matches!(
            policy.resolved(EncodeOutcome::WorkerGone(death(1)), &output()),
            Resolution::QueueStopped { .. }
        ));
    }

    #[test]
    fn every_resolution_describes_itself_for_the_log() {
        assert!(Resolution::JobFinished {
            state: JobState::Completed,
            message: "out.mkv".into()
        }
        .describe()
        .contains("finished"));
        assert!(Resolution::JobCancelled {
            message: "cancelled on request".into()
        }
        .describe()
        .contains("cancelled"));
        assert!(Resolution::JobCrashed {
            reason: "the worker process 1 exited with code 3: boom".into(),
            output: output(),
            backoff: Duration::from_secs(2),
        }
        .describe()
        .contains("exited with code 3"));
    }
}
