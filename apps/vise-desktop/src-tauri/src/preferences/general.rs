//! The general tab: how sources are opened and scanned.
//!
//! Five preferences, and each one has a reader outside this module:
//!
//! * **Scan when a source is opened** — `main.rs` starts the scan from
//!   `select_source`, so a user who leaves this on never has to press Scan for
//!   a single file.
//! * **Preview frames per title** — the default for the scan command and for
//!   the preview window's still count, both of which are asked for the value
//!   rather than holding their own.
//! * **Keep duplicate titles** — the same switch the worker's initialise
//!   command carries, so a folder of near-identical files is listed as it is on
//!   disk rather than collapsed.
//! * **Default range** — the mode the range control starts on when a title is
//!   chosen, so the choice a user makes once is the one they get every time.
//! * **Language** — the words the interface is written in, which `locale.rs`
//!   reads at start-up and every time the window pushes a new language.

use serde::{Deserialize, Serialize};

use crate::preferences::{
    number_control, want_one_of, whole_in_range, ChoiceGroupViewModel, PreferenceTabViewModel,
    ToggleViewModel,
};
use crate::tabs::common::ChoiceOption;
use crate::viewmodel::RangeMode;
use vise_core::SUPPORTED;

/// Field identifier for the preview frame count.
pub const PREVIEW_COUNT: &str = "general.preview_count";
/// Field identifier for the duplicate-title switch.
pub const KEEP_DUPLICATE_TITLES: &str = "general.keep_duplicate_titles";
/// Field identifier for the default range mode.
pub const DEFAULT_RANGE_MODE: &str = "general.default_range_mode";
/// Field identifier for the scan-on-open switch.
pub const SCAN_ON_OPEN: &str = "general.scan_on_open";
/// Field identifier for the interface language.
pub const LANGUAGE: &str = "general.language";

/// The value the picker sends for "whatever this machine is set to".
///
/// A name rather than an empty tag, because an empty field is indistinguishable
/// from a preference nobody has set yet, and this build has one answer to "no
/// language has been chosen".
pub const FOLLOW_SYSTEM: &str = "system";

/// Which language the interface is written in.
///
/// Two answers rather than one tag, because a machine that has never chosen
/// anything and a machine that has chosen "whatever this one is set to" have to
/// keep agreeing while the machine's own setting changes underneath.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguagePreference {
    /// Whatever this machine reports.
    FollowSystem,
    /// One language this build ships, by tag.
    ///
    /// Read as a string rather than as an enum because the set of languages is
    /// the translation resources' business: a build that adds a language must not
    /// have to change this type, and a record naming a language this build does
    /// not have has to be readable so the language module can resolve it through
    /// its own chain rather than being refused here.
    Locale(String),
}

impl Default for LanguagePreference {
    fn default() -> Self {
        Self::FollowSystem
    }
}

impl LanguagePreference {
    /// The value the record stores.
    pub fn id(&self) -> String {
        match self {
            LanguagePreference::FollowSystem => FOLLOW_SYSTEM.to_string(),
            LanguagePreference::Locale(tag) => tag.clone(),
        }
    }

    /// What following the machine resolves to right now.
    ///
    /// Kept beside the preference rather than stored in it, so a machine whose
    /// own setting changes is followed without the record having to be rewritten
    /// — which is the whole difference between following something and having
    /// copied it.
    pub fn resolves_to(&self) -> vise_core::Locale {
        match self {
            LanguagePreference::FollowSystem => vise_core::locale_from_environment(),
            LanguagePreference::Locale(tag) => vise_core::resolve_locale(tag).locale,
        }
    }

    /// The tag to read the messages under.
    pub fn tag(&self) -> String {
        self.resolves_to().tag().to_string()
    }

    /// Read the value the interface layer sent, or say why it was refused.
    ///
    /// Only a value that is not a name at all is refused. A language this build
    /// does not ship is stored as asked for and resolved through the fallback
    /// chain rather than turned away: refusing would leave a record written by
    /// another build unreadable, and would stop a user from asking for the
    /// language their machine is in even when the picker cannot show it.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = value
            .as_str()
            .ok_or_else(|| format!("{LANGUAGE} has to be the name of a language, not a number."))?;
        let wanted = typed.trim().to_ascii_lowercase();
        if wanted.is_empty() {
            return Err(format!(
                "{LANGUAGE} has to name a language; an empty name is not one."
            ));
        }
        if wanted == FOLLOW_SYSTEM {
            return Ok(LanguagePreference::FollowSystem);
        }
        Ok(LanguagePreference::Locale(
            vise_core::i18n::normalize_locale_tag(&wanted),
        ))
    }
}

/// Largest number of stills a scan will extract.
///
/// The worker bounds this itself, so the field agrees with the number it is
/// about rather than advertising a choice the worker would quietly reduce.
pub const MAX_PREVIEW_FRAMES: u32 = 32;

/// Stills extracted per title when nothing has been chosen.
///
/// The preview window already asked for ten, so ten is the default here too:
/// a machine that has never opened the preferences should not find its preview
/// strip a different length because a new surface was added.
pub const DEFAULT_PREVIEW_FRAMES: u32 = 10;

/// Which span of a title an encode takes, as a starting point.
///
/// The names are this build's own; `viewmodel::RangeMode` already turns text
/// into the three modes the range control offers, so this is the same question
/// asked once at start-up rather than on every selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RangePreference {
    /// The whole title.
    #[default]
    Whole,
    /// Every chapter in the title.
    Chapters,
    /// Whatever two points the user marks.
    PointToPoint,
}

impl RangePreference {
    /// Every mode, in the order the picker lists them.
    pub const ALL: [RangePreference; 3] = [
        RangePreference::Whole,
        RangePreference::Chapters,
        RangePreference::PointToPoint,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            RangePreference::Whole => "whole",
            RangePreference::Chapters => "chapters",
            RangePreference::PointToPoint => "point_to_point",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            RangePreference::Whole => "The whole title",
            RangePreference::Chapters => "Its chapters",
            RangePreference::PointToPoint => "Two points I mark",
        }
    }

    /// What choosing this means, before a title has been scanned.
    pub fn hint(self) -> &'static str {
        match self {
            RangePreference::Whole => "Every title starts at the beginning and runs to the end.",
            RangePreference::Chapters => {
                "Titles that carry chapter markers start on their chapters; the rest still start at the beginning."
            }
            RangePreference::PointToPoint => {
                "Titles start waiting for a start point and an end point."
            }
        }
    }

    /// The range mode a newly chosen title starts in.
    pub fn to_range_mode(self) -> RangeMode {
        match self {
            RangePreference::Whole => RangeMode::Whole,
            RangePreference::Chapters => RangeMode::Chapters,
            RangePreference::PointToPoint => RangeMode::PointToPoint,
        }
    }

    /// Read a mode the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            DEFAULT_RANGE_MODE,
            value,
            &[
                (RangePreference::Whole.id(), RangePreference::Whole.label()),
                (
                    RangePreference::Chapters.id(),
                    RangePreference::Chapters.label(),
                ),
                (
                    RangePreference::PointToPoint.id(),
                    RangePreference::PointToPoint.label(),
                ),
            ],
        )?;
        RangePreference::ALL
            .into_iter()
            .find(|mode| mode.id() == typed)
            .ok_or_else(|| format!("'{typed}' is not a range this build starts on."))
    }
}

/// The general preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// Still frames extracted per title by a scan.
    pub preview_count: u32,
    /// Keep titles a source reports more than once.
    pub keep_duplicate_titles: bool,
    /// The range a newly chosen title starts in.
    pub default_range_mode: RangePreference,
    /// Start a scan as soon as a source is opened.
    pub scan_on_open: bool,
    /// The language the interface is written in.
    pub language: LanguagePreference,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneralSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// Scanning on open is off, because a folder scan can take a while and a
    /// window that starts one without being asked is a window that appears to
    /// hang. The preview count is on because the preview window asks for stills
    /// the moment it is opened, and an empty answer would give it nothing. The
    /// language follows this machine, because a window that opened in English on
    /// a machine set to something else has ignored an answer it was given.
    pub fn new() -> Self {
        Self {
            preview_count: DEFAULT_PREVIEW_FRAMES,
            keep_duplicate_titles: false,
            default_range_mode: RangePreference::Whole,
            scan_on_open: false,
            language: LanguagePreference::FollowSystem,
        }
    }
}

/// Read the preview frame count.
pub fn read_preview_count(value: &serde_json::Value) -> Result<u32, String> {
    whole_in_range(PREVIEW_COUNT, value, 0, MAX_PREVIEW_FRAMES)
}

/// The problems these preferences have.
pub fn validate(settings: &GeneralSettings) -> Vec<String> {
    let mut problems = Vec::new();
    if settings.preview_count > MAX_PREVIEW_FRAMES {
        problems.push(format!(
            "Preview frames per title has to be {MAX_PREVIEW_FRAMES} or fewer; {} was stored.",
            settings.preview_count
        ));
    }
    problems
}

/// Build the general tab.
pub fn tab_view(settings: &GeneralSettings) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "general",
        "General",
        "How sources are opened, how they are scanned, and what a scan leaves behind.",
    );
    let options: Vec<ChoiceOption> = RangePreference::ALL
        .iter()
        .map(|mode| ChoiceOption::new(mode.id(), mode.label()))
        .collect();
    tab.choices.push(
        ChoiceGroupViewModel::build(
            DEFAULT_RANGE_MODE,
            "Default range",
            "The range a title starts in when it is chosen. A title can always be given a different range before it is queued.",
            settings.default_range_mode.id(),
            &options,
            true,
            "",
        )
        .with_hint("Chosen once here rather than on every title."),
    );
    tab.choices.push(language_choice(&settings.language));
    tab.numbers.push(number_control(
        "generalPreviewCount",
        "Preview frames per title",
        &format!(
            "Stills the scan extracts for each title, up to {MAX_PREVIEW_FRAMES}. The preview window shows them as a strip."
        ),
        settings.preview_count,
        0,
        MAX_PREVIEW_FRAMES,
        "frames",
    ));
    tab.toggles.push(ToggleViewModel::new(
        "generalScanOnOpen",
        "Scan when a source is opened",
        "Opens the source and starts the scan without pressing Scan.",
        settings.scan_on_open,
    ));
    tab.toggles.push(ToggleViewModel::new(
        "generalKeepDuplicateTitles",
        "Keep duplicate titles",
        "Lists every title a folder scan finds, instead of collapsing the ones it considers the same.",
        settings.keep_duplicate_titles,
    ));
    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

/// The language picker, as one closed set.
///
/// Following the machine comes first because it is the answer a machine with no
/// preference is already giving, and every language after it is named in itself:
/// a picker of translated names is a picker nobody who cannot read the current
/// language can use.
fn language_choice(stored: &LanguagePreference) -> ChoiceGroupViewModel {
    let mut options = vec![ChoiceOption::new(FOLLOW_SYSTEM, "Follow the system")];
    options.extend(
        SUPPORTED
            .iter()
            .map(|locale| ChoiceOption::new(locale.tag(), locale.endonym())),
    );
    let selected = stored.id();
    ChoiceGroupViewModel::build(
        LANGUAGE,
        "Language",
        "The language the interface is written in. It changes as soon as it is chosen; nothing else in the window is lost.",
        &selected,
        &options,
        true,
        "",
    )
    .with_hint("A language this build does not have is shown in the language this build was written in.")
}

/// Read the switch a page sends for this tab, for the tests below.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::want_switch;

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = GeneralSettings::new();
        assert_eq!(settings.preview_count, DEFAULT_PREVIEW_FRAMES);
        assert!(!settings.keep_duplicate_titles);
        assert!(!settings.scan_on_open);
        assert_eq!(settings.default_range_mode, RangePreference::Whole);
        assert_eq!(settings.language, LanguagePreference::FollowSystem);
        assert_eq!(settings, GeneralSettings::default());
    }

    #[test]
    fn a_language_follows_the_machine_until_it_is_chosen() {
        let following = LanguagePreference::FollowSystem;
        assert_eq!(following.id(), FOLLOW_SYSTEM);
        assert_eq!(following.tag(), vise_core::locale_from_environment().tag());
        let chosen = LanguagePreference::Locale("ar".to_string());
        assert_eq!(chosen.id(), "ar");
        assert_eq!(chosen.tag(), "ar");
        // The machine changing its mind moves the window that is following it,
        // which is the difference between following something and having copied
        // it.
        assert_eq!(chosen.tag(), "ar", "a chosen language does not move");
    }

    #[test]
    fn a_language_the_picker_sent_is_read_and_one_it_cannot_show_is_kept() {
        assert_eq!(
            LanguagePreference::read(&value("\"system\"")).expect("system"),
            LanguagePreference::FollowSystem
        );
        assert_eq!(
            LanguagePreference::read(&value("\"ar\"")).expect("arabic"),
            LanguagePreference::Locale("ar".to_string())
        );
        assert_eq!(
            LanguagePreference::read(&value("\" DE \"")).expect("german"),
            LanguagePreference::Locale("de".to_string())
        );
        // A language this build does not ship is stored as asked for and resolved
        // through the fallback chain, rather than refused: refusing would leave a
        // record written by another build unreadable for no gain.
        let shipped_elsewhere = LanguagePreference::read(&value("\"kl\"")).expect("greenlandic");
        assert_eq!(
            shipped_elsewhere,
            LanguagePreference::Locale("kl".to_string())
        );
        assert_eq!(shipped_elsewhere.tag(), "en", "and falls back to the base");
        // A regional tag is reduced to what decides anything.
        assert_eq!(
            LanguagePreference::read(&value("\"es_MX\"")).expect("regional"),
            LanguagePreference::Locale("es-mx".to_string())
        );
        let error = LanguagePreference::read(&value("3")).expect_err("not a name");
        assert!(error.contains(LANGUAGE), "{error}");
    }

    #[test]
    fn the_tab_offers_every_language_in_its_own_name() {
        let view = tab_view(&GeneralSettings::new());
        let picker = view
            .choices
            .iter()
            .find(|choice| choice.id == LANGUAGE)
            .expect("the language picker");
        assert_eq!(picker.selected, FOLLOW_SYSTEM);
        assert_eq!(picker.options.len(), SUPPORTED.len() + 1);
        assert_eq!(picker.options[0].id, FOLLOW_SYSTEM);
        assert_eq!(picker.options[0].label, "Follow the system");
        let arabic = picker
            .options
            .iter()
            .find(|option| option.id == "ar")
            .expect("arabic is offered");
        assert_eq!(arabic.label, "العربية");

        // A chosen language is marked as chosen, and the picker is the same
        // control with a different row selected rather than a second control.
        let chosen = GeneralSettings {
            language: LanguagePreference::Locale("de".to_string()),
            ..GeneralSettings::new()
        };
        let view = tab_view(&chosen);
        let picker = view
            .choices
            .iter()
            .find(|choice| choice.id == LANGUAGE)
            .expect("the language picker");
        assert_eq!(picker.selected, "de");
        assert_eq!(picker.options.len(), SUPPORTED.len() + 1);
        assert!(picker.available);
        assert_eq!(view.choices.len(), 2);
    }

    #[test]
    fn the_language_survives_a_record_round_trip() {
        let stored = GeneralSettings {
            language: LanguagePreference::Locale("ja".to_string()),
            ..GeneralSettings::new()
        };
        let text = serde_json::to_string(&stored).expect("write");
        let read: GeneralSettings = serde_json::from_str(&text).expect("read");
        assert_eq!(read, stored);
        // A record from before this preference existed reads without it and comes
        // back following the machine, which is what a user who never chose
        // expects.
        let old: GeneralSettings =
            serde_json::from_str(r#"{"preview_count":4}"#).expect("an older record");
        assert_eq!(old.language, LanguagePreference::FollowSystem);
        assert_eq!(old.preview_count, 4);
    }

    #[test]
    fn the_preview_frame_count_is_bounded_and_the_bounds_are_quoted() {
        assert_eq!(read_preview_count(&value("0")).expect("none"), 0);
        assert_eq!(
            read_preview_count(&value(&MAX_PREVIEW_FRAMES.to_string())).expect("most"),
            MAX_PREVIEW_FRAMES
        );
        let error = read_preview_count(&value("33")).expect_err("one too many");
        assert!(error.contains("between 0 and 32"), "{error}");
        let error = read_preview_count(&value("8.5")).expect_err("not whole");
        assert!(error.contains("whole number"), "{error}");
        assert!(read_preview_count(&value("\"8\"")).is_err());
    }

    #[test]
    fn a_switch_reads_from_a_switch_and_nothing_else() {
        assert!(want_switch(SCAN_ON_OPEN, &value("true")).expect("on"));
        let error = want_switch(SCAN_ON_OPEN, &value("1")).expect_err("a number is not a switch");
        assert!(error.contains("on or off"), "{error}");
    }

    #[test]
    fn the_range_preference_uses_the_mode_the_range_control_already_understands() {
        assert_eq!(RangePreference::Whole.to_range_mode(), RangeMode::Whole);
        assert_eq!(
            RangePreference::Chapters.to_range_mode(),
            RangeMode::Chapters
        );
        assert_eq!(
            RangePreference::PointToPoint.to_range_mode(),
            RangeMode::PointToPoint
        );
        // The identifier has to be one the range control can be given, or the
        // preference would set a mode the control cannot show.
        for mode in RangePreference::ALL {
            assert_eq!(RangeMode::parse(mode.id()), Some(mode.to_range_mode()));
        }
    }

    #[test]
    fn an_unknown_range_is_refused_with_the_names_that_would_work() {
        let error = RangePreference::read(&value("\"point\"")).expect_err("unknown");
        assert!(error.contains("point"), "{error}");
        assert!(error.contains("whole"), "{error}");
        assert!(error.contains("point_to_point"), "{error}");
        // Surrounding space and case are the page's problem, not the user's.
        assert_eq!(
            RangePreference::read(&value("\" Chapters \"")).expect("chapters"),
            RangePreference::Chapters
        );
    }

    #[test]
    fn the_tab_shows_every_control_with_its_bounds_and_its_value() {
        let view = tab_view(&GeneralSettings::new());
        assert_eq!(view.id, "general");
        assert_eq!(view.numbers.len(), 1);
        assert_eq!(view.numbers[0].min, Some(0.0));
        assert_eq!(view.numbers[0].max, Some(f64::from(MAX_PREVIEW_FRAMES)));
        assert_eq!(
            view.numbers[0].value,
            Some(f64::from(DEFAULT_PREVIEW_FRAMES))
        );
        assert_eq!(view.toggles.len(), 2);
        assert!(view.toggles.iter().all(|toggle| toggle.available));
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.facts.is_empty(), "this tab reports nothing to observe");
    }

    #[test]
    fn a_stored_value_outside_the_bounds_is_reported_rather_than_trusted() {
        // A record can arrive from a file this build did not write, so the range
        // check runs on the stored value and not only on what was typed.
        let stored = GeneralSettings {
            preview_count: 500,
            ..GeneralSettings::new()
        };
        let view = tab_view(&stored);
        assert!(!view.ready);
        assert!(
            view.problems[0].contains("32 or fewer"),
            "{:?}",
            view.problems
        );
        assert!(validate(&stored).len() == 1);
    }
}
