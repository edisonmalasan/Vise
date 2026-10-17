//! The advanced tab: process isolation, logging, and scheduling.
//!
//! Three preferences, each read by a different place:
//!
//! * **Process isolation** — whether the window starts a worker process or runs
//!   the pipeline inside itself. `main.rs` builds the backend from this, and a
//!   change discards the running backend so the next operation uses the new
//!   answer. A portable settings file beside the executable is the operator's
//!   answer and outranks the user's, so when one has taken isolation away the
//!   control is shown closed with that reason rather than accepting a value the
//!   window will not honour. The preference itself is kept, so removing the
//!   portable file brings the user's choice back.
//! * **How much is logged** — the verbosity rank sent with every scan and encode
//!   command, which the worker's log service filters on, and the floor of the
//!   activity window's own filter, so the two never disagree about what is
//!   being kept.
//! * **Worker priority** — the scheduling priority the worker asks for on
//!   itself, because a process cannot be created above the priority of whoever
//!   created it. A platform with no way to move a running process says so and
//!   the control is closed rather than stored and ignored.

use serde::{Deserialize, Serialize};
use vise_ipc::contract::LogLevel;

use crate::preferences::{
    want_one_of, ChoiceGroupViewModel, FactViewModel, IsolationAvailability,
    PreferenceTabViewModel, ToggleViewModel,
};
use crate::tabs::common::ChoiceOption;

/// Field identifier for the process isolation switch.
pub const PROCESS_ISOLATION: &str = "advanced.process_isolation";
/// Field identifier for the log level.
pub const LOG_LEVEL: &str = "advanced.log_level";
/// Field identifier for the worker priority.
pub const PROCESS_PRIORITY: &str = "advanced.process_priority";

/// Read a log level the interface layer sent.
///
/// The four levels are the worker's own, so a level chosen here is the same
/// vocabulary the log window filters with and the same ranks the worker filters
/// on; there is no second table to fall out of step.
pub fn read_log_level(value: &serde_json::Value) -> Result<LogLevel, String> {
    let typed = want_one_of(
        LOG_LEVEL,
        value,
        &LogLevel::ALL
            .iter()
            .map(|level| (level.id(), level.label()))
            .collect::<Vec<(&str, &str)>>(),
    )?;
    LogLevel::parse(&typed).ok_or_else(|| format!("'{typed}' is not a level of detail."))
}

/// How much scheduling priority the worker asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PriorityPreference {
    /// Give way to other programs.
    BelowNormal,
    /// Whatever the platform gives a process by default.
    #[default]
    Normal,
    /// Take more of the machine than other programs get.
    AboveNormal,
}

impl PriorityPreference {
    /// Every choice, in the order the picker lists them.
    pub const ALL: [PriorityPreference; 3] = [
        PriorityPreference::BelowNormal,
        PriorityPreference::Normal,
        PriorityPreference::AboveNormal,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            PriorityPreference::BelowNormal => "below_normal",
            PriorityPreference::Normal => "normal",
            PriorityPreference::AboveNormal => "above_normal",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            PriorityPreference::BelowNormal => "Below normal",
            PriorityPreference::Normal => "Normal",
            PriorityPreference::AboveNormal => "Above normal",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            PriorityPreference::BelowNormal => {
                "The worker gives way to other programs, which makes a long encode take longer."
            }
            PriorityPreference::Normal => {
                "The worker takes a share of the machine like any other program."
            }
            PriorityPreference::AboveNormal => {
                "The worker asks for more of the machine than other programs get."
            }
        }
    }

    /// The level on the scale the worker contract carries.
    ///
    /// The scale is signed with zero in the middle, which is the middle of every
    /// platform's priority scale, so the number means the same thing on each.
    pub fn level(self) -> i32 {
        match self {
            PriorityPreference::BelowNormal => -1,
            PriorityPreference::Normal => 0,
            PriorityPreference::AboveNormal => 1,
        }
    }

    /// Read a priority the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            PROCESS_PRIORITY,
            value,
            &[
                (
                    PriorityPreference::BelowNormal.id(),
                    PriorityPreference::BelowNormal.label(),
                ),
                (
                    PriorityPreference::Normal.id(),
                    PriorityPreference::Normal.label(),
                ),
                (
                    PriorityPreference::AboveNormal.id(),
                    PriorityPreference::AboveNormal.label(),
                ),
            ],
        )?;
        PriorityPreference::ALL
            .into_iter()
            .find(|priority| priority.id() == typed)
            .ok_or_else(|| format!("'{typed}' is not a priority this build offers."))
    }
}

/// The advanced preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdvancedSettings {
    /// Whether the worker runs as its own process.
    pub process_isolation: bool,
    /// The least severe level that is recorded.
    pub log_level: LogLevel,
    /// What the worker asks the platform for.
    pub process_priority: PriorityPreference,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// Isolation is on because a crashed encoder should not take the window with
    /// it. The log level is `info`, which is what keeps a failure explainable
    /// without turning a long encode into thousands of lines. The priority is
    /// normal, because a build that takes more of the machine than it was given
    /// is a build nobody leaves running.
    pub fn new() -> Self {
        Self {
            process_isolation: true,
            log_level: LogLevel::Info,
            process_priority: PriorityPreference::Normal,
        }
    }
}

/// The problems these preferences have.
///
/// There are none this build can find, and that is a fact rather than an omission:
/// all three are stored as closed enumerations or as a boolean, so a record that
/// held a value this build cannot use was already refused as unreadable when it
/// was read. The rule is kept because the tab builds its problem list from it and
/// because it is where the next rule belongs.
pub fn validate(_settings: &AdvancedSettings) -> Vec<String> {
    Vec::new()
}

/// Build the advanced tab.
pub fn tab_view(
    settings: &AdvancedSettings,
    isolation: &IsolationAvailability,
) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "advanced",
        "Advanced",
        "Process isolation, how much is logged, and how the worker is scheduled.",
    );
    let mut toggle = ToggleViewModel::new(
        "advancedProcessIsolation",
        "Run the worker as a separate process",
        "An encoder that crashes does not take the window with it. Turning it off runs the pipeline inside the window.",
        settings.process_isolation,
    );
    if let Some(reason) = isolation.blocked.as_deref() {
        // The stored preference is kept and shown, and the control is closed: a
        // portable settings file is the operator's answer, and accepting a value
        // here would store one that the window is about to ignore.
        toggle = toggle
            .with_hint(
                "Your choice is kept and takes effect if the portable settings file is removed.",
            )
            .blocked(reason);
    } else if !settings.process_isolation {
        toggle = toggle.with_hint(
            "The pipeline runs inside this window, so an encoder that crashes can take the window with it.",
        );
    }
    tab.toggles.push(toggle);

    let levels = LogLevel::ALL;
    let level_hint = level_effect(settings.log_level);
    tab.choices.push(
        ChoiceGroupViewModel::build(
            LOG_LEVEL,
            "How much is logged",
            "Sent to the worker with every scan and encode, and used as the floor of the activity window.",
            settings.log_level.id(),
            &levels
                .iter()
                .map(|level| ChoiceOption::new(level.id(), level.label()))
                .collect::<Vec<_>>(),
            true,
            "",
        )
        .with_hint(&level_hint),
    );

    tab.choices.push(
        ChoiceGroupViewModel::build(
            PROCESS_PRIORITY,
            "Worker priority",
            "Asked for by the worker on itself, because a process cannot be created above whoever created it.",
            settings.process_priority.id(),
            &PriorityPreference::ALL
                .iter()
                .map(|priority| ChoiceOption::new(priority.id(), priority.label()))
                .collect::<Vec<_>>(),
            priority_supported(),
            if priority_supported() {
                ""
            } else {
                "This platform has no way to move a running process to a priority, so this is kept at normal."
            },
        )
        .with_hint("Raising it makes an encode finish sooner and everything else slower."),
    );

    // The rank is what the worker actually filters on, so it is shown as a
    // read-out rather than as a field: typing a number here would be a second
    // answer to the question the picker above has already answered.
    tab.facts.push(FactViewModel::present(
        "advancedLogRank",
        "Log verbosity rank",
        format!(
            "{} — 0 keeps failures, 3 keeps everything.",
            settings.log_level.rank()
        ),
    ));

    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

/// What a log level keeps, in words, for the line under the control.
pub fn level_effect(level: LogLevel) -> String {
    match level {
        LogLevel::Error => "Keeps only failures.".to_string(),
        LogLevel::Warning => "Keeps failures and warnings.".to_string(),
        LogLevel::Info => "Keeps what the application and the worker did.".to_string(),
        LogLevel::Debug => "Keeps everything, which on a long encode is a great deal.".to_string(),
    }
}

/// Whether this platform can move a running process to a priority.
///
/// The window has to answer the same way the worker will, or the control would
/// offer a choice the worker is about to refuse.
pub fn priority_supported() -> bool {
    cfg!(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    fn available() -> IsolationAvailability {
        IsolationAvailability { blocked: None }
    }

    fn taken(reason: &str) -> IsolationAvailability {
        IsolationAvailability {
            blocked: Some(reason.to_string()),
        }
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = AdvancedSettings::new();
        assert!(settings.process_isolation);
        assert_eq!(settings.log_level, LogLevel::Info);
        assert_eq!(settings.process_priority, PriorityPreference::Normal);
        assert_eq!(settings, AdvancedSettings::default());
    }

    #[test]
    fn a_log_level_is_the_workers_own_vocabulary() {
        assert_eq!(
            read_log_level(&value("\"debug\"")).expect("debug"),
            LogLevel::Debug
        );
        assert_eq!(
            read_log_level(&value("\" ERROR \"")).expect("error"),
            LogLevel::Error
        );
        // The rank the worker filters on is the rank stored, not a second table.
        assert_eq!(
            read_log_level(&value("\"debug\"")).expect("debug").rank(),
            3
        );
        assert!(read_log_level(&value("\"chatty\""))
            .expect_err("unknown")
            .contains("chatty"));
        // Every level round-trips through the name the page sends.
        for level in LogLevel::ALL {
            let raw = serde_json::Value::String(level.id().to_string());
            assert_eq!(read_log_level(&raw).expect("round trip"), level);
        }
        assert!(level_effect(LogLevel::Debug).contains("everything"));
        // A stored value this build cannot name was already refused as
        // unreadable when the record was read, so there is nothing left here to
        // report.
        assert!(validate(&AdvancedSettings::new()).is_empty());
    }

    #[test]
    fn the_priority_scale_is_signed_with_the_default_in_the_middle() {
        assert_eq!(PriorityPreference::BelowNormal.level(), -1);
        assert_eq!(PriorityPreference::Normal.level(), 0);
        assert_eq!(PriorityPreference::AboveNormal.level(), 1);
        for priority in PriorityPreference::ALL {
            assert!(priority.hint().ends_with('.'), "{}", priority.hint());
            assert_eq!(
                PriorityPreference::read(&serde_json::Value::String(priority.id().to_string()))
                    .expect("round trip"),
                priority
            );
        }
        assert!(PriorityPreference::read(&value("\"realtime\"")).is_err());
    }

    #[test]
    fn the_tab_shows_the_log_level_the_window_and_the_worker_share() {
        let view = tab_view(&AdvancedSettings::new(), &available());
        assert_eq!(view.toggles.len(), 1);
        assert_eq!(view.toggles[0].id, "advancedProcessIsolation");
        assert!(view.toggles[0].available);
        let level = view
            .choices
            .iter()
            .find(|choice| choice.id == LOG_LEVEL)
            .expect("log level control");
        assert_eq!(level.selected, "info");
        assert_eq!(level.options.len(), 4);
        let ranks: Vec<u8> = level
            .options
            .iter()
            .map(|option| LogLevel::parse(&option.id).expect("level").rank())
            .collect();
        assert_eq!(ranks, vec![0, 1, 2, 3], "the list is least to most verbose");
        // The rank is a read-out, not a field: there is no way to type a second
        // answer to the question the picker above has already answered.
        assert!(view.numbers.is_empty(), "{:?}", view.numbers);
        let rank = &view.facts[0];
        assert_eq!(rank.id, "advancedLogRank");
        assert!(rank.value.contains('2'), "{}", rank.value);
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_portable_settings_file_that_took_isolation_away_closes_the_control_with_its_reason() {
        let view = tab_view(
            &AdvancedSettings::new(),
            &taken("process isolation is disabled in the portable settings file"),
        );
        let toggle = &view.toggles[0];
        assert!(!toggle.available);
        assert!(
            toggle.reason.contains("portable settings file"),
            "{}",
            toggle.reason
        );
        // The preference is still shown as the user left it rather than being
        // rewritten, so removing the file brings it back.
        assert!(toggle.checked);
        assert!(toggle.hint.contains("kept"), "{}", toggle.hint);
    }

    #[test]
    fn the_priority_control_is_closed_on_a_platform_that_cannot_move_a_process() {
        let view = tab_view(&AdvancedSettings::new(), &available());
        let priority = view
            .choices
            .iter()
            .find(|choice| choice.id == PROCESS_PRIORITY)
            .expect("priority control");
        // The window and the worker have to agree, so the control says exactly
        // what the worker would have said on this platform.
        assert_eq!(priority.available, priority_supported());
        if !priority.available {
            assert!(priority.reason.contains("no way"), "{}", priority.reason);
        }
    }
}
