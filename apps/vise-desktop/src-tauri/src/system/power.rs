//! When the machine is held awake, and who holds it.
//!
//! A long encode that lets the machine sleep is an encode that finishes hours
//! later than it had to. So the hold is taken while an encode is running and let
//! go the moment the pipeline is idle — not for a fixed period that would outlive
//! the encode and leave a desktop awake all night.
//!
//! The interesting part is *which process* holds it. When the pipeline is in a
//! worker process, that process holds it: the hold then dies with the encode
//! thread that asked for it, and a window that crashes mid-encode cannot leave
//! somebody's machine awake until they log off. When process isolation is off —
//! which is what a portable settings file can arrange — the pipeline is inside
//! this window, so the window holds it instead. Anything else would be two
//! processes holding one state and one of them letting go of the other's hold.
//!
//! The decision itself is a pure function of the queue, the preference, and which
//! process is doing the encoding, so it is checked without a window; the hold
//! itself is [`vise_ipc::power`], which is one platform call.

use vise_ipc::power::PowerScope;

use crate::system::SystemFacts;

/// What the preferences ask for while an encode runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerPolicy {
    /// Whether the machine should be held awake at all.
    pub prevent_sleep: bool,
    /// What it should keep doing while it is held.
    pub scope: PowerScope,
}

impl Default for PowerPolicy {
    fn default() -> Self {
        PowerPolicy {
            prevent_sleep: true,
            scope: PowerScope::System,
        }
    }
}

/// What to do about the hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Take a hold at this scope.
    Hold { scope: String },
    /// Let the machine sleep.
    Release,
    /// Nothing, because what is already in place is what was wanted.
    StayPut,
}

/// The decision, with the sentence the activity log records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// What to do.
    pub action: Action,
    /// Why, in words that fit in a log line.
    pub reason: String,
}

/// Decide what this window should do about the hold.
///
/// Four answers, and the fourth is the one that is easy to get wrong: when the
/// encoder is a separate process, the hold belongs to it and the window's answer
/// is that it is not doing anything. That is a decision, not an omission, and it
/// is why the reason says so.
pub fn decide(policy: PowerPolicy, facts: &SystemFacts, already_held: bool) -> Decision {
    let idle = facts.running == 0;
    if idle {
        return match already_held {
            true => Decision {
                action: Action::Release,
                reason: "the queue is idle, so the machine is being let go of".to_string(),
            },
            false => Decision {
                action: Action::StayPut,
                reason: "the queue is idle, so the machine is not being held".to_string(),
            },
        };
    }
    if !policy.prevent_sleep {
        return match already_held {
            true => Decision {
                action: Action::Release,
                reason:
                    "the preferences ask for no hold while encoding, so the machine is being let go of"
                        .to_string(),
            },
            false => Decision {
                action: Action::StayPut,
                reason: "the preferences ask for no hold while encoding".to_string(),
            },
        };
    }
    if !facts.in_process {
        return Decision {
            action: Action::StayPut,
            reason: "the worker process holds the machine awake while it encodes".to_string(),
        };
    }
    let scope = policy.scope.id().to_string();
    match already_held {
        true => Decision {
            action: Action::StayPut,
            reason: "this window is already holding the machine awake".to_string(),
        },
        false => Decision {
            action: Action::Hold {
                scope: scope.clone(),
            },
            reason: format!(
                "the pipeline is running in this window, so it is holding the machine awake: {}",
                policy.scope.effect()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(running: usize, in_process: bool) -> SystemFacts {
        SystemFacts {
            running,
            queued: running + 1,
            in_process,
            ..SystemFacts::default()
        }
    }

    #[test]
    fn an_encode_in_this_window_takes_the_hold_it_asked_for() {
        let decision = decide(PowerPolicy::default(), &facts(1, true), false);
        assert_eq!(
            decision.action,
            Action::Hold {
                scope: "system".to_string()
            }
        );
        assert!(
            decision.reason.contains("this window"),
            "{}",
            decision.reason
        );
    }

    #[test]
    fn taking_a_hold_twice_is_not_twice_a_hold() {
        let decision = decide(PowerPolicy::default(), &facts(1, true), true);
        assert_eq!(decision.action, Action::StayPut);
        assert!(decision.reason.contains("already"), "{}", decision.reason);
    }

    #[test]
    fn the_window_lets_go_when_the_pipeline_goes_idle() {
        let decision = decide(PowerPolicy::default(), &facts(0, true), true);
        assert_eq!(decision.action, Action::Release);
        assert!(decision.reason.contains("idle"), "{}", decision.reason);
    }

    #[test]
    fn an_idle_queue_with_nothing_held_does_nothing() {
        let decision = decide(PowerPolicy::default(), &facts(0, true), false);
        assert_eq!(decision.action, Action::StayPut);
    }

    #[test]
    fn a_worker_holding_the_machine_is_the_answer_when_isolation_is_on() {
        // The window's answer is "not me" rather than a second hold: two
        // processes holding one state is how a machine ends up awake all night.
        let decision = decide(PowerPolicy::default(), &facts(1, false), false);
        assert_eq!(decision.action, Action::StayPut);
        assert!(
            decision.reason.contains("worker process"),
            "{}",
            decision.reason
        );
    }

    #[test]
    fn a_preference_that_asks_for_no_hold_wins_over_a_running_encode() {
        let policy = PowerPolicy {
            prevent_sleep: false,
            ..PowerPolicy::default()
        };
        let decision = decide(policy, &facts(1, true), false);
        assert_eq!(decision.action, Action::StayPut);
        assert!(decision.reason.contains("no hold"), "{}", decision.reason);
        // A hold taken under the old preference is given up rather than kept for
        // the rest of the session.
        let released = decide(policy, &facts(1, true), true);
        assert_eq!(released.action, Action::Release);
    }

    #[test]
    fn the_display_scope_reaches_the_decision_unchanged() {
        let policy = PowerPolicy {
            prevent_sleep: true,
            scope: PowerScope::SystemAndDisplay,
        };
        let decision = decide(policy, &facts(1, true), false);
        assert_eq!(
            decision.action,
            Action::Hold {
                scope: "system_and_display".to_string()
            }
        );
        assert!(decision.reason.contains("screen"), "{}", decision.reason);
    }

    #[test]
    fn every_answer_says_why_in_one_line() {
        let answers = [
            decide(PowerPolicy::default(), &facts(1, true), false),
            decide(PowerPolicy::default(), &facts(0, true), true),
            decide(PowerPolicy::default(), &facts(0, true), false),
            decide(PowerPolicy::default(), &facts(1, false), false),
        ];
        for answer in answers {
            assert!(!answer.reason.is_empty());
        }
    }
}
