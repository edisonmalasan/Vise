//! How the application lives on the desktop: the tray, notifications, power,
//! user actions around an encode, updates, portable state, the queue on disk,
//! disk space, and processor placement.
//!
//! Every capability here is a *decision* plus a *mechanism*, and the two are kept
//! apart. The decisions are pure functions over facts, and every one of them is
//! covered by a unit test that runs without the operating system: what the tray
//! menu should say, whether a notification is worth showing, when to hold the
//! machine awake, what an interrupted queue item becomes, whether there is room
//! on the disk, where a command line's variables go, and which release the
//! appcast offers. The mechanisms are one call to the platform behind a trait,
//! so a build on a machine that lacks the capability reports the capability as
//! missing rather than pretending, and the tests still run.
//!
//! Nothing here talks to the media tools and nothing here runs the pipeline.
//! Encoding stays in the worker, and the window's job is to arrange things
//! around it: hold the machine awake while it runs, watch the disk it is
//! writing to, notice when a job finishes, and remember the queue when the
//! window closes.

pub mod actions;
pub mod bundle;
pub mod diskspace;
pub mod notify;
pub mod persistence;
pub mod power;
pub mod results;
pub mod tray;
pub mod updates;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::system::notify::{Notification, Notifier};
use crate::system::tray::{TrayFacts, TrayModel};

/// What the window knows about itself and its queue when it refreshes the
/// integration.
///
/// Passed in rather than read from the desktop state so that every decision below
/// is a function of a plain value, and can therefore be tested with a queue that
/// never existed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SystemFacts {
    /// Jobs that are waiting for an encoder.
    pub queued: usize,
    /// Jobs the encoder is holding right now.
    pub running: usize,
    /// Jobs that have finished, one way or another.
    pub finished: usize,
    /// Whether the window is on screen and in front.
    pub window_visible: bool,
    /// Whether the pipeline is running inside this window rather than in a
    /// worker process.
    pub in_process: bool,
    /// Whether a portable settings file was found.
    pub portable: bool,
    /// Whether a newer release is on offer.
    pub update_available: bool,
    /// Whether the queue is paused for a reason the user can act on.
    pub queue_paused: Option<String>,
    /// Jobs that finished well.
    pub completed: usize,
    /// Jobs that failed.
    pub failed: usize,
    /// The file and the reason of the failure that just happened, when one did.
    pub last_failure: Option<(String, String)>,
}

/// One thing the window was asked to do by the integration.
///
/// Effects are data rather than calls so the whole refresh can be checked in a
/// test: a queue that finishes while the window is in front must produce a log
/// line and no notification, and a queue that finishes while the window is not
/// visible must produce both. The window carries the effects out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "camelCase")]
pub enum Effect {
    /// Say something in the activity log.
    Log(String),
    /// Show a notification.
    Notify(Notification),
    /// Hold the machine awake at this scope.
    HoldPower(String),
    /// Let the machine sleep again.
    ReleasePower,
    /// Republish the tray icon's menu and tooltip.
    Tray(TrayModel),
    /// Check for a newer release, if a schedule says it is time.
    CheckForUpdates,
}

/// Everything the window holds so it can integrate with the desktop.
///
/// The tray, the notifications, the update check, and the hold on the machine are
/// all *state*, not plumbing: each of them is a decision that has already been
/// made, so the window can publish them and the tray can describe them without
/// asking anything twice.
#[derive(Debug)]
pub struct Integration {
    /// Where the queue snapshot is written.
    queue_path: PathBuf,
    /// What shows a notification when one is decided on.
    notifier: std::sync::Arc<notify::FallbackNotifier>,
    /// The log behind the notifier, kept so a line survives a refusal.
    notification_log: Option<std::sync::Arc<notify::LogNotifier>>,
    /// Whether this window is holding the machine awake.
    power_held: bool,
    /// The scope it is holding at.
    power_scope: String,
    /// The tray model as it was last published.
    tray: Option<TrayModel>,
    /// The last update check this build performed, for the preferences tab.
    updates: updates::UpdateState,
    /// Lines already said, so a fact that has not changed is not said again.
    ///
    /// The tray tooltip and the log both repeat themselves by default, and a
    /// window that re-announces "the queue is empty" every time it polls would
    /// make the activity log unreadable within a minute.
    said: Vec<String>,
    /// The facts as they were the last time this was refreshed.
    ///
    /// Held so that the two events which are about a *change* — a queue that has
    /// just finished, a job that has just failed — are noticed exactly once. A
    /// window that notified on every poll would show the same failure a hundred
    /// times while it was open.
    previous: Option<SystemFacts>,
}

impl Default for Integration {
    /// A window that has not been given anywhere to write its queue yet.
    ///
    /// Every value is the unremarkable one, and the queue path is empty rather
    /// than a guess: a window that was built without being told where its state
    /// goes writes nothing until somebody says where.
    fn default() -> Integration {
        Integration {
            queue_path: PathBuf::new(),
            notifier: std::sync::Arc::new(notify::FallbackNotifier::default()),
            notification_log: None,
            power_held: false,
            power_scope: String::new(),
            tray: None,
            updates: updates::UpdateState::new(),
            said: Vec::new(),
            previous: None,
        }
    }
}

impl Integration {
    /// A window whose queue snapshot lives at `queue_path`.
    pub fn new(queue_path: PathBuf) -> Integration {
        Integration {
            queue_path,
            notifier: std::sync::Arc::new(notify::FallbackNotifier::default()),
            notification_log: None,
            power_held: false,
            power_scope: String::new(),
            tray: None,
            updates: updates::UpdateState::new(),
            said: Vec::new(),
            previous: None,
        }
    }

    /// Whether this window is holding the machine awake.
    ///
    /// Read by the tests and by the tray, and the answer the two have to agree
    /// on: a tooltip that says the machine is held awake while it is not would be
    /// the one line nobody can act on.
    #[allow(
        dead_code,
        reason = "read by the tests; the tray is told through a model"
    )]
    pub fn power_held(&self) -> bool {
        self.power_held
    }

    /// Where the queue snapshot is written.
    pub fn queue_path(&self) -> &std::path::Path {
        &self.queue_path
    }

    /// Put the platform's notification area behind the fallback, so that a window
    /// which found one shows notifications there.
    ///
    /// Set once at start-up, from the window handle the platform gave us: a
    /// notifier that needs a window cannot be built before there is one.
    pub fn use_notifier(&mut self, primary: std::sync::Arc<dyn notify::Notifier>) {
        let fallback: std::sync::Arc<notify::LogNotifier> =
            std::sync::Arc::new(notify::LogNotifier::new());
        let log: std::sync::Arc<dyn notify::Notifier> = fallback.clone();
        self.notifier = std::sync::Arc::new(notify::FallbackNotifier::new(primary, log));
        self.notification_log = Some(fallback);
    }

    /// Show a notification, and write it to the log either way.
    ///
    /// The log is kept here as well as behind the notifier so that a line the
    /// user can read later does not depend on the platform having accepted the
    /// notification: the activity log is the record, and the desktop is a copy.
    pub fn show(&mut self, notification: Notification) {
        if let Some(log) = &self.notification_log {
            let _ = log.notify(&notification);
        }
        let _ = self.notifier.show(&notification);
    }

    /// One line saying how notifications are being delivered.
    pub fn notifier_description(&self) -> String {
        self.notifier.describe()
    }

    /// Whether a notification can be shown outside the window at all.
    pub fn notifier_available(&self) -> bool {
        self.notifier.available()
    }

    /// How the update check stands.
    pub fn updates(&self) -> &updates::UpdateState {
        &self.updates
    }

    /// Record the outcome of an update check so the preferences tab can show it.
    pub fn observe_update(&mut self, outcome: updates::CheckOutcome, checked_on: (u32, u32, u32)) {
        self.updates.observe(outcome, checked_on);
    }
    /// Bring the tray, the power hold, and the log line up to date.
    ///
    /// Returns the effects for the window to carry out. Nothing here touches the
    /// operating system: the hold and the tray publication are effects, and the
    /// log line is an effect, so what the integration decided can be checked
    /// before anything is done about it.
    pub fn refresh(
        &mut self,
        facts: &SystemFacts,
        policy: power::PowerPolicy,
        notify: notify::NotifyPolicy,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        let decision = power::decide(policy, facts, self.power_held);
        match &decision.action {
            power::Action::Hold { scope } => {
                self.power_held = true;
                self.power_scope = scope.clone();
                effects.push(Effect::HoldPower(scope.clone()));
            }
            power::Action::Release => {
                if self.power_held {
                    self.power_held = false;
                    self.power_scope.clear();
                    effects.push(Effect::ReleasePower);
                }
            }
            power::Action::StayPut => {}
        }
        if let Some(line) = self.line_once(&format!("power:{}", decision.reason)) {
            effects.push(Effect::Log(line));
        }

        let tray = tray::build(&TrayFacts::of(facts, self.power_held));
        let changed = self.tray.as_ref() != Some(&tray);
        if changed {
            self.tray = Some(tray.clone());
            effects.push(Effect::Tray(tray));
        }

        // A queue that has stopped for want of something says so, once, and says it
        // in the notification area as well as in the log: a queue that paused and
        // looked stuck is the failure a user cannot diagnose from the window.
        if let Some(queued_reason) = facts.queue_paused.as_deref() {
            // One gate for both: a pause that has not changed says nothing new, and
            // a window that re-announced it on every poll would fill the
            // notification area with the same sentence all night.
            let fresh = self.line_once(&format!("paused:{queued_reason}")).is_some();
            if fresh {
                if let Some(line) = self.take_line(&format!("paused:{queued_reason}")) {
                    effects.push(Effect::Log(line));
                }
                if let Some(notification) = notify::decide(
                    &notify::Event::QueuePaused {
                        reason: queued_reason.to_string(),
                    },
                    notify,
                ) {
                    effects.push(Effect::Notify(notification));
                }
            }
        }
        // The two events that are about a change rather than a state are noticed by
        // comparing with the last refresh, so a queue that finishes once is
        // announced once however often the window is polled afterwards.
        if let Some(previous) = self.previous.clone() {
            let drained = previous.running + previous.queued > 0
                && facts.running == 0
                && facts.queued == 0
                && facts.finished > previous.finished;
            if drained {
                if let Some(line) = self.line_once("queue-drained") {
                    effects.push(Effect::Log(format!(
                        "{line}: {} finished and {} failed",
                        facts.completed, facts.failed
                    )));
                }
                let event = notify::Event::QueueFinished {
                    completed: facts.completed,
                    failed: facts.failed,
                };
                // A queue that finished where the user could watch it finish is
                // something they have already seen, so it is not a notification.
                if let Some(notification) =
                    notify::decide_with_focus(&event, notify, facts.window_visible)
                {
                    effects.push(Effect::Notify(notification));
                }
            }
            if facts.failed > previous.failed {
                if let Some((output, reason)) = facts.last_failure.clone() {
                    if let Some(notification) =
                        notify::decide(&notify::Event::EncodeFailed { output, reason }, notify)
                    {
                        effects.push(Effect::Notify(notification));
                    }
                }
            }
        }
        self.previous = Some(facts.clone());
        effects
    }
    /// The line for `key`, if it has not been said since the fact behind it changed.
    fn take_line(&mut self, key: &str) -> Option<String> {
        key.split_once(':').map(|(_, reason)| reason.to_string())
    }

    /// Say a line once, for as long as the fact behind it holds.
    ///
    /// The key is the fact, so a line that is still true is not repeated and a
    /// line that stops being true is said again when it does.
    fn line_once(&mut self, key: &str) -> Option<String> {
        if self.said.iter().any(|said| said == key) {
            return None;
        }
        self.said.push(key.to_string());
        // Only the most recent facts matter; a queue that has run through fifty
        // states should not leave fifty keys behind to be compared every time.
        if self.said.len() > 32 {
            self.said.remove(0);
        }
        Some(
            key.split_once(':')
                .map(|(_, reason)| reason.to_string())
                .unwrap_or_else(|| key.to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::notify::NotifyPolicy;
    use crate::system::power::PowerPolicy;
    use vise_ipc::power::PowerScope;

    fn policy() -> PowerPolicy {
        PowerPolicy {
            prevent_sleep: true,
            scope: PowerScope::System,
        }
    }

    fn quiet() -> NotifyPolicy {
        NotifyPolicy {
            enabled: true,
            scope: crate::system::notify::NotifyScope::All,
        }
    }

    #[test]
    fn a_refresh_with_nothing_happening_says_nothing_and_publishes_once() {
        let mut integration = Integration::new(PathBuf::from("queue.json"));
        let facts = SystemFacts::default();
        let first = integration.refresh(&facts, policy(), quiet());
        // The tray is published once, the reason the queue is idle is said once, and
        // nothing else happens: an idle queue is not a reason to hold a machine
        // awake.
        assert_eq!(
            first
                .iter()
                .filter(|effect| matches!(effect, Effect::Tray(_)))
                .count(),
            1
        );
        assert!(
            first
                .iter()
                .all(|effect| !matches!(effect, Effect::HoldPower(_))),
            "{:?}",
            first
        );
        assert!(
            first
                .iter()
                .all(|effect| !matches!(effect, Effect::ReleasePower)),
            "nothing was held, so nothing is let go of"
        );
        let second = integration.refresh(&facts, policy(), quiet());
        assert!(
            second.is_empty(),
            "nothing changed, so nothing is republished: {second:?}"
        );
    }

    #[test]
    fn an_encode_in_a_worker_process_leaves_the_hold_to_the_worker() {
        let mut integration = Integration::new(PathBuf::from("queue.json"));
        let facts = SystemFacts {
            running: 1,
            queued: 3,
            ..SystemFacts::default()
        };
        let effects = integration.refresh(&facts, policy(), quiet());
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::HoldPower(_))),
            "the worker holds the machine, not the window: {effects:?}"
        );
        assert!(!integration.power_held());
        let log: Vec<String> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Log(line) => Some(line.clone()),
                _ => None,
            })
            .collect();
        assert!(log.iter().any(|line| line.contains("worker")), "{log:?}");
    }

    #[test]
    fn an_encode_in_this_window_holds_and_then_lets_go() {
        let mut integration = Integration::new(PathBuf::from("queue.json"));
        let facts = SystemFacts {
            running: 1,
            in_process: true,
            ..SystemFacts::default()
        };
        integration.refresh(&facts, policy(), quiet());
        assert!(integration.power_held());

        let idle = SystemFacts {
            in_process: true,
            ..SystemFacts::default()
        };
        let effects = integration.refresh(&idle, policy(), quiet());
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::ReleasePower)),
            "{effects:?}"
        );
        assert!(!integration.power_held());
    }

    #[test]
    fn a_queue_paused_for_want_of_space_is_announced_once() {
        let mut integration = Integration::new(PathBuf::from("queue.json"));
        let facts = SystemFacts {
            queued: 2,
            queue_paused: Some("only 1.2 GB free and this job needs 4.0 GB".to_string()),
            ..SystemFacts::default()
        };
        let effects = integration.refresh(&facts, policy(), quiet());
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Notify(_))),
            "{effects:?}"
        );
        let again = integration.refresh(&facts, policy(), quiet());
        assert!(
            !again
                .iter()
                .any(|effect| matches!(effect, Effect::Notify(_))),
            "the same pause is not announced on every poll: {again:?}"
        );
    }
}
