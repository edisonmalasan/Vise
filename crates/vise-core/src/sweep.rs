//! Quality sweep: one source, a range of quality values, and a table of results.
//!
//! Choosing a quality value by moving a slider and watching one file is a
//! guess. Choosing it by encoding the same twenty seconds at twenty values and
//! comparing the results is evidence. This module is the whole of that tool
//! except the encoding itself.
//!
//! # Three parts, and only two of them do anything
//!
//! - a **plan** ([`plan`]): which runs, decided from a range and bounded;
//! - an **executor** ([`SweepExecutor`]): one run in flight at a time, with
//!   progress and cancellation;
//! - a **results table** ([`SweepResults`]): what came back, in a fixed order.
//!
//! None of it runs the pipeline. Each run is an ordinary [`JobSpec`], so the
//! worker encodes it the same way it encodes anything else, and none of it
//! touches the session or the queue: a sweep is somebody's experiment, and an
//! experiment that reorders somebody's queue is not one they asked for.
//!
//! # Bounded on purpose
//!
//! Every encode costs minutes, so the two ways a sweep could go wrong are
//! handled rather than trusted:
//!
//! - **too many runs.** [`plan`] refuses an empty range and clamps a range that
//!   reaches outside the encoder's scale, and it widens the step until the plan
//!   is at most [`MAX_SWEEP_RUNS`] long. The plan records that it did so, so the
//!   results table can say the range was narrowed rather than silently
//!   answering a different question than the one asked.
//! - **a range nobody chose.** A range wider than [`CONFIRMED_SPAN`] is only
//!   planned when the request says the user confirmed it. Twenty-six encodes is
//!   a deliberate decision; five is not.
//!
//! # Cancelled means cancelled
//!
//! [`SweepExecutor::cancel`] stops the *next* run from being handed out and
//! marks the sweep cancelled. A run already in flight is not pretended away:
//! it still reports an outcome, and the row says the sweep was stopped rather
//! than showing a result for work that was interrupted halfway.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{EncodingMode, JobSpec, VideoCodec};

/// Largest number of encodes one sweep may ever plan.
///
/// Twelve is deliberately modest: enough to see the shape of the size/quality
/// curve, few enough that somebody pressing start has a fair idea of the wait.
pub const MAX_SWEEP_RUNS: usize = 12;

/// Widest range that is planned without the user confirming it.
///
/// One encode per value across the whole scale is fifty-two encodes of a whole
/// film. Nobody means that; the bound is here so that a control which lost a
/// `step` of `1` cannot quietly start it.
pub const CONFIRMED_SPAN: u32 = 26;

/// Lowest value on the encoder's quality scale.
pub const MIN_QUALITY: u32 = 0;

/// Highest value on the encoder's quality scale.
pub const MAX_QUALITY: u32 = 51;

/// Folder one sweep writes its runs into, relative to the chosen output folder.
pub const SWEEP_FOLDER: &str = "vise-quality-sweep";

/// The quality values a sweep should try.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SweepRange {
    /// Lowest value, ascending.
    pub from: u32,
    /// Highest value, inclusive.
    pub to: u32,
    /// Gap between values.
    pub step: u32,
}

impl Default for SweepRange {
    /// A small range either side of the middle of the scale, which is where a
    /// first sweep usually wants to look.
    fn default() -> Self {
        SweepRange {
            from: 18,
            to: 28,
            step: 2,
        }
    }
}

impl SweepRange {
    /// A range from `from` to `to` in steps of `step`.
    pub fn new(from: u32, to: u32, step: u32) -> Self {
        SweepRange { from, to, step }
    }

    /// How many values the range names before any bound is applied.
    pub fn span(&self) -> u32 {
        if self.to <= self.from {
            return 0;
        }
        (self.to - self.from) / self.step.max(1) + 1
    }

    /// Whether the range is wider than a plan will accept without confirmation.
    pub fn needs_confirmation(&self) -> bool {
        self.to.saturating_sub(self.from).saturating_add(1) > CONFIRMED_SPAN
    }
}

/// What a sweep should do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SweepRequest {
    /// The job every run is a copy of, with one quality value changed.
    pub base: JobSpec,
    /// The values to try.
    pub range: SweepRange,
    /// Folder the runs are written into.
    ///
    /// Separate from the job's own destination because a sweep is not the
    /// output: twenty files written over the user's chosen filename would be
    /// the worst possible way to answer "which value should I use".
    pub output_folder: String,
    /// Whether the user confirmed a range wider than [`CONFIRMED_SPAN`].
    pub confirmed: bool,
}

impl Default for SweepRequest {
    fn default() -> Self {
        SweepRequest {
            base: JobSpec::default(),
            range: SweepRange::default(),
            output_folder: String::new(),
            confirmed: false,
        }
    }
}

/// One encode the sweep will run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRun {
    /// Position in the plan, from zero.
    pub index: usize,
    /// The quality value this run uses.
    pub quality: u32,
    /// The job to hand to the worker.
    pub job: JobSpec,
}

/// The runs a sweep will make, and anything the request asked for that it will
/// not do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SweepPlan {
    /// The runs, in the order they will be handed out.
    pub runs: Vec<SweepRun>,
    /// Why there are no runs, when there are none.
    pub refusal: String,
    /// Whether the range was narrowed to stay inside [`MAX_SWEEP_RUNS`].
    pub narrowed: bool,
    /// The values the range was clamped to before the step was widened.
    pub values: SweepRange,
}

impl SweepPlan {
    /// A plan that will not run, with the reason.
    pub fn refused(reason: impl Into<String>) -> Self {
        SweepPlan {
            runs: Vec::new(),
            refusal: reason.into(),
            narrowed: false,
            values: SweepRange::default(),
        }
    }

    /// How many encodes the plan will make.
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    /// Whether the plan has nothing to run.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Whether the plan can be handed to an executor.
    pub fn is_runnable(&self) -> bool {
        !self.runs.is_empty()
    }

    /// One line describing what was decided.
    pub fn summary(&self) -> String {
        if !self.is_runnable() {
            return self.refusal.clone();
        }
        let base = format!(
            "{} run(s) from quality {} to {}",
            self.runs.len(),
            self.values.from,
            self.values.to
        );
        if self.narrowed {
            format!("{base}, with the step widened to stay inside the {MAX_SWEEP_RUNS}-run bound")
        } else {
            base
        }
    }
}

/// Decide which runs a request asks for.
///
/// Pure, and the only place the bounds are applied, so the executor can be as
/// simple as "hand out the next one". The order of the checks is the order of
/// the refusals a person reads: nothing to sweep, a range that cannot be read,
/// a range that is too wide to be a decision, and only then the arithmetic.
pub fn plan(request: &SweepRequest) -> SweepPlan {
    let range = request.range;
    if range.step == 0 {
        return SweepPlan::refused(
            "A quality sweep needs a step of at least 1; 0 would run the same value forever.",
        );
    }
    if range.to < range.from {
        return SweepPlan::refused(format!(
            "A quality sweep runs from a low value to a high one; {} is above {}.",
            range.from, range.to
        ));
    }
    let from = range.from.clamp(MIN_QUALITY, MAX_QUALITY);
    let to = range.to.clamp(MIN_QUALITY, MAX_QUALITY);
    let mut step = range.step.max(1);
    let mut values = SweepRange::new(from, to, step);
    let mut run_count = values.span() as usize;
    if run_count == 0 {
        run_count = 1;
    }

    let span = to.saturating_sub(from).saturating_add(1);
    if !request.confirmed && span > CONFIRMED_SPAN {
        return SweepPlan::refused(format!(
            "{span} encodes is more than a range anybody means to sweep. Choose a range of {CONFIRMED_SPAN} values or fewer, or confirm the wider one."
        ));
    }

    // Widening the step rather than dropping values is deliberate: a sweep of
    // 18..40 by 2 answers "what does this range look like", and the best
    // answer to that is still a sweep of that range with fewer samples rather
    // than a sweep of a different range the user never asked for.
    let mut narrowed = false;
    while run_count > MAX_SWEEP_RUNS && step <= to.saturating_sub(from) {
        step += 1;
        values.step = step;
        run_count = values.span() as usize;
        narrowed = true;
    }
    if run_count > MAX_SWEEP_RUNS {
        return SweepPlan::refused(format!(
            "A sweep of quality {from} to {to} needs more than {MAX_SWEEP_RUNS} values even at a step of 1; choose a narrower range."
        ));
    }

    let mut runs = Vec::with_capacity(run_count);
    let mut quality = from;
    while quality <= to && runs.len() < MAX_SWEEP_RUNS {
        runs.push(SweepRun {
            index: runs.len(),
            quality,
            job: run_job(&request.base, quality, &request.output_folder),
        });
        match quality.checked_add(step) {
            Some(next) => quality = next,
            None => break,
        }
    }
    SweepPlan {
        runs,
        refusal: String::new(),
        narrowed,
        values,
    }
}

/// One run's job: the base job with the quality value put on it and a
/// destination of its own.
fn run_job(base: &JobSpec, quality: u32, folder: &str) -> JobSpec {
    let mut job = base.clone();
    job.quality = Some(quality);
    job.cq = Some(quality);
    job.crf = Some(quality);
    job.constant_quality_value = Some(quality);
    job.encoding_mode = Some(EncodingMode::ConstantQuality);
    job.output_path = run_output_path(base, quality, folder);
    job
}

/// Where one run's file is written.
///
/// The quality value is part of the name rather than a folder level, so the
/// twenty files sit together in one list the person can open, and so two runs
/// of two different sweeps of the same source cannot collide.
pub fn run_output_path(base: &JobSpec, quality: u32, folder: &str) -> String {
    let stem = Path::new(&base.output_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .or_else(|| {
            Path::new(&base.input_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| !stem.trim().is_empty())
        })
        .unwrap_or("title");
    let extension = Path::new(&base.output_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("mkv");
    let directory = if folder.trim().is_empty() {
        PathBuf::from(SWEEP_FOLDER)
    } else {
        PathBuf::from(folder.trim()).join(SWEEP_FOLDER)
    };
    directory
        .join(format!("q{quality:02}-{stem}.{extension}"))
        .to_string_lossy()
        .to_string()
}

/// Where a sweep for this job would write, for the surface to show.
pub fn sweep_folder(folder: &str) -> String {
    if folder.trim().is_empty() {
        return SWEEP_FOLDER.to_string();
    }
    PathBuf::from(folder.trim())
        .join(SWEEP_FOLDER)
        .to_string_lossy()
        .to_string()
}

/// What one run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepOutcome {
    /// The quality value that was tried.
    pub quality: u32,
    /// Whether the file was produced.
    pub succeeded: bool,
    /// Size of the produced file, when there is one.
    pub bytes: u64,
    /// How long the encode took.
    pub seconds: f64,
    /// Why it failed, when it did.
    pub message: String,
    /// Whether the sweep was cancelled while this run was in flight.
    pub cancelled: bool,
}

impl SweepOutcome {
    /// A run that produced a file.
    pub fn succeeded(quality: u32, bytes: u64, seconds: f64) -> Self {
        SweepOutcome {
            quality,
            succeeded: true,
            bytes,
            seconds,
            message: String::new(),
            cancelled: false,
        }
    }

    /// A run that did not.
    pub fn failed(quality: u32, message: impl Into<String>) -> Self {
        SweepOutcome {
            quality,
            succeeded: false,
            bytes: 0,
            seconds: 0.0,
            message: message.into(),
            cancelled: false,
        }
    }
}

/// Where a sweep has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SweepState {
    /// Nothing has been started.
    Idle,
    /// A run is in flight or more are waiting.
    Running,
    /// Stopped on request; the runs already finished are still results.
    Cancelled,
    /// Every planned run has reported.
    Finished,
}

impl SweepState {
    /// Whether the sweep is going to ask for anything else.
    pub fn is_running(self) -> bool {
        matches!(self, SweepState::Running)
    }
}

/// Progress, as the surface shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepProgress {
    /// Where the sweep has got to.
    pub state: SweepState,
    /// Runs that have reported.
    pub completed: usize,
    /// Runs the plan holds.
    pub total: usize,
    /// The quality value being encoded right now.
    pub current: Option<u32>,
    /// Whether the sweep was cancelled.
    pub cancelled: bool,
    /// One line describing the progress.
    pub summary: String,
}

/// One row of the results table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepResultRow {
    /// The quality value that was tried.
    pub quality: u32,
    /// Whether the file was produced.
    pub succeeded: bool,
    /// Size of the produced file.
    pub bytes: u64,
    /// How long the encode took.
    pub seconds: f64,
    /// Size relative to the smallest run that succeeded.
    ///
    /// This is the column that makes a sweep worth running: a value that costs
    /// twice the disk of the cheapest value that still looked acceptable is the
    /// decision, expressed as a number.
    pub relative_size: f64,
    /// Whether the sweep was cancelled during this run.
    pub cancelled: bool,
    /// Why it failed, when it did.
    pub message: String,
}

/// Everything the sweep produced, in a fixed order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SweepResults {
    /// The rows, lowest quality first.
    pub rows: Vec<SweepResultRow>,
    /// How many runs succeeded.
    pub succeeded: usize,
    /// How many runs did not.
    pub failed: usize,
    /// Whether the sweep was cancelled before it finished.
    pub cancelled: bool,
}

impl SweepResults {
    /// Rows ordered by quality, with the relative size filled in.
    ///
    /// The order is by quality rather than by completion, because the table is
    /// a comparison and a comparison in encode order is a table somebody has to
    /// sort before they can read it.
    pub fn of(mut outcomes: Vec<SweepOutcome>, cancelled: bool) -> Self {
        outcomes.sort_by_key(|outcome| outcome.quality);
        let smallest = outcomes
            .iter()
            .filter(|outcome| outcome.succeeded && outcome.bytes > 0)
            .map(|outcome| outcome.bytes)
            .min()
            .unwrap_or(0);
        let rows: Vec<SweepResultRow> = outcomes
            .into_iter()
            .map(|outcome| SweepResultRow {
                relative_size: if smallest > 0 && outcome.succeeded {
                    (outcome.bytes as f64 / smallest as f64 * 100.0).round() / 100.0
                } else {
                    0.0
                },
                quality: outcome.quality,
                succeeded: outcome.succeeded,
                bytes: outcome.bytes,
                seconds: outcome.seconds,
                cancelled: outcome.cancelled,
                message: outcome.message,
            })
            .collect();
        let succeeded = rows.iter().filter(|row| row.succeeded).count();
        SweepResults {
            failed: rows.len().saturating_sub(succeeded),
            rows,
            succeeded,
            cancelled,
        }
    }

    /// One line describing the table.
    pub fn summary(&self) -> String {
        if self.rows.is_empty() {
            return "No sweep runs have finished yet.".to_string();
        }
        let mut text = format!(
            "{} run(s): {} succeeded, {} did not",
            self.rows.len(),
            self.succeeded,
            self.failed
        );
        if self.cancelled {
            text.push_str("; the sweep was cancelled");
        }
        text
    }
}

/// Runs one encode at a time and remembers what came back.
///
/// The state machine is deliberately small, because the only thing it has to
/// guarantee is the thing that matters: **one encode in flight**. Two encodes
/// at once on a machine the user is also using is the failure mode a sweep is
/// most likely to cause, and it is the one this type makes unrepresentable.
#[derive(Debug, Clone)]
pub struct SweepExecutor {
    runs: Vec<SweepRun>,
    values: SweepRange,
    narrowed: bool,
    /// Index of the run in flight, if one is.
    in_flight: Option<usize>,
    outcomes: Vec<SweepOutcome>,
    cancelled: bool,
}

impl SweepExecutor {
    /// An executor for a runnable plan.
    pub fn start(plan: &SweepPlan) -> Self {
        SweepExecutor {
            runs: plan.runs.clone(),
            values: plan.values,
            narrowed: plan.narrowed,
            in_flight: None,
            outcomes: Vec::new(),
            cancelled: false,
        }
    }

    /// How many runs the plan holds.
    pub fn total(&self) -> usize {
        self.runs.len()
    }

    /// How many runs have reported.
    pub fn completed(&self) -> usize {
        self.outcomes.len()
    }

    /// Whether anything is being encoded right now.
    pub fn is_busy(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Whether the sweep has stopped.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the sweep has stopped and nothing is in flight.
    pub fn is_finished(&self) -> bool {
        self.cancelled && self.in_flight.is_none()
    }

    /// Whether the plan was narrowed to stay inside the run bound.
    pub fn was_narrowed(&self) -> bool {
        self.narrowed
    }

    /// The range as it was finally planned.
    pub fn values(&self) -> SweepRange {
        self.values
    }

    /// The next run, when one may be handed out.
    ///
    /// `None` while a run is in flight, after the sweep is cancelled, and when
    /// the plan is exhausted. Those three are the same answer for the same
    /// reason: there is nothing for the caller to do about any of them, and an
    /// error it could act on would be a lie.
    pub fn next_run(&mut self) -> Option<&SweepRun> {
        if self.cancelled || self.in_flight.is_some() {
            return None;
        }
        let index = self.outcomes.len();
        let run = self.runs.get(index)?;
        self.in_flight = Some(index);
        Some(run)
    }

    /// Report what a run produced.
    ///
    /// Returns `false` when there is nothing in flight to report, which happens
    /// when a caller reports the same run twice. The duplicate is dropped rather
    /// than stored: two rows for one quality value would be a table that lies
    /// about how many encodes ran.
    pub fn finish_run(&mut self, outcome: SweepOutcome) -> bool {
        if self.in_flight.take().is_none() {
            return false;
        }
        self.outcomes.push(outcome);
        true
    }

    /// Stop handing out runs.
    ///
    /// A run already in flight is left alone and still reports; the caller
    /// decides whether to stop the encoder. What is refused here is a *new*
    /// encode, which is the part the user asked to stop.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Where the sweep has got to.
    pub fn progress(&self) -> SweepProgress {
        let state = if self.runs.is_empty() {
            SweepState::Idle
        } else if self.cancelled {
            if self.in_flight.is_some() || self.outcomes.len() < self.runs.len() {
                SweepState::Cancelled
            } else {
                SweepState::Finished
            }
        } else if self.outcomes.len() >= self.runs.len() {
            SweepState::Finished
        } else {
            SweepState::Running
        };
        let completed = self.outcomes.len();
        let total = self.runs.len();
        let current = self
            .in_flight
            .and_then(|index| self.runs.get(index))
            .map(|run| run.quality);
        SweepProgress {
            state,
            completed,
            total,
            current,
            cancelled: self.cancelled,
            summary: match state {
                SweepState::Idle => "The sweep has not been started.".to_string(),
                SweepState::Running => format!(
                    "{completed} of {total} run(s) finished{}",
                    match current {
                        Some(quality) => format!(", encoding quality {quality}"),
                        None => String::new(),
                    }
                ),
                SweepState::Cancelled => {
                    format!("the sweep was cancelled after {completed} of {total} run(s)")
                }
                SweepState::Finished => format!("all {total} run(s) finished"),
            },
        }
    }

    /// The results so far.
    pub fn results(&self) -> SweepResults {
        SweepResults::of(self.outcomes.clone(), self.cancelled)
    }

    /// The runs, for the surface to list before they start.
    pub fn runs(&self) -> &[SweepRun] {
        &self.runs
    }
}

/// The quality values the pipeline would send for a codec, for the surface to
/// explain the range against.
///
/// Exposed because the bounds in this module are the same ones the encoder
/// arguments use, and a range that is quietly clamped is only honest if the
/// thing it was clamped against can be seen.
pub fn quality_bounds(codec: VideoCodec) -> (u32, u32) {
    // The intra-frame and archival codecs are lossless by construction, so the
    // scale does not apply to them; saying so is better than planning a sweep
    // whose every run produces the same bytes.
    if codec.is_intra_only() {
        return (0, 0);
    }
    (MIN_QUALITY, MAX_QUALITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContainerFormat;

    fn base() -> JobSpec {
        JobSpec {
            input_path: "C:\\media\\raw.mkv".into(),
            output_path: "D:\\out\\Episode 1.mkv".into(),
            container: ContainerFormat::Mkv,
            ..JobSpec::default()
        }
    }

    fn request(range: SweepRange, confirmed: bool) -> SweepRequest {
        SweepRequest {
            base: base(),
            range,
            output_folder: "D:\\out".into(),
            confirmed,
        }
    }

    #[test]
    fn a_plan_is_one_job_per_quality_value_and_nothing_else() {
        let plan = plan(&request(SweepRange::new(20, 24, 2), false));
        assert!(plan.is_runnable(), "{}", plan.refusal);
        assert_eq!(
            plan.runs.iter().map(|r| r.quality).collect::<Vec<_>>(),
            vec![20, 22, 24]
        );
        assert_eq!(
            plan.runs.iter().map(|r| r.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(!plan.narrowed);
    }

    #[test]
    fn every_run_is_the_base_job_with_one_value_changed_and_its_own_destination() {
        let plan = plan(&request(SweepRange::new(20, 22, 2), false));
        for run in &plan.runs {
            assert_eq!(run.job.input_path, base().input_path);
            assert_eq!(run.job.quality, Some(run.quality));
            assert_eq!(run.job.cq, Some(run.quality));
            assert_eq!(run.job.encoding_mode, Some(EncodingMode::ConstantQuality));
            assert_ne!(run.job.output_path, base().output_path);
            assert!(
                run.job.output_path.contains(SWEEP_FOLDER),
                "{}",
                run.job.output_path
            );
            assert!(run
                .job
                .output_path
                .contains(&format!("q{:02}", run.quality)));
            assert!(run.job.output_path.ends_with(".mkv"));
        }
        assert_ne!(plan.runs[0].job.output_path, plan.runs[1].job.output_path);
    }

    #[test]
    fn a_range_with_no_step_or_no_order_is_refused_with_a_reason() {
        let zero = plan(&request(SweepRange::new(20, 24, 0), false));
        assert!(!zero.is_runnable());
        assert!(zero.refusal.contains("step"), "{}", zero.refusal);
        assert_eq!(zero.len(), 0, "and nothing is planned behind the refusal");

        let backwards = plan(&request(SweepRange::new(30, 20, 2), false));
        assert!(!backwards.is_runnable());
        assert!(backwards.refusal.contains("30"), "{}", backwards.refusal);
    }

    #[test]
    fn a_range_outside_the_scale_is_clamped_rather_than_refused() {
        let plan = plan(&request(SweepRange::new(0, 200, 100), true));
        assert!(plan.is_runnable(), "{}", plan.refusal);
        assert!(plan.values.from >= MIN_QUALITY);
        assert!(plan.values.to <= MAX_QUALITY);
        for run in &plan.runs {
            assert!((MIN_QUALITY..=MAX_QUALITY).contains(&run.quality));
        }
    }

    #[test]
    fn a_wide_range_is_only_planned_when_the_user_confirmed_it() {
        let wide = SweepRange::new(0, 51, 1);
        assert!(wide.needs_confirmation());
        let refused = plan(&request(wide, false));
        assert!(!refused.is_runnable());
        assert!(
            refused.refusal.contains("confirm"),
            "the reason names the way out: {}",
            refused.refusal
        );

        let confirmed = plan(&request(wide, true));
        assert!(confirmed.is_runnable(), "{}", confirmed.refusal);
        assert!(confirmed.narrowed, "and it says it narrowed the step");
    }

    #[test]
    fn no_plan_is_ever_longer_than_the_bound() {
        // Every step, every width, confirmed or not: the bound is the bound.
        for step in 1..=9u32 {
            for span in [1u32, 2, 5, 12, 26, 51] {
                for confirmed in [false, true] {
                    let range = SweepRange::new(0, span.saturating_sub(1), step);
                    let planned = plan(&request(range, confirmed));
                    assert!(
                        planned.len() <= MAX_SWEEP_RUNS,
                        "step {step} span {span} planned {}",
                        planned.len()
                    );
                }
            }
        }
    }

    #[test]
    fn a_range_too_long_to_sample_is_narrowed_by_widening_the_step_not_by_changing_the_range() {
        let planned = plan(&request(SweepRange::new(10, 50, 1), true));
        assert!(planned.is_runnable());
        assert!(planned.narrowed);
        assert_eq!(planned.values.from, 10);
        assert_eq!(planned.values.to, 50);
        assert!(planned.values.step > 1);
        assert_eq!(planned.runs.first().map(|r| r.quality), Some(10));
        assert_eq!(planned.runs.last().map(|r| r.quality), Some(50));
        assert!(
            planned.summary().contains("widened"),
            "{}",
            planned.summary()
        );
    }

    #[test]
    fn a_narrow_range_is_planned_exactly_as_it_was_asked() {
        let planned = plan(&request(SweepRange::new(18, 26, 2), false));
        assert!(!planned.narrowed);
        assert_eq!(planned.values.step, 2);
        assert_eq!(planned.len(), 5);
        assert_eq!(planned.summary(), "5 run(s) from quality 18 to 26");
    }

    #[test]
    fn one_run_at_a_time_is_the_only_thing_the_executor_has_to_get_right() {
        let planned = plan(&request(SweepRange::new(20, 24, 2), false));
        let mut sweep = SweepExecutor::start(&planned);
        let first = sweep.next_run().expect("first run").clone();
        assert!(sweep.is_busy());
        assert!(
            sweep.next_run().is_none(),
            "a second encode is never handed out while one is running"
        );
        assert_eq!(sweep.progress().current, Some(first.quality));

        assert!(sweep.finish_run(SweepOutcome::succeeded(first.quality, 100, 3.0)));
        let second = sweep.next_run().expect("second run").clone();
        assert_eq!(second.quality, 22);
        assert_eq!(
            sweep.progress().completed,
            1,
            "one run has reported and one is in flight"
        );
    }

    #[test]
    fn a_run_reported_twice_is_reported_once() {
        let planned = plan(&request(SweepRange::new(20, 22, 2), false));
        let mut sweep = SweepExecutor::start(&planned);
        let run = sweep.next_run().expect("first").clone();
        assert!(sweep.finish_run(SweepOutcome::succeeded(run.quality, 10, 1.0)));
        assert!(
            !sweep.finish_run(SweepOutcome::succeeded(run.quality, 10, 1.0)),
            "there is nothing in flight the second time"
        );
        assert_eq!(sweep.completed(), 1);
    }

    #[test]
    fn cancelling_stops_the_next_run_and_leaves_the_finished_ones_alone() {
        let planned = plan(&request(SweepRange::new(20, 30, 2), false));
        let mut sweep = SweepExecutor::start(&planned);
        let run = sweep.next_run().expect("first").clone();
        sweep.finish_run(SweepOutcome::succeeded(run.quality, 100, 1.0));

        sweep.cancel();
        assert!(
            sweep.next_run().is_none(),
            "no further encode is handed out"
        );
        assert!(sweep.is_cancelled());
        let progress = sweep.progress();
        assert_eq!(progress.state, SweepState::Cancelled);
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 6);
        assert!(progress.cancelled);
        assert!(
            progress.summary.contains("cancelled"),
            "{}",
            progress.summary
        );

        let results = sweep.results();
        assert_eq!(results.rows.len(), 1);
        assert!(results.cancelled);
        assert!(results.summary().contains("cancelled"));
    }

    #[test]
    fn a_run_in_flight_when_the_sweep_is_cancelled_still_reports_and_says_it_was_stopped() {
        let planned = plan(&request(SweepRange::new(20, 24, 2), false));
        let mut sweep = SweepExecutor::start(&planned);
        sweep.next_run().expect("first");
        sweep.cancel();
        assert_eq!(sweep.progress().state, SweepState::Cancelled);
        assert!(sweep.is_busy());

        let mut outcome = SweepOutcome::succeeded(20, 40, 2.0);
        outcome.cancelled = true;
        assert!(sweep.finish_run(outcome));
        assert!(sweep.is_finished());
        let rows = sweep.results().rows;
        assert_eq!(rows.len(), 1);
        assert!(rows[0].cancelled, "the row says it was stopped");
    }

    #[test]
    fn a_finished_sweep_asks_for_nothing_more() {
        let planned = plan(&request(SweepRange::new(20, 22, 2), false));
        let mut sweep = SweepExecutor::start(&planned);
        for expected in [20, 22] {
            let run = sweep.next_run().expect("a run").clone();
            assert_eq!(run.quality, expected);
            sweep.finish_run(SweepOutcome::succeeded(expected, 100, 1.0));
        }
        assert!(sweep.next_run().is_none());
        let progress = sweep.progress();
        assert_eq!(progress.state, SweepState::Finished);
        assert_eq!(progress.summary, "all 2 run(s) finished");
    }

    #[test]
    fn the_table_is_ordered_by_quality_and_scaled_against_the_smallest_success() {
        let outcomes = vec![
            SweepOutcome::succeeded(24, 400, 8.0),
            SweepOutcome::succeeded(20, 100, 4.0),
            SweepOutcome::failed(22, "the encoder refused it"),
        ];
        let results = SweepResults::of(outcomes, false);
        assert_eq!(
            results.rows.iter().map(|r| r.quality).collect::<Vec<_>>(),
            vec![20, 22, 24]
        );
        assert_eq!(results.rows[0].relative_size, 1.0);
        assert_eq!(results.rows[2].relative_size, 4.0);
        assert_eq!(
            results.rows[1].relative_size, 0.0,
            "a failed run has no size"
        );
        assert_eq!(results.succeeded, 2);
        assert_eq!(results.failed, 1);
        assert!(
            results.summary().contains("2 succeeded"),
            "{}",
            results.summary()
        );
    }

    #[test]
    fn an_empty_table_says_so_rather_than_looking_like_a_failure() {
        let results = SweepResults::of(Vec::new(), false);
        assert!(results.rows.is_empty());
        assert_eq!(results.succeeded, 0);
        assert_eq!(results.summary(), "No sweep runs have finished yet.");
    }

    #[test]
    fn two_sweeps_of_the_same_source_write_different_files() {
        let first = run_output_path(&base(), 20, "D:\\out");
        let second = run_output_path(&base(), 24, "D:\\out");
        assert_ne!(first, second);
        assert!(first.contains("q20"));
        assert!(second.contains("q24"));
        // And neither of them is the destination the user chose.
        assert_ne!(first, base().output_path);
    }

    #[test]
    fn a_sweep_with_nowhere_to_write_falls_back_to_a_folder_of_its_own() {
        let job = run_output_path(&base(), 20, "");
        assert!(job.starts_with(SWEEP_FOLDER), "{job}");
        assert_eq!(sweep_folder(""), SWEEP_FOLDER);
        assert!(sweep_folder("D:\\out").ends_with(SWEEP_FOLDER));
    }

    #[test]
    fn the_quality_bounds_say_when_a_codec_has_no_scale() {
        assert_eq!(quality_bounds(VideoCodec::H264), (MIN_QUALITY, MAX_QUALITY));
        assert_eq!(quality_bounds(VideoCodec::Av1), (MIN_QUALITY, MAX_QUALITY));
        assert_eq!(
            quality_bounds(VideoCodec::ProRes),
            (0, 0),
            "an intra codec is lossless by construction, so there is nothing to sweep"
        );
    }
}
