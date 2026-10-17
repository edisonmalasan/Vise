//! The error view, the copyable report, the countdown alert, and the about view.
//!
//! Failures in this application are worth keeping: a job that failed at two in
//! the morning should still be explainable at breakfast. The error view
//! therefore keeps the reason, where it came from, and what the window was
//! doing, and it renders a plain-text report that can be pasted into a bug
//! report without any of the interface chrome.
//!
//! The countdown alert exists because the queue can destroy work: cancelling
//! an encode or clearing finished jobs cannot be undone, so those actions ask
//! first and wait. The wait is counted down here, in Rust, so the count the
//! user reads is the same count that arms the confirm control.

use serde::{Deserialize, Serialize};

use vise_core::VISE_VERSION;

/// How serious a recorded problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Something that stopped one operation.
    #[default]
    Error,
    /// Something that will stop an operation later.
    Warning,
}

/// One recorded problem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    /// Monotonic number so the page can key rows without tracking order.
    pub id: u64,
    /// What the window was doing.
    pub context: String,
    /// The reason, as the window or worker reported it.
    pub detail: String,
    /// Where the problem came from.
    pub source: String,
    /// How serious it is.
    pub severity: Severity,
}

impl ErrorEntry {
    /// A recorded problem.
    pub fn new(id: u64, source: &str, context: &str, detail: impl Into<String>) -> Self {
        Self {
            id,
            context: context.trim().to_string(),
            detail: detail.into().trim().to_string(),
            source: source.trim().to_string(),
            severity: Severity::Error,
        }
    }

    /// The entry, downgraded to a warning.
    pub fn warn(mut self) -> Self {
        self.severity = Severity::Warning;
        self
    }

    /// One line for the list.
    pub fn headline(&self) -> String {
        if self.context.is_empty() {
            self.detail.clone()
        } else {
            format!("{}: {}", self.context, self.detail)
        }
    }
}

/// One recorded problem, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntryViewModel {
    /// Monotonic number.
    pub id: u64,
    /// What the window was doing.
    pub context: String,
    /// The reason, as reported.
    pub detail: String,
    /// Where the problem came from.
    pub source: String,
    /// Label for the severity.
    pub severity_label: String,
    /// Tone the page paints the row with.
    pub tone: String,
    /// The single line summary of this row.
    pub headline: String,
    /// The plain-text form of this one problem, for the clipboard.
    pub copy_text: String,
}

/// Every recorded problem, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorViewModel {
    /// One row per problem, newest first.
    pub entries: Vec<ErrorEntryViewModel>,
    /// How many problems are recorded.
    pub total: usize,
    /// How many of them are errors rather than warnings.
    pub error_count: usize,
    /// Whether anything at all was recorded.
    pub has_entries: bool,
    /// Whether the page should offer the clear control.
    pub can_clear: bool,
    /// The whole report as plain text.
    pub copy_text: String,
}

/// The problems the window has recorded.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ErrorState {
    entries: Vec<ErrorEntry>,
    next_id: u64,
}

impl ErrorState {
    /// Record a problem and hand back its number.
    pub fn record(&mut self, source: &str, context: &str, detail: impl Into<String>) -> u64 {
        self.next_id += 1;
        let entry = ErrorEntry::new(self.next_id, source, context, detail);
        let id = entry.id;
        self.entries.push(entry);
        id
    }

    /// Record a problem that will stop something later rather than now.
    pub fn push_warning(&mut self, source: &str, context: &str, detail: impl Into<String>) -> u64 {
        let entry = ErrorEntry::new(self.next_id + 1, source, context, detail).warn();
        self.next_id += 1;
        let id = entry.id;
        self.entries.push(entry);
        id
    }

    /// The problems, newest first.
    pub fn entries(&self) -> &[ErrorEntry] {
        &self.entries
    }

    /// Forget every recorded problem.
    pub fn clear(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        count
    }

    /// Everything the error view shows.
    pub fn view(&self) -> ErrorViewModel {
        let entries: Vec<ErrorEntryViewModel> = self
            .entries
            .iter()
            .rev()
            .map(|entry| ErrorEntryViewModel {
                id: entry.id,
                context: entry.context.clone(),
                detail: entry.detail.clone(),
                source: entry.source.clone(),
                severity_label: severity_label(entry.severity).to_string(),
                tone: match entry.severity {
                    Severity::Error => "error",
                    Severity::Warning => "muted",
                }
                .to_string(),
                headline: entry.headline(),
                copy_text: entry_report(entry),
            })
            .collect();
        let total = entries.len();
        let error_count = self
            .entries
            .iter()
            .filter(|entry| entry.severity == Severity::Error)
            .count();
        ErrorViewModel {
            copy_text: report(
                &format!("Vise Transcoder {}", VISE_VERSION),
                self.entries.iter().rev(),
            ),
            entries,
            total,
            error_count,
            has_entries: total > 0,
            can_clear: total > 0,
        }
    }
}

/// The severity label.
pub fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "Error",
        Severity::Warning => "Warning",
    }
}

/// The plain-text report for one problem.
fn entry_report(entry: &ErrorEntry) -> String {
    format!(
        "{}\nwhere: {}\nreported by: {}\nseverity: {}",
        entry.headline(),
        if entry.context.is_empty() {
            "not recorded"
        } else {
            entry.context.as_str()
        },
        if entry.source.is_empty() {
            "the window"
        } else {
            entry.source.as_str()
        },
        severity_label(entry.severity)
    )
}

/// A plain-text report for a list of problems.
///
/// The problems arrive oldest first, as they were recorded, and the report
/// prints them newest first so the failure that just happened is the first
/// thing the reader sees.
fn report<'a, I>(heading: &str, entries: I) -> String
where
    I: Iterator<Item = &'a ErrorEntry>,
{
    let mut text = heading.to_string();
    let mut count = 0;
    for entry in entries {
        text.push_str("\n\n");
        text.push_str(&entry_report(entry));
        count += 1;
    }
    if count == 0 {
        text.push_str("\n\nNo problems were recorded.");
    }
    text
}

/// Report a list of problems, newest first, for the clipboard.
pub fn report_text(entries: &[ErrorEntry]) -> String {
    report(
        &format!("Vise Transcoder {}", VISE_VERSION),
        entries.iter().rev(),
    )
}

// ---------------------------------------------------------------------------
// Countdown alert
// ---------------------------------------------------------------------------

/// An action that cannot be undone.
///
/// The names are what the page sends back, so they are spelled in full rather
/// than squeezed together: `start_queue` and not `startqueue`. A confirmation
/// is the one place a mistyped name would quietly do nothing, so the spelling
/// is pinned by a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertAction {
    /// Cancel a running or queued encode.
    CancelJob,
    /// Remove a job from the queue.
    RemoveJob,
    /// Cancel everything the queue is doing.
    CancelQueue,
    /// Forget the finished jobs.
    ClearFinished,
    /// Empty the queue.
    ClearQueue,
    /// Start every queued job at once.
    StartQueue,
    /// Delete a stored preset.
    DeletePreset,
}

impl AlertAction {
    /// The verb on the confirm control.
    pub fn verb(self) -> &'static str {
        match self {
            AlertAction::CancelJob => "Cancel the encode",
            AlertAction::RemoveJob => "Remove from the queue",
            AlertAction::CancelQueue => "Cancel the queue",
            AlertAction::ClearFinished => "Clear finished jobs",
            AlertAction::ClearQueue => "Empty the queue",
            AlertAction::StartQueue => "Start the queue",
            AlertAction::DeletePreset => "Delete the preset",
        }
    }

    /// Whether the action is destructive, which decides whether the countdown
    /// is offered at all.
    pub fn is_destructive(self) -> bool {
        !matches!(self, AlertAction::StartQueue)
    }
}

/// A pending destructive action waiting for its countdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountdownAlert {
    /// The action waiting to be confirmed.
    pub action: AlertAction,
    /// What the action will do, in one line.
    pub headline: String,
    /// Why the wait is there.
    pub detail: String,
    /// The job the action applies to, when it applies to one.
    pub job_id: Option<u64>,
    /// The preset the action applies to, when it applies to one.
    pub preset_id: Option<String>,
    /// Seconds left before the confirm control arms.
    pub seconds_left: u32,
    /// Whether the confirm control may be used yet.
    pub armed: bool,
    /// Whether Escape or the cancel control backs out.
    pub can_cancel: bool,
}

/// How long a destructive action waits before its confirm control arms.
pub const COUNTDOWN_SECONDS: u32 = 3;

/// A countdown alert as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountdownAlertViewModel {
    /// The action waiting to be confirmed.
    pub action: AlertAction,
    /// The verb on the confirm control.
    pub verb: String,
    /// What the action will do, in one line.
    pub headline: String,
    /// Why the wait is there.
    pub detail: String,
    /// Seconds left before the confirm control arms.
    pub seconds_left: u32,
    /// Whether the confirm control may be used yet.
    pub armed: bool,
    /// Whether the confirm control should read as a warning.
    pub destructive: bool,
    /// Element identifier of the confirm control.
    pub confirm_id: String,
    /// Element identifier of the back-out control.
    pub cancel_id: String,
    /// One line a screen reader reads while the countdown runs.
    pub live_label: String,
}

/// The countdown the window runs for a pending action.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertState {
    pending: Option<CountdownAlert>,
}

impl AlertState {
    /// Ask for `action` before it happens.
    ///
    /// An action that is not destructive needs no wait, so it is armed
    /// immediately; everything else gets the full countdown.
    pub fn request(
        &mut self,
        action: AlertAction,
        headline: impl Into<String>,
        job_id: Option<u64>,
        preset_id: Option<String>,
    ) -> CountdownAlertViewModel {
        let seconds_left = if action.is_destructive() {
            COUNTDOWN_SECONDS
        } else {
            0
        };
        let alert = CountdownAlert {
            action,
            headline: headline.into(),
            detail: if action.is_destructive() {
                "This cannot be undone, so the control unlocks after a short wait.".to_string()
            } else {
                "The queue starts in the order it is listed.".to_string()
            },
            job_id,
            preset_id,
            seconds_left,
            armed: seconds_left == 0,
            can_cancel: true,
        };
        self.pending = Some(alert);
        self.view()
    }

    /// The alert waiting, if any.
    pub fn pending(&self) -> Option<&CountdownAlert> {
        self.pending.as_ref()
    }

    /// Whether the action has been confirmed and may now run.
    ///
    /// An action that has not finished its countdown is left pending, so an early
    /// confirmation is a no-op rather than a way past the wait.
    pub fn take_confirmed(&mut self) -> Option<CountdownAlert> {
        let ready = self
            .pending
            .as_ref()
            .map(|alert| alert.armed)
            .unwrap_or(false);
        if !ready {
            return None;
        }
        self.pending.take()
    }

    /// Drop the pending action without running it.
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// Count one second off the wait, arming the control when it reaches zero.
    pub fn tick(&mut self) -> Option<CountdownAlertViewModel> {
        let alert = self.pending.as_mut()?;
        if alert.seconds_left > 0 {
            alert.seconds_left -= 1;
        }
        alert.armed = alert.seconds_left == 0;
        Some(self.view())
    }

    /// The alert as the page renders it.
    pub fn view(&self) -> CountdownAlertViewModel {
        let alert = self.pending.as_ref();
        let action = alert.map(|alert| alert.action);
        let seconds_left = alert.map(|alert| alert.seconds_left).unwrap_or(0);
        let armed = alert.map(|alert| alert.armed).unwrap_or(false);
        CountdownAlertViewModel {
            action: action.unwrap_or(AlertAction::CancelQueue),
            verb: action.map(AlertAction::verb).unwrap_or("Do it").to_string(),
            headline: alert
                .map(|alert| alert.headline.clone())
                .unwrap_or_else(|| "Nothing is waiting.".to_string()),
            detail: alert.map(|alert| alert.detail.clone()).unwrap_or_default(),
            seconds_left,
            armed,
            destructive: action.map(AlertAction::is_destructive).unwrap_or(true),
            confirm_id: format!("{ALERT_SURFACE}confirm"),
            cancel_id: format!("{ALERT_SURFACE}cancel"),
            live_label: if armed {
                "The action is ready to confirm.".to_string()
            } else {
                format!("Waiting {seconds_left} second(s) before the action can be confirmed.")
            },
        }
    }
}

/// Surface identifier for the error view.
pub const ERROR_SURFACE: &str = "error";
/// Surface identifier for the countdown alert.
pub const ALERT_SURFACE: &str = "alert";
/// Surface identifier for the about view.
pub const ABOUT_SURFACE: &str = "about";

// ---------------------------------------------------------------------------
// About view
// ---------------------------------------------------------------------------

/// One attribution link in the about view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutLink {
    /// What the link is for.
    pub label: String,
    /// Where it goes.
    pub url: String,
}

/// The about view, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutViewModel {
    /// Application name.
    pub name: String,
    /// Release version, taken from the core crate rather than the page.
    pub version: String,
    /// One line for the window title and the about header.
    pub heading: String,
    /// Which build profile this binary was made with.
    pub profile: String,
    /// Which platform this binary is running on.
    pub platform: String,
    /// Which architecture this binary is running on.
    pub architecture: String,
    /// Where the encoding actually happens.
    pub engine: String,
    /// What the license is, in this build's own words.
    pub license: String,
    /// Attributions and licence references.
    pub links: Vec<AboutLink>,
    /// The plain-text form, for copying into a bug report.
    pub copy_text: String,
}

/// Build the about view.
///
/// The version comes from `vise_core::VISE_VERSION` so the window, the command
/// line, and any packaging script all report the same number, and the licence
/// line is this project's own: it does not claim a licence the repository does
/// not carry.
pub fn about(engine: &str) -> AboutViewModel {
    let version = VISE_VERSION.to_string();
    let name = "Vise Transcoder".to_string();
    let profile = if cfg!(debug_assertions) {
        "Development build (unoptimised, with debug assertions)"
    } else {
        "Release build (optimised)"
    }
    .to_string();
    let platform = std::env::consts::OS.to_string();
    let architecture = std::env::consts::ARCH.to_string();
    let engine = engine.trim().to_string();
    let license = "This build ships without a licence file. The terms that apply to the \
         source are the ones written in the project README and in each dependency's own \
         licence."
        .to_string();
    let links = vec![
        AboutLink {
            label: "The Rust programming language".to_string(),
            url: "https://www.rust-lang.org/".to_string(),
        },
        AboutLink {
            label: "The Tauri application framework".to_string(),
            url: "https://tauri.app/".to_string(),
        },
        AboutLink {
            label: "The FFmpeg project, which does the encoding".to_string(),
            url: "https://ffmpeg.org/".to_string(),
        },
        AboutLink {
            label: "Serde, the serialisation library used by the worker contract".to_string(),
            url: "https://serde.rs/".to_string(),
        },
    ];
    let mut copy_text =
        format!("{name} {version}\n{profile}\nplatform: {platform} ({architecture})");
    for line in [
        format!(
            "engine: {}",
            if engine.is_empty() {
                "not reported"
            } else {
                &engine
            }
        ),
        format!("licence: {license}"),
    ] {
        copy_text.push('\n');
        copy_text.push_str(&line);
    }
    for link in &links {
        copy_text.push_str(&format!("\n{}: {}", link.label, link.url));
    }

    AboutViewModel {
        heading: format!("{name} {version}"),
        name,
        version,
        profile,
        platform,
        architecture,
        engine,
        license,
        links,
        copy_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_problem_is_listed_newest_first_with_its_own_report() {
        let mut state = ErrorState::default();
        state.record("worker", "encoding job 3", "the encoder exited with code 1");
        state.record("window", "scanning", "could not read the file");
        let view = state.view();
        assert_eq!(view.total, 2);
        assert_eq!(view.error_count, 2);
        assert!(view.has_entries);
        assert!(view.can_clear);
        assert_eq!(view.entries[0].context, "scanning");
        assert_eq!(view.entries[1].detail, "the encoder exited with code 1");
        assert!(
            view.entries[1].copy_text.contains("encoding job 3"),
            "{:?}",
            view.entries[1]
        );
        assert!(
            view.entries[1].copy_text.contains("worker"),
            "{:?}",
            view.entries[1]
        );
        assert!(view.copy_text.starts_with("Vise Transcoder 0.1.0"));
    }

    #[test]
    fn an_empty_error_view_explains_itself_instead_of_showing_nothing() {
        let view = ErrorState::default().view();
        assert!(!view.has_entries);
        assert!(!view.can_clear);
        assert!(view.copy_text.contains("No problems"), "{}", view.copy_text);
    }

    #[test]
    fn a_warning_is_labelled_and_counted_apart_from_an_error() {
        let mut state = ErrorState::default();
        state.push_warning("window", "queueing", "the output folder does not exist yet");
        let view = state.view();
        assert_eq!(view.total, 1);
        assert_eq!(view.error_count, 0);
        assert_eq!(view.entries[0].severity_label, "Warning");
        assert_eq!(view.entries[0].tone, "muted");
    }

    #[test]
    fn clearing_forgets_every_problem_and_reports_how_many_went() {
        let mut state = ErrorState::default();
        state.record("window", "one", "first");
        state.record("window", "two", "second");
        assert_eq!(state.clear(), 2);
        assert!(state.entries().is_empty());
        // Numbers keep moving on so a cleared problem cannot be confused with a
        // later one that happens to say the same thing.
        assert_eq!(state.record("window", "three", "third"), 3);
    }

    #[test]
    fn a_problem_without_a_context_still_reads_as_a_line() {
        let entry = ErrorEntry::new(1, "", "", "something failed");
        assert_eq!(entry.headline(), "something failed");
        assert!(entry_report(&entry).contains("not recorded"));
    }

    #[test]
    fn a_destructive_action_waits_before_it_can_be_confirmed() {
        let mut alerts = AlertState::default();
        let view = alerts.request(
            AlertAction::ClearQueue,
            "Empty the queue of 4 jobs.",
            None,
            None,
        );
        assert_eq!(view.seconds_left, COUNTDOWN_SECONDS);
        assert!(!view.armed);
        assert!(view.destructive);
        assert!(view.live_label.contains("3 second"), "{}", view.live_label);

        for _ in 0..COUNTDOWN_SECONDS {
            alerts.tick();
        }
        let armed = alerts.view();
        assert_eq!(armed.seconds_left, 0);
        assert!(armed.armed);
        assert!(armed.live_label.contains("ready"));
        assert_eq!(armed.verb, "Empty the queue");
        assert_eq!(armed.confirm_id, "alertconfirm");
        assert_eq!(armed.cancel_id, "alertcancel");
    }

    #[test]
    fn an_action_that_is_not_destructive_is_armed_at_once() {
        let mut alerts = AlertState::default();
        let view = alerts.request(AlertAction::StartQueue, "Start 4 jobs.", None, None);
        assert_eq!(view.seconds_left, 0);
        assert!(view.armed);
        assert!(!view.destructive);
        assert_eq!(view.detail, "The queue starts in the order it is listed.");
    }

    #[test]
    fn an_action_is_taken_once_and_only_after_the_wait() {
        let mut alerts = AlertState::default();
        alerts.request(AlertAction::RemoveJob, "Remove job 7.", Some(7), None);
        assert!(
            alerts.take_confirmed().is_none(),
            "confirming early does nothing"
        );
        for _ in 0..COUNTDOWN_SECONDS {
            alerts.tick();
        }
        let confirmed = alerts.take_confirmed().expect("confirmed");
        assert_eq!(confirmed.action, AlertAction::RemoveJob);
        assert_eq!(confirmed.job_id, Some(7));
        assert!(
            alerts.take_confirmed().is_none(),
            "the alert does not fire twice"
        );
    }

    #[test]
    fn backing_out_drops_the_pending_action() {
        let mut alerts = AlertState::default();
        alerts.request(AlertAction::CancelJob, "Cancel job 2.", Some(2), None);
        alerts.cancel();
        assert!(alerts.pending().is_none());
        assert!(alerts.take_confirmed().is_none());
        // A view with nothing pending still renders, so the page can hold the
        // overlay markup open for a frame without a half-filled dialog.
        assert_eq!(alerts.view().headline, "Nothing is waiting.");
    }

    #[test]
    fn the_about_view_reports_the_core_version_and_this_build() {
        let view = about("worker process 4211 on 127.0.0.1:53219");
        assert_eq!(view.version, vise_core::VISE_VERSION);
        assert_eq!(view.heading, format!("Vise Transcoder {}", VISE_VERSION));
        assert!(view.profile.contains("build"), "{}", view.profile);
        assert!(!view.platform.is_empty());
        assert!(!view.architecture.is_empty());
        assert_eq!(view.engine, "worker process 4211 on 127.0.0.1:53219");
        assert!(view.license.contains("README"), "{}", view.license);
        assert!(view
            .links
            .iter()
            .all(|link| link.url.starts_with("https://")));
        assert_eq!(view.links.len(), 4);
        assert!(view.copy_text.contains(&view.version));
        assert!(view.copy_text.contains("ffmpeg.org") || view.copy_text.contains("FFmpeg"));
    }

    #[test]
    fn an_engine_that_is_not_reported_still_renders() {
        let view = about("   ");
        assert!(
            view.copy_text.contains("not reported"),
            "{}",
            view.copy_text
        );
    }

    #[test]
    fn every_action_name_is_spelled_the_way_the_page_sends_it() {
        // A confirmation is the one place where a name that does not round-trip
        // does nothing at all rather than failing loudly, so each one is pinned.
        for (action, name) in [
            (AlertAction::CancelJob, "cancel_job"),
            (AlertAction::RemoveJob, "remove_job"),
            (AlertAction::CancelQueue, "cancel_queue"),
            (AlertAction::ClearFinished, "clear_finished"),
            (AlertAction::ClearQueue, "clear_queue"),
            (AlertAction::StartQueue, "start_queue"),
            (AlertAction::DeletePreset, "delete_preset"),
        ] {
            let json = serde_json::to_string(&action).expect("serialise");
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<AlertAction>(&json).expect("parse"),
                action
            );
        }
    }
}
