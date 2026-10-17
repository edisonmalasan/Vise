//! The when-done tab: what happens once the queue has nothing left to do.
//!
//! Two preferences, and one thing the window reads them for: the moment the last
//! job in the queue reaches a finished state.
//!
//! * **When the queue finishes** — three answers that this build can honour and
//!   three that it cannot. Doing nothing, asking through the countdown alert, and
//!   closing the window are all reachable from here and are acted on by
//!   `main.rs` when the queue drains. Suspending, hibernating, and signing out
//!   are offered as closed with the reason naming what is missing, because they
//!   are the three that a desktop build is expected to have and this one does
//!   not: there is no session control in the window, so offering them would be
//!   three controls that record a wish.
//! * **Wait before finishing** — how long the window waits after the last job
//!   finishes before acting, which is what makes an answer you did not mean to
//!   give recoverable: the queue is not closed the instant the encoder stops.

use serde::{Deserialize, Serialize};

use crate::preferences::{
    number_control, want_one_of, ChoiceGroupViewModel, PreferenceTabViewModel,
};
use crate::tabs::common::ChoiceOption;

/// Field identifier for the action.
pub const ACTION: &str = "whendone.action";
/// Field identifier for the wait.
pub const DELAY_SECONDS: &str = "whendone.delay_seconds";

/// Longest wait the window will sit through before acting.
pub const MAX_DELAY_SECONDS: u32 = 3600;

/// What happens when the queue has nothing left to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WhenDoneAction {
    /// Leave the window open and idle.
    #[default]
    Nothing,
    /// Ask, through the same countdown every other destructive action uses.
    Ask,
    /// Close the application window.
    CloseWindow,
    /// Suspend the machine.
    Suspend,
    /// Hibernate the machine.
    Hibernate,
    /// Sign the session out.
    SignOut,
}

impl WhenDoneAction {
    /// Every action, in the order the picker lists them.
    pub const ALL: [WhenDoneAction; 6] = [
        WhenDoneAction::Nothing,
        WhenDoneAction::Ask,
        WhenDoneAction::CloseWindow,
        WhenDoneAction::Suspend,
        WhenDoneAction::Hibernate,
        WhenDoneAction::SignOut,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            WhenDoneAction::Nothing => "nothing",
            WhenDoneAction::Ask => "ask",
            WhenDoneAction::CloseWindow => "close_window",
            WhenDoneAction::Suspend => "suspend",
            WhenDoneAction::Hibernate => "hibernate",
            WhenDoneAction::SignOut => "sign_out",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            WhenDoneAction::Nothing => "Do nothing",
            WhenDoneAction::Ask => "Ask me what to do",
            WhenDoneAction::CloseWindow => "Close this window",
            WhenDoneAction::Suspend => "Suspend the machine",
            WhenDoneAction::Hibernate => "Hibernate the machine",
            WhenDoneAction::SignOut => "Sign out",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            WhenDoneAction::Nothing => "The window stays open with an empty queue.",
            WhenDoneAction::Ask => {
                "The countdown alert opens and nothing happens until the wait is confirmed or cancelled."
            }
            WhenDoneAction::CloseWindow => {
                "The window closes. Anything not saved by then is not saved."
            }
            WhenDoneAction::Suspend => "The machine goes to sleep with the queue still listed.",
            WhenDoneAction::Hibernate => "The machine writes its memory to disk and sleeps.",
            WhenDoneAction::SignOut => "The session ends, which closes every window on it.",
        }
    }

    /// Whether this build can do it.
    ///
    /// The three this build cannot are the three that need session control the
    /// window does not have; the answers are shown with their descriptions rather
    /// than dropped, so a user can see what the choice would have been.
    pub fn is_available(self) -> bool {
        !matches!(
            self,
            WhenDoneAction::Suspend | WhenDoneAction::Hibernate | WhenDoneAction::SignOut
        )
    }

    /// Why it cannot be chosen, when it cannot.
    pub fn unavailable_reason(self) -> String {
        match self {
            WhenDoneAction::Suspend | WhenDoneAction::Hibernate | WhenDoneAction::SignOut => {
                "This build does not control the machine's session, so it cannot do this."
                    .to_string()
            }
            _ => String::new(),
        }
    }

    /// Whether choosing this destroys anything the user can see.
    ///
    /// A close is reversible only by opening the window again, so it asks through
    /// the countdown; nothing and asking are the two that do not.
    pub fn is_destructive(self) -> bool {
        matches!(self, WhenDoneAction::CloseWindow)
    }

    /// Read an action the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            ACTION,
            value,
            &[
                (
                    WhenDoneAction::Nothing.id(),
                    WhenDoneAction::Nothing.label(),
                ),
                (WhenDoneAction::Ask.id(), WhenDoneAction::Ask.label()),
                (
                    WhenDoneAction::CloseWindow.id(),
                    WhenDoneAction::CloseWindow.label(),
                ),
                (
                    WhenDoneAction::Suspend.id(),
                    WhenDoneAction::Suspend.label(),
                ),
                (
                    WhenDoneAction::Hibernate.id(),
                    WhenDoneAction::Hibernate.label(),
                ),
                (
                    WhenDoneAction::SignOut.id(),
                    WhenDoneAction::SignOut.label(),
                ),
            ],
        )?;
        let action = WhenDoneAction::ALL
            .into_iter()
            .find(|candidate| candidate.id() == typed)
            .ok_or_else(|| {
                format!("'{typed}' is not something this build does when the queue finishes.")
            })?;
        if !action.is_available() {
            // Refused rather than shown closed and stored: this is the value
            // arriving through a route that skipped the control, and storing it
            // would leave a preference nothing acts on.
            return Err(format!(
                "{} cannot be chosen: {}",
                action.label(),
                action.unavailable_reason()
            ));
        }
        Ok(action)
    }
}

/// The when-done preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhenDoneSettings {
    /// What happens when the queue drains.
    pub action: WhenDoneAction,
    /// How long to wait before acting.
    pub delay_seconds: u32,
}

impl Default for WhenDoneSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl WhenDoneSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// Doing nothing, immediately: a window that closed itself because a queue
    /// drained would be the wrong answer to give to somebody who has not asked
    /// for it, and the wait only means something once an answer does something.
    pub fn new() -> Self {
        Self {
            action: WhenDoneAction::Nothing,
            delay_seconds: 0,
        }
    }
}

/// Read the wait.
pub fn read_delay(value: &serde_json::Value) -> Result<u32, String> {
    crate::preferences::whole_in_range(DELAY_SECONDS, value, 0, MAX_DELAY_SECONDS)
}

/// The problems these preferences have.
pub fn validate(settings: &WhenDoneSettings) -> Vec<String> {
    let mut problems = Vec::new();
    if !settings.action.is_available() {
        problems.push(format!(
            "{} cannot be chosen: {}",
            settings.action.label(),
            settings.action.unavailable_reason()
        ));
    }
    if settings.delay_seconds > MAX_DELAY_SECONDS {
        problems.push(format!(
            "Wait before finishing has to be {MAX_DELAY_SECONDS} seconds or fewer; {} was stored.",
            settings.delay_seconds
        ));
    }
    problems
}

/// Build the when-done tab.
pub fn tab_view(settings: &WhenDoneSettings) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "whendone",
        "When done",
        "What happens once the queue has nothing left to do.",
    );
    let options: Vec<ChoiceOption> = WhenDoneAction::ALL
        .iter()
        .map(|action| match action.unavailable_reason() {
            reason if !reason.is_empty() => {
                ChoiceOption::taken(action.id(), action.label(), reason)
            }
            _ => ChoiceOption::new(action.id(), action.label()),
        })
        .collect();
    let mut control = ChoiceGroupViewModel::build(
        ACTION,
        "When the queue finishes",
        "Acts once the last job reaches a finished state.",
        settings.action.id(),
        &options,
        true,
        "",
    )
    .with_hint(settings.action.hint());
    if !settings.action.is_available() {
        // A stored action this build cannot do is reported rather than hidden:
        // the user chose it somewhere else and is owed the reason it is inert.
        control = control.blocked(settings.action.unavailable_reason());
    }
    tab.choices.push(control);
    let wait_hint = match settings.action {
        WhenDoneAction::Nothing => "Nothing is acted on, so the wait has no effect.".to_string(),
        _ => format!(
            "The window waits this long after the last job finishes. {MAX_DELAY_SECONDS} seconds is the longest wait, and a wait is what makes an action you did not mean to give recoverable."
        ),
    };
    tab.numbers.push(number_control(
        "whendoneDelaySeconds",
        "Wait before finishing",
        &wait_hint,
        settings.delay_seconds,
        0,
        MAX_DELAY_SECONDS,
        "seconds",
    ));
    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = WhenDoneSettings::new();
        assert_eq!(settings.action, WhenDoneAction::Nothing);
        assert_eq!(settings.delay_seconds, 0);
        assert_eq!(settings, WhenDoneSettings::default());
    }

    #[test]
    fn the_three_answers_this_build_cannot_give_are_offered_but_closed() {
        for action in [
            WhenDoneAction::Suspend,
            WhenDoneAction::Hibernate,
            WhenDoneAction::SignOut,
        ] {
            assert!(!action.is_available(), "{action:?}");
            assert!(
                action.unavailable_reason().contains("session"),
                "{}",
                action.unavailable_reason()
            );
        }
        for action in [
            WhenDoneAction::Nothing,
            WhenDoneAction::Ask,
            WhenDoneAction::CloseWindow,
        ] {
            assert!(action.is_available(), "{action:?}");
            assert!(action.unavailable_reason().is_empty());
        }
    }

    #[test]
    fn an_answer_this_build_cannot_give_is_refused_rather_than_stored() {
        let error = WhenDoneAction::read(&value("\"hibernate\"")).expect_err("cannot");
        assert!(error.contains("Hibernate"), "{error}");
        assert!(error.contains("session"), "{error}");
        assert_eq!(
            WhenDoneAction::read(&value("\"close_window\"")).expect("close"),
            WhenDoneAction::CloseWindow
        );
        assert!(WhenDoneAction::read(&value("\"reboot\"")).is_err());
    }

    #[test]
    fn only_the_close_is_destructive_so_only_the_close_asks() {
        assert!(WhenDoneAction::CloseWindow.is_destructive());
        assert!(!WhenDoneAction::Nothing.is_destructive());
        assert!(!WhenDoneAction::Ask.is_destructive());
    }

    #[test]
    fn the_wait_is_bounded_and_the_bounds_are_quoted() {
        assert_eq!(read_delay(&value("0")).expect("none"), 0);
        assert_eq!(
            read_delay(&value(&MAX_DELAY_SECONDS.to_string())).expect("most"),
            MAX_DELAY_SECONDS
        );
        let error = read_delay(&value("3601")).expect_err("one too many");
        assert!(error.contains("between 0 and 3600"), "{error}");
        let error = read_delay(&value("1.5")).expect_err("not whole");
        assert!(error.contains("whole number"), "{error}");
    }

    #[test]
    fn the_tab_closes_the_three_it_cannot_do_and_keeps_their_descriptions() {
        let view = tab_view(&WhenDoneSettings::new());
        assert_eq!(view.choices.len(), 1);
        let control = &view.choices[0];
        assert!(control.available);
        assert_eq!(control.options.len(), WhenDoneAction::ALL.len());
        let closed: Vec<&str> = control
            .options
            .iter()
            .filter(|option| !option.available)
            .map(|option| option.id.as_str())
            .collect();
        assert_eq!(closed, vec!["suspend", "hibernate", "sign_out"]);
        for option in control.options.iter().filter(|o| !o.available) {
            assert!(
                !option.label.is_empty(),
                "a closed option still names itself"
            );
            assert!(option.reason.contains("session"), "{}", option.reason);
        }
        assert_eq!(view.numbers.len(), 1);
        assert!(
            view.numbers[0].hint.contains("no effect"),
            "{}",
            view.numbers[0].hint
        );
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_stored_action_this_build_cannot_do_is_reported_on_the_tab_and_in_the_problems() {
        // A record can arrive from a file this build did not write, so the rule
        // runs on the stored value rather than only on what was typed.
        let stored = WhenDoneSettings {
            action: WhenDoneAction::SignOut,
            delay_seconds: 0,
        };
        let view = tab_view(&stored);
        assert!(!view.ready);
        assert!(!view.choices[0].available);
        assert!(
            view.problems[0].contains("cannot be chosen"),
            "{:?}",
            view.problems
        );
    }

    #[test]
    fn a_wait_with_nothing_to_act_on_says_it_has_no_effect() {
        let view = tab_view(&WhenDoneSettings {
            action: WhenDoneAction::Nothing,
            delay_seconds: 30,
        });
        assert!(
            view.numbers[0].hint.contains("no effect"),
            "{}",
            view.numbers[0].hint
        );
        let acting = tab_view(&WhenDoneSettings {
            action: WhenDoneAction::CloseWindow,
            delay_seconds: 30,
        });
        assert!(
            acting.numbers[0].hint.contains("3600"),
            "{}",
            acting.numbers[0].hint
        );
    }
}
