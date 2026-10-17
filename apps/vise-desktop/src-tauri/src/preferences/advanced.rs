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
    want_one_of, ChoiceGroupViewModel, FactViewModel, IsolationAvailability, NumberViewModel,
    PreferenceTabViewModel, TextViewModel, ToggleViewModel,
};
use crate::tabs::common::ChoiceOption;

/// Field identifier for the process isolation switch.
pub const PROCESS_ISOLATION: &str = "advanced.process_isolation";
/// Field identifier for the log level.
pub const LOG_LEVEL: &str = "advanced.log_level";
/// Field identifier for the worker priority.
pub const PROCESS_PRIORITY: &str = "advanced.process_priority";
/// Field identifier for keeping the machine awake while an encode runs.
pub const PREVENT_SLEEP: &str = "advanced.prevent_sleep";
/// Field identifier for what the machine keeps doing while it is held awake.
pub const POWER_SCOPE: &str = "advanced.power_scope";
/// Field identifier for where the worker is placed on a machine with several
/// processor groups.
pub const NUMA_MODE: &str = "advanced.numa_mode";
/// Field identifier for the command run before an encode.
pub const PRE_ACTION: &str = "advanced.pre_action";
/// Field identifier for the command run after an encode.
pub const POST_ACTION: &str = "advanced.post_action";
/// Field identifier for how long either command may run.
pub const ACTION_TIMEOUT: &str = "advanced.action_timeout_seconds";
/// Field identifier for what happens when a pre-encode command does not work.
pub const ACTION_ON_FAILURE: &str = "advanced.action_on_failure";
/// Field identifier for whether notifications are shown at all.
pub const NOTIFICATIONS: &str = "advanced.notifications";
/// Field identifier for which events become notifications.
pub const NOTIFICATION_SCOPE: &str = "advanced.notification_scope";

/// The names `POWER_SCOPE` accepts.
pub const POWER_VALUES: [&str; 2] = ["system", "system_and_display"];

/// The names `NOTIFICATION_SCOPE` accepts.
pub const NOTIFICATION_SCOPES: [&str; 3] = ["never", "failures", "all"];

/// The names `NUMA_MODE` accepts on a machine that reported its layout.
///
/// Built from the layout rather than written down, because offering a group the
/// machine does not have is a control that cannot be honoured — and a machine that
/// could not be asked gets only the two answers that do not need one.
pub fn numa_values(groups: u16) -> Vec<String> {
    let mut values = vec![vise_ipc::numa::NumaMode::Auto.id()];
    for group in 0..groups {
        values.push(vise_ipc::numa::pin_id(group));
    }
    values.push(vise_ipc::numa::NumaMode::Off.id());
    values
}

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
    /// Whether the machine is held awake while an encode runs.
    pub prevent_sleep: bool,
    /// What the machine keeps doing while it is held awake.
    pub power_scope: String,
    /// Where the worker is placed on a machine with several processor groups.
    pub numa_mode: String,
    /// The command run before each encode.
    pub pre_action: String,
    /// The command run after each encode.
    pub post_action: String,
    /// How long either command may run, in seconds.
    pub action_timeout_seconds: u32,
    /// What happens when a pre-encode command does not work.
    pub action_on_failure: i32,
    /// Whether notifications are shown outside the window.
    pub notifications: bool,
    /// Which events are turned into notifications.
    pub notification_scope: String,
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
    /// is a build nobody leaves running. The machine is held awake while encoding,
    /// because a sleep halfway through a five-hour encode costs the five hours;
    /// the display is not held, because nobody watches a progress bar for that
    /// long. There are no commands around the encode, because a build that ran
    /// something nobody asked for would be a build nobody could reason about.
    pub fn new() -> Self {
        Self {
            process_isolation: true,
            log_level: LogLevel::Info,
            process_priority: PriorityPreference::Normal,
            prevent_sleep: true,
            power_scope: vise_ipc::power::PowerScope::System.id().to_string(),
            numa_mode: vise_ipc::numa::NumaMode::Auto.id(),
            pre_action: String::new(),
            post_action: String::new(),
            action_timeout_seconds: 300,
            action_on_failure: 0,
            notifications: true,
            notification_scope: crate::system::notify::NotifyScope::All.id().to_string(),
        }
    }

    /// What the machine should keep doing while it is held awake.
    pub fn power(&self) -> vise_ipc::power::PowerScope {
        vise_ipc::power::PowerScope::of(&self.power_scope)
    }

    /// Where the worker should be placed.
    pub fn numa(&self) -> vise_ipc::numa::NumaMode {
        vise_ipc::numa::read_mode(&self.numa_mode)
    }

    /// Which events are turned into notifications.
    pub fn notification_scope(&self) -> crate::system::notify::NotifyScope {
        crate::system::notify::NotifyScope::of(&self.notification_scope)
    }

    /// The resolved action settings, as the encode paths read them.
    pub fn actions(&self) -> ActionSettings {
        ActionSettings {
            pre: self.pre_action.clone(),
            post: self.post_action.clone(),
            timeout: self.action_timeout_seconds,
            on_failure: self.action_on_failure,
        }
    }
}

/// The commands that run around an encode, as one value.
///
/// Gathered here rather than read field by field at three call sites, so that the
/// timeout and the failure rule that apply to one apply to the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ActionSettings {
    /// The command run before each encode.
    pub pre: String,
    /// The command run after each encode.
    pub post: String,
    /// How long either command may run, in seconds.
    pub timeout: u32,
    /// What happens when a pre-encode command does not work.
    pub on_failure: i32,
}

impl Default for ActionSettings {
    fn default() -> Self {
        Self {
            pre: String::new(),
            post: String::new(),
            timeout: 300,
            on_failure: 0,
        }
    }
}

/// The problems these preferences have.
///
/// Two rules, both about commands rather than about switches. A command that does
/// not tokenise is a command the user has not finished writing, and storing it as
/// though it worked would make the encode fail later with a message about the
/// encode; and a timeout of no seconds is not a timeout at all.
pub fn validate(settings: &AdvancedSettings) -> Vec<String> {
    let mut problems = Vec::new();
    for (label, command) in [
        ("the pre-encode command", &settings.pre_action),
        ("the post-encode command", &settings.post_action),
    ] {
        if command.trim().is_empty() {
            continue;
        }
        if let Err(reason) = crate::system::actions::tokenize(command) {
            problems.push(format!("{label} cannot be read: {reason}."));
        }
    }
    if settings.action_timeout_seconds == 0 {
        problems
            .push("A command around an encode needs a timeout of at least one second.".to_string());
    }
    problems
}

/// Build the advanced tab.
///
/// The controls here are the ones this build can carry out: what the machine is
/// held awake for, where the worker is placed, the two commands around an encode,
/// and what is turned into a notification. Each names the code path that reads it,
/// and the read-outs under them are values rather than fields, so there is no
/// second answer to the same question anywhere on the tab.
pub fn tab_view(
    settings: &AdvancedSettings,
    isolation: &IsolationAvailability,
    machine: &MachineFacts,
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

    tab.toggles.push(ToggleViewModel::new(
        "advancedPreventSleep",
        "Keep the machine awake while an encode runs",
        "The machine is held awake for as long as the pipeline is running, and let go the moment it stops. The worker holds it when the encoder is a separate process, and this window does when the pipeline runs inside it.",
        settings.prevent_sleep,
    ));
    let scope = vise_ipc::power::PowerScope::of(&settings.power_scope);
    tab.choices.push(
        ChoiceGroupViewModel::build(
            POWER_SCOPE,
            "What stays on",
            "Only used while the machine is being held awake.",
            scope.id(),
            &[
                vise_ipc::power::PowerScope::System,
                vise_ipc::power::PowerScope::SystemAndDisplay,
            ]
            .iter()
            .map(|value| ChoiceOption::new(value.id(), value.label()))
            .collect::<Vec<_>>(),
            power_available(),
            if power_available() {
                ""
            } else {
                "this platform cannot hold the machine awake, so what stays on has nothing to do"
            },
        )
        .with_hint(scope.effect()),
    );

    let topology = machine.topology.clone();
    let off = vise_ipc::numa::NumaMode::Off;
    let auto = vise_ipc::numa::NumaMode::Auto;
    let placement_options: Vec<ChoiceOption> = [ChoiceOption::new(&auto.id(), "Let Vise decide")]
        .into_iter()
        .chain((0..topology.groups).map(|group| {
            let id = vise_ipc::numa::pin_id(group);
            let label = vise_ipc::numa::NumaMode::PinTo(group).label();
            ChoiceOption::new(&id, &label)
        }))
        .chain([{
            let label = off.label();
            ChoiceOption::new("off", &label)
        }])
        .collect();
    tab.choices.push(
        ChoiceGroupViewModel::build(
            NUMA_MODE,
            "Where the worker runs",
            "Asked for on the worker itself, because a process can restrict itself where nothing else can restrict it reliably.",
            &settings.numa_mode,
            &placement_options,
            topology.measured(),
            if topology.measured() {
                ""
            } else {
                "this machine did not report its processor layout, so there is nothing to choose"
            },
        )
        .with_hint(settings.numa().effect()),
    );
    tab.facts.push(match machine.topology.measured() {
        true => FactViewModel::present(
            "advancedTopology",
            "Processor layout",
            machine.topology.describe(),
        ),
        false => FactViewModel::unknown(
            "advancedTopology",
            "Processor layout",
            machine.topology.describe(),
        ),
    });
    let placement = vise_ipc::numa::placement_for(
        &machine.topology,
        settings.numa(),
        vise_ipc::numa::WorkerPriority::of(settings.process_priority.level()),
    );
    tab.facts.push(FactViewModel::present(
        "advancedPlacement",
        "Where it will run",
        placement.reason,
    ));

    tab.texts.push(
        TextViewModel::new(
            "advancedPreAction",
            "Command to run before an encode",
            &settings.pre_action,
            "notify.exe %output%",
        )
        .with_hint(
            "The program and its arguments, with %output%, %input%, %title%, %job_id%, %container%, and %folder% replaced. There is no shell: the program is started directly, so a title containing & or ; is only a file name.",
        ),
    );
    tab.texts.push(
        TextViewModel::new(
            "advancedPostAction",
            "Command to run after an encode",
            &settings.post_action,
            r#"copy.exe %output% %folder%\library"#,
        )
        .with_hint(
            "Runs after the file is written, whether the encode worked or not. Nothing it does can fail an encode that is already finished.",
        ),
    );
    tab.numbers.push(
        NumberViewModel::new(
            "advancedActionTimeout",
            "How long either command may run",
            &settings.action_timeout_seconds.to_string(),
            "seconds",
        )
        .with_hint("A command that takes longer is stopped so the queue is not held up."),
    );
    tab.choices.push(
        ChoiceGroupViewModel::build(
            ACTION_ON_FAILURE,
            "When the command before an encode does not work",
            "A command that fails can either be reported and carried on past, or stop the encode before it starts.",
            if settings.action_on_failure < 0 {
                "stop_encode"
            } else {
                "continue"
            },
            &[
                ChoiceOption::new("continue", "Carry on and say what happened"),
                ChoiceOption::new("stop_encode", "Do not start the encode"),
            ],
            true,
            "",
        )
        .with_hint(
            "A command that cannot be read at all always stops the encode: running nothing is not what a command is for.",
        ),
    );
    let pre = settings.pre_action.trim();
    let post = settings.post_action.trim();
    tab.facts.push(FactViewModel::present(
        "advancedActions",
        "Commands around an encode",
        if pre.is_empty() && post.is_empty() {
            "none are configured".to_string()
        } else {
            format!(
                "{} before, {} after",
                if pre.is_empty() { "none" } else { "one" },
                if post.is_empty() { "none" } else { "one" }
            )
        },
    ));

    tab.toggles.push(ToggleViewModel::new(
        "advancedNotifications",
        "Show notifications outside the window",
        "A notification reaches somebody who is not looking at this window. Everything decided is written to the activity log either way.",
        settings.notifications,
    ));
    tab.choices.push(
        ChoiceGroupViewModel::build(
            NOTIFICATION_SCOPE,
            "What a notification is about",
            "A queue that finishes where you can see it finishing is not a notification.",
            settings.notification_scope().id(),
            &crate::system::notify::NotifyScope::ALL
                .iter()
                .map(|scope| ChoiceOption::new(scope.id(), scope.label()))
                .collect::<Vec<_>>(),
            true,
            "",
        )
        .with_hint(settings.notification_scope().hint()),
    );
    tab.facts.push(match machine.notifications.available {
        true => FactViewModel::present(
            "advancedNotifier",
            "Where notifications are shown",
            machine.notifications.description.clone(),
        ),
        false => FactViewModel::missing(
            "advancedNotifier",
            "Where notifications are shown",
            machine.notifications.description.clone(),
        ),
    });

    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

/// What the machine was found to be, as the Advanced tab shows it.
///
/// Detection is asked once and held, like the hardware tab's: a layout and a
/// notification area are both things a window asks the platform about rather than
/// something it can work out for itself on every paint.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineFacts {
    /// The processor layout the machine reported.
    pub topology: vise_ipc::numa::Topology,
    /// Whether this platform can hold the machine awake.
    pub power_available: bool,
    /// The notifications this window can show.
    pub notifications: crate::system::notify::NotificationState,
}

impl Default for MachineFacts {
    fn default() -> Self {
        MachineFacts {
            topology: vise_ipc::numa::topology(),
            power_available: vise_ipc::power::available(),
            notifications: crate::system::notify::NotificationState {
                available: false,
                description: "the window has not looked yet".to_string(),
                scope: crate::system::notify::NotifyScope::All,
                window_visible: false,
            },
        }
    }
}

impl MachineFacts {
    /// The facts of a window that has found out what this machine can do.
    pub fn of(notifications: crate::system::notify::NotificationState) -> MachineFacts {
        MachineFacts {
            notifications,
            ..MachineFacts::default()
        }
    }
}

/// Whether this platform can hold the machine awake.
pub fn power_available() -> bool {
    vise_ipc::power::available()
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

    fn machine() -> MachineFacts {
        MachineFacts::default()
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
        let view = tab_view(&AdvancedSettings::new(), &available(), &machine());
        // Three switches, each a thing the window acts on: isolation, the hold on
        // the machine, and whether notifications are shown at all.
        let switches: Vec<&str> = view
            .toggles
            .iter()
            .map(|toggle| toggle.id.as_str())
            .collect();
        assert_eq!(
            switches,
            vec![
                "advancedProcessIsolation",
                "advancedPreventSleep",
                "advancedNotifications",
            ]
        );
        assert!(view.toggles.iter().all(|toggle| toggle.available));
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
        // The only number on the tab is the timeout of a command around an encode,
        // and it is a field because it is a thing the user can change; the log rank
        // above is a read-out because it is not.
        assert_eq!(view.numbers.len(), 1, "{:?}", view.numbers);
        assert_eq!(view.numbers[0].id, "advancedActionTimeout");
        assert_eq!(view.numbers[0].text, "300");
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
            &machine(),
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
        let view = tab_view(&AdvancedSettings::new(), &available(), &machine());
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
