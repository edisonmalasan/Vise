//! The chapters tab: which markers go into the output, and editing them.
//!
//! Markers come from the source, and a user can drop one, retime one, rename
//! one, or add one of their own. The list the tab shows is the list the encoder
//! gets, so a marker that is excluded is simply not in it, and an edited list
//! only reaches the output when chapter editing is switched on — otherwise the
//! source markers would be copied over the top.

use serde::{Deserialize, Serialize};
use vise_core::{Chapter, ChapterMode, ChapterProcessingOptions};

use crate::tabs::common::{ChoiceGroup, ChoiceGroupViewModel, NumField, TextViewModel};
use crate::tabs::source::{ChapterFacts, TitleFacts};

/// What happens to the chapter markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterHandling {
    /// Copy the source markers unchanged.
    Copy,
    /// Write the marker list the user is looking at.
    Edit,
    /// Write no markers at all.
    None,
}

impl ChapterHandling {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            ChapterHandling::Copy => "copy",
            ChapterHandling::Edit => "edit",
            ChapterHandling::None => "none",
        }
    }

    /// The mode named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "copy" | "keep" | "source" => Some(ChapterHandling::Copy),
            "edit" | "edited" | "replace" => Some(ChapterHandling::Edit),
            "none" | "drop" | "off" => Some(ChapterHandling::None),
            _ => None,
        }
    }
}

/// One marker as the window holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkerSettings {
    /// Chapter name.
    pub name: String,
    /// Start in seconds, as typed.
    pub start: NumField,
    /// End in seconds, as typed.
    pub end: NumField,
    /// Whether the marker goes into the output.
    pub included: bool,
}

impl Default for MarkerSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            start: NumField::blank(),
            end: NumField::blank(),
            included: true,
        }
    }
}

impl MarkerSettings {
    /// A marker for a chapter the scan found.
    pub fn of(chapter: &ChapterFacts) -> Self {
        Self {
            name: chapter.name.clone(),
            start: NumField::of(trim_seconds(chapter.start_seconds)),
            end: NumField::of(trim_seconds(chapter.end_seconds)),
            included: true,
        }
    }
}

fn trim_seconds(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    if (value.fract()).abs() < 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.3}")
    }
}

/// The chapter choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterSettings {
    /// What happens to the markers.
    pub mode: String,
    /// The marker list, edited.
    pub markers: Vec<MarkerSettings>,
    /// Whether the list has been touched, which is what marks it as the user's.
    pub touched: bool,
}

impl ChapterSettings {
    /// What happens to the markers.
    pub fn handling(&self) -> ChapterHandling {
        ChapterHandling::parse(&self.mode).unwrap_or(ChapterHandling::Copy)
    }

    /// Fill the list from the source, keeping any list the user has edited.
    pub fn sync(&mut self, facts: &TitleFacts) {
        if self.touched {
            return;
        }
        self.markers = facts.chapters.iter().map(MarkerSettings::of).collect();
    }

    /// Add a marker after the last one, or at `start` when it is given.
    pub fn add(&mut self, marker: MarkerSettings) {
        self.markers.push(marker);
        self.touched = true;
    }

    /// Change one marker, keeping its place in the list.
    pub fn edit(&mut self, number: u32, marker: MarkerSettings) -> Result<(), String> {
        let index = number.saturating_sub(1) as usize;
        if index >= self.markers.len() {
            return Err(format!("This list has no marker {number}."));
        }
        self.markers[index] = marker;
        self.touched = true;
        Ok(())
    }

    /// Drop one marker.
    pub fn remove(&mut self, number: u32) -> Result<(), String> {
        let index = number.saturating_sub(1) as usize;
        if index >= self.markers.len() {
            return Err(format!("This list has no marker {number}."));
        }
        self.markers.remove(index);
        self.touched = true;
        Ok(())
    }

    /// Include or exclude one marker.
    pub fn include(&mut self, number: u32, included: bool) -> Result<(), String> {
        let index = number.saturating_sub(1) as usize;
        let Some(marker) = self.markers.get_mut(index) else {
            return Err(format!("This list has no marker {number}."));
        };
        marker.included = included;
        self.touched = true;
        Ok(())
    }
}

/// Chapter choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChaptersInput {
    /// What happens to the markers.
    pub mode: Option<String>,
}

impl ChaptersInput {
    /// Apply the change to the marker handling.
    pub fn apply(self, settings: &mut ChapterSettings) -> Result<(), String> {
        if let Some(mode) = self.mode {
            settings.mode = ChapterHandling::parse(&mode)
                .ok_or_else(|| {
                    format!("'{}' is not a chapter mode this build offers.", mode.trim())
                })?
                .as_str()
                .to_string();
        }
        Ok(())
    }
}

/// A marker change as the interface layer sends it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkerInput {
    /// Which change to make.
    pub action: String,
    /// Which marker it applies to, counting from one.
    pub number: Option<u32>,
    /// Chapter name.
    pub name: Option<String>,
    /// Start in seconds.
    pub start: Option<String>,
    /// End in seconds.
    pub end: Option<String>,
    /// Whether the marker goes into the output.
    pub included: Option<bool>,
}

impl MarkerInput {
    /// Apply the change to the marker list.
    pub fn apply(self, settings: &mut ChapterSettings) -> Result<(), String> {
        let action = self.action.trim().to_ascii_lowercase();
        match action.as_str() {
            "add" => {
                settings.add(MarkerSettings {
                    name: self.name.unwrap_or_default().trim().to_string(),
                    start: NumField {
                        text: self.start.unwrap_or_default(),
                    },
                    end: NumField {
                        text: self.end.unwrap_or_default(),
                    },
                    included: self.included.unwrap_or(true),
                });
                Ok(())
            }
            "edit" => {
                let number = self.number.ok_or_else(|| {
                    "Say which marker to change, counting from one.".to_string()
                })?;
                let index = number.saturating_sub(1) as usize;
                let existing = settings
                    .markers
                    .get(index)
                    .cloned()
                    .ok_or_else(|| format!("This list has no marker {number}."))?;
                settings.edit(
                    number,
                    MarkerSettings {
                        name: self.name.unwrap_or(existing.name).trim().to_string(),
                        start: match self.start {
                            Some(text) => NumField { text },
                            None => existing.start,
                        },
                        end: match self.end {
                            Some(text) => NumField { text },
                            None => existing.end,
                        },
                        included: self.included.unwrap_or(existing.included),
                    },
                )
            }
            "remove" => {
                let number = self
                    .number
                    .ok_or_else(|| "Say which marker to remove, counting from one.".to_string())?;
                settings.remove(number)
            }
            "include" | "exclude" => {
                let number = self.number.ok_or_else(|| {
                    "Say which marker to include or exclude, counting from one.".to_string()
                })?;
                settings.include(number, action == "include")
            }
            other => Err(format!(
                "'{other}' is not a chapter change this build offers; use add, edit, remove, include, or exclude."
            )),
        }
    }
}

/// One marker as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerViewModel {
    /// Position in the list, counting from one.
    pub number: u32,
    /// Chapter name.
    pub name: String,
    /// Name field for editing.
    pub name_field: TextViewModel,
    /// Start field.
    pub start: NumFieldViewModel,
    /// End field.
    pub end: NumFieldViewModel,
    /// Whether the marker goes into the output.
    pub included: bool,
    /// What the marker reads as.
    pub label: String,
    /// What has to be fixed about this marker.
    pub problem: String,
}

/// A marker's time field as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumFieldViewModel {
    /// Element identifier of the field.
    pub id: String,
    /// What the user typed.
    pub text: String,
    /// Whether the field can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether the typed value is usable.
    pub valid: bool,
    /// Why it is not, when it is not.
    pub problem: String,
}

/// The chapters tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersTabViewModel {
    /// What happens to the markers.
    pub mode: ChoiceGroupViewModel,
    /// Every marker in the list.
    pub markers: Vec<MarkerViewModel>,
    /// One line describing the plan.
    pub summary: String,
    /// Whether the title has any markers to work with.
    pub available: bool,
    /// Why the tab has nothing to show, when it cannot be used.
    pub reason: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Build the chapters tab.
pub fn tab_view(settings: &ChapterSettings, facts: &TitleFacts) -> ChaptersTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let handling = settings.handling();

    let mut mode = ChoiceGroupViewModel::build(
        "chapterMode",
        "Chapters",
        "What happens to the chapter markers.",
        handling.as_str(),
        &ChoiceGroup::of([
            ("copy", "Copy the source markers"),
            ("edit", "Write the markers on this list"),
            ("none", "Leave them out"),
        ])
        .options,
        true,
        "",
    );

    let source_has_markers = !facts.chapters.is_empty();
    if handling == ChapterHandling::Copy && !source_has_markers {
        mode = mode.blocked("This title has no chapter markers to copy.");
    }
    if handling == ChapterHandling::Edit && settings.markers.is_empty() {
        mode = mode.blocked("The marker list is empty, so there is nothing to write.");
        problems.push("Editing chapters needs at least one marker on the list.".to_string());
    }
    if settings.touched && handling == ChapterHandling::Copy {
        problems.push(
            "The markers have been edited but chapter editing is set to copy the source markers, so the edits would not be used."
                .to_string(),
        );
    }

    let editable = handling == ChapterHandling::Edit;
    let mut markers: Vec<MarkerViewModel> = Vec::new();
    let mut previous_end: Option<f64> = None;
    for (index, marker) in settings.markers.iter().enumerate() {
        let number = index as u32 + 1;
        let start = read_time(&marker.start, &format!("Marker {number} start"));
        let end = read_time(&marker.end, &format!("Marker {number} end"));
        let mut problem = String::new();
        if let (Ok(Some(start_value)), Some(previous)) = (start.as_ref(), previous_end) {
            if *start_value + 0.001 < previous {
                problem = "Markers have to run in order; this one starts before the one above it."
                    .to_string();
            }
        }
        match (&start, &end) {
            (Err(e), _) => problem = e.clone(),
            (_, Err(e)) => problem = e.clone(),
            (Ok(Some(start_value)), Ok(Some(end_value))) => {
                if *end_value <= *start_value {
                    problem = "A marker has to end after it starts.".to_string();
                }
            }
            (Ok(start_value), Ok(end_value)) => {
                if start_value.is_none() && end_value.is_none() && editable {
                    problem = "Give the marker a start time.".to_string();
                }
            }
        }
        if let Ok(Some(start_value)) = &start {
            if let Ok(Some(end_value)) = &end {
                if facts.has_duration() && *end_value > facts.duration_seconds + 0.5 {
                    problem = format!(
                        "This marker ends after the title does, at {}.",
                        crate::viewmodel::format_duration(*end_value)
                    );
                }
            }
            // The next marker has to start where this one finished, not where it
            // started, or the two would overlap.
            previous_end = end.as_ref().ok().copied().flatten().or(Some(*start_value));
        }
        if !problem.is_empty() {
            problems.push(format!("Marker {number}: {problem}"));
        }
        let name = if marker.name.trim().is_empty() {
            format!("Marker {number}")
        } else {
            marker.name.trim().to_string()
        };
        let label = match (&start, &end) {
            (Ok(Some(start_value)), Ok(Some(end_value))) => format!(
                "{name} · {} – {}",
                crate::viewmodel::format_duration(*start_value),
                crate::viewmodel::format_duration(*end_value)
            ),
            _ => name.clone(),
        };
        let closed = if editable {
            String::new()
        } else {
            "Chapter editing is off, so the list is read from the source instead.".to_string()
        };
        let mut name_field = TextViewModel::new(
            &format!("markerName{number}"),
            "Name",
            marker.name.trim(),
            "Chapter name",
        )
        .blocked(closed.clone());
        if !problem.is_empty() {
            name_field = name_field.invalid(problem.clone());
        }
        markers.push(MarkerViewModel {
            number,
            name,
            name_field,
            start: NumFieldViewModel {
                id: format!("markerStart{number}"),
                text: marker.start.text.trim().to_string(),
                available: editable,
                reason: closed.clone(),
                valid: start.is_ok(),
                problem: start.err().unwrap_or_default(),
            },
            end: NumFieldViewModel {
                id: format!("markerEnd{number}"),
                text: marker.end.text.trim().to_string(),
                available: editable,
                reason: closed.clone(),
                valid: end.is_ok(),
                problem: end.err().unwrap_or_default(),
            },
            included: marker.included,
            label,
            problem,
        });
    }

    let included = settings
        .markers
        .iter()
        .filter(|marker| marker.included)
        .count();
    let summary = match handling {
        ChapterHandling::None => "No chapter markers will be written.".to_string(),
        ChapterHandling::Copy => {
            if source_has_markers {
                format!("The {included} source marker(s) are copied unchanged.")
            } else {
                "This title has no markers to copy.".to_string()
            }
        }
        ChapterHandling::Edit => format!(
            "{included} of {} marker(s) on this list will be written.",
            settings.markers.len()
        ),
    };

    let (available, reason) = if !facts.is_chosen() {
        (false, "Choose a title first.".to_string())
    } else if !source_has_markers && !settings.touched {
        (
            false,
            "This title has no chapter markers; add one of your own if the output needs them."
                .to_string(),
        )
    } else {
        (true, String::new())
    };

    ChaptersTabViewModel {
        mode,
        markers,
        summary,
        available,
        reason,
        ready: problems.is_empty(),
        problems,
    }
}

/// Read a time in seconds, refusing anything that is not one.
fn read_time(field: &NumField, label: &str) -> Result<Option<f64>, String> {
    let value = field
        .number(label)
        .map_err(|_| format!("{label} must be a number of seconds."))?;
    if let Some(value) = value {
        if value < 0.0 {
            return Err(format!("{label} cannot be negative."));
        }
    }
    Ok(value)
}

/// The chapter choices as the job model wants them.
pub fn to_job_options(settings: &ChapterSettings) -> ChapterProcessingOptions {
    let handling = settings.handling();
    let chapters: Vec<Chapter> = settings
        .markers
        .iter()
        .filter(|marker| marker.included)
        .map(|marker| Chapter {
            start_seconds: marker.start.number("Start").ok().flatten(),
            end_seconds: marker.end.number("End").ok().flatten(),
            title: Some(marker.name.trim().to_string()).filter(|name| !name.is_empty()),
            id: None,
            tags: Default::default(),
        })
        .collect();
    ChapterProcessingOptions {
        mode: Some(match handling {
            ChapterHandling::Copy => ChapterMode::Copy,
            ChapterHandling::Edit => ChapterMode::Edit,
            ChapterHandling::None => ChapterMode::None,
        }),
        copy: handling == ChapterHandling::Copy,
        edit: handling == ChapterHandling::Edit,
        chapters,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            duration_seconds: 3600.0,
            chapters: vec![
                ChapterFacts {
                    number: 1,
                    name: "Opening".into(),
                    start_seconds: 0.0,
                    end_seconds: 600.0,
                },
                ChapterFacts {
                    number: 2,
                    name: "Middle".into(),
                    start_seconds: 600.0,
                    end_seconds: 1800.0,
                },
            ],
            ..Default::default()
        }
    }

    fn synced() -> ChapterSettings {
        let mut settings = ChapterSettings::default();
        settings.sync(&facts());
        settings
    }

    #[test]
    fn the_source_markers_are_copied_by_default() {
        let settings = synced();
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.markers.len(), 2);
        assert_eq!(view.markers[0].label, "Opening · 0:00 – 10:00");
        assert!(view.markers.iter().all(|marker| marker.included));
        assert!(view.summary.contains("copied unchanged"));
        assert!(
            !view.markers[0].start.available,
            "the list is read from the source"
        );
        assert!(view.markers[0]
            .start
            .reason
            .contains("Chapter editing is off"));
    }

    #[test]
    fn editing_a_marker_makes_the_tab_ask_for_editing_to_be_switched_on() {
        let mut settings = synced();
        MarkerInput {
            action: "edit".into(),
            number: Some(1),
            name: Some("Cold open".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("edit");
        assert_eq!(settings.markers[0].name, "Cold open");
        let view = tab_view(&settings, &facts());
        assert!(!view.ready);
        assert!(
            view.problems
                .iter()
                .any(|p| p.contains("chapter editing is set to copy")),
            "{:?}",
            view.problems
        );

        settings.mode = "edit".into();
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.markers[0].start.available);
        assert!(view.summary.contains("2 of 2 marker(s) on this list"));
    }

    #[test]
    fn a_marker_can_be_edited_retimed_and_removed() {
        let mut settings = synced();
        settings.mode = "edit".into();
        MarkerInput {
            action: "edit".into(),
            number: Some(2),
            start: Some("700".into()),
            end: Some("1900".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("retime");
        assert_eq!(settings.markers[1].start, NumField::of("700"));
        let view = tab_view(&settings, &facts());
        assert_eq!(view.markers[1].label, "Middle · 11:40 – 31:40");

        MarkerInput {
            action: "add".into(),
            name: Some("End".into()),
            start: Some("1900".into()),
            end: Some("3600".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("add");
        assert_eq!(settings.markers.len(), 3);

        MarkerInput {
            action: "remove".into(),
            number: Some(2),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("remove");
        assert_eq!(settings.markers.len(), 2);
        assert_eq!(settings.markers[1].name, "End");
    }

    #[test]
    fn excluding_a_marker_takes_it_out_of_the_written_list() {
        let mut settings = synced();
        settings.mode = "edit".into();
        MarkerInput {
            action: "exclude".into(),
            number: Some(1),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("exclude");
        assert!(!settings.markers[0].included);
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.summary.contains("1 of 2"));
        let options = to_job_options(&settings);
        assert_eq!(options.chapters.len(), 1);
        assert_eq!(options.chapters[0].title.as_deref(), Some("Middle"));
        assert_eq!(options.mode, Some(ChapterMode::Edit));
    }

    #[test]
    fn markers_must_run_in_order_and_end_after_they_start() {
        let mut settings = synced();
        settings.mode = "edit".into();
        MarkerInput {
            action: "edit".into(),
            number: Some(2),
            start: Some("300".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("retime backwards");
        let view = tab_view(&settings, &facts());
        assert!(!view.ready);
        assert!(
            view.problems.iter().any(|p| p.contains("run in order")),
            "{:?}",
            view.problems
        );

        let mut backwards = synced();
        backwards.mode = "edit".into();
        MarkerInput {
            action: "edit".into(),
            number: Some(1),
            end: Some("0".into()),
            ..Default::default()
        }
        .apply(&mut backwards)
        .expect("zero length");
        let view = tab_view(&backwards, &facts());
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("end after it starts")));
    }

    #[test]
    fn a_marker_cannot_end_after_the_title_does() {
        let mut settings = synced();
        settings.mode = "edit".into();
        MarkerInput {
            action: "edit".into(),
            number: Some(2),
            end: Some("7200".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("too late");
        let view = tab_view(&settings, &facts());
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("ends after the title does")));
    }

    #[test]
    fn a_marker_with_a_time_that_is_not_a_number_is_named_in_the_message() {
        let mut settings = synced();
        settings.mode = "edit".into();
        MarkerInput {
            action: "edit".into(),
            number: Some(1),
            start: Some("soon".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("bad time");
        let view = tab_view(&settings, &facts());
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("must be a number of seconds")));
        assert!(!view.markers[0].start.valid);

        MarkerInput {
            action: "edit".into(),
            number: Some(1),
            start: Some("-5".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("negative");
        let view = tab_view(&settings, &facts());
        assert!(view.problems.iter().any(|p| p.contains("negative")));
    }

    #[test]
    fn leaving_chapters_out_writes_nothing() {
        let mut settings = synced();
        settings.mode = "none".into();
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.summary, "No chapter markers will be written.");
        let options = to_job_options(&settings);
        assert_eq!(options.mode, Some(ChapterMode::None));
        assert!(!options.build().contains("-map_chapters"));
    }

    #[test]
    fn a_title_without_markers_says_so_and_still_allows_adding_one() {
        let bare = TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            duration_seconds: 120.0,
            ..Default::default()
        };
        let mut settings = ChapterSettings::default();
        settings.sync(&bare);
        let view = tab_view(&settings, &bare);
        assert!(!view.available);
        assert!(
            view.reason.contains("no chapter markers"),
            "{}",
            view.reason
        );
        assert!(!view.mode.available);

        MarkerInput {
            action: "add".into(),
            name: Some("Only".into()),
            start: Some("0".into()),
            end: Some("120".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("add");
        settings.mode = "edit".into();
        let view = tab_view(&settings, &bare);
        assert!(view.available, "the user has a list of their own now");
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_change_that_names_nothing_is_refused() {
        let mut settings = synced();
        let unknown = MarkerInput {
            action: "shuffle".into(),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown change");
        assert!(unknown.contains("shuffle"), "{unknown}");

        let missing = MarkerInput {
            action: "remove".into(),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("no number");
        assert!(missing.contains("which marker"), "{missing}");

        let absent = MarkerInput {
            action: "remove".into(),
            number: Some(9),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("no such marker");
        assert!(absent.contains("no marker 9"), "{absent}");

        let mode = ChaptersInput {
            mode: Some("sideways".into()),
        }
        .apply(&mut settings)
        .expect_err("unknown mode");
        assert!(mode.contains("sideways"), "{mode}");
    }

    #[test]
    fn a_valid_change_is_applied() {
        let mut settings = ChapterSettings::default();
        ChaptersInput {
            mode: Some("Edit".into()),
        }
        .apply(&mut settings)
        .expect("mode");
        assert_eq!(settings.handling(), ChapterHandling::Edit);
        assert!(!settings.touched);
        settings.sync(&facts());
        assert!(!settings.touched, "reading the source list is not an edit");
    }
}
