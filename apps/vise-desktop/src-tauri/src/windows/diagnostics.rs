//! The diagnostics surface.
//!
//! One window for the six things a person needs when something has gone wrong or
//! when they are deciding between two settings: the per-encode logs, the
//! hardware report, the quality sweep, the queue table, and the bundle.
//!
//! # Everything here is a decision made before anything is done
//!
//! The surface decides *what* would happen — which runs a sweep would make,
//! whether a range needs confirming, what the last export was, where the logs
//! live — and hands the window plain values to carry out. That split is the
//! reason the tests here need no window, no worker, and no filesystem: every
//! question this surface answers is a question about values.
//!
//! # The sweep never touches the queue
//!
//! It runs its own jobs, writes them to a folder of its own, and records its own
//! results. The session is not touched, the queue's order is not touched, and a
//! sweep is refused outright while the queue has an encode running — not
//! because the worker would stop it, but because the two would then be competing
//! for the one encoder and the user would be shown a table that is really a
//! measurement of contention.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use vise_core::{
    CapabilityReport, SweepExecutor, SweepPlan, SweepProgress, SweepRange, SweepRequest,
    SweepResults,
};
use vise_ipc::enclog::{self, LogFile, RotationPolicy};

/// Identifier of the diagnostics surface.
pub const SURFACE: &str = "diagnostics";

/// One per-encode log file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeLogViewModel {
    /// The file's name, which is the encode it belongs to.
    pub name: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Seconds since it was last written.
    pub age_seconds: u64,
    /// Whether it is the log being written right now.
    pub in_progress: bool,
    /// Whether a rotation would leave it alone.
    ///
    /// Shown because "will this be deleted before I read it" is the question a
    /// person opens this surface with.
    pub kept: bool,
    /// Size as a person reads it.
    pub size_label: String,
}

/// One row of the sweep's results table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRowViewModel {
    /// The quality value tried.
    pub quality: u32,
    /// Whether the file was produced.
    pub succeeded: bool,
    /// Size of the produced file.
    pub size_label: String,
    /// Size against the smallest successful run.
    pub relative_label: String,
    /// How long it took.
    pub seconds_label: String,
    /// Why it failed, when it did.
    pub message: String,
    /// Whether the sweep was stopped during this run.
    pub cancelled: bool,
}

/// The sweep, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepViewModel {
    /// The values the last plan would use.
    pub range: SweepRange,
    /// How many runs that is.
    pub planned: usize,
    /// What the plan decided, in one line.
    pub plan_summary: String,
    /// Where a refused plan says the range is not one this build will sweep.
    pub refusal: String,
    /// Whether the range was narrowed to stay inside the run bound.
    pub narrowed: bool,
    /// Where the runs are written.
    pub folder: String,
    /// Progress.
    pub progress: SweepProgress,
    /// The table, lowest quality first.
    pub rows: Vec<SweepRowViewModel>,
    /// One line describing the table.
    pub results_summary: String,
    /// Whether there is anything to start.
    pub can_start: bool,
    /// Why there is not, when there is not.
    pub start_reason: String,
}

/// One report this surface produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportViewModel {
    /// What kind of thing it was: results, capabilities, bundle.
    pub kind: String,
    /// The file it was written to.
    pub path: String,
    /// One line describing it.
    pub summary: String,
    /// Whether it could be written.
    pub written: bool,
    /// Why not, when not.
    pub problem: String,
}

/// The hardware report, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityViewModel {
    /// One line describing the detection.
    pub summary: String,
    /// Whether the tools were found at all.
    pub detected: bool,
    /// Hardware encoders that were found.
    pub hardware_encoders: Vec<String>,
    /// Acceleration names that were published.
    pub accelerators: Vec<String>,
    /// One line per thing worth knowing.
    pub notes: Vec<String>,
    /// Whether Dolby Vision RPU coding is available.
    pub dolby_vision_rpu: bool,
    /// Whether HDR10+ dynamic metadata can be forwarded.
    pub hdr10_plus_sei: bool,
}

/// The whole surface, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsViewModel {
    /// Where the per-encode logs are.
    pub log_folder: String,
    /// The per-encode logs, newest first.
    pub encode_logs: Vec<EncodeLogViewModel>,
    /// One line about the log folder.
    pub logs_summary: String,
    /// The hardware report, when detection has run.
    pub capability: Option<CapabilityViewModel>,
    /// The sweep.
    pub sweep: SweepViewModel,
    /// The last report of each kind this surface produced.
    pub reports: Vec<ReportViewModel>,
    /// One line describing the surface.
    pub summary: String,
}

impl From<&CapabilityReport> for CapabilityViewModel {
    fn from(report: &CapabilityReport) -> Self {
        CapabilityViewModel {
            summary: report.summary(),
            detected: report.detected,
            hardware_encoders: report.hardware_encoders.clone(),
            accelerators: report.accelerators.clone(),
            notes: report.notes.clone(),
            dolby_vision_rpu: report.hdr_dolby_vision_rpu,
            hdr10_plus_sei: report.hdr_hdr10_plus_sei,
        }
    }
}

/// Everything the surface holds between openings.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticsState {
    /// Folder the per-encode logs are written into.
    log_folder: PathBuf,
    /// What the last sweep was asked for.
    request: SweepRequest,
    /// The last plan, whether or not it was runnable.
    plan: SweepPlan,
    /// The running or finished sweep.
    sweep: Option<SweepExecutor>,
    /// The last hardware report, when detection has run.
    capability: Option<CapabilityReport>,
    /// The reports this surface has produced.
    reports: Vec<ReportViewModel>,
}

impl DiagnosticsState {
    /// A surface whose logs live in `log_folder`.
    pub fn new(log_folder: PathBuf) -> Self {
        DiagnosticsState {
            log_folder,
            request: SweepRequest::default(),
            plan: SweepPlan::default(),
            sweep: None,
            capability: None,
            reports: Vec::new(),
        }
    }

    /// Folder the per-encode logs are written into.
    pub fn log_folder(&self) -> &PathBuf {
        &self.log_folder
    }

    /// Set the range the next sweep would use.
    pub fn set_range(&mut self, range: SweepRange) -> &SweepPlan {
        self.request.range = range;
        self.replan()
    }

    /// Confirm a range that is wider than the build will sweep on its own.
    pub fn set_confirmed(&mut self, confirmed: bool) -> &SweepPlan {
        self.request.confirmed = confirmed;
        self.replan()
    }

    /// Choose the folder the sweep's runs are written into.
    pub fn set_output_folder(&mut self, folder: impl Into<String>) -> &SweepPlan {
        self.request.output_folder = folder.into();
        self.replan()
    }

    /// Replace the job every sweep run is a copy of.
    pub fn set_base_job(&mut self, job: vise_core::JobSpec) -> &SweepPlan {
        self.request.base = job;
        self.replan()
    }

    /// The plan as it stands.
    pub fn plan(&self) -> &SweepPlan {
        &self.plan
    }

    fn replan(&mut self) -> &SweepPlan {
        self.plan = vise_core::sweep::plan(&self.request);
        &self.plan
    }

    /// Whether the sweep is running.
    pub fn is_sweeping(&self) -> bool {
        self.sweep
            .as_ref()
            .is_some_and(|sweep| sweep.progress().state.is_running())
    }

    /// Whether a sweep is waiting for something to be done with it.
    pub fn is_busy(&self) -> bool {
        self.sweep.as_ref().is_some_and(|sweep| sweep.is_busy())
    }

    /// Whether the real queue has an encode running.
    ///
    /// Injected rather than read from the queue so that the rule can be tested
    /// without building a queue, and so that the one caller of this rule is the
    /// one place that knows what "the real queue" is.
    pub fn sweep_refusal(&self, queue_running: bool) -> Option<String> {
        if self.is_sweeping() {
            return Some("a quality sweep is already running.".to_string());
        }
        if self.is_busy() {
            return Some(
                "the last sweep is still finishing an encode; wait for it or cancel it."
                    .to_string(),
            );
        }
        if queue_running {
            return Some(
                "the queue has an encode running, and a sweep would be competing with it for the same encoder."
                    .to_string(),
            );
        }
        if self.request.base.input_path.trim().is_empty() {
            return Some(
                "open a source first: a sweep encodes the source that is loaded.".to_string(),
            );
        }
        if !self.plan.is_runnable() {
            return Some(if self.plan.refusal.is_empty() {
                "choose a quality range first.".to_string()
            } else {
                self.plan.refusal.clone()
            });
        }
        None
    }

    /// Start a sweep from the current plan.
    pub fn start_sweep(&mut self) -> Result<SweepPlan, String> {
        if !self.plan.is_runnable() {
            return Err(if self.plan.refusal.is_empty() {
                "there is no sweep to start.".to_string()
            } else {
                self.plan.refusal.clone()
            });
        }
        self.sweep = Some(SweepExecutor::start(&self.plan));
        Ok(self.plan.clone())
    }

    /// The next run, when one may be handed out.
    ///
    /// Owned rather than borrowed because the encode it describes is driven
    /// without the window's lock held, and a reference into the state cannot
    /// outlive it.
    pub fn next_run(&mut self) -> Option<vise_core::SweepRun> {
        self.sweep
            .as_mut()
            .and_then(|sweep| sweep.next_run().cloned())
    }

    /// Record what a run produced.
    pub fn finish_run(&mut self, outcome: vise_core::SweepOutcome) -> bool {
        self.sweep
            .as_mut()
            .is_some_and(|sweep| sweep.finish_run(outcome))
    }

    /// Stop the sweep from handing out another run.
    pub fn cancel_sweep(&mut self) -> bool {
        match self.sweep.as_mut() {
            Some(sweep) => {
                sweep.cancel();
                true
            }
            None => false,
        }
    }

    /// Where a sweep would write.
    pub fn sweep_folder(&self) -> String {
        vise_core::sweep::sweep_folder(&self.request.output_folder)
    }

    /// Progress.
    pub fn sweep_progress(&self) -> SweepProgress {
        self.sweep
            .as_ref()
            .map(|sweep| sweep.progress())
            .unwrap_or_else(|| SweepProgress {
                state: vise_core::sweep::SweepState::Idle,
                completed: 0,
                total: self.plan.len(),
                current: None,
                cancelled: false,
                summary: "No quality sweep has been started.".to_string(),
            })
    }

    /// The results so far.
    pub fn sweep_results(&self) -> SweepResults {
        self.sweep
            .as_ref()
            .map(|sweep| sweep.results())
            .unwrap_or_default()
    }

    /// Record the hardware report.
    pub fn observe_capabilities(&mut self, report: CapabilityReport) {
        self.capability = Some(report);
    }

    /// The hardware report, when detection has run.
    pub fn capabilities(&self) -> Option<&CapabilityReport> {
        self.capability.as_ref()
    }

    /// Record a report this surface produced.
    pub fn record_report(&mut self, report: ReportViewModel) {
        self.reports.retain(|existing| existing.kind != report.kind);
        self.reports.push(report);
    }

    /// The reports this surface has produced, newest last.
    ///
    /// Read through the view model by the page; this is for the tests that check
    /// one report per kind was kept rather than one per export.
    #[cfg_attr(not(test), allow(dead_code, reason = "the view model carries them"))]
    pub fn reports(&self) -> &[ReportViewModel] {
        &self.reports
    }

    /// The per-encode logs, newest first, with the rotation policy applied.
    ///
    /// `files` is injected rather than read from a folder so that the listing
    /// and the "will this be removed" answer can be tested without a disk and
    /// without a clock.
    pub fn encode_logs(
        &self,
        files: &[LogFile],
        policy: &RotationPolicy,
    ) -> Vec<EncodeLogViewModel> {
        let plan = enclog::decide_rotation(policy, files);
        let mut rows: Vec<EncodeLogViewModel> = files
            .iter()
            .map(|file| EncodeLogViewModel {
                name: file.name.clone(),
                bytes: file.bytes,
                age_seconds: file.age_seconds,
                in_progress: file.role == enclog::LogRole::InProgress,
                kept: plan.keep.contains(&file.name),
                size_label: size_label(file.bytes),
            })
            .collect();
        rows.sort_by(|left, right| {
            left.age_seconds
                .cmp(&right.age_seconds)
                .then_with(|| left.name.cmp(&right.name))
        });
        rows
    }

    /// The whole surface, as the page renders it.
    pub fn view(
        &self,
        files: &[LogFile],
        policy: &RotationPolicy,
        queue_running: bool,
    ) -> DiagnosticsViewModel {
        let logs = self.encode_logs(files, policy);
        let progress = self.sweep_progress();
        let results = self.sweep_results();
        let refusal = self.sweep_refusal(queue_running);
        let dropped = logs.iter().filter(|log| !log.kept).count();
        let logs_summary = if logs.is_empty() {
            "No encode has written a log yet.".to_string()
        } else {
            format!(
                "{} encode log(s), {}{}",
                logs.len(),
                size_label(logs.iter().map(|log| log.bytes).sum()),
                if dropped > 0 {
                    format!(", and {dropped} would be removed by the rotation policy")
                } else {
                    String::new()
                }
            )
        };
        let log_count = logs.len();
        DiagnosticsViewModel {
            logs_summary,
            log_folder: self.log_folder.to_string_lossy().to_string(),
            encode_logs: logs,
            capability: self.capability.as_ref().map(Into::into),
            sweep: SweepViewModel {
                range: self.request.range,
                planned: self.plan.len(),
                plan_summary: self.plan.summary(),
                refusal: self.plan.refusal.clone(),
                narrowed: self.plan.narrowed,
                folder: self.sweep_folder(),
                progress: progress.clone(),
                rows: results
                    .rows
                    .iter()
                    .map(|row| SweepRowViewModel {
                        quality: row.quality,
                        succeeded: row.succeeded,
                        size_label: size_label(row.bytes),
                        relative_label: if row.succeeded && row.relative_size > 0.0 {
                            format!("{:.2}x", row.relative_size)
                        } else {
                            String::new()
                        },
                        seconds_label: format!("{:.1}s", row.seconds),
                        message: row.message.clone(),
                        cancelled: row.cancelled,
                    })
                    .collect(),
                results_summary: results.summary(),
                can_start: refusal.is_none(),
                start_reason: refusal.unwrap_or_default(),
            },
            reports: self.reports.clone(),
            summary: format!("{log_count} log(s); sweep: {}", progress.summary),
        }
    }
}

/// A byte count as a person reads it.
pub fn size_label(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::sweep::{SweepOutcome, SweepState};
    use vise_ipc::enclog::LogRole;

    fn state() -> DiagnosticsState {
        let mut state = DiagnosticsState::new(PathBuf::from("/cfg/vise/logs"));
        state.set_base_job(vise_core::JobSpec {
            input_path: "C:\\media\\raw.mkv".into(),
            output_path: "D:\\out\\Episode 1.mkv".into(),
            ..Default::default()
        });
        state.set_output_folder("D:\\out");
        state.set_range(SweepRange::new(20, 26, 2));
        state
    }

    #[test]
    fn a_range_is_planned_the_moment_it_is_set() {
        let mut state = state();
        let plan = state.set_range(SweepRange::new(18, 30, 2)).clone();
        assert!(plan.is_runnable(), "{}", plan.refusal);
        assert_eq!(plan.len(), 7);
        assert_eq!(plan.runs[0].quality, 18);
        assert!(plan
            .runs
            .iter()
            .all(|run| run.job.output_path.contains(vise_core::sweep::SWEEP_FOLDER)));
        assert_eq!(state.plan().len(), 7, "and the plan is kept");
    }

    #[test]
    fn a_range_nobody_confirmed_is_refused_and_the_surface_says_why() {
        let mut state = state();
        state.set_range(SweepRange::new(0, 51, 1));
        assert!(!state.plan().is_runnable());
        let reason = state.sweep_refusal(false).expect("a refusal");
        assert!(reason.contains("confirm"), "{reason}");
        assert!(state.start_sweep().is_err(), "and it cannot be started");

        state.set_confirmed(true);
        assert!(state.plan().is_runnable());
        assert!(state.sweep_refusal(false).is_none());
    }

    #[test]
    fn a_sweep_is_refused_while_the_real_queue_is_encoding() {
        let mut state = state();
        let reason = state.sweep_refusal(true).expect("a refusal");
        assert!(reason.contains("competing"), "{reason}");
        assert!(state.start_sweep().is_ok(), "but it can still be armed");
    }

    #[test]
    fn a_sweep_with_no_source_says_so_rather_than_running_nothing() {
        let mut state = DiagnosticsState::new(PathBuf::from("/cfg/vise/logs"));
        state.set_range(SweepRange::new(20, 22, 2));
        let reason = state.sweep_refusal(false).expect("a refusal");
        assert!(reason.contains("open a source"), "{reason}");
    }

    #[test]
    fn a_sweep_hands_out_one_run_at_a_time_and_keeps_its_own_table() {
        let mut state = state();
        state.start_sweep().expect("started");
        let first = state.next_run().expect("first run");
        assert_eq!(first.quality, 20);
        assert!(
            state.next_run().is_none(),
            "and not a second one at the same time"
        );
        assert_eq!(state.sweep_progress().current, Some(20));

        state.finish_run(SweepOutcome::succeeded(20, 1024, 4.0));
        let second = state.next_run().expect("second run");
        assert_eq!(second.quality, 22);
        let results = state.sweep_results();
        assert_eq!(results.rows.len(), 1);
        assert!(state.is_busy());
    }

    #[test]
    fn a_sweep_never_writes_over_the_users_chosen_output() {
        let mut state = state();
        state.start_sweep().expect("started");
        for expected in [20, 22, 24, 26] {
            let run = state.next_run().expect("a run");
            assert_eq!(run.quality, expected);
            assert_ne!(
                run.job.output_path, "D:\\out\\Episode 1.mkv",
                "the destination the user chose is not one of the runs"
            );
            assert!(
                run.job.output_path.contains("D:\\out"),
                "{}",
                run.job.output_path
            );
            state.finish_run(SweepOutcome::succeeded(expected, 100, 1.0));
        }
        assert!(state.next_run().is_none());
        assert_eq!(state.sweep_progress().state, SweepState::Finished);
        assert_eq!(state.sweep_results().rows.len(), 4);
    }

    #[test]
    fn cancelling_stops_the_next_run_and_the_table_keeps_what_finished() {
        let mut state = state();
        state.start_sweep().expect("started");
        let run = state.next_run().expect("a run");
        state.finish_run(SweepOutcome::succeeded(run.quality, 100, 1.0));
        assert!(state.cancel_sweep());
        assert!(state.next_run().is_none());
        let view = state.view(&[], &RotationPolicy::default(), false);
        assert!(
            view.sweep.can_start,
            "a cancelled sweep is finished, not stuck: starting again is how it is re-run"
        );
        assert!(view.sweep.progress.cancelled);
        assert_eq!(view.sweep.rows.len(), 1);
        assert!(view.sweep.results_summary.contains("cancelled"));
    }

    #[test]
    fn cancelling_a_sweep_that_never_started_says_so() {
        let mut state = state();
        assert!(!state.cancel_sweep());
    }

    #[test]
    fn a_second_sweep_cannot_start_while_the_first_is_running() {
        let mut state = state();
        state.start_sweep().expect("started");
        state.next_run().expect("a run");
        let reason = state.sweep_refusal(false).expect("a refusal");
        assert!(reason.contains("already running"), "{reason}");
    }

    #[test]
    fn the_logs_are_listed_newest_first_and_say_whether_they_would_be_removed() {
        let state = state();
        let files = vec![
            LogFile::retained("encode-old.log", 900_000, 9000),
            LogFile::retained("encode-new.log", 100, 5).with_role(LogRole::InProgress),
            LogFile::retained("encode-mid.log", 100, 500),
        ];
        let policy = RotationPolicy {
            max_bytes: 1000,
            max_files: 10,
        };
        let view = state.view(&files, &policy, false);
        assert_eq!(
            view.encode_logs
                .iter()
                .map(|log| log.name.as_str())
                .collect::<Vec<_>>(),
            vec!["encode-new.log", "encode-mid.log", "encode-old.log"],
            "newest first"
        );
        assert!(view.encode_logs[0].in_progress);
        assert!(view.encode_logs[0].kept);
        assert!(
            !view.encode_logs[2].kept,
            "the oldest and largest is disposable"
        );
        assert!(
            view.logs_summary.contains("would be removed"),
            "{}",
            view.logs_summary
        );
        assert!(
            view.logs_summary.contains("879.1 KiB"),
            "{}",
            view.logs_summary
        );
    }

    #[test]
    fn a_folder_with_no_logs_says_so_rather_than_showing_an_empty_table() {
        let state = state();
        let view = state.view(&[], &RotationPolicy::default(), false);
        assert!(view.encode_logs.is_empty());
        assert_eq!(view.logs_summary, "No encode has written a log yet.");
        assert_eq!(view.log_folder, "/cfg/vise/logs");
    }

    #[test]
    fn the_surface_shows_no_capability_report_until_detection_has_run() {
        let mut state = state();
        assert!(state
            .view(&[], &RotationPolicy::default(), false)
            .capability
            .is_none());
        let report = CapabilityReport::from_detected(
            &vise_core::HardwareCapabilities {
                ffmpeg_path: Some(std::path::PathBuf::from("/opt/ffmpeg")),
                hwaccels: vec!["cuda".into()],
                decoders: vec!["h264".into()],
                encoders: vec!["hevc_nvenc".into()],
            },
            &vise_core::HdrCapabilities {
                dolby_vision_rpu: true,
                hdr10_plus_sei: false,
            },
        );
        state.observe_capabilities(report);
        let view = state.view(&[], &RotationPolicy::default(), false);
        let capability = view.capability.expect("a report");
        assert!(capability.detected);
        assert_eq!(capability.hardware_encoders, vec!["hevc_nvenc"]);
        assert!(capability.dolby_vision_rpu);
        assert!(!capability.hdr10_plus_sei);
        assert!(capability.summary.contains("1 encoder(s)"));
    }

    #[test]
    fn a_report_is_remembered_once_per_kind() {
        let mut state = state();
        for path in ["a.csv", "b.csv"] {
            state.record_report(ReportViewModel {
                kind: "results".into(),
                path: path.into(),
                summary: "one".into(),
                written: true,
                problem: String::new(),
            });
        }
        state.record_report(ReportViewModel {
            kind: "bundle".into(),
            path: "c.zip".into(),
            summary: "two".into(),
            written: true,
            problem: String::new(),
        });
        let view = state.view(&[], &RotationPolicy::default(), false);
        assert_eq!(view.reports.len(), 2, "one per kind, not one per export");
        assert_eq!(
            view.reports
                .iter()
                .find(|r| r.kind == "results")
                .map(|r| r.path.as_str()),
            Some("b.csv")
        );
    }

    #[test]
    fn a_size_reads_as_a_person_expects() {
        assert_eq!(size_label(0), "0 B");
        assert_eq!(size_label(999), "999 B");
        assert_eq!(size_label(1024), "1.0 KiB");
        assert_eq!(size_label(1536), "1.5 KiB");
        assert_eq!(size_label(1024 * 1024 * 3), "3.0 MiB");
        assert_eq!(size_label(1024u64.pow(4) * 2), "2.0 TiB");
    }

    #[test]
    fn the_sweep_view_carries_the_plan_and_the_proof_it_is_bounded() {
        let mut state = state();
        state.set_range(SweepRange::new(10, 50, 1));
        state.set_confirmed(true);
        let view = state.view(&[], &RotationPolicy::default(), false);
        assert!(view.sweep.narrowed);
        assert!(
            view.sweep.plan_summary.contains("widened"),
            "{}",
            view.sweep.plan_summary
        );
        assert!(view.sweep.planned <= vise_core::sweep::MAX_SWEEP_RUNS);
        assert_eq!(
            view.sweep.folder,
            format!("D:\\out\\{}", vise_core::sweep::SWEEP_FOLDER)
        );
        assert!(view.sweep.can_start);
        assert!(view.summary.contains("No quality sweep has been started."));
    }

    #[test]
    fn a_refused_plan_reaches_the_page_rather_than_only_the_test() {
        let mut state = state();
        state.set_range(SweepRange::new(30, 20, 2));
        let view = state.view(&[], &RotationPolicy::default(), false);
        assert!(view.sweep.refusal.contains("30"), "{}", view.sweep.refusal);
        assert_eq!(view.sweep.planned, 0);
        assert!(!view.sweep.can_start);
        assert!(view.sweep.start_reason.contains("30"));
    }
}
