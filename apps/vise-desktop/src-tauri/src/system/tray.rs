//! The notification-area icon: what it says and what its menu offers.
//!
//! The tray is the surface somebody uses when the window is not in front, so what
//! it carries is the answer to two questions at all times — what is the queue
//! doing, and what can I do about it right now — plus the three things a desktop
//! application is expected to offer from there: come back to the window, open the
//! queue, and leave.
//!
//! The model is built in Rust from plain facts and published to the platform by
//! the window, because a menu that a page composes is a menu whose items can
//! disagree with what the queue is doing. An item the window cannot carry out is
//! disabled and says why, in the same place, rather than being pressed and
//! discovered to do nothing.

use serde::{Deserialize, Serialize};

use crate::system::SystemFacts;

/// Identifier of every item the tray menu offers.
///
/// Written down once, because the model, the window's routing table, and the
/// platform menu are three places that have to agree on what a press means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayAction {
    /// Bring the window to the front.
    ShowWindow,
    /// Open the queue window.
    OpenQueue,
    /// Pause the queue.
    PauseQueue,
    /// Resume the queue.
    ResumeQueue,
    /// Cancel whatever the encoder is holding.
    CancelCurrent,
    /// Keep the machine awake while encoding.
    PreventSleep,
    /// Look for a newer release.
    CheckForUpdates,
    /// Show the about window.
    About,
    /// Close the application.
    Quit,
}

impl TrayAction {
    /// Every action, in the order the menu lists them.
    pub const ALL: [TrayAction; 9] = [
        TrayAction::ShowWindow,
        TrayAction::OpenQueue,
        TrayAction::PauseQueue,
        TrayAction::ResumeQueue,
        TrayAction::CancelCurrent,
        TrayAction::PreventSleep,
        TrayAction::CheckForUpdates,
        TrayAction::About,
        TrayAction::Quit,
    ];

    /// Stable identifier the platform menu and the window both use.
    pub fn id(self) -> &'static str {
        match self {
            TrayAction::ShowWindow => "trayShowWindow",
            TrayAction::OpenQueue => "trayOpenQueue",
            TrayAction::PauseQueue => "trayPauseQueue",
            TrayAction::ResumeQueue => "trayResumeQueue",
            TrayAction::CancelCurrent => "trayCancelCurrent",
            TrayAction::PreventSleep => "trayPreventSleep",
            TrayAction::CheckForUpdates => "trayCheckForUpdates",
            TrayAction::About => "trayAbout",
            TrayAction::Quit => "trayQuit",
        }
    }

    /// The action an identifier names, when it names one of ours.
    pub fn parse(id: &str) -> Option<TrayAction> {
        TrayAction::ALL.into_iter().find(|action| action.id() == id)
    }

    /// Label on the menu item.
    pub fn label(self) -> &'static str {
        match self {
            TrayAction::ShowWindow => "Show Vise Transcoder",
            TrayAction::OpenQueue => "Open the queue",
            TrayAction::PauseQueue => "Pause the queue",
            TrayAction::ResumeQueue => "Resume the queue",
            TrayAction::CancelCurrent => "Cancel the current encode",
            TrayAction::PreventSleep => "Keep the machine awake while encoding",
            TrayAction::CheckForUpdates => "Look for a release now",
            TrayAction::About => "About Vise Transcoder",
            TrayAction::Quit => "Quit",
        }
    }
}

/// What kind of row an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayItemKind {
    /// A row that does something when it is pressed.
    Normal,
    /// A row that carries a tick, for a state that is on or off.
    Tick,
    /// A divider.
    Separator,
    /// A line of text that cannot be pressed.
    ReadOnly,
}

/// One row of the tray menu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItem {
    /// What pressing the row asks for, when it can be pressed.
    pub action: Option<TrayAction>,
    /// Stable identifier for the row.
    pub id: String,
    /// Text on the row.
    pub label: String,
    /// What kind of row it is.
    pub kind: TrayItemKind,
    /// Whether it carries a tick.
    pub ticked: bool,
    /// Whether it can be pressed.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl TrayItem {
    /// A row that can be pressed.
    fn action(action: TrayAction) -> TrayItem {
        TrayItem {
            action: Some(action),
            id: action.id().to_string(),
            label: action.label().to_string(),
            kind: TrayItemKind::Normal,
            ticked: false,
            available: true,
            reason: String::new(),
        }
    }

    /// The same row, disabled with a reason.
    fn closed(mut self, reason: impl Into<String>) -> TrayItem {
        self.available = false;
        self.reason = reason.into();
        self
    }

    /// The same row, carrying a tick or not.
    fn ticked(mut self, ticked: bool) -> TrayItem {
        self.kind = TrayItemKind::Tick;
        self.ticked = ticked;
        self
    }

    /// A line of text.
    fn read_only(id: &str, label: impl Into<String>) -> TrayItem {
        TrayItem {
            action: None,
            id: id.to_string(),
            label: label.into(),
            kind: TrayItemKind::ReadOnly,
            ticked: false,
            available: false,
            reason: String::new(),
        }
    }

    /// A divider.
    fn separator() -> TrayItem {
        TrayItem {
            action: None,
            id: String::new(),
            label: String::new(),
            kind: TrayItemKind::Separator,
            ticked: false,
            available: false,
            reason: String::new(),
        }
    }
}

/// The facts the menu is built from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrayFacts {
    /// Jobs waiting for an encoder.
    pub queued: usize,
    /// Jobs the encoder is holding.
    pub running: usize,
    /// Jobs that have finished.
    pub finished: usize,
    /// Whether the window is on screen.
    pub window_visible: bool,
    /// Whether the queue is paused, and why.
    pub paused: Option<String>,
    /// Whether a newer release is on offer.
    pub update_available: bool,
    /// Whether the machine is being held awake.
    pub power_held: bool,
    /// Whether a portable settings file was found.
    pub portable: bool,
    /// Whether an encode runs in this window rather than in a worker.
    pub in_process: bool,
}

impl Default for TrayFacts {
    fn default() -> Self {
        TrayFacts {
            queued: 0,
            running: 0,
            finished: 0,
            window_visible: true,
            paused: None,
            update_available: false,
            power_held: false,
            portable: false,
            in_process: false,
        }
    }
}

impl TrayFacts {
    /// Read the facts out of what the window knows.
    pub fn of(facts: &SystemFacts, power_held: bool) -> TrayFacts {
        TrayFacts {
            queued: facts.queued,
            running: facts.running,
            finished: facts.finished,
            window_visible: facts.window_visible,
            paused: facts.queue_paused.clone(),
            update_available: facts.update_available,
            power_held,
            portable: facts.portable,
            in_process: facts.in_process,
        }
    }

    /// Whether there is anything left for the queue to do.
    pub fn busy(&self) -> bool {
        self.queued > 0 || self.running > 0
    }
}

/// The whole tray menu, as the window publishes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayModel {
    /// The tooltip over the icon, which is where the queue's state is stated in
    /// one line.
    pub tooltip: String,
    /// The rows, in order.
    pub items: Vec<TrayItem>,
}

/// One line saying what the queue is doing.
pub fn status_line(facts: &TrayFacts) -> String {
    if let Some(reason) = facts.paused.as_deref() {
        return format!("Vise Transcoder — the queue is paused: {reason}");
    }
    match (facts.running, facts.queued) {
        (0, 0) => "Vise Transcoder — nothing to do".to_string(),
        (0, queued) => format!("Vise Transcoder — {queued} job(s) waiting"),
        (running, 0) => format!("Vise Transcoder — encoding, {running} running"),
        (running, queued) => format!("Vise Transcoder — encoding {running}, {queued} waiting"),
    }
}

/// Build the tray menu from the facts.
pub fn build(facts: &TrayFacts) -> TrayModel {
    let mut items = Vec::new();
    items.push(
        TrayItem::action(TrayAction::ShowWindow).closed_if(facts.window_visible, || {
            "the window is already in front".to_string()
        }),
    );
    items.push(TrayItem::action(TrayAction::OpenQueue));
    items.push(TrayItem::read_only("trayStatus", status_line(facts)));
    items.push(TrayItem::separator());

    if facts.paused.is_some() {
        items.push(TrayItem::action(TrayAction::ResumeQueue));
    } else {
        items.push(
            TrayItem::action(TrayAction::PauseQueue).closed_unless(facts.busy(), || {
                "there is nothing in the queue to pause".to_string()
            }),
        );
    }
    items.push(
        TrayItem::action(TrayAction::CancelCurrent).closed_unless(facts.running > 0, || {
            "the encoder is not holding a job".to_string()
        }),
    );

    items.push(TrayItem::separator());
    items.push(TrayItem::action(TrayAction::PreventSleep).ticked(facts.power_held));
    if facts.portable {
        // A portable build has its state beside the executable, which is worth
        // saying where somebody will look for it rather than in the log they have
        // not opened.
        items.push(TrayItem::read_only(
            "trayPortable",
            "Portable: settings and the queue live beside the executable",
        ));
    }
    items.push(
        TrayItem::action(TrayAction::CheckForUpdates).closed_unless(facts.update_available, || {
            "no newer release is on offer".to_string()
        }),
    );
    items.push(TrayItem::separator());
    items.push(TrayItem::action(TrayAction::About));
    items.push(TrayItem::action(TrayAction::Quit));
    TrayModel {
        tooltip: status_line(facts),
        items,
    }
}

impl TrayItem {
    /// Close the row when `closed` holds.
    fn closed_if(self, closed: bool, reason: impl FnOnce() -> String) -> TrayItem {
        if closed {
            self.closed(reason())
        } else {
            self
        }
    }

    /// Close the row unless `open` holds.
    fn closed_unless(self, open: bool, reason: impl FnOnce() -> String) -> TrayItem {
        self.closed_if(!open, reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TrayFacts {
        TrayFacts {
            queued: 3,
            running: 1,
            finished: 2,
            ..TrayFacts::default()
        }
    }

    fn item<'a>(model: &'a TrayModel, id: &str) -> &'a TrayItem {
        model
            .items
            .iter()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("{id} is in the menu"))
    }

    #[test]
    fn every_action_has_one_identifier_and_it_reads_back() {
        let mut ids: Vec<&str> = TrayAction::ALL.iter().map(|a| a.id()).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "two actions share an identifier");
        for action in TrayAction::ALL {
            assert_eq!(TrayAction::parse(action.id()), Some(action));
            assert!(action.label().ends_with('.') || action.label().len() > 3);
            assert!(!action.label().is_empty());
        }
        assert_eq!(TrayAction::parse("trayRebuild"), None);
    }

    #[test]
    fn the_menu_offers_the_desktop_basics_and_a_status_line() {
        let model = build(&facts());
        for action in [
            TrayAction::ShowWindow,
            TrayAction::OpenQueue,
            TrayAction::PauseQueue,
            TrayAction::CancelCurrent,
            TrayAction::PreventSleep,
            TrayAction::About,
            TrayAction::Quit,
        ] {
            assert!(
                model.items.iter().any(|item| item.action == Some(action)),
                "{:?} is missing",
                action
            );
        }
        assert!(model.tooltip.contains("encoding 1"), "{}", model.tooltip);
        assert!(model.tooltip.contains("3 waiting"), "{}", model.tooltip);
        assert_eq!(
            item(&model, "trayStatus").label,
            model.tooltip,
            "the status row and the tooltip say the same thing"
        );
        // The window being in front closes the one row that would do nothing.
        assert!(!item(&model, TrayAction::ShowWindow.id()).available);
        assert!(item(&model, TrayAction::ShowWindow.id())
            .reason
            .contains("already in front"));
    }

    #[test]
    fn a_pause_and_a_resume_are_never_both_on_offer() {
        let running = build(&facts());
        assert!(item(&running, TrayAction::PauseQueue.id()).available);
        assert!(running
            .items
            .iter()
            .all(|item| item.action != Some(TrayAction::ResumeQueue)));

        let paused = build(&TrayFacts {
            paused: Some("not enough room on the disk".to_string()),
            queued: 2,
            ..TrayFacts::default()
        });
        assert!(item(&paused, TrayAction::ResumeQueue.id()).available);
        assert!(paused
            .items
            .iter()
            .all(|item| item.action != Some(TrayAction::PauseQueue)));
        assert!(
            paused.tooltip.contains("not enough room"),
            "the tooltip says why: {}",
            paused.tooltip
        );
    }

    #[test]
    fn a_closed_row_says_why_in_its_own_place() {
        let idle = build(&TrayFacts::default());
        for id in [
            TrayAction::PauseQueue.id(),
            TrayAction::CancelCurrent.id(),
            TrayAction::CheckForUpdates.id(),
        ] {
            let row = item(&idle, id);
            assert!(!row.available, "{id} should be closed");
            assert!(!row.reason.is_empty(), "{id} should say why");
        }
        assert_eq!(idle.tooltip, "Vise Transcoder — nothing to do");
    }

    #[test]
    fn the_machine_awake_row_carries_a_tick_and_the_release_row_appears_with_one() {
        let held = build(&TrayFacts {
            power_held: true,
            queued: 1,
            ..TrayFacts::default()
        });
        let row = item(&held, TrayAction::PreventSleep.id());
        assert_eq!(row.kind, TrayItemKind::Tick);
        assert!(row.ticked);
        let free = build(&TrayFacts::default());
        assert!(!item(&free, TrayAction::PreventSleep.id()).ticked);
    }

    #[test]
    fn a_portable_build_says_where_its_state_lives() {
        let portable = build(&TrayFacts {
            portable: true,
            ..TrayFacts::default()
        });
        assert!(item(&portable, "trayPortable")
            .label
            .contains("beside the executable"));
        let installed = build(&TrayFacts::default());
        assert!(
            installed.items.iter().all(|item| item.id != "trayPortable"),
            "an installed build says nothing about portable state it is not in"
        );
    }

    #[test]
    fn a_release_that_is_on_offer_can_be_pressed_and_one_that_is_not_cannot() {
        let offered = build(&TrayFacts {
            update_available: true,
            ..TrayFacts::default()
        });
        assert!(item(&offered, TrayAction::CheckForUpdates.id()).available);
        let quiet = build(&TrayFacts::default());
        // The row is still there — a menu that loses a row is a menu that changes
        // shape — but a press of it resolves to nothing to do.
        assert!(!item(&quiet, TrayAction::CheckForUpdates.id()).available);
    }

    #[test]
    fn the_menu_never_has_two_rows_for_one_action_or_no_identifier() {
        let model = build(&TrayFacts {
            queued: 2,
            running: 1,
            update_available: true,
            paused: None,
            window_visible: false,
            portable: true,
            ..TrayFacts::default()
        });
        let mut ids: Vec<&str> = model
            .items
            .iter()
            .filter(|item| item.action.is_some())
            .map(|item| item.id.as_str())
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(model
            .items
            .iter()
            .all(|item| !item.id.is_empty() || item.kind == TrayItemKind::Separator));
    }
}
