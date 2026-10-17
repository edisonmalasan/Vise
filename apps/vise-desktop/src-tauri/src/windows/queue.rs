//! The queue window and the queue selection sheet.
//!
//! The queue is the only place in the application where work is destroyed on
//! purpose: a job is cancelled, removed, or cleared. So the queue keeps its own
//! order and its own priority rather than borrowing either from the session,
//! and every destructive action goes through the countdown alert before it
//! happens.
//!
//! Priority is the run order, made explicit. A job can be moved anywhere in the
//! list, and it can be marked high or low; the queue runs high jobs first, then
//! normal ones, then low ones, keeping the order the user set inside each band.
//! That means a reordering and a priority change can never disagree about which
//! job runs next, because both are answers to the same question.
//!
//! The selection sheet is the other half: it takes the titles a scan found and
//! decides which of them get queued and where each one is written. Naming is
//! where most of its rules live, because two titles that resolve to the same
//! output path would silently overwrite each other.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use vise_core::{Job, JobId, JobSpec, JobState};

/// Identifier of the queue surface.
pub const SURFACE: &str = "queue";
/// Identifier of the queue selection surface.
pub const SELECTION_SURFACE: &str = "queue-selection";

/// How urgent a queued job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum JobPriority {
    /// Runs after everything else.
    Low,
    /// Runs in the middle band.
    #[default]
    Normal,
    /// Runs before everything else.
    High,
}

impl JobPriority {
    /// Every priority, least to most urgent.
    pub fn all() -> [JobPriority; 3] {
        [JobPriority::Low, JobPriority::Normal, JobPriority::High]
    }

    /// Stable identifier the page sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            JobPriority::Low => "low",
            JobPriority::Normal => "normal",
            JobPriority::High => "high",
        }
    }

    /// Label beside the priority control.
    pub fn label(self) -> &'static str {
        match self {
            JobPriority::Low => "Low",
            JobPriority::Normal => "Normal",
            JobPriority::High => "High",
        }
    }

    /// What this priority does to the run order.
    pub fn effect(self) -> &'static str {
        match self {
            JobPriority::Low => "Runs after every normal and high job.",
            JobPriority::Normal => "Runs after the high jobs and before the low ones.",
            JobPriority::High => "Runs before every normal and low job.",
        }
    }

    /// The priority named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "low" | "later" => Some(JobPriority::Low),
            "normal" | "standard" | "medium" => Some(JobPriority::Normal),
            "high" | "first" => Some(JobPriority::High),
            _ => None,
        }
    }

    /// The band this priority runs in, counting from the top.
    fn band(self) -> usize {
        match self {
            JobPriority::High => 0,
            JobPriority::Normal => 1,
            JobPriority::Low => 2,
        }
    }
}

/// Whether a job has finished, one way or another.
pub fn is_finished(state: JobState) -> bool {
    matches!(
        state,
        JobState::Completed | JobState::Failed | JobState::Cancelled
    )
}

/// Whether a job can be handed to the encoder.
pub fn is_runnable(state: JobState) -> bool {
    matches!(state, JobState::Queued | JobState::Paused)
}

/// Whether a job is holding an encoder right now.
pub fn is_running(state: JobState) -> bool {
    matches!(state, JobState::Running | JobState::Paused)
}

/// The state label, as the queue row shows it.
pub fn state_label(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "Queued",
        JobState::Running => "Running",
        JobState::Paused => "Paused",
        JobState::Completed => "Finished",
        JobState::Failed => "Failed",
        JobState::Cancelled => "Cancelled",
    }
}

/// The order the queue runs jobs in, and what each one is worth.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueState {
    /// The order the user set, oldest entry first.
    order: Vec<JobId>,
    /// What each job is worth.
    priority: BTreeMap<JobId, JobPriority>,
}

impl QueueState {
    /// Bring the queue in line with the session.
    ///
    /// A job that has appeared is added at the end, because it was just queued
    /// and has no position yet; a job that has gone is forgotten, so a stale
    /// identifier cannot be raised to the top of an empty list.
    pub fn sync(&mut self, jobs: &[Job]) {
        self.order.retain(|id| jobs.iter().any(|job| job.id == *id));
        self.priority
            .retain(|id, _| jobs.iter().any(|job| job.id == *id));
        for job in jobs {
            if !self.order.contains(&job.id) {
                self.order.push(job.id);
                self.priority.entry(job.id).or_default();
            }
        }
    }

    /// Record a job that has just been queued.
    pub fn push(&mut self, id: JobId) {
        if !self.order.contains(&id) {
            self.order.push(id);
        }
        self.priority.entry(id).or_default();
    }

    /// Forget a job that has left the session.
    pub fn forget(&mut self, id: JobId) {
        self.order.retain(|open| *open != id);
        self.priority.remove(&id);
    }

    /// The priority of `id`, defaulting to normal.
    pub fn priority_of(&self, id: JobId) -> JobPriority {
        self.priority.get(&id).copied().unwrap_or_default()
    }

    /// Set the priority of `id`.
    pub fn set_priority(&mut self, id: JobId, priority: JobPriority) {
        self.priority.insert(id, priority);
    }

    /// Set the priority by name, refusing a name the queue does not use.
    pub fn set_priority_name(&mut self, id: JobId, text: &str) -> Result<JobPriority, String> {
        let priority = JobPriority::parse(text)
            .ok_or_else(|| format!("'{text}' is not a priority this build offers."))?;
        self.set_priority(id, priority);
        Ok(priority)
    }

    /// Move `id` by `delta` places in the order the user set.
    pub fn move_by(&mut self, id: JobId, delta: i32) -> Result<usize, String> {
        let Some(position) = self.order.iter().position(|open| *open == id) else {
            return Err(format!("job {id} is not in the queue."));
        };
        let last = self.order.len() as i64 - 1;
        let wanted = (position as i64 + i64::from(delta)).clamp(0, last);
        let target = wanted as usize;
        if target == position {
            return Ok(position);
        }
        self.order.remove(position);
        self.order.insert(target, id);
        Ok(target)
    }

    /// The jobs in the order they run: high first, then normal, then low, and
    /// the user's own order inside each band.
    pub fn run_order<'a>(&self, jobs: &'a [Job]) -> Vec<&'a Job> {
        let mut listed: Vec<&Job> = jobs
            .iter()
            .filter(|job| self.order.contains(&job.id))
            .collect();
        listed.sort_by_key(|job| {
            (
                self.priority_of(job.id).band(),
                position_in(&self.order, job.id),
            )
        });
        listed
    }

    /// Where in the run order a job sits, counting from one.
    pub fn run_position(&self, jobs: &[Job], id: JobId) -> Option<usize> {
        self.run_order(jobs)
            .iter()
            .position(|job| job.id == id)
            .map(|position| position + 1)
    }

    /// The jobs that are waiting for an encoder, in the run order.
    pub fn pending(&self, jobs: &[Job]) -> Vec<JobId> {
        self.run_order(jobs)
            .into_iter()
            .filter(|job| is_runnable(job.status.state) && !is_running(job.status.state))
            .map(|job| job.id)
            .collect()
    }

    /// The jobs that have finished, in the run order.
    pub fn finished(&self, jobs: &[Job]) -> Vec<JobId> {
        self.run_order(jobs)
            .into_iter()
            .filter(|job| is_finished(job.status.state))
            .map(|job| job.id)
            .collect()
    }

    /// The jobs the encoder is holding right now.
    pub fn active(&self, jobs: &[Job]) -> Vec<JobId> {
        self.run_order(jobs)
            .into_iter()
            .filter(|job| job.status.state == JobState::Running)
            .map(|job| job.id)
            .collect()
    }

    /// The first pending job, which is the one the encoder picks up next.
    pub fn next_job(&self, jobs: &[Job]) -> Option<JobId> {
        self.pending(jobs).into_iter().next()
    }

    /// Forget every job that has finished, handing their identifiers back.
    pub fn clear_finished(&mut self, jobs: &[Job]) -> Vec<JobId> {
        let done = self.finished(jobs);
        for id in &done {
            self.forget(*id);
        }
        done
    }
}

/// Where `id` sits in `order`, counting from zero.
fn position_in(order: &[JobId], id: JobId) -> usize {
    order
        .iter()
        .position(|open| *open == id)
        .unwrap_or(usize::MAX)
}

/// One priority in the picker, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuePriorityViewModel {
    /// Stable identifier sent back on a change.
    pub id: String,
    /// Label on the option.
    pub label: String,
    /// What this priority does to the run order.
    pub effect: String,
    /// How many queued jobs are at this priority.
    pub count: usize,
    /// Whether this is the chosen priority.
    pub selected: bool,
}

/// One control on a queue row, with its availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueActionViewModel {
    /// Stable identifier the page sends back.
    pub id: String,
    /// Label on the control.
    pub label: String,
    /// Whether it can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether using it asks for confirmation first.
    pub confirms: bool,
}

impl QueueActionViewModel {
    /// An action that is on offer.
    pub fn open(id: &str, label: &str, confirms: bool) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: true,
            reason: String::new(),
            confirms,
        }
    }

    /// The action, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// One queued job, as the queue row shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemViewModel {
    /// The job's identifier.
    pub id: u64,
    /// File name of the source, without its folders.
    pub input_label: String,
    /// Where the finished file is written.
    pub output_label: String,
    /// The output container, as the queue shows it.
    pub container_label: String,
    /// Stable state identifier.
    pub state: String,
    /// State as the user reads it.
    pub state_label: String,
    /// Tone the page paints the row with.
    pub tone: String,
    /// Progress as a whole percentage.
    pub percent: u32,
    /// Whether a progress bar is worth showing.
    pub progress_visible: bool,
    /// Estimated time remaining, as a label.
    pub eta_label: String,
    /// What the encoder reported, when it reported anything.
    pub message: String,
    /// Stable priority identifier.
    pub priority: String,
    /// Priority as the user reads it.
    pub priority_label: String,
    /// What the priority does to the run order.
    pub priority_effect: String,
    /// Where this job sits in the run order, counting from one.
    pub run_position: Option<usize>,
    /// Whether this is the job the encoder will pick up next.
    pub next_in_line: bool,
    /// Every control on the row.
    pub actions: Vec<QueueActionViewModel>,
}

impl QueueItemViewModel {
    /// Build the row for `job`.
    pub fn of(job: &Job, state: &QueueState, jobs: &[Job]) -> Self {
        let job_state = job.status.state;
        let running = job_state == JobState::Running;
        let finished = is_finished(job_state);
        let priority = state.priority_of(job.id);
        let pending = state.pending(jobs);
        let fraction = if finished && job_state == JobState::Completed {
            1.0
        } else {
            job.status.progress.fraction
        };
        let percent = (fraction.clamp(0.0, 1.0) * 100.0).round() as u32;
        let eta = job.status.progress.eta_ms.filter(|ms| *ms > 0);
        Self {
            id: job.id,
            input_label: file_name(&job.spec.input_path),
            output_label: job.spec.output_path.clone(),
            container_label: format!("{:?}", job.spec.container).to_ascii_lowercase(),
            state: state_name(job_state).to_string(),
            state_label: state_label(job_state).to_string(),
            tone: match job_state {
                JobState::Completed => "ok",
                JobState::Failed => "error",
                JobState::Cancelled => "muted",
                JobState::Paused => "warn",
                JobState::Queued => "muted",
                JobState::Running => "info",
            }
            .to_string(),
            percent,
            progress_visible: running,
            eta_label: match eta {
                Some(ms) => format!(
                    "about {} left",
                    crate::viewmodel::format_duration(ms as f64 / 1000.0)
                ),
                None => String::new(),
            },
            message: job.status.message.clone().unwrap_or_default(),
            priority: priority.as_str().to_string(),
            priority_label: priority.label().to_string(),
            priority_effect: priority.effect().to_string(),
            run_position: state.run_position(jobs, job.id),
            next_in_line: pending.first() == Some(&job.id),
            actions: vec![
                QueueActionViewModel::open("run", "Run", false).blocked_or(
                    is_runnable(job_state),
                    || {
                        if running {
                            "This job is already being encoded.".to_string()
                        } else if finished {
                            "This job has finished; queue it again to run it.".to_string()
                        } else {
                            format!(
                                "This job is {}.",
                                state_label(job_state).to_ascii_lowercase()
                            )
                        }
                    },
                ),
                QueueActionViewModel::open("pause", "Pause", false).blocked_or(running, || {
                    if job_state == JobState::Paused {
                        "This job is already paused.".to_string()
                    } else if finished {
                        "This job has finished.".to_string()
                    } else {
                        "This job is not running.".to_string()
                    }
                }),
                QueueActionViewModel::open("resume", "Resume", false).blocked_or(
                    job_state == JobState::Paused,
                    || {
                        if running {
                            "This job is not paused.".to_string()
                        } else if finished {
                            "This job has finished.".to_string()
                        } else {
                            "This job is not paused.".to_string()
                        }
                    },
                ),
                QueueActionViewModel::open("cancel", "Cancel", true)
                    .blocked_or(!finished, || "This job has finished.".to_string()),
                QueueActionViewModel::open("remove", "Remove", true).blocked_or(!running, || {
                    "Cancel this job before removing it.".to_string()
                }),
                QueueActionViewModel::open("move_up", "Move up", false)
                    .blocked_or(state.run_position(jobs, job.id).unwrap_or(1) > 1, || {
                        "This job is already first in the list.".to_string()
                    }),
                QueueActionViewModel::open("move_down", "Move down", false).blocked_or(
                    state
                        .run_position(jobs, job.id)
                        .is_some_and(|p| p < state.run_order(jobs).len()),
                    || "This job is already last in the list.".to_string(),
                ),
                QueueActionViewModel::open("priority_high", "Raise priority", false)
                    .blocked_or(priority != JobPriority::High, || {
                        "This job is already first in line.".to_string()
                    }),
                QueueActionViewModel::open("priority_low", "Lower priority", false)
                    .blocked_or(priority != JobPriority::Low, || {
                        "This job is already last in line.".to_string()
                    }),
            ],
        }
    }
}

impl QueueActionViewModel {
    /// The action when `available` holds, or blocked with the reason it does not.
    fn blocked_or(self, available: bool, reason: impl FnOnce() -> String) -> Self {
        if available {
            self
        } else {
            self.blocked(reason())
        }
    }
}

/// The stable name of a job state.
pub fn state_name(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Paused => "paused",
        JobState::Completed => "completed",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

/// The file name out of a path, for a compact queue row.
pub fn file_name(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "(no source)".to_string();
    }
    trimmed
        .rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

/// The queue window, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueViewModel {
    /// One row per job, in the order the queue runs them.
    pub items: Vec<QueueItemViewModel>,
    /// Every priority the picker offers.
    pub priorities: Vec<QueuePriorityViewModel>,
    /// The chosen priority, as the picker shows it.
    pub selected_priority: String,
    /// How many jobs are queued.
    pub total: usize,
    /// How many are waiting for an encoder.
    pub pending: usize,
    /// How many are being encoded.
    pub running: usize,
    /// How many have finished.
    pub finished: usize,
    /// Whether there is nothing to show.
    pub empty: bool,
    /// Whether the queue can be started as a whole.
    pub can_start: bool,
    /// Why the queue cannot be started, when it cannot.
    pub start_reason: String,
    /// The whole-queue controls.
    pub bulk: Vec<QueueActionViewModel>,
    /// The next job the encoder will pick up, as a line.
    pub summary: String,
    /// One line describing the output of the queue.
    pub progress_summary: String,
}

/// Build the queue window for `jobs`.
pub fn queue_view(state: &QueueState, jobs: &[Job]) -> QueueViewModel {
    let items: Vec<QueueItemViewModel> = state
        .run_order(jobs)
        .into_iter()
        .map(|job| QueueItemViewModel::of(job, state, jobs))
        .collect();
    let pending = state.pending(jobs);
    let running = state.active(jobs).len();
    let finished = state.finished(jobs).len();
    let total = items.len();
    let start_reason = if pending.is_empty() {
        if total == 0 {
            "Nothing is queued.".to_string()
        } else if running > 0 {
            "Every queued job is already with the encoder.".to_string()
        } else {
            "No job is waiting; queue one before starting the queue.".to_string()
        }
    } else {
        String::new()
    };
    let bulk = vec![
        QueueActionViewModel::open("start_queue", "Start queue", true)
            .blocked_or(!pending.is_empty(), || start_reason.clone()),
        QueueActionViewModel::open("cancel_all", "Cancel everything", true).blocked_or(
            running > 0,
            || {
                if total == 0 {
                    "The queue is empty.".to_string()
                } else {
                    "Nothing is being encoded right now.".to_string()
                }
            },
        ),
        QueueActionViewModel::open("clear_finished", "Clear finished", true)
            .blocked_or(finished > 0, || "No job has finished yet.".to_string()),
        // Emptying the queue throws away every job that is not being encoded, so
        // it is on offer only when there is something to throw away and nothing
        // in flight that would be lost part-way through an encode.
        QueueActionViewModel::open("empty_queue", "Empty queue", true).blocked_or(
            total > 0 && total > running,
            || {
                if total == 0 {
                    "The queue is empty.".to_string()
                } else {
                    format!(
                        "{} job(s) are still in the queue; cancel them before emptying it.",
                        running
                    )
                }
            },
        ),
    ];
    let summary = match state.next_job(jobs) {
        Some(next) => format!("Next: job {next}."),
        None if total == 0 => "The queue is empty.".to_string(),
        None => "Nothing is waiting for the encoder.".to_string(),
    };
    let selected = items
        .first()
        .map(|item| item.priority.clone())
        .unwrap_or_else(|| JobPriority::Normal.as_str().to_string());
    let priorities: Vec<QueuePriorityViewModel> = JobPriority::all()
        .into_iter()
        .map(|priority| QueuePriorityViewModel {
            id: priority.as_str().to_string(),
            label: priority.label().to_string(),
            effect: priority.effect().to_string(),
            count: items
                .iter()
                .filter(|item| item.priority == priority.as_str())
                .count(),
            selected: priority.as_str() == selected,
        })
        .collect();
    let progress_summary = if total == 0 {
        "No jobs.".to_string()
    } else {
        let done: u32 = items
            .iter()
            .filter(|item| item.state == "completed")
            .count() as u32;
        format!(
            "{done} of {total} finished, {running} encoding, {} waiting.",
            pending.len()
        )
    };
    QueueViewModel {
        items,
        priorities,
        selected_priority: selected,
        total,
        pending: pending.len(),
        running,
        finished,
        empty: total == 0,
        can_start: start_reason.is_empty(),
        start_reason,
        bulk,
        summary,
        progress_summary,
    }
}

// ---------------------------------------------------------------------------
// Queue selection
// ---------------------------------------------------------------------------

/// The longest name a queued output file may have.
pub const MAX_OUTPUT_NAME: usize = 120;

/// One title offered by the selection sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueTitleViewModel {
    /// The title's index, counting from one.
    pub index: u32,
    /// Title as the sheet lists it.
    pub label: String,
    /// Its facts on one line.
    pub facts: String,
    /// Whether the user chose it.
    pub chosen: bool,
    /// Name of the file it will be written as.
    pub output_name: String,
    /// The full path it will be written to.
    pub output_path: String,
    /// Whether the name is usable.
    pub output_valid: bool,
    /// Why the name is not usable, when it is not.
    pub output_problem: String,
    /// Whether another chosen title already claims that path.
    pub output_collides: bool,
    /// The range this job would cover, as a label.
    pub range_label: String,
    /// Whether a range can be chosen per title.
    pub range_available: bool,
    /// Why a range cannot be chosen, when it cannot.
    pub range_reason: String,
}

/// The queue selection sheet, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSelectionViewModel {
    /// One entry per title the scan found.
    pub titles: Vec<QueueTitleViewModel>,
    /// Where the files are written.
    pub output_directory: String,
    /// Whether the chosen directory can be used.
    pub directory_valid: bool,
    /// Why it cannot, when it cannot.
    pub directory_problem: String,
    /// The naming pattern the sheet proposes.
    pub name_pattern: String,
    /// Whether the naming pattern can be used.
    pub pattern_valid: bool,
    /// Why it cannot, when it cannot.
    pub pattern_problem: String,
    /// How many titles are chosen.
    pub chosen: usize,
    /// Whether the sheet can queue what is chosen.
    pub can_queue: bool,
    /// Everything that has to be fixed first.
    pub problems: Vec<String>,
    /// The full paths that would be written.
    pub planned: Vec<String>,
    /// One line summarising the plan.
    pub summary: String,
}

/// The selection sheet as the page sends it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QueueSelectionInput {
    /// Where the files are written.
    pub output_directory: String,
    /// The naming pattern, with `{title}` and `{n}` in it.
    pub name_pattern: String,
    /// Per-title overrides of the name.
    pub names: BTreeMap<String, String>,
    /// Which title indices are queued.
    pub titles: Vec<u32>,
}

/// What the sheet proposes before the user changes anything.
pub const DEFAULT_NAME_PATTERN: &str = "{title}";

/// Turn a naming pattern into a file name for one title.
///
/// `{title}` is the title's name and `{n}` its index. A name is refused when it
/// is empty, too long, or carries a character no filesystem accepts, because
/// the encoder would otherwise be asked to write somewhere it cannot.
pub fn output_name(pattern: &str, title_name: &str, index: u32) -> Result<String, String> {
    let stem = title_name.trim();
    let stem = if stem.is_empty() {
        format!("Title {index}")
    } else {
        stem.to_string()
    };
    let name = pattern
        .trim()
        .replace("{title}", &stem)
        .replace("{n}", &index.to_string());
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Every queued title needs a file name.".to_string());
    }
    if name.chars().count() > MAX_OUTPUT_NAME {
        return Err(format!(
            "A queued file name has to be {MAX_OUTPUT_NAME} characters or fewer; '{name}' is longer."
        ));
    }
    let bad = name.chars().find(|c| {
        matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (*c as u32) < 0x20
    });
    if let Some(bad) = bad {
        return Err(format!(
            "'{bad}' cannot appear in a file name; choose another name."
        ));
    }
    if name == "." || name == ".." {
        return Err(format!("'{name}' is not a file name."));
    }
    Ok(name)
}

/// Join an output directory and a file name into a path.
pub fn output_path(directory: &str, name: &str) -> String {
    let directory = directory.trim().trim_end_matches(['\\', '/']);
    if directory.is_empty() {
        return name.trim().to_string();
    }
    format!("{directory}\\{name}")
}

/// The file name one title would be written as.
///
/// A name the user typed for that title wins over the shared pattern, so a
/// single title can be renamed without changing the rest of the batch.
pub fn title_stem(
    pattern: &str,
    names: &BTreeMap<String, String>,
    index: u32,
    title_name: &str,
) -> String {
    match names.get(&index.to_string()) {
        Some(text) if !text.trim().is_empty() => text.trim().to_string(),
        _ => output_name(pattern, title_name, index).unwrap_or_else(|_| pattern.trim().to_string()),
    }
}

/// The full file name, extension included, for one title.
fn title_file(
    pattern: &str,
    names: &BTreeMap<String, String>,
    index: u32,
    title_name: &str,
    extension: &str,
) -> String {
    let stem = title_stem(pattern, names, index, title_name);
    if extension.trim().is_empty() {
        stem
    } else {
        format!("{stem}.{}", extension.trim().trim_start_matches('.'))
    }
}

/// Build the queue plans for `input`, checking every name before anything is
/// queued so a half-written batch never happens.
pub fn plan(
    input: &QueueSelectionInput,
    titles: &[(u32, String)],
    extension: &str,
) -> Result<Vec<(u32, String)>, String> {
    let directory = input.output_directory.trim();
    if directory.is_empty() {
        return Err("Choose a folder to write the finished files into.".to_string());
    }
    if input.titles.is_empty() {
        return Err("Choose at least one title to queue.".to_string());
    }
    let pattern = if input.name_pattern.trim().is_empty() {
        DEFAULT_NAME_PATTERN
    } else {
        input.name_pattern.as_str()
    };
    let mut planned: Vec<(u32, String)> = Vec::new();
    for index in input.titles.iter().copied() {
        let Some((_, name)) = titles.iter().find(|(other, _)| *other == index) else {
            return Err(format!("There is no title {index} in this source."));
        };
        let stem = match input.names.get(&index.to_string()) {
            Some(text) if !text.trim().is_empty() => text.trim().to_string(),
            _ => output_name(pattern, name, index)?,
        };
        // The extension goes through the same name check, so a stem that was
        // fine on its own cannot become an unwritable file once it is suffixed.
        let file = if extension.trim().is_empty() {
            stem
        } else {
            output_name(
                &format!("{stem}.{}", extension.trim().trim_start_matches('.')),
                "",
                index,
            )?
        };
        planned.push((index, output_path(directory, &file)));
    }
    for (position, (index, path)) in planned.iter().enumerate() {
        for (other, other_path) in planned.iter().skip(position + 1) {
            if other_path.eq_ignore_ascii_case(path) {
                return Err(format!(
                    "Titles {index} and {other} would both be written to '{path}'; give one of them another name."
                ));
            }
        }
    }
    Ok(planned)
}

/// Build the selection sheet for the titles a scan found.
pub fn selection_view(
    input: &QueueSelectionInput,
    titles: &[(u32, String, String)],
    extension: &str,
    directory_valid: bool,
    directory_problem: &str,
) -> QueueSelectionViewModel {
    let pattern = if input.name_pattern.trim().is_empty() {
        DEFAULT_NAME_PATTERN.to_string()
    } else {
        input.name_pattern.clone()
    };
    let pattern_problem = match output_name(&pattern, "Title 1", 1) {
        Ok(_) => String::new(),
        Err(problem) => problem,
    };
    let entries: Vec<QueueTitleViewModel> = titles
        .iter()
        .map(|(index, name, facts)| {
            let file = title_file(&pattern, &input.names, *index, name, extension);
            let path = output_path(&input.output_directory, &file);
            let collides = input.titles.contains(index)
                && input.titles.iter().any(|other| {
                    *other != *index && {
                        let other_name = titles
                            .iter()
                            .find(|(candidate, _, _)| candidate == other)
                            .map(|(_, candidate, _)| candidate.as_str())
                            .unwrap_or_default();
                        let other_file =
                            title_file(&pattern, &input.names, *other, other_name, extension);
                        output_path(&input.output_directory, &other_file)
                            .eq_ignore_ascii_case(&path)
                    }
                });
            let output_valid = output_name(&pattern, name, *index).is_ok();
            QueueTitleViewModel {
                index: *index,
                label: format!("Title {index}"),
                facts: facts.clone(),
                chosen: input.titles.contains(index),
                output_name: file,
                output_path: path,
                output_valid,
                output_problem: if output_valid {
                    String::new()
                } else {
                    pattern_problem.clone()
                },
                output_collides: collides,
                range_label: "Whole title".to_string(),
                range_available: false,
                range_reason:
                    "The job model has no start and end field, so a range cannot be carried into a \
                     queued job."
                        .to_string(),
            }
        })
        .collect();
    let chosen = input.titles.len();
    let planned: Vec<String> = plan(input, &collect(titles), extension)
        .map(|planned| planned.into_iter().map(|(_, path)| path).collect())
        .unwrap_or_default();
    let mut problems: Vec<String> = Vec::new();
    if !directory_valid {
        problems.push(directory_problem.to_string());
    }
    if !pattern_problem.is_empty() {
        problems.push(pattern_problem.clone());
    }
    if chosen == 0 {
        problems.push("Choose at least one title to queue.".to_string());
    }
    if let Err(problem) = plan(input, &collect(titles), extension) {
        if !problems.iter().any(|recorded| recorded == &problem) {
            problems.push(problem);
        }
    }
    let can_queue = problems.is_empty();
    QueueSelectionViewModel {
        summary: if titles.is_empty() {
            "Scan a source to see the titles it contains.".to_string()
        } else if can_queue {
            format!(
                "{} title(s) will be written to {}.",
                chosen,
                input.output_directory.trim()
            )
        } else {
            format!(
                "{} title(s) chosen; {} thing(s) to fix.",
                chosen,
                problems.len()
            )
        },
        titles: entries,
        output_directory: input.output_directory.clone(),
        directory_valid,
        directory_problem: directory_problem.to_string(),
        name_pattern: pattern,
        pattern_valid: pattern_problem.is_empty(),
        pattern_problem,
        chosen,
        can_queue,
        problems,
        planned,
    }
}

/// The `(index, name)` pairs the planner needs.
fn collect(titles: &[(u32, String, String)]) -> Vec<(u32, String)> {
    titles
        .iter()
        .map(|(index, name, _)| (*index, name.clone()))
        .collect()
}

/// Build a job specification for one queued title.
///
/// The specification comes from the tabs, so a queued job cannot disagree with
/// what the tabs showed; only the title index and the output path differ from
/// the single-title case.
pub fn titled_spec(base: &JobSpec, title_index: u32, output_path: &str) -> JobSpec {
    let mut spec = base.clone();
    spec.title_index = Some(title_index);
    spec.output_path = output_path.to_string();
    spec
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::{JobStatus, Progress};

    fn spec(input: &str, output: &str) -> JobSpec {
        JobSpec {
            input_path: input.to_string(),
            output_path: output.to_string(),
            ..Default::default()
        }
    }

    fn job(id: JobId, input: &str) -> Job {
        Job::new(id, spec(input, "out.mp4"))
    }

    fn with_state(mut job: Job, job_state: JobState) -> Job {
        job.status = JobStatus {
            state: job_state,
            progress: Progress::new(0.0),
            message: Some(state_label(job_state).to_ascii_lowercase()),
        };
        job
    }

    fn synced() -> (QueueState, Vec<Job>) {
        let jobs = vec![
            with_state(job(1, "C:\\media\\a.mkv"), JobState::Queued),
            with_state(job(2, "C:\\media\\b.mkv"), JobState::Running),
            with_state(job(3, "C:\\media\\c.mkv"), JobState::Failed),
        ];
        let mut state = QueueState::default();
        state.sync(&jobs);
        (state, jobs)
    }

    #[test]
    fn the_queue_shows_its_jobs_in_the_order_it_runs_them() {
        let (state, jobs) = synced();
        let view = queue_view(&state, &jobs);
        assert_eq!(view.total, 3);
        assert!(!view.empty);
        assert_eq!(
            view.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(view.items[0].input_label, "a.mkv");
        assert_eq!(view.items[1].state_label, "Running");
        assert_eq!(view.items[1].tone, "info");
        assert_eq!(view.items[2].state_label, "Failed");
        assert_eq!(view.items[2].tone, "error");
        assert_eq!(view.pending, 1);
        assert_eq!(view.running, 1);
        assert_eq!(view.finished, 1);
        assert!(view.items[0].next_in_line);
        assert!(
            !view.items[1].next_in_line,
            "the running job is not waiting"
        );
        assert!(view.summary.contains("job 1"));
        assert!(view.progress_summary.contains("0 of 3 finished"));
    }

    #[test]
    fn a_new_job_joins_at_the_end_and_a_removed_one_is_forgotten() {
        let (mut state, jobs) = synced();
        state.push(4);
        let mut more = jobs.clone();
        more.push(job(4, "C:\\media\\d.mkv"));
        assert_eq!(
            state
                .run_order(&more)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        state.forget(2);
        let mut fewer = more.clone();
        fewer.retain(|job| job.id != 2);
        assert_eq!(
            state
                .run_order(&fewer)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![1, 3, 4]
        );
        assert_eq!(
            state.priority_of(2),
            JobPriority::Normal,
            "forgotten with it"
        );
    }

    #[test]
    fn syncing_drops_jobs_that_left_and_admits_jobs_that_arrived() {
        let (mut state, jobs) = synced();
        state.set_priority(1, JobPriority::High);
        let mut fewer = jobs.clone();
        fewer.retain(|job| job.id != 1);
        state.sync(&fewer);
        assert!(state.run_position(&fewer, 1).is_none());
        assert_eq!(state.priority_of(1), JobPriority::Normal);

        let mut more = fewer.clone();
        more.push(job(9, "C:\\media\\z.mkv"));
        state.sync(&more);
        assert_eq!(state.run_position(&more, 9), Some(3));
    }

    #[test]
    fn a_job_moves_up_and_down_and_stops_at_the_ends() {
        let (mut state, jobs) = synced();
        assert_eq!(state.move_by(3, -1).expect("up"), 1);
        assert_eq!(
            state
                .run_order(&jobs)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![1, 3, 2]
        );
        assert_eq!(state.move_by(3, -5).expect("clamped"), 0);
        assert_eq!(state.move_by(1, -1).expect("already first"), 0);
        assert_eq!(state.move_by(1, 5).expect("clamped down"), 2);
        assert!(state
            .move_by(99, 1)
            .expect_err("not queued")
            .contains("not in the queue"));
    }

    #[test]
    fn priority_moves_a_job_across_the_bands_without_reordering_the_rest() {
        let (mut state, jobs) = synced();
        state.set_priority(3, JobPriority::High);
        assert_eq!(
            state
                .run_order(&jobs)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![3, 1, 2],
            "the high job runs first, and 1 then 2 keep their order"
        );
        state.set_priority(1, JobPriority::Low);
        assert_eq!(
            state
                .run_order(&jobs)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(
            state.next_job(&jobs),
            Some(1),
            "only the queued job is waiting"
        );
        state.set_priority(2, JobPriority::High);
        assert_eq!(
            state
                .run_order(&jobs)
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    #[test]
    fn a_priority_this_build_does_not_use_is_refused_by_name() {
        let (mut state, _) = synced();
        assert_eq!(
            state.set_priority_name(1, "first").expect("named"),
            JobPriority::High
        );
        assert!(state
            .set_priority_name(1, "whenever")
            .expect_err("unknown priority")
            .contains("whenever"));
        assert_eq!(
            state.priority_of(1),
            JobPriority::High,
            "the old priority stands"
        );
        assert_eq!(JobPriority::Normal.effect(), JobPriority::Normal.effect());
        assert_eq!(JobPriority::parse("Later"), Some(JobPriority::Low));
        assert_eq!(JobPriority::parse("nope"), None);
        assert_eq!(JobPriority::all().len(), 3);
    }

    #[test]
    fn every_row_control_says_why_it_is_closed_or_confirms_first() {
        let (state, jobs) = synced();
        let view = queue_view(&state, &jobs);
        let by_id = |id: &str| {
            view.items[0]
                .actions
                .iter()
                .find(|action| action.id == id)
                .cloned()
                .expect("action")
        };
        assert!(by_id("run").available);
        assert!(by_id("move_up").reason.contains("first"));
        assert!(by_id("move_down").available);
        assert!(by_id("cancel").confirms, "cancel asks first");
        assert!(by_id("remove").available, "a queued job can go straight");
        assert!(by_id("priority_high").available);
        assert!(by_id("priority_low").available);

        let running = &view.items[1].actions;
        assert!(running
            .iter()
            .any(|a| a.id == "run" && !a.available && a.reason.contains("already being encoded")));
        assert!(running.iter().any(|a| a.id == "pause" && a.available));
        assert!(running.iter().any(|a| a.id == "resume" && !a.available));
        assert!(running.iter().any(|a| a.id == "cancel" && a.available));
        assert!(running
            .iter()
            .any(|a| a.id == "remove" && !a.available && a.reason.contains("Cancel this job")));

        let failed = &view.items[2].actions;
        assert!(failed
            .iter()
            .any(|a| a.id == "run" && a.reason.contains("finished")));
        assert!(failed.iter().any(|a| a.id == "cancel" && !a.available));
        assert!(failed.iter().any(|a| a.id == "remove" && a.available));
    }

    #[test]
    fn a_running_job_reports_its_progress_and_a_finished_one_does_not() {
        let (mut state, mut jobs) = synced();
        jobs[1].status.progress = Progress::new(0.423);
        jobs[1].status.progress.eta_ms = Some(65_000);
        let view = queue_view(&state, &jobs);
        assert_eq!(view.items[1].percent, 42);
        assert!(view.items[1].progress_visible);
        assert_eq!(view.items[1].eta_label, "about 1:05 left");
        assert_eq!(view.items[2].percent, 0);
        assert!(!view.items[2].progress_visible);
        assert!(view.items[2].eta_label.is_empty());

        jobs[0].status.progress = Progress::new(1.0);
        let done = with_state(job(4, "d.mkv"), JobState::Completed);
        let mut all = jobs.clone();
        all.push(done);
        state.sync(&all);
        let view = queue_view(&state, &all);
        let finished = view
            .items
            .iter()
            .find(|item| item.id == 4)
            .expect("the finished job");
        assert_eq!(finished.percent, 100);
        assert!(!finished.progress_visible);
    }

    #[test]
    fn an_empty_queue_says_so_and_closes_its_whole_queue_controls() {
        let state = QueueState::default();
        let view = queue_view(&state, &[]);
        assert!(view.empty);
        assert_eq!(view.summary, "The queue is empty.");
        assert_eq!(view.progress_summary, "No jobs.");
        assert!(!view.can_start);
        assert!(view.start_reason.contains("Nothing is queued"));
        assert!(view.bulk.iter().all(|action| !action.available));
        assert!(view.items.is_empty());
    }

    #[test]
    fn the_whole_queue_controls_open_only_when_they_have_work() {
        let (state, jobs) = synced();
        let view = queue_view(&state, &jobs);
        let bulk = |id: &str| {
            view.bulk
                .iter()
                .find(|action| action.id == id)
                .cloned()
                .expect("bulk action")
        };
        assert!(bulk("start_queue").available);
        assert!(bulk("start_queue").confirms, "starting work asks first");
        assert!(bulk("cancel_all").available);
        assert!(bulk("clear_finished").available);
        assert!(
            bulk("empty_queue").available,
            "there is a finished job to throw away: {}",
            bulk("empty_queue").reason
        );

        let mut all_running = jobs.clone();
        for job in &mut all_running {
            job.status.state = JobState::Running;
        }
        let view = queue_view(&state, &all_running);
        assert!(!view.can_start);
        assert!(view.start_reason.contains("already with the encoder"));
        let bulk = |id: &str| {
            view.bulk
                .iter()
                .find(|action| action.id == id)
                .cloned()
                .expect("bulk action")
        };
        assert!(
            bulk("cancel_all").available,
            "every job is being encoded, so there is something to cancel"
        );
        assert!(!bulk("clear_finished").available, "nothing has finished yet");
        assert!(bulk("clear_finished").reason.contains("No job has finished"));
        assert!(
            !bulk("empty_queue").available,
            "emptying the queue would lose an encode in flight"
        );
        assert!(bulk("empty_queue").reason.contains("cancel them before emptying"));
    }

    #[test]
    fn nothing_is_offered_to_cancel_when_the_encoder_is_idle() {
        let (state, mut jobs) = synced();
        for job in &mut jobs {
            job.status.state = JobState::Queued;
        }
        let view = queue_view(&state, &jobs);
        let cancel = view
            .bulk
            .iter()
            .find(|action| action.id == "cancel_all")
            .expect("bulk action");
        assert!(!cancel.available, "there is nothing running to cancel");
        assert!(cancel.reason.contains("Nothing is being encoded"));
    }

    #[test]
    fn an_empty_queue_offers_nothing_to_clear_empty_or_cancel() {
        let state = QueueState::default();
        let view = queue_view(&state, &[]);
        assert!(view.empty);
        assert_eq!(view.total, 0);
        assert_eq!(view.summary, "The queue is empty.");
        for action in &view.bulk {
            assert!(!action.available, "{} is on offer", action.id);
            assert!(!action.reason.is_empty(), "{} has no reason", action.id);
        }
    }

    #[test]
    fn finished_jobs_are_the_ones_a_clear_takes() {
        let (mut state, mut jobs) = synced();
        jobs[0].status.state = JobState::Completed;
        state.set_priority(2, JobPriority::High);
        assert_eq!(
            state.finished(&jobs),
            vec![1, 3],
            "in the queue's own order"
        );
        assert_eq!(state.clear_finished(&jobs), vec![1, 3]);
        assert!(
            state.finished(&jobs).is_empty(),
            "the queue no longer knows about them"
        );
        assert!(state.clear_finished(&jobs).is_empty());
        assert_eq!(
            state.priority_of(2),
            JobPriority::High,
            "clearing left the rest alone"
        );
    }

    #[test]
    fn a_name_pattern_expands_the_title_and_its_number() {
        assert_eq!(
            output_name("{title}", "My Film", 1).expect("name"),
            "My Film"
        );
        assert_eq!(
            output_name("{title} ({n})", "My Film", 4).expect("name"),
            "My Film (4)"
        );
        assert_eq!(
            output_name("part-{n}", "", 2).expect("name"),
            "part-2",
            "a title with no name falls back to its number"
        );
        assert!(output_name("  ", "Film", 1)
            .expect_err("empty")
            .contains("file name"));
    }

    #[test]
    fn a_name_the_filesystem_would_refuse_is_refused_with_the_character_named() {
        let error = output_name("{title}", "A/B", 1).expect_err("slash");
        assert!(error.contains('/'), "{error}");
        assert!(output_name("{title}", "A:B", 1)
            .expect_err("colon")
            .contains(':'));
        assert!(output_name("{title}", "..", 1)
            .expect_err("dots")
            .contains("not a file name"));
        let long = "x".repeat(MAX_OUTPUT_NAME + 1);
        assert!(output_name(&long, "", 1)
            .expect_err("too long")
            .contains("120"));
    }

    #[test]
    fn two_titles_that_would_write_the_same_file_are_refused_together() {
        let titles = vec![(1, "Episode".to_string()), (2, "Episode".to_string())];
        let input = QueueSelectionInput {
            output_directory: "C:\\out".to_string(),
            name_pattern: "{title}".to_string(),
            names: BTreeMap::new(),
            titles: vec![1, 2],
        };
        let error = plan(&input, &titles, "mp4").expect_err("same file");
        assert!(error.contains("would both be written"), "{error}");
        assert!(error.contains("C:\\out\\Episode.mp4"), "{error}");

        let mut distinct = input.clone();
        distinct
            .names
            .insert("2".to_string(), "Episode two".to_string());
        let planned = plan(&distinct, &titles, "mp4").expect("distinct");
        assert_eq!(
            planned,
            vec![
                (1, "C:\\out\\Episode.mp4".to_string()),
                (2, "C:\\out\\Episode two.mp4".to_string())
            ]
        );
    }

    #[test]
    fn a_plan_needs_a_folder_and_at_least_one_title() {
        let titles = vec![(1, "Film".to_string())];
        let mut input = QueueSelectionInput {
            output_directory: String::new(),
            name_pattern: "{title}".to_string(),
            names: BTreeMap::new(),
            titles: vec![1],
        };
        assert!(plan(&input, &titles, "mp4")
            .expect_err("no folder")
            .contains("folder"));
        input.output_directory = "C:\\out".to_string();
        input.titles.clear();
        assert!(plan(&input, &titles, "mp4")
            .expect_err("no titles")
            .contains("at least one title"));
        input.titles = vec![9];
        assert!(plan(&input, &titles, "mp4")
            .expect_err("unknown title")
            .contains("no title 9"));
    }

    #[test]
    fn the_sheet_lists_every_title_and_marks_the_chosen_ones() {
        let titles = vec![
            (1u32, "Film".to_string(), "1:42:10 · 1920x1080".to_string()),
            (2, "Extra".to_string(), "0:05:00 · 640x360".to_string()),
        ];
        let input = QueueSelectionInput {
            output_directory: "C:\\out".to_string(),
            name_pattern: "{title}".to_string(),
            names: BTreeMap::new(),
            titles: vec![2],
        };
        let view = selection_view(&input, &titles, "mp4", true, "");
        assert_eq!(view.titles.len(), 2);
        assert!(!view.titles[0].chosen);
        assert!(view.titles[1].chosen);
        assert_eq!(view.titles[1].output_name, "Extra.mp4");
        assert_eq!(view.titles[1].output_path, "C:\\out\\Extra.mp4");
        assert_eq!(view.titles[1].range_label, "Whole title");
        assert!(!view.titles[0].range_available);
        assert!(
            view.titles[0]
                .range_reason
                .contains("no start and end field"),
            "{}",
            view.titles[0].range_reason
        );
        assert_eq!(view.chosen, 1);
        assert!(view.can_queue);
        assert_eq!(view.planned, vec!["C:\\out\\Extra.mp4".to_string()]);
        assert!(view.summary.contains("1 title(s)"));
    }

    #[test]
    fn the_sheet_reports_a_collision_rather_than_queueing_two_writes_to_one_file() {
        let titles = vec![
            (1u32, "Same".to_string(), "facts".to_string()),
            (2, "Same".to_string(), "facts".to_string()),
        ];
        let input = QueueSelectionInput {
            output_directory: "C:\\out".to_string(),
            name_pattern: "{title}".to_string(),
            names: BTreeMap::new(),
            titles: vec![1, 2],
        };
        let view = selection_view(&input, &titles, "mp4", true, "");
        assert!(view.titles[0].output_collides);
        assert!(view.titles[1].output_collides);
        assert!(!view.can_queue);
        assert!(
            view.problems
                .iter()
                .any(|p| p.contains("would both be written")),
            "{:?}",
            view.problems
        );
    }

    #[test]
    fn the_sheet_lists_every_thing_that_has_to_be_fixed() {
        let titles = vec![(1u32, "Same".to_string(), "facts".to_string())];
        let input = QueueSelectionInput {
            output_directory: String::new(),
            name_pattern: "bad/name".to_string(),
            names: BTreeMap::new(),
            titles: vec![1],
        };
        let view = selection_view(
            &input,
            &titles,
            "mp4",
            false,
            "Choose a folder to write into.",
        );
        assert!(!view.can_queue);
        assert!(!view.directory_valid);
        assert_eq!(view.directory_problem, "Choose a folder to write into.");
        assert!(!view.pattern_valid);
        assert!(view.problems.len() >= 3, "{:?}", view.problems);
    }

    #[test]
    fn a_source_with_no_titles_tells_the_user_to_scan_first() {
        let input = QueueSelectionInput::default();
        let view = selection_view(&input, &[], "mp4", true, "");
        assert!(view.titles.is_empty());
        assert!(view.summary.contains("Scan a source"));
        assert!(!view.can_queue);
    }

    #[test]
    fn a_queued_title_only_differs_from_the_base_job_by_its_index_and_output() {
        let base = spec("C:\\media\\movie.mkv", "C:\\out\\chosen.mp4");
        let queued = titled_spec(&base, 3, "C:\\out\\title-3.mp4");
        assert_eq!(queued.title_index, Some(3));
        assert_eq!(queued.output_path, "C:\\out\\title-3.mp4");
        assert_eq!(queued.input_path, base.input_path);
        assert_eq!(base.title_index, None, "the base job is not touched");
    }

    #[test]
    fn a_path_is_joined_with_one_separator() {
        assert_eq!(output_path("C:\\out", "a.mp4"), "C:\\out\\a.mp4");
        assert_eq!(output_path("C:\\out\\", "a.mp4"), "C:\\out\\a.mp4");
        assert_eq!(output_path("  ", "a.mp4"), "a.mp4");
        assert_eq!(file_name("C:\\media\\a.mkv"), "a.mkv");
        assert_eq!(file_name("a.mkv"), "a.mkv");
        assert_eq!(file_name("  "), "(no source)");
    }
}
