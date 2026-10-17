//! The preferences surface: what the user configures once and then forgets.
//!
//! A preference is only worth having if something reads it, so every control
//! here names the code path that consumes it, and a control that has no such
//! path is shown closed with the reason rather than stored and ignored. The
//! consumers are listed in each tab's module documentation and are all outside
//! this module: the media work stays in the worker, and the pipeline stays in
//! the core.
//!
//! The record lives in the one settings file through the core settings API that
//! the theme, the direction, and the recent source list already use. There is no
//! second store, so a preference cannot disagree with the file it is written to.
//!
//! Reading is version-aware in two places. The document itself is stamped with
//! the version of the build that wrote it (see `vise_core::AppSettings`), and
//! this record carries its own number, because a record added by a newer build
//! has to be left alone rather than half-understood. An older record is migrated
//! forward: the output folder older builds kept at the top level of the document
//! becomes the folder this tab edits, so a user who already chose one sees it
//! rather than an empty field.
//!
//! Validation lives here, not in the page. A range check, a folder check, or a
//! pair of choices that cannot both be true is refused with a message naming the
//! field, and nothing is written until every field in the change has passed.

pub mod advanced;
pub mod general;
pub mod hardware;
pub mod output;
pub mod updates;
pub mod whendone;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vise_core::{load_settings, save_settings, AppSettings, SETTINGS_UNVERSIONED};
use vise_ipc::portable::PortableSettings;

use crate::tabs::common::{ChoiceGroupViewModel, NumberViewModel, TextViewModel, ToggleViewModel};

pub use output::CollisionPreference;

/// Member of the settings file that holds these preferences.
pub const PREFERENCES_KEY: &str = "preferences";

/// Version of this record.
///
/// Separate from the document's version on purpose: the document gains members
/// for reasons that have nothing to do with what a control means, while this
/// number only moves when a preference changes shape. A record stamped with a
/// higher number than this build understands is left untouched.
pub const PREFERENCES_VERSION: u32 = 1;

/// Identifier of the preferences surface.
pub const SURFACE: &str = "preferences";

/// Why a stored record cannot be used as it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RecordState {
    /// Read as it was written by this build.
    #[default]
    Current,
    /// There was no record, so every preference is at its default.
    Absent,
    /// The record was written by an older build and has been migrated.
    Migrated,
    /// The record was written by a newer build and is left exactly as found.
    FromTheFuture,
    /// The record was not readable as one, so every preference is at its default.
    Unreadable,
}

impl RecordState {
    /// Whether the record in the file may be written over.
    ///
    /// A record from a newer build is somebody else's data: this build cannot
    /// say what it means, so writing over it would throw away settings that a
    /// downgrade and an upgrade round trip would otherwise have preserved.
    pub fn allows_writing(self) -> bool {
        !matches!(self, RecordState::FromTheFuture)
    }

    /// One line saying where these preferences came from.
    pub fn note(self, version: u32) -> String {
        match self {
            RecordState::Current => format!(
                "Read from the settings file, which is written by this build (record version {version})."
            ),
            RecordState::Absent => {
                "No preferences have been stored yet, so everything is at its default.".to_string()
            }
            RecordState::Migrated => format!(
                "These preferences were written by an older build and have been brought forward to version {version}."
            ),
            RecordState::FromTheFuture => format!(
                "These preferences were written by a newer build (record version {version}). They are shown as that build left them and are not changed, because this build cannot be sure what they mean."
            ),
            RecordState::Unreadable => {
                "The stored preferences could not be read, so everything is at its default. The stored record has been left alone.".to_string()
            }
        }
    }
}

/// Everything one preferences surface holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreferencesSettings {
    /// Version of this record.
    #[serde(default = "current_preferences_version")]
    pub version: u32,
    /// How sources are opened and scanned.
    #[serde(default)]
    pub general: general::GeneralSettings,
    /// Where finished files are written and what happens on a name collision.
    #[serde(default)]
    pub output: output::OutputSettings,
    /// Process isolation, logging, and scheduling.
    #[serde(default)]
    pub advanced: advanced::AdvancedSettings,
    /// Hardware acceleration and the device decoding runs on.
    #[serde(default)]
    pub hardware: hardware::HardwareSettings,
    /// How often the application looks for a newer release.
    #[serde(default)]
    pub updates: updates::UpdateSettings,
    /// What happens when the queue finishes.
    #[serde(default)]
    pub when_done: whendone::WhenDoneSettings,
}

/// The version a record written by this build carries.
fn current_preferences_version() -> u32 {
    PREFERENCES_VERSION
}

impl Default for PreferencesSettings {
    fn default() -> Self {
        Self {
            version: PREFERENCES_VERSION,
            general: general::GeneralSettings::new(),
            output: output::OutputSettings::new(),
            advanced: advanced::AdvancedSettings::new(),
            hardware: hardware::HardwareSettings::new(),
            updates: updates::UpdateSettings::new(),
            when_done: whendone::WhenDoneSettings::new(),
        }
    }
}

impl PreferencesSettings {
    /// The preferences a machine that has never chosen anything has.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every field identifier this surface accepts, with the tab it belongs to.
    ///
    /// One list, so a field name the page sends cannot be one this build has
    /// never heard of without the error saying so, and so the tests can check
    /// that every control the page is given has a name that reaches a setter.
    pub fn fields() -> &'static [(&'static str, &'static str)] {
        &[
            (general::PREVIEW_COUNT, "general"),
            (general::KEEP_DUPLICATE_TITLES, "general"),
            (general::DEFAULT_RANGE_MODE, "general"),
            (general::SCAN_ON_OPEN, "general"),
            (output::FOLDER, "output"),
            (output::NAME_PATTERN, "output"),
            (output::OVERWRITE, "output"),
            (output::COLLISION, "output"),
            (advanced::PROCESS_ISOLATION, "advanced"),
            (advanced::LOG_LEVEL, "advanced"),
            (advanced::PROCESS_PRIORITY, "advanced"),
            (hardware::ACCELERATION, "hardware"),
            (hardware::DECODE_DEVICE, "hardware"),
            (updates::FREQUENCY, "updates"),
            (updates::CHANNEL, "updates"),
            (whendone::ACTION, "whendone"),
            (whendone::DELAY_SECONDS, "whendone"),
        ]
    }

    /// Read one field from the record, as the code paths that consume it need it.
    ///
    /// The consumers do not read this record; they read these four values, which
    /// is what keeps a preference from having two meanings.
    pub fn to_worker_init(&self) -> vise_ipc::contract::InitCommand {
        vise_ipc::contract::InitCommand {
            log_verbosity: self.advanced.log_level.rank(),
            enable_hardware_acceleration: self.hardware.uses_hardware(),
            hw_decode: self.hardware.decode_preference(),
            hw_decode_device: self.hardware.decode_device.clone(),
            process_priority: self.advanced.process_priority.level(),
            ..Default::default()
        }
    }

    /// Whether a source should be scanned the moment it is opened.
    pub fn scans_on_open(&self) -> bool {
        self.general.scan_on_open
    }

    /// How many stills a scan should extract when the page does not say.
    pub fn preview_count(&self) -> u32 {
        self.general.preview_count
    }
}

/// What the record in the file was, and what became of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordNote {
    /// Which of the outcomes this is.
    pub state: RecordState,
    /// Version the record declared, or zero when it declared none.
    pub written_version: u32,
    /// Version the record is on now.
    pub version: u32,
    /// Version the settings document declared, or zero when it declared none.
    pub document_version: u32,
    /// Whether the document itself needs migrating.
    pub document_needs_migration: bool,
    /// Whether the document was written by a newer build.
    pub document_from_the_future: bool,
    /// Where the file is, in the words the page shows.
    pub location: String,
    /// One line saying what happened.
    pub note: String,
    /// Whether a change may be written back.
    pub can_save: bool,
    /// Why it may not, when it may not.
    pub blocked_reason: String,
}

impl RecordNote {
    /// A note for a file that could not be read at all.
    fn unreadable_file(location: String, problem: &str) -> Self {
        Self {
            state: RecordState::Unreadable,
            written_version: 0,
            version: PREFERENCES_VERSION,
            document_version: 0,
            document_needs_migration: false,
            document_from_the_future: false,
            location,
            note: format!(
                "The settings file could not be read ({problem}), so every preference is at its default."
            ),
            can_save: true,
            blocked_reason: String::new(),
        }
    }

    /// The note, as the page renders it.
    pub fn view(&self) -> RecordNoteViewModel {
        RecordNoteViewModel {
            state: self.state,
            state_label: record_state_label(self.state).to_string(),
            note: self.note.clone(),
            location: self.location.clone(),
            version: self.version,
            written_version: self.written_version,
            document_version: self.document_version,
            document_needs_migration: self.document_needs_migration,
            document_from_the_future: self.document_from_the_future,
            migrated: matches!(self.state, RecordState::Migrated),
            can_save: self.can_save,
            blocked_reason: self.blocked_reason.clone(),
        }
    }
}

/// The note, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordNoteViewModel {
    /// Which of the outcomes this is.
    pub state: RecordState,
    /// Label for the outcome.
    pub state_label: String,
    /// One line saying what happened.
    pub note: String,
    /// Where the settings live.
    pub location: String,
    /// Version the record is on.
    pub version: u32,
    /// Version the record declared, or zero.
    pub written_version: u32,
    /// Version the settings document declared, or zero.
    pub document_version: u32,
    /// Whether the document itself needs migrating.
    pub document_needs_migration: bool,
    /// Whether the document was written by a newer build.
    pub document_from_the_future: bool,
    /// Whether the record was brought forward.
    pub migrated: bool,
    /// Whether a change may be written.
    pub can_save: bool,
    /// Why it may not, when it may not.
    pub blocked_reason: String,
}

/// Label for a record outcome.
pub fn record_state_label(state: RecordState) -> &'static str {
    match state {
        RecordState::Current => "Saved",
        RecordState::Absent => "Not saved yet",
        RecordState::Migrated => "Migrated",
        RecordState::FromTheFuture => "From a newer version",
        RecordState::Unreadable => "Not readable",
    }
}

/// A read-only fact about the machine, as the page renders it.
///
/// Detection reports what is present rather than what should be done about it,
/// so these rows carry a state instead of a control: a list of encoders the
/// build found is information, and offering a switch beside each one would be
/// offering a choice nothing reads. The same holds for a value this surface has
/// worked out from a preference, such as the verbosity rank a log level becomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactViewModel {
    /// Stable identifier the page uses as a row key.
    pub id: String,
    /// What the row is about.
    pub label: String,
    /// What was found.
    pub value: String,
    /// `present`, `missing`, or `unknown`.
    pub state: String,
    /// Whether the thing is there.
    pub available: bool,
    /// Why it is not, when it is not.
    pub reason: String,
}

impl FactViewModel {
    /// A row for something that was found.
    pub fn present(id: &str, label: &str, value: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            value: value.into(),
            state: "present".to_string(),
            available: true,
            reason: String::new(),
        }
    }
    /// A row for something that was not found.
    pub fn missing(id: &str, label: &str, reason: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            value: "Not found".to_string(),
            state: "missing".to_string(),
            available: false,
            reason: reason.into(),
        }
    }

    /// A row for something detection could not answer.
    pub fn unknown(id: &str, label: &str, reason: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            value: "Not checked".to_string(),
            state: "unknown".to_string(),
            available: false,
            reason: reason.into(),
        }
    }
}

/// A control that does one thing rather than holding a value.
///
/// Restoring the defaults, taking a restore back, looking for the media tools
/// again, and looking for a release are all buttons. They are described here
/// rather than as empty fields, because a field that cannot be typed into and
/// cannot be filled in is a control that lies about what it is; a button carries
/// its label and its reason and nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionViewModel {
    /// Stable identifier the page uses to route the press.
    pub id: String,
    /// Label on the control.
    pub label: String,
    /// One line explaining what pressing it does.
    pub hint: String,
    /// Whether it can be pressed.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl ActionViewModel {
    /// A button that is on offer.
    pub fn new(id: &str, label: &str, hint: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            available: true,
            reason: String::new(),
        }
    }

    /// The button, closed with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// Identifier of the action that restores the defaults.
pub const RESET_ACTION: &str = "preferencesReset";
/// Identifier of the action that takes a restore back.
pub const UNDO_ACTION: &str = "preferencesUndo";

/// Every action control this surface offers.
///
/// Written down once so the page's routing table and the window's descriptions
/// cannot drift apart: a name on offer here that the page does not route would be
/// a button that does nothing when pressed.
pub const PREFERENCE_ACTIONS: [(&str, &str, &str); 4] = [
    (
        RESET_ACTION,
        "Restore the defaults",
        "Put every preference on every tab back to how this build ships.",
    ),
    (
        UNDO_ACTION,
        "Put my preferences back",
        "Undo the restore that was just done.",
    ),
    (
        "preferencesRecheckHardware",
        "Look for the media tools again",
        "Ask the media tools what this machine can decode and encode with.",
    ),
    (
        "preferencesCheckNow",
        "Look for a release now",
        "Ask the release server what the newest build is.",
    ),
];

/// What the action named `id` does, in one line.
pub fn action_hint(id: &str) -> &'static str {
    PREFERENCE_ACTIONS
        .iter()
        .find(|(action, _, _)| *action == id)
        .map(|(_, _, hint)| *hint)
        .unwrap_or("")
}

/// One tab of the preferences surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceTabViewModel {
    /// Stable identifier the page uses to pick the panel.
    pub id: String,
    /// Label on the tab.
    pub label: String,
    /// One line describing what the tab holds.
    pub hint: String,
    /// The single-choice controls on this tab.
    pub choices: Vec<ChoiceGroupViewModel>,
    /// The switches on this tab.
    pub toggles: Vec<ToggleViewModel>,
    /// The numbers on this tab.
    pub numbers: Vec<NumberViewModel>,
    /// The text fields on this tab.
    pub texts: Vec<TextViewModel>,
    /// The read-only facts on this tab.
    pub facts: Vec<FactViewModel>,
    /// The buttons on this tab.
    pub actions: Vec<ActionViewModel>,
    /// One line per group of facts.
    pub group_titles: Vec<String>,
    /// Anything that has to be fixed on this tab.
    pub problems: Vec<String>,
    /// Whether the tab is free of problems.
    pub ready: bool,
}

impl PreferenceTabViewModel {
    /// An empty tab, so a tab with nothing on it still renders.
    pub fn empty(id: &str, label: &str, hint: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            choices: Vec::new(),
            toggles: Vec::new(),
            numbers: Vec::new(),
            texts: Vec::new(),
            facts: Vec::new(),
            actions: Vec::new(),
            group_titles: Vec::new(),
            problems: Vec::new(),
            ready: true,
        }
    }

    /// Close every control on this tab with one reason.
    ///
    /// Used when the record is not this build's to change. Closing the controls
    /// rather than only refusing the write is the point: a page that can still
    /// turn a switch on and be told no is a page that looks broken, whereas a
    /// control that is closed and says why is an answer.
    pub fn closed_everything(mut self, reason: &str) -> Self {
        for choice in &mut self.choices {
            choice.available = false;
            choice.reason = reason.to_string();
            for option in &mut choice.options {
                option.available = false;
                option.reason = reason.to_string();
            }
        }
        for toggle in &mut self.toggles {
            toggle.available = false;
            toggle.reason = reason.to_string();
        }
        for number in &mut self.numbers {
            number.available = false;
            number.reason = reason.to_string();
        }
        for text in &mut self.texts {
            text.available = false;
            text.reason = reason.to_string();
        }
        for action in &mut self.actions {
            action.available = false;
            action.reason = reason.to_string();
        }
        self.problems.push(reason.to_string());
        self.ready = false;
        self
    }
}

/// The label on the action named `id`, from the one list.
///
/// The two buttons outside the tabs are described by the same table the buttons
/// inside them are, so the wording of "restore the defaults" is written down once
/// rather than in the view model and in the page.
pub fn action_label(id: &str) -> &'static str {
    PREFERENCE_ACTIONS
        .iter()
        .find(|(action, _, _)| *action == id)
        .map(|(_, label, _)| *label)
        .unwrap_or("Do it")
}

/// What the reset control is offering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetViewModel {
    /// Label on the restore control.
    pub label: String,
    /// What happens if it is pressed, in one line.
    pub effect: String,
    /// Whether the defaults could be restored.
    pub available: bool,
    /// Why they could not, when they could not.
    pub reason: String,
    /// Whether a confirmation is required before anything is changed.
    ///
    /// Always true. The answer is here rather than in the page so a control that
    /// skipped it could not be drawn from this description.
    pub confirms: bool,
    /// Whether restoring can be taken back.
    pub can_undo: bool,
    /// Label on the take-back control.
    pub undo_label: String,
    /// What the take-back does, in one line.
    pub undo_effect: String,
    /// Whether anything is not at its default.
    pub changed: bool,
    /// The fields that are not at their default, named for the status line.
    pub changed_fields: Vec<String>,
    /// How many preferences are not at their default.
    pub changed_count: usize,
    /// One line saying what the control will do.
    pub summary: String,
    /// One line saying what happened last, for a screen reader to read out.
    pub announcement: String,
}

/// What the page is told about the whole surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesViewModel {
    /// The tab strip, in order.
    pub tabs: Vec<PreferenceTabDescriptor>,
    /// Which tab the page shows.
    pub active: String,
    /// The general tab.
    pub general: PreferenceTabViewModel,
    /// The output tab.
    pub output: PreferenceTabViewModel,
    /// The advanced tab.
    pub advanced: PreferenceTabViewModel,
    /// The hardware tab.
    pub hardware: PreferenceTabViewModel,
    /// The updates tab.
    pub updates: PreferenceTabViewModel,
    /// The when-done tab.
    pub when_done: PreferenceTabViewModel,
    /// The reset control.
    pub reset: ResetViewModel,
    /// Where the preferences came from.
    pub storage: RecordNoteViewModel,
    /// Anything that has to be fixed anywhere on the surface.
    pub problems: Vec<String>,
    /// Whether the surface is free of problems.
    pub ready: bool,
}

/// One entry in the preferences tab strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceTabDescriptor {
    /// Stable identifier the page uses to pick the panel.
    pub id: String,
    /// Label on the tab.
    pub label: String,
    /// One line the page shows as the tab's hint.
    pub hint: String,
    /// Whether anything on the tab needs fixing.
    pub needs_attention: bool,
}

/// The tab order, with the label and the hint the strip shows.
pub const PREFERENCE_TABS: [(&str, &str, &str); 6] = [
    (
        "general",
        "General",
        "How sources are opened, how they are scanned, and what a scan leaves behind.",
    ),
    (
        "output",
        "Output",
        "Where finished files are written, how they are named, and what a name collision does.",
    ),
    (
        "advanced",
        "Advanced",
        "Process isolation, how much is logged, and how the worker is scheduled.",
    ),
    (
        "hardware",
        "Hardware",
        "Whether the encoder uses the graphics card, and what decoding runs on.",
    ),
    (
        "updates",
        "Updates",
        "How often a newer release is looked for, and which one is followed.",
    ),
    (
        "whendone",
        "When done",
        "What happens once the queue has nothing left to do.",
    ),
];

/// The state of the preferences surface.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    /// What the user has chosen.
    settings: PreferencesSettings,
    /// What the file said about those choices.
    note: RecordNote,
    /// The preferences as they were before the last reset.
    ///
    /// Reset is destructive, so it asks first and this is what makes the answer
    /// afterwards recoverable: the previous record is kept for exactly one reset,
    /// and the view model says whether a take-back is on offer.
    undo: Option<PreferencesSettings>,
    /// What hardware detection found the last time it was asked.
    hardware: hardware::HardwareAvailability,
    /// Where the settings live.
    settings_path: PathBuf,
    /// Whether the portable settings file has taken isolation away.
    isolation_block: Option<String>,
    /// Which tab the page is on.
    active: String,
    /// What the last reset or take-back did, in one line.
    ///
    /// Held rather than derived, because the interesting sentence is the one
    /// about what just happened; a line recomputed from the record would say the
    /// record is at its defaults, which is true and useless.
    announcement: String,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            settings: PreferencesSettings::new(),
            note: RecordNote {
                state: RecordState::Absent,
                written_version: 0,
                version: PREFERENCES_VERSION,
                document_version: 0,
                document_needs_migration: false,
                document_from_the_future: false,
                location: "not saved anywhere yet".to_string(),
                note: RecordState::Absent.note(PREFERENCES_VERSION),
                can_save: true,
                blocked_reason: String::new(),
            },
            undo: None,
            hardware: hardware::HardwareAvailability::unchecked(),
            settings_path: PathBuf::new(),
            isolation_block: None,
            active: PREFERENCE_TABS[0].0.to_string(),
            announcement: "Nothing has been restored yet.".to_string(),
        }
    }
}

impl PreferencesState {
    /// Read the preferences, migrate what needs migrating, and take the first
    /// look at the machine.
    pub fn load(settings_path: &Path, portable: &PortableSettings) -> Self {
        let location = settings_path.display().to_string();
        let mut state = Self {
            settings_path: settings_path.to_path_buf(),
            isolation_block: isolation_block(portable),
            ..Self::default()
        };
        match load_settings(settings_path) {
            Ok(settings) => {
                let (settings, note) = read_record(&settings, &location);
                state.settings = settings;
                state.note = note;
            }
            Err(problem) => {
                state.note = RecordNote::unreadable_file(location, &problem.to_string());
            }
        }
        state
    }

    /// Record what hardware detection found.
    ///
    /// Detection is a pair of subprocess calls, so the answer is held rather
    /// than asked for on every read: the page can re-ask through a command, and
    /// everything between those two asks paints the same list.
    pub fn observe_hardware(&mut self, capabilities: &vise_core::HardwareCapabilities) {
        self.hardware = hardware::HardwareAvailability::of(capabilities);
    }

    /// Record what the portable settings file decided about process isolation.
    pub fn observe_portable(&mut self, portable: &PortableSettings) {
        self.isolation_block = isolation_block(portable);
    }

    /// The preferences as they stand.
    pub fn settings(&self) -> &PreferencesSettings {
        &self.settings
    }

    /// Whether `tab` names a tab this surface offers.
    pub fn is_tab(tab: &str) -> bool {
        tab_is_offered(tab)
    }

    /// Show `tab`, falling back to the first when the name is not one of ours.
    pub fn set_active(&mut self, tab: &str) -> Result<(), String> {
        let wanted = tab.trim();
        if !Self::is_tab(wanted) {
            return Err(format!(
                "'{wanted}' is not a preferences tab; choose {}.",
                tab_names()
            ));
        }
        self.active = wanted.to_string();
        Ok(())
    }

    /// Apply one change, validate it, and keep it.
    ///
    /// The value arrives as JSON because the page has one control per kind and
    /// this is the one place that knows which kind each field is. A value of the
    /// wrong shape is refused by name rather than coerced, so a page that sends
    /// the wrong field cannot quietly set the wrong preference.
    pub fn set(&mut self, field: &str, value: &serde_json::Value) -> Result<(), String> {
        if !self.note.can_save {
            return Err(self.note.blocked_reason.clone());
        }
        let field = field.trim();
        if !PreferencesSettings::fields()
            .iter()
            .any(|(name, _)| *name == field)
        {
            return Err(format!(
                "'{field}' is not a preference this build offers; choose {}.",
                field_names()
            ));
        }
        let mut next = self.settings.clone();
        match field {
            general::PREVIEW_COUNT => {
                next.general.preview_count = general::read_preview_count(value)?
            }
            general::KEEP_DUPLICATE_TITLES => {
                next.general.keep_duplicate_titles = want_switch(field, value)?
            }
            general::DEFAULT_RANGE_MODE => {
                next.general.default_range_mode = general::RangePreference::read(value)?
            }
            general::SCAN_ON_OPEN => next.general.scan_on_open = want_switch(field, value)?,
            output::FOLDER => next.output.folder = output::read_folder(value)?,
            output::NAME_PATTERN => next.output.name_pattern = output::read_pattern(value)?,
            output::OVERWRITE => next.output.overwrite = want_switch(field, value)?,
            output::COLLISION => next.output.collision = output::CollisionPreference::read(value)?,
            advanced::PROCESS_ISOLATION => {
                next.advanced.process_isolation = want_switch(field, value)?
            }
            advanced::LOG_LEVEL => next.advanced.log_level = advanced::read_log_level(value)?,
            advanced::PROCESS_PRIORITY => {
                next.advanced.process_priority = advanced::PriorityPreference::read(value)?
            }
            hardware::ACCELERATION => {
                next.hardware.acceleration = hardware::AccelerationPreference::read(value)?
            }
            hardware::DECODE_DEVICE => {
                let device = hardware::read_device(value)?;
                // A name is only a choice if detection found it. Storing one the
                // machine does not have would be a preference nothing reads, and
                // the media tools would quietly fall back to software.
                let reason = self.hardware.decode_device_reason(&device);
                if !reason.is_empty() {
                    return Err(reason);
                }
                next.hardware.decode_device = device;
            }
            updates::FREQUENCY => next.updates.frequency = updates::CheckFrequency::read(value)?,
            updates::CHANNEL => next.updates.channel = updates::UpdateChannel::read(value)?,
            whendone::ACTION => next.when_done.action = whendone::WhenDoneAction::read(value)?,
            whendone::DELAY_SECONDS => next.when_done.delay_seconds = whendone::read_delay(value)?,
            _ => {
                return Err(format!("'{field}' is not a preference this build offers."));
            }
        }
        // The rules that span fields run before anything is written, so a
        // combination that cannot be honoured never reaches the file.
        output::settle_collision(&mut next.output);
        // A change is refused when it *causes* a problem, not merely when a
        // problem exists: an output folder that has not been chosen yet is a
        // problem the Output tab is already showing, and it must not make every
        // other preference on every other tab unchangeable.
        let already = validate(&self.settings);
        if let Some(problem) = validate(&next)
            .into_iter()
            .find(|problem| !already.contains(problem))
        {
            return Err(problem);
        }
        self.settings = next;
        self.persist()
    }

    /// Put every preference back to its default and keep what was there.
    ///
    /// Returns what changed so the window can say it out loud rather than leaving
    /// the user to notice. The previous record is held so the answer can be taken
    /// back, which is what makes a destructive control defensible here. What
    /// changed is also remembered, because the status line the page reads after
    /// the reset has to say what was just done rather than what the record now
    /// looks like, and those two are not the same sentence.
    pub fn reset(&mut self) -> Vec<String> {
        let changed = self.changed_fields();
        self.undo = Some(std::mem::replace(
            &mut self.settings,
            PreferencesSettings::new(),
        ));
        // A reset restores the defaults of this build, so a record written by a
        // newer one is still not something to write over; the caller is told the
        // preferences in memory are the defaults either way.
        let _ = self.persist();
        self.announcement = if changed.is_empty() {
            "Nothing to restore.".to_string()
        } else {
            format!("Restored the defaults. Changed: {}.", changed.join(", "))
        };
        changed
    }

    /// Put back the preferences a reset replaced.
    pub fn undo_reset(&mut self) -> Result<Vec<String>, String> {
        let Some(previous) = self.undo.take() else {
            return Err("There is nothing to take back: no reset has been done.".to_string());
        };
        self.settings = previous;
        self.persist()?;
        self.announcement =
            "Put your preferences back. They are as they were before the restore.".to_string();
        Ok(self.changed_fields())
    }

    /// Whether a reset can be taken back.
    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    /// The fields that are not at their default, as a user would name them.
    pub fn changed_fields(&self) -> Vec<String> {
        let defaults = PreferencesSettings::new();
        let mut changed = Vec::new();
        if self.settings.general != defaults.general {
            changed.push("General".to_string());
        }
        if self.settings.output != defaults.output {
            changed.push("Output".to_string());
        }
        if self.settings.advanced != defaults.advanced {
            changed.push("Advanced".to_string());
        }
        if self.settings.hardware != defaults.hardware {
            changed.push("Hardware".to_string());
        }
        if self.settings.updates != defaults.updates {
            changed.push("Updates".to_string());
        }
        if self.settings.when_done != defaults.when_done {
            changed.push("When done".to_string());
        }
        changed
    }

    /// Write the record back into the settings file.
    fn persist(&mut self) -> Result<(), String> {
        if !self.note.state.allows_writing() {
            return Ok(());
        }
        let mut settings = load_settings(&self.settings_path).unwrap_or_default();
        let value = serde_json::to_value(&self.settings)
            .map_err(|e| format!("the preferences could not be encoded: {e}"))?;
        settings.extra.insert(PREFERENCES_KEY.to_string(), value);
        // The document carries its own answer to whether the folder to write into
        // is this one, and other parts of the application already read it there,
        // so the preference keeps it in step rather than leaving two values. An
        // empty folder is mirrored as an absent one rather than as an empty
        // string: no folder has been chosen yet, and a blank string in the
        // document reads as a folder that was chosen and is wrong.
        let folder = self.settings.output.folder.trim();
        settings.output_directory = if folder.is_empty() {
            None
        } else {
            Some(folder.to_string())
        };
        save_settings(&self.settings_path, &settings)
            .map_err(|e| format!("the settings could not be saved: {e}"))?;
        if self.note.state != RecordState::Current {
            self.note.state = RecordState::Current;
            self.note.written_version = PREFERENCES_VERSION;
            self.note.version = PREFERENCES_VERSION;
            self.note.note = RecordState::Current.note(PREFERENCES_VERSION);
        }
        if self.note.document_needs_migration {
            // Saving wrote the document out at the current version, so the
            // question has been answered by the write rather than by a guess.
            self.note.document_version = settings.version;
            self.note.document_needs_migration = false;
        }
        Ok(())
    }

    /// Everything the page paints, decided once.
    pub fn view(&self) -> PreferencesViewModel {
        let availability = IsolationAvailability {
            blocked: self.isolation_block.clone(),
        };
        let general = general::tab_view(&self.settings.general);
        let output = output::tab_view(&self.settings.output);
        let advanced_tab = advanced::tab_view(&self.settings.advanced, &availability);
        let hardware = hardware::tab_view(&self.settings.hardware, &self.hardware);
        let updates = updates::tab_view(&self.settings.updates, updates::today());
        let when_done = whendone::tab_view(&self.settings.when_done);
        let mut problems = Vec::new();
        // A record this build cannot read is shown as the newer build left it and
        // every control is closed, because a control that can be used and then
        // refused is worse than one that says it is not on offer.
        let block = if self.note.state.allows_writing() {
            None
        } else {
            Some(self.note.blocked_reason.clone())
        };
        let (general, output, advanced_tab, hardware, updates, when_done) = match block.as_deref() {
            Some(reason) => (
                general.closed_everything(reason),
                output.closed_everything(reason),
                advanced_tab.closed_everything(reason),
                hardware.closed_everything(reason),
                updates.closed_everything(reason),
                when_done.closed_everything(reason),
            ),
            None => (general, output, advanced_tab, hardware, updates, when_done),
        };
        for (_, tab) in [
            ("general", &general),
            ("output", &output),
            ("advanced", &advanced_tab),
            ("hardware", &hardware),
            ("updates", &updates),
            ("whendone", &when_done),
        ] {
            problems.extend(tab.problems.iter().cloned());
        }
        problems.extend(self.note.problems());
        let ready_of = |id: &str| {
            [
                &general,
                &output,
                &advanced_tab,
                &hardware,
                &updates,
                &when_done,
            ]
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.ready)
            .unwrap_or(true)
        };
        let descriptors = PREFERENCE_TABS
            .iter()
            .map(|(id, label, hint)| PreferenceTabDescriptor {
                id: (*id).to_string(),
                label: (*label).to_string(),
                hint: (*hint).to_string(),
                needs_attention: !ready_of(id),
            })
            .collect();
        let changed = self.changed_fields();
        let active = if Self::is_tab(&self.active) {
            self.active.clone()
        } else {
            PREFERENCE_TABS[0].0.to_string()
        };
        PreferencesViewModel {
            tabs: descriptors,
            active,
            reset: ResetViewModel {
                label: action_label(RESET_ACTION).to_string(),
                effect: action_hint(RESET_ACTION).to_string(),
                available: self.note.state.allows_writing(),
                reason: if self.note.state.allows_writing() {
                    String::new()
                } else {
                    self.note.blocked_reason.clone()
                },
                // A reset throws away every choice on this surface, so it asks
                // through the same countdown every other destructive action uses.
                confirms: true,
                can_undo: self.can_undo(),
                undo_label: action_label(UNDO_ACTION).to_string(),
                undo_effect: action_hint(UNDO_ACTION).to_string(),
                changed: !changed.is_empty(),
                changed_count: changed.len(),
                changed_fields: changed.clone(),
                summary: if changed.is_empty() {
                    "Every preference is at its default.".to_string()
                } else {
                    format!(
                        "{} of the 6 groups are not at their default: {}.",
                        changed.len(),
                        changed.join(", ")
                    )
                },
                announcement: self.announcement.clone(),
            },
            general,
            output,
            advanced: advanced_tab,
            hardware,
            updates,
            when_done,
            storage: self.note.view(),
            ready: problems.is_empty(),
            problems,
        }
    }
}

/// The note, for the problems list on the surface.
impl RecordNote {
    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.can_save {
            problems.push(self.blocked_reason.clone());
        }
        if self.document_from_the_future {
            problems.push(format!(
                "The settings file is written by a newer build (document version {}); it is read but not changed.",
                self.document_version
            ));
        }
        problems
    }
}

/// Whether process isolation may be chosen, and why not when it may not.
pub struct IsolationAvailability {
    /// What has taken the choice away, when something has.
    pub blocked: Option<String>,
}

/// Why the portable settings file has taken isolation away, when it has.
fn isolation_block(portable: &PortableSettings) -> Option<String> {
    if portable.should_isolate() {
        return None;
    }
    Some(portable.isolation_disabled_reason())
}

/// Read the record out of a settings document.
///
/// Three outcomes matter and they are different: a record this build wrote, a
/// record it has to bring forward, and a record it must not touch. The last one
/// is why the version is read from the raw value rather than from the decoded
/// record — a version too high to decode is exactly the case that has to be
/// recognised.
fn read_record(settings: &AppSettings, location: &str) -> (PreferencesSettings, RecordNote) {
    let raw = settings.extra.get(PREFERENCES_KEY).cloned();
    let mut note = RecordNote {
        state: RecordState::Absent,
        written_version: 0,
        version: PREFERENCES_VERSION,
        document_version: settings.written_version,
        document_needs_migration: settings.needs_migration(),
        document_from_the_future: settings.from_the_future(),
        location: location.to_string(),
        note: RecordState::Absent.note(PREFERENCES_VERSION),
        can_save: true,
        blocked_reason: String::new(),
    };
    let Some(raw) = raw else {
        // There is no record, so the one answer an older document can still
        // hold is the folder it kept at the top level. Adopting it here is the
        // migration that matters: a user who chose a folder before this surface
        // existed sees it rather than an empty field.
        let mut preferences = PreferencesSettings::new();
        if let Some(folder) = settings
            .output_directory
            .as_ref()
            .filter(|f| !f.trim().is_empty())
        {
            preferences.output.folder = folder.clone();
            note.state = RecordState::Migrated;
            note.written_version = SETTINGS_UNVERSIONED;
            note.note = format!(
                "No preferences have been stored yet, so the defaults apply. The output folder this document already named ({folder}) has been adopted as the folder in the Output tab."
            );
        }
        return (preferences, note);
    };

    let declared = raw
        .as_object()
        .and_then(|members| members.get("version"))
        .and_then(serde_json::Value::as_u64)
        .map(|version| version.min(u64::from(u32::MAX)) as u32);

    if let Some(version) = declared.filter(|version| *version > PREFERENCES_VERSION) {
        note.state = RecordState::FromTheFuture;
        note.written_version = version;
        note.can_save = false;
        note.blocked_reason = format!(
            "These preferences were written by a newer build (record version {version}), so this build will not change them."
        );
        note.note = RecordState::FromTheFuture.note(version);
        // The record is shown as the newer build left it. A build that cannot
        // read a record does not get to pretend the defaults are what was
        // chosen, so the version is what is reported and every control is closed
        // with the reason.
        return (PreferencesSettings::new(), note);
    }

    let mut preferences: PreferencesSettings = match serde_json::from_value(raw.clone()) {
        Ok(preferences) => preferences,
        Err(problem) => {
            note.state = RecordState::Unreadable;
            note.note = format!(
                "The stored preferences could not be read ({problem}), so every preference is at its default. The stored record has been left alone."
            );
            return (PreferencesSettings::new(), note);
        }
    };
    if declared.is_none() || declared != Some(PREFERENCES_VERSION) {
        // An older record is brought forward. Its members keep their defaults
        // where they are missing, so migrating costs nothing that was there.
        note.state = RecordState::Migrated;
        note.written_version = declared.unwrap_or(0);
        note.note = RecordState::Migrated.note(PREFERENCES_VERSION);
    } else {
        note.state = RecordState::Current;
        note.written_version = PREFERENCES_VERSION;
        note.note = RecordState::Current.note(PREFERENCES_VERSION);
    }
    // The document's folder and the record's folder are one answer, so a file
    // where they disagree is resolved in favour of the record: that is the one
    // the Output tab edits.
    if preferences.output.folder.trim().is_empty() {
        if let Some(folder) = settings
            .output_directory
            .as_ref()
            .filter(|folder| !folder.trim().is_empty())
        {
            preferences.output.folder = folder.clone();
        }
    }
    preferences.version = PREFERENCES_VERSION;
    (preferences, note)
}

/// The problems a record has, whatever is stored.
///
/// Every rule runs here as well as in each setter, so a record that arrived
/// from a file, a reset, or a take-back is checked the same way a typed change
/// is. A preference that cannot be honoured is reported rather than acted on.
pub fn validate(settings: &PreferencesSettings) -> Vec<String> {
    let mut problems = Vec::new();
    problems.extend(general::validate(&settings.general));
    problems.extend(output::validate(&settings.output));
    problems.extend(advanced::validate(&settings.advanced));
    problems.extend(hardware::validate(&settings.hardware));
    problems.extend(whendone::validate(&settings.when_done));
    problems
}

/// A switch value, or the reason the value is not one.
fn want_switch(field: &str, value: &serde_json::Value) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| {
        format!(
            "{} has to be on or off, not {}.",
            field_label(field),
            describe(value)
        )
    })
}

/// The names of every tab, for an error that has to list them.
fn tab_names() -> String {
    PREFERENCE_TABS
        .iter()
        .map(|(id, label, _)| format!("{label} ({id})"))
        .collect::<Vec<String>>()
        .join(", ")
}

/// The names of every field, for an error that has to list them.
fn field_names() -> String {
    PreferencesSettings::fields()
        .iter()
        .map(|(field, _)| *field)
        .collect::<Vec<&str>>()
        .join(", ")
}

/// The label a field is shown with, which is what an error names.
///
/// Errors name what the user is looking at rather than the name the page sends,
/// because the page's name is an identifier and an identifier in a sentence is
/// not help.
fn field_label(field: &str) -> &'static str {
    match field {
        general::PREVIEW_COUNT => "Preview frames per title",
        general::KEEP_DUPLICATE_TITLES => "Keep duplicate titles",
        general::DEFAULT_RANGE_MODE => "Default range",
        general::SCAN_ON_OPEN => "Scan when a source is opened",
        output::FOLDER => "Output folder",
        output::NAME_PATTERN => "File name pattern",
        output::OVERWRITE => "Replace files that already exist",
        output::COLLISION => "When two jobs want the same file name",
        advanced::PROCESS_ISOLATION => "Run the worker as a separate process",
        advanced::LOG_LEVEL => "How much is logged",
        advanced::PROCESS_PRIORITY => "Worker priority",
        hardware::ACCELERATION => "Hardware acceleration",
        hardware::DECODE_DEVICE => "Decode on",
        updates::FREQUENCY => "Check for updates",
        updates::CHANNEL => "Update channel",
        whendone::ACTION => "When the queue finishes",
        whendone::DELAY_SECONDS => "Wait before finishing",
        _ => "That preference",
    }
}

/// A value, described the way it would be read back, for an error message.
fn describe(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "nothing".to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) => format!("'{text}'"),
        other => format!("a {}", kind_of(other)),
    }
}

/// The kind of a JSON value, in words.
fn kind_of(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "nothing",
        serde_json::Value::Bool(_) => "switch",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "word",
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "group of values",
    }
}

/// Read a number field, refusing anything that is not a finite number.
pub(crate) fn want_number(field: &str, value: &serde_json::Value) -> Result<f64, String> {
    let number = value.as_f64().ok_or_else(|| {
        format!(
            "{} has to be a number, not {}.",
            field_label(field),
            describe(value)
        )
    })?;
    if !number.is_finite() {
        return Err(format!("{} has to be a finite number.", field_label(field)));
    }
    Ok(number)
}

/// Read a whole number inside an inclusive range.
pub(crate) fn whole_in_range(
    field: &str,
    value: &serde_json::Value,
    min: u32,
    max: u32,
) -> Result<u32, String> {
    let number = want_number(field, value)?;
    if number.fract() != 0.0 {
        return Err(format!(
            "{} has to be a whole number, not {}.",
            field_label(field),
            number
        ));
    }
    if number < f64::from(min) || number > f64::from(max) {
        return Err(format!(
            "{} has to be between {min} and {max}, not {}.",
            field_label(field),
            number
        ));
    }
    Ok(number as u32)
}

/// Read a word field, refusing anything that is not text.
pub(crate) fn want_text(field: &str, value: &serde_json::Value) -> Result<String, String> {
    value.as_str().map(str::to_string).ok_or_else(|| {
        format!(
            "{} has to be text, not {}.",
            field_label(field),
            describe(value)
        )
    })
}

/// Read a word that has to be one of a fixed set.
///
/// The set is named in the message rather than counted, because a user who typed
/// something they did not mean to type needs the words they could have meant.
pub(crate) fn want_one_of(
    field: &str,
    value: &serde_json::Value,
    options: &[(&str, &str)],
) -> Result<String, String> {
    let typed = want_text(field, value)?;
    let wanted = typed.trim().to_ascii_lowercase();
    if options.iter().any(|(id, _)| *id == wanted) {
        return Ok(wanted);
    }
    let names = options
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<&str>>()
        .join(", ");
    Err(format!(
        "{} does not take '{}'; choose {}.",
        field_label(field),
        typed.trim(),
        names
    ))
}

/// A number control, built from a stored value.
pub(crate) fn number_control(
    id: &str,
    label: &str,
    hint: &str,
    value: u32,
    min: u32,
    max: u32,
    unit: &str,
) -> NumberViewModel {
    NumberViewModel::new(id, label, &value.to_string(), unit)
        .ranged(f64::from(min), f64::from(max))
        // The value is sent as well as the text: the page renders one of them and
        // a read-out that only shows the typed text cannot be read by anything
        // that is not looking at the field.
        .showing(&value.to_string(), f64::from(value))
        .with_hint(hint)
}

/// A text control, built from a stored value.
pub(crate) fn text_control(
    id: &str,
    label: &str,
    hint: &str,
    text: &str,
    placeholder: &str,
) -> TextViewModel {
    TextViewModel::new(id, label, text, placeholder).with_hint(hint)
}

/// Whether the identifier names a preferences tab.
fn tab_is_offered(id: &str) -> bool {
    PREFERENCE_TABS.iter().any(|(tab, _, _)| *tab == id.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    fn temp_settings(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-preferences-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir.join("settings.json")
    }

    /// A state over a scratch file, with isolation available.
    fn state(name: &str) -> PreferencesState {
        PreferencesState::load(
            &temp_settings(name),
            &vise_ipc::portable::default_settings(),
        )
    }

    /// A state whose output folder is a real folder, so the Output tab is clean
    /// and a test can tell one problem from another.
    fn ready(name: &str) -> PreferencesState {
        let path = temp_settings(name);
        let folder = path.parent().unwrap().join("finished");
        fs::create_dir_all(&folder).expect("output folder");
        let mut state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        state
            .set(
                output::FOLDER,
                &serde_json::Value::String(folder.display().to_string()),
            )
            .expect("an output folder that exists");
        state
    }

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    #[test]
    fn the_surface_offers_six_tabs_and_one_of_them_is_shown() {
        let state = ready("tabs");
        let view = state.view();
        assert_eq!(view.tabs.len(), 6);
        assert_eq!(view.tabs[0].id, "general");
        assert_eq!(view.tabs[5].id, "whendone");
        assert_eq!(view.active, "general");
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.reset.available);
        assert!(view.reset.changed, "the folder is not the default");
        assert!(
            view.reset.confirms,
            "a reset asks before it changes anything"
        );
        assert!(!view.reset.can_undo);
        for tab in &view.tabs {
            assert!(!tab.needs_attention, "{}", tab.id);
        }
    }

    #[test]
    fn a_folder_that_has_not_been_chosen_is_shown_rather_than_refusing_everything_else() {
        // The Output tab is the one place a missing folder belongs to. If it
        // blocked every other preference on every other tab, a fresh install
        // would be a surface where nothing can be changed until the folder is
        // found first.
        let mut state = state("unset-folder");
        let view = state.view();
        assert!(!view.ready);
        assert!(
            view.problems
                .iter()
                .any(|problem| problem.contains("Output folder")),
            "{:?}",
            view.problems
        );
        assert!(view.output.texts[0].problem.contains("Output folder"));
        assert!(!view.output.ready);
        // Every other tab is still ready.
        for tab in [
            &view.general,
            &view.advanced,
            &view.hardware,
            &view.updates,
            &view.when_done,
        ] {
            assert!(tab.ready, "{}: {:?}", tab.id, tab.problems);
        }
        state
            .set(general::SCAN_ON_OPEN, &value("true"))
            .expect("an unrelated preference is still changeable");
    }

    #[test]
    fn every_tab_name_is_accepted_and_anything_else_is_refused_by_name() {
        let mut state = state("tab-names");
        for (id, _, _) in PREFERENCE_TABS {
            state.set_active(id).expect(id);
            assert_eq!(state.active, id);
        }
        let error = state.set_active("registry").expect_err("unknown tab");
        assert!(error.contains("registry"), "{error}");
        assert!(error.contains("General"), "{error}");
        assert_eq!(state.active, "whendone", "a refused name changes nothing");
        assert!(tab_is_offered("output"));
        assert!(!tab_is_offered("registry"));
    }

    #[test]
    fn every_field_the_page_is_given_reaches_a_setter() {
        // The list of names is what the errors quote, and each name has to be one
        // `set` knows how to apply. A name in the list with no arm would be
        // offered and then refused, which is worse than not offering it.
        let mut state = state("fields");
        for (field, _) in PreferencesSettings::fields() {
            let result = state.set(field, &value("null"));
            if let Err(problem) = result {
                assert!(
                    !problem.contains("not a preference this build offers"),
                    "{field}: {problem}"
                );
            }
        }
        let error = state
            .set("general.pause_on_low_disk", &value("true"))
            .expect_err("unknown");
        assert!(error.contains("general.pause_on_low_disk"), "{error}");
        assert!(error.contains("general.preview_count"), "{error}");
    }

    #[test]
    fn a_change_is_validated_before_it_is_written() {
        let path = temp_settings("validate");
        let mut state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        let error = state
            .set(general::PREVIEW_COUNT, &value("99"))
            .expect_err("out of range");
        assert!(error.contains("Preview frames per title"), "{error}");
        assert!(error.contains("between 0 and 32"), "{error}");
        // Nothing was written, so the file still has no record in it.
        assert!(
            !path.exists()
                || !std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains("preferences")
        );

        state
            .set(general::PREVIEW_COUNT, &value("12"))
            .expect("a valid count");
        let stored = load_settings(&path).expect("read back");
        assert_eq!(stored.written_version, vise_core::SETTINGS_VERSION);
        let again = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(again.settings().general.preview_count, 12);
    }

    #[test]
    fn a_value_of_the_wrong_shape_is_refused_by_name_and_never_coerced() {
        let mut state = state("shapes");
        let error = state
            .set(general::SCAN_ON_OPEN, &value("\"yes\""))
            .expect_err("a word is not a switch");
        assert!(error.contains("on or off"), "{error}");
        assert!(error.contains("yes"), "{error}");

        let error = state
            .set(general::PREVIEW_COUNT, &value("\"12\""))
            .expect_err("a word is not a number");
        assert!(error.contains("a number"), "{error}");

        let error = state
            .set(output::FOLDER, &value("7"))
            .expect_err("a number is not a folder");
        assert!(error.contains("text"), "{error}");
    }

    #[test]
    fn a_change_reaches_the_code_paths_that_consume_it() {
        let mut state = state("consumers");
        // Detection is what decides whether a named decode device is a choice at
        // all, so a machine that has one has to be the machine under test.
        state.observe_hardware(&vise_core::HardwareCapabilities {
            ffmpeg_path: Some(std::path::PathBuf::from("C:\\tools\\ffmpeg.exe")),
            hwaccels: vec!["d3d11va".to_string()],
            ..Default::default()
        });
        state
            .set(advanced::LOG_LEVEL, &value("\"debug\""))
            .expect("log level");
        state
            .set(hardware::ACCELERATION, &value("\"when_available\""))
            .expect("acceleration");
        state
            .set(hardware::DECODE_DEVICE, &value("\"d3d11va\""))
            .expect("decode device");
        state
            .set(advanced::PROCESS_PRIORITY, &value("\"above_normal\""))
            .expect("priority");

        let init = state.settings().to_worker_init();
        assert_eq!(init.log_verbosity, 3, "debug keeps everything");
        assert!(init.enable_hardware_acceleration);
        assert_eq!(init.hw_decode, 1);
        assert_eq!(init.hw_decode_device, "d3d11va");
        assert_eq!(init.process_priority, 1);
    }

    #[test]
    fn a_decode_device_this_machine_does_not_have_is_refused_rather_than_stored() {
        // Storing one would be a preference the media tools quietly ignore, which
        // is the one thing this surface is not allowed to do.
        let mut state = state("no-device");
        state.observe_hardware(&vise_core::HardwareCapabilities {
            ffmpeg_path: Some(std::path::PathBuf::from("C:\\tools\\ffmpeg.exe")),
            hwaccels: vec!["d3d11va".to_string()],
            ..Default::default()
        });
        let error = state
            .set(hardware::DECODE_DEVICE, &value("\"vaapi\""))
            .expect_err("not on this machine");
        assert!(error.contains("vaapi"), "{error}");
        assert_eq!(
            state.settings().hardware.decode_device,
            hardware::AUTOMATIC_DECODE,
            "the stored answer is untouched"
        );
        // The two answers that are not a device are always on offer.
        for answer in [hardware::AUTOMATIC_DECODE, hardware::SOFTWARE_DECODE] {
            state
                .set(hardware::DECODE_DEVICE, &value(&format!("\"{answer}\"")))
                .expect(answer);
        }
        assert_eq!(
            state.settings().hardware.decode_device,
            hardware::SOFTWARE_DECODE
        );
    }

    #[test]
    fn a_decode_device_cannot_be_named_before_the_media_tools_have_been_looked_for() {
        // Detection has not run, so "this machine does not have it" is not
        // something the window knows yet, and saying so is better than guessing.
        let mut state = state("unprobed-device");
        let error = state
            .set(hardware::DECODE_DEVICE, &value("\"d3d11va\""))
            .expect_err("not looked for");
        assert!(error.contains("not been looked for"), "{error}");
    }

    #[test]
    fn every_action_the_surface_offers_is_one_the_page_is_told_about() {
        // The page routes a press by identifier, so a button whose identifier is
        // not in this list would be a control that does nothing when pressed, and
        // one in this list that no tab offers is a promise nothing keeps.
        let offered: Vec<&str> = PREFERENCE_ACTIONS.iter().map(|(id, _, _)| *id).collect();
        let mut seen: Vec<&str> = Vec::new();
        let view = state("actions").view();
        for tab in [
            &view.general,
            &view.output,
            &view.advanced,
            &view.hardware,
            &view.updates,
            &view.when_done,
        ] {
            for action in &tab.actions {
                assert!(
                    offered.contains(&action.id.as_str()),
                    "{}: '{}' is not one this build routes",
                    tab.id,
                    action.id
                );
                assert!(!seen.contains(&action.id.as_str()), "{}", action.id);
                seen.push(action.id.as_str());
                assert!(!action.label.is_empty(), "{}", action.id);
                assert!(!action.hint.is_empty(), "{}", action.id);
            }
        }
        // The two the surface puts outside the tabs: the restore and the take-back.
        assert!(!seen.contains(&RESET_ACTION));
        assert!(!seen.contains(&UNDO_ACTION));
        assert_eq!(view.reset.label, "Restore the defaults");
        assert_eq!(view.reset.undo_label, "Put my preferences back");
        assert!(!view.reset.effect.is_empty());
        assert!(!view.reset.undo_effect.is_empty());
        assert_eq!(action_label(RESET_ACTION), "Restore the defaults");
        assert!(action_label("preferencesNothing").eq("Do it"));
        assert!(
            offered.contains(&"preferencesRecheckHardware"),
            "the hardware tab offers a way to ask again"
        );
        assert!(
            offered.contains(&"preferencesCheckNow"),
            "the updates tab offers the control that is closed, with its reason"
        );
    }

    // ------------------------------------------------- persistence and migration

    /// Write a settings file whose contents are exactly `body`.
    fn write_raw(path: &std::path::Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, body).expect("write settings");
    }

    #[test]
    fn the_record_round_trips_through_the_settings_file() {
        let path = temp_settings("round-trip");
        let mut state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        let folder = path.parent().unwrap().join("out");
        fs::create_dir_all(&folder).expect("folder");
        state
            .set(
                output::FOLDER,
                &serde_json::Value::String(folder.display().to_string()),
            )
            .expect("folder");
        state
            .set(general::PREVIEW_COUNT, &value("4"))
            .expect("stills");
        state
            .set(general::SCAN_ON_OPEN, &value("true"))
            .expect("scan on open");
        state
            .set(general::DEFAULT_RANGE_MODE, &value("\"chapters\""))
            .expect("range");
        state
            .set(output::NAME_PATTERN, &value("\"{title} ({n})\""))
            .expect("pattern");
        state
            .set(output::COLLISION, &value("\"renumber\""))
            .expect("collision");
        state
            .set(advanced::LOG_LEVEL, &value("\"warning\""))
            .expect("log level");
        state
            .set(hardware::ACCELERATION, &value("\"never\""))
            .expect("acceleration");
        state
            .set(hardware::DECODE_DEVICE, &value("\"software\""))
            .expect("decode");
        state
            .set(updates::FREQUENCY, &value("\"daily\""))
            .expect("frequency");
        state
            .set(updates::CHANNEL, &value("\"preview\""))
            .expect("channel");
        state
            .set(whendone::ACTION, &value("\"close_window\""))
            .expect("when done");
        state
            .set(whendone::DELAY_SECONDS, &value("30"))
            .expect("delay");
        let written = state.settings().clone();

        let read_back = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(
            read_back.settings(),
            &written,
            "the record is what was stored"
        );
        let view = read_back.view();
        assert_eq!(view.storage.state, RecordState::Current);
        assert_eq!(view.storage.written_version, PREFERENCES_VERSION);
        assert_eq!(view.storage.document_version, vise_core::SETTINGS_VERSION);
        assert!(!view.storage.document_needs_migration);
        assert!(view.storage.can_save);
        assert!(view.ready, "{:?}", view.problems);

        // The document's own output-folder member is kept in step with the
        // record, because that is the member the rest of the application reads.
        let document = load_settings(&path).expect("read");
        assert_eq!(
            document.output_directory.as_deref(),
            Some(folder.display().to_string()).as_deref()
        );
        assert!(document.extra.contains_key(PREFERENCES_KEY));
    }

    #[test]
    fn a_document_with_no_record_adopts_the_folder_it_already_named() {
        // The migration that matters: older builds kept the output folder at the
        // top level of the document, so a user who chose one sees it rather than
        // an empty field the first time they open this surface.
        let path = temp_settings("adopt");
        let folder = path.parent().unwrap().join("finished");
        fs::create_dir_all(&folder).expect("folder");
        let document = AppSettings {
            output_directory: Some(folder.display().to_string()),
            written_version: SETTINGS_UNVERSIONED,
            ..AppSettings::default()
        };
        save_settings(&path, &document).expect("written");
        // The version is forced back down so the file really is an older one.
        write_raw(
            &path,
            &serde_json::json!({"output_directory": folder.display().to_string()}).to_string(),
        );

        let state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(state.settings().output.folder, folder.display().to_string());
        let view = state.view();
        assert_eq!(view.storage.state, RecordState::Migrated);
        assert!(
            view.storage.note.contains("adopted"),
            "{}",
            view.storage.note
        );
        assert!(
            view.storage.document_needs_migration,
            "the document is older"
        );
        assert!(view.ready, "{:?}", view.problems);

        // Writing anything moves the document forward and the note with it.
        let mut state = state;
        state
            .set(general::PREVIEW_COUNT, &value("6"))
            .expect("stills");
        let view = state.view();
        assert!(!view.storage.document_needs_migration);
        let document: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(document["version"], vise_core::SETTINGS_VERSION);
        assert_eq!(document["preferences"]["version"], PREFERENCES_VERSION);
        assert_eq!(document["output_directory"], folder.display().to_string());
    }

    #[test]
    fn an_older_record_is_brought_forward_without_losing_what_it_held() {
        let path = temp_settings("older-record");
        let folder = path.parent().unwrap().join("out");
        fs::create_dir_all(&folder).expect("folder");
        // A record with no version and only two of the six groups in it.
        let mut document = AppSettings::default();
        document.extra.insert(
            PREFERENCES_KEY.to_string(),
            serde_json::json!({
                "general": {"preview_count": 3},
                "output": {"folder": folder.display().to_string()}
            }),
        );
        save_settings(&path, &document).expect("written");
        write_raw(
            &path,
            &serde_json::json!({
                "preferences": {
                    "general": {"preview_count": 3},
                    "output": {"folder": folder.display().to_string()}
                }
            })
            .to_string(),
        );

        let view = PreferencesState::load(&path, &vise_ipc::portable::default_settings()).view();
        assert_eq!(view.storage.state, RecordState::Migrated);
        assert_eq!(view.storage.written_version, 0, "it declared none");
        assert_eq!(
            view.general.numbers[0].value,
            Some(3.0),
            "what it held is kept"
        );
        assert_eq!(view.output.texts[0].text, folder.display().to_string());
        // The groups it did not have are at their defaults rather than missing.
        assert_eq!(
            view.advanced
                .choices
                .iter()
                .find(|choice| choice.id == advanced::LOG_LEVEL)
                .map(|choice| choice.selected.clone()),
            Some("info".to_string())
        );
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_record_from_a_newer_build_is_read_never_written_and_never_lost() {
        let path = temp_settings("newer-record");
        let future = PREFERENCES_VERSION + 3;
        write_raw(
            &path,
            &serde_json::json!({
                "version": vise_core::SETTINGS_VERSION,
                "preferences": {
                    "version": future,
                    "general": {"preview_count": 21},
                    "something_this_build_does_not_know": {"kept": true}
                }
            })
            .to_string(),
        );

        let mut state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        let view = state.view();
        assert_eq!(view.storage.state, RecordState::FromTheFuture);
        assert!(!view.storage.can_save);
        assert!(
            view.storage.blocked_reason.contains("newer build"),
            "{}",
            view.storage.blocked_reason
        );
        assert_eq!(view.storage.written_version, future);
        assert!(
            view.problems
                .iter()
                .any(|problem| problem.contains("newer build")),
            "{:?}",
            view.problems
        );
        // Every tab is closed, because nothing this build knows how to change
        // would be the same answer as what is stored there.
        assert!(view
            .general
            .choices
            .iter()
            .all(|control| !control.available));
        assert!(view
            .general
            .toggles
            .iter()
            .all(|control| !control.available));
        assert!(view
            .general
            .numbers
            .iter()
            .all(|control| !control.available));
        assert!(view.output.texts.iter().all(|control| !control.available));
        let error = state
            .set(general::PREVIEW_COUNT, &value("4"))
            .expect_err("writes are refused");
        assert!(error.contains("newer build"), "{error}");

        // The file on disk is exactly as it was found.
        let body = fs::read_to_string(&path).expect("read");
        assert!(
            body.contains("something_this_build_does_not_know"),
            "{body}"
        );
        assert_eq!(
            load_settings(&path).expect("read back").written_version,
            vise_core::SETTINGS_VERSION,
            "the document is not downgraded either"
        );
    }

    #[test]
    fn a_record_that_is_not_readable_gives_the_defaults_and_is_left_alone() {
        // A document that is not an object at all is refused by the settings
        // reader before it is ever a record; that case is one test below.
        for (name, record) in [
            (
                "not-an-object",
                serde_json::Value::String("dark".to_string()),
            ),
            (
                "wrong-shapes",
                serde_json::json!({"general": {"preview_count": "three"}}),
            ),
            (
                "unknown-choice",
                serde_json::json!({"version": PREFERENCES_VERSION, "general": {"default_range_mode": "middle"}}),
            ),
        ] {
            let path = temp_settings(name);
            let body = serde_json::json!({PREFERENCES_KEY: record}).to_string();
            write_raw(&path, &body);
            let state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
            let view = state.view();
            assert_eq!(
                view.storage.state,
                RecordState::Unreadable,
                "{name}: every preference is at its default"
            );
            assert_eq!(
                state.settings(),
                &PreferencesSettings::new(),
                "{name}: nothing was half-read"
            );
            assert!(
                view.storage.note.contains("left alone"),
                "{name}: {}",
                view.storage.note
            );
            assert_eq!(
                fs::read_to_string(&path).expect("read"),
                body,
                "{name}: the stored record is untouched"
            );
        }
    }

    #[test]
    fn a_settings_file_that_is_not_readable_gives_the_defaults_and_says_so() {
        let path = temp_settings("broken-document");
        write_raw(&path, "{ not json");
        let state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        let view = state.view();
        assert_eq!(view.storage.state, RecordState::Unreadable);
        assert!(
            view.storage.note.contains("could not be read"),
            "{}",
            view.storage.note
        );
        assert!(
            view.storage
                .note
                .contains(path.display().to_string().as_str())
                || view.storage.location.contains("settings.json"),
            "{}",
            view.storage.location
        );
        // A change still works: the file is rewritten with a record in it, which
        // is the only way a hand-broken settings file is ever repaired.
        let mut state = state;
        state
            .set(general::PREVIEW_COUNT, &value("5"))
            .expect("stills");
        let again = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(again.settings().general.preview_count, 5);
        assert_eq!(again.view().storage.state, RecordState::Current);
    }

    // ------------------------------------------------------------- reset

    #[test]
    fn a_reset_puts_everything_back_and_can_be_taken_back_once() {
        // The same scratch file as the state under test, so what is read back is
        // the reset rather than the defaults of a file nothing ever wrote to.
        let path = temp_settings("reset");
        let folder = path.parent().unwrap().join("finished");
        fs::create_dir_all(&folder).expect("output folder");
        let mut state = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        state
            .set(
                output::FOLDER,
                &serde_json::Value::String(folder.display().to_string()),
            )
            .expect("an output folder that exists");
        state
            .set(advanced::LOG_LEVEL, &value("\"debug\""))
            .expect("log level");
        state
            .set(updates::FREQUENCY, &value("\"monthly\""))
            .expect("frequency");
        let before = state.settings().clone();
        assert!(!state.can_undo());

        let changed = state.reset();
        assert!(changed.contains(&"Advanced".to_string()), "{changed:?}");
        assert!(changed.contains(&"Updates".to_string()), "{changed:?}");
        assert_eq!(state.settings(), &PreferencesSettings::new());
        assert!(state.can_undo(), "the previous record is held");
        let view = state.view();
        assert!(view.reset.can_undo);
        assert!(!view.reset.changed);
        assert_eq!(view.reset.undo_label, "Put my preferences back");
        assert!(
            view.reset.announcement.contains("Restored the defaults"),
            "{}",
            view.reset.announcement
        );
        assert!(
            view.reset.announcement.contains("Advanced"),
            "{}",
            view.reset.announcement
        );
        // The reset is on disk, not just in memory: the record read back is this
        // build's defaults, stamped with this build's version.
        let stored = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(stored.settings(), &PreferencesSettings::new());
        let body: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(body["preferences"]["version"], PREFERENCES_VERSION);
        assert_eq!(
            body["preferences"]["advanced"]["log_level"], "Info",
            "the stored level is the default one, not the one that was there"
        );

        // And it can be taken back.
        state.undo_reset().expect("undo");
        assert_eq!(state.settings(), &before);
        let restored = PreferencesState::load(&path, &vise_ipc::portable::default_settings());
        assert_eq!(restored.settings(), &before, "the take-back is on disk too");
        assert!(!state.can_undo(), "one reset is undoable, not a history");
        let error = state.undo_reset().expect_err("nothing to take back");
        assert!(error.contains("nothing to take back"), "{error}");
    }

    #[test]
    fn a_reset_onto_an_unchanged_record_reports_nothing_and_keeps_the_take_back() {
        let mut state = ready("reset-unchanged");
        // The folder is set by `ready`, so there is something to restore from.
        let changed = state.reset();
        assert!(changed.contains(&"Output".to_string()), "{changed:?}");
        assert!(state.can_undo());
        state.undo_reset().expect("undo");
        assert!(!state.settings().output.folder.is_empty());
    }
}
