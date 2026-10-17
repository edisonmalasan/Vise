//! The general tab: how sources are opened and scanned.
//!
//! Four preferences, and each one has a reader outside this module:
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

use serde::{Deserialize, Serialize};

use crate::preferences::{
    number_control, want_one_of, whole_in_range, ChoiceGroupViewModel, PreferenceTabViewModel,
    ToggleViewModel,
};
use crate::tabs::common::ChoiceOption;
use crate::viewmodel::RangeMode;

/// Field identifier for the preview frame count.
pub const PREVIEW_COUNT: &str = "general.preview_count";
/// Field identifier for the duplicate-title switch.
pub const KEEP_DUPLICATE_TITLES: &str = "general.keep_duplicate_titles";
/// Field identifier for the default range mode.
pub const DEFAULT_RANGE_MODE: &str = "general.default_range_mode";
/// Field identifier for the scan-on-open switch.
pub const SCAN_ON_OPEN: &str = "general.scan_on_open";

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
    /// the moment it is opened, and an empty answer would give it nothing.
    pub fn new() -> Self {
        Self {
            preview_count: DEFAULT_PREVIEW_FRAMES,
            keep_duplicate_titles: false,
            default_range_mode: RangePreference::Whole,
            scan_on_open: false,
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
        assert_eq!(settings, GeneralSettings::default());
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
        assert_eq!(view.choices.len(), 1);
        assert_eq!(view.choices[0].id, DEFAULT_RANGE_MODE);
        assert_eq!(view.choices[0].selected, "whole");
        assert_eq!(view.choices[0].options.len(), RangePreference::ALL.len());
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
