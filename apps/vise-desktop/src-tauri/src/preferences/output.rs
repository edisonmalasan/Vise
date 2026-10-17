//! The output tab: where finished files are written and what a collision does.
//!
//! Four preferences, and each one is read by the queue rather than by this
//! module:
//!
//! * **Output folder** — the folder the queue selection sheet starts with, and
//!   the folder the main window's output path is measured against. It is also
//!   written back to the settings document's own output-folder member, because
//!   that member is what the rest of the application already reads, and two
//!   answers to one question is worse than either.
//! * **File name pattern** — the same `{title}` and `{n}` tokens the selection
//!   sheet already resolves, so a pattern chosen here and a pattern typed there
//!   produce the same file name.
//! * **Replace files that already exist** and **what two jobs wanting the same
//!   name does** — one answer between them. `windows::queue` reads both as a
//!   single `CollisionPolicy` and resolves the whole batch before anything is
//!   queued, so two titles that would resolve to one file are either given
//!   different names or refused together rather than the second overwriting the
//!   first.
//!
//! The two are mutually exclusive by construction rather than by a rule the user
//! has to learn: turning the switch on settles the collision choice to
//! "replace", and choosing any other collision turns the switch off. Asking for
//! both is refused by name.

use serde::{Deserialize, Serialize};

use crate::preferences::{
    text_control, want_one_of, ChoiceGroupViewModel, PreferenceTabViewModel, ToggleViewModel,
};
use crate::tabs::common::ChoiceOption;
use crate::windows::queue::DEFAULT_NAME_PATTERN;

/// Field identifier for the output folder.
pub const FOLDER: &str = "output.folder";
/// Field identifier for the file name pattern.
pub const NAME_PATTERN: &str = "output.name_pattern";
/// Field identifier for the replace switch.
pub const OVERWRITE: &str = "output.overwrite";
/// Field identifier for the collision choice.
pub const COLLISION: &str = "output.collision";

/// What happens when two jobs want to write the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPreference {
    /// Refuse the batch and name the file.
    #[default]
    Ask,
    /// Add a number to the second and later names.
    Renumber,
    /// Leave out the titles whose files already exist.
    Skip,
    /// Write over what is there.
    Replace,
}

impl CollisionPreference {
    /// Every choice, in the order the picker lists them.
    pub const ALL: [CollisionPreference; 4] = [
        CollisionPreference::Ask,
        CollisionPreference::Renumber,
        CollisionPreference::Skip,
        CollisionPreference::Replace,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            CollisionPreference::Ask => "ask",
            CollisionPreference::Renumber => "renumber",
            CollisionPreference::Skip => "skip",
            CollisionPreference::Replace => "replace",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            CollisionPreference::Ask => "Ask me, and queue nothing",
            CollisionPreference::Renumber => "Add a number until the name is free",
            CollisionPreference::Skip => "Leave out the ones already written",
            CollisionPreference::Replace => "Write over what is there",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            CollisionPreference::Ask => {
                "The whole batch is refused and the file that is in the way is named."
            }
            CollisionPreference::Renumber => {
                "A title whose name is taken — by an earlier title in the batch, or by a file that is already there — becomes ' (2)', then ' (3)', so nothing is lost."
            }
            CollisionPreference::Skip => {
                "A title whose file is already there is left out of the queue, and the rest go ahead."
            }
            CollisionPreference::Replace => "The finished file is written over the one that is there.",
        }
    }

    /// Whether this choice writes over a file that already exists.
    ///
    /// The switch and the choice are one answer, so this is what settles the
    /// switch: a preference that says "replace" and a switch that says "do not"
    /// would be two questions where there is one.
    pub fn replaces(self) -> bool {
        matches!(self, CollisionPreference::Replace)
    }

    /// Read a choice the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            COLLISION,
            value,
            &[
                (
                    CollisionPreference::Ask.id(),
                    CollisionPreference::Ask.label(),
                ),
                (
                    CollisionPreference::Renumber.id(),
                    CollisionPreference::Renumber.label(),
                ),
                (
                    CollisionPreference::Skip.id(),
                    CollisionPreference::Skip.label(),
                ),
                (
                    CollisionPreference::Replace.id(),
                    CollisionPreference::Replace.label(),
                ),
            ],
        )?;
        CollisionPreference::ALL
            .into_iter()
            .find(|choice| choice.id() == typed)
            .ok_or_else(|| format!("'{typed}' is not something this build does about a collision."))
    }
}

/// The output preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    /// The folder finished files are written into.
    pub folder: String,
    /// The pattern a file name is built from.
    pub name_pattern: String,
    /// Whether an existing file is written over.
    pub overwrite: bool,
    /// What two jobs wanting the same name does.
    pub collision: CollisionPreference,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// The folder starts empty, which is not a usable answer and is reported as
    /// such: there is no folder this build can guess that is right on somebody
    /// else's machine, and a queue that wrote into a guessed folder would be
    /// worse than one that asks. The name pattern is the one the selection sheet
    /// already uses, and a collision asks rather than guessing.
    pub fn new() -> Self {
        Self {
            folder: String::new(),
            name_pattern: DEFAULT_NAME_PATTERN.to_string(),
            overwrite: false,
            collision: CollisionPreference::Ask,
        }
    }

    /// Whether these two answers agree, which they always do once settled.
    pub fn is_settled(&self) -> bool {
        self.overwrite == self.collision.replaces()
    }
}

/// Settle the switch and the collision choice into one answer.
///
/// Called after every change rather than trusted, so the record can never hold a
/// switch and a choice that disagree — including a record that arrived from a
/// file this build did not write.
pub fn settle_collision(settings: &mut OutputSettings) {
    if settings.overwrite {
        settings.collision = CollisionPreference::Replace;
    } else if settings.collision.replaces() {
        settings.overwrite = true;
    }
}

/// Read the output folder.
///
/// The folder is checked for being a folder here rather than at encode time,
/// because a folder that is not there is a mistake the user can still correct,
/// and finding out when they press Queue is too late.
pub fn read_folder(value: &serde_json::Value) -> Result<String, String> {
    let typed = crate::preferences::want_text(FOLDER, value)?;
    let folder = typed.trim().to_string();
    if folder.is_empty() {
        return Err(
            "Output folder needs a folder; type one or choose it from the folder picker."
                .to_string(),
        );
    }
    if folder.chars().any(|character| character.is_control()) {
        return Err("Output folder cannot contain a control character.".to_string());
    }
    let path = std::path::Path::new(&folder);
    if !path.is_dir() {
        return Err(format!(
            "{folder} is not a folder that exists. Create it, or choose one that does."
        ));
    }
    Ok(folder)
}

/// Read the file name pattern.
///
/// The pattern is checked with the same rules the selection sheet applies to a
/// name it builds, because a pattern that cannot produce a file name would
/// otherwise be stored and only discovered when a batch was queued.
pub fn read_pattern(value: &serde_json::Value) -> Result<String, String> {
    let typed = crate::preferences::want_text(NAME_PATTERN, value)?;
    let pattern = typed.trim().to_string();
    if pattern.is_empty() {
        return Err("File name pattern cannot be empty; {title} is the title's own name and {n} its number."
            .to_string());
    }
    // `{title}` and `{n}` are replaced before anything else, so a pattern made
    // only of tokens has to be checked on a name it would actually produce.
    let sample = crate::windows::queue::output_name(&pattern, "Title 1", 1)?;
    let bad = sample.chars().find(|character| {
        matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        )
    });
    if let Some(bad) = bad {
        return Err(format!(
            "File name pattern leaves '{bad}' in the file name; choose a pattern that does not."
        ));
    }
    Ok(pattern)
}

/// The problems these preferences have.
pub fn validate(settings: &OutputSettings) -> Vec<String> {
    let mut problems = Vec::new();
    let folder = settings.folder.trim();
    if folder.is_empty() {
        problems.push(
            "Output folder is empty; choose the folder finished files are written into."
                .to_string(),
        );
    } else if !std::path::Path::new(folder).is_dir() {
        problems.push(format!(
            "Output folder {folder} is not a folder that exists."
        ));
    }
    if settings.name_pattern.trim().is_empty() {
        problems.push("File name pattern is empty.".to_string());
    }
    if !settings.is_settled() {
        problems.push(
            "Replace files that already exist and the collision choice cannot both be set."
                .to_string(),
        );
    }
    problems
}

/// Build the output tab.
pub fn tab_view(settings: &OutputSettings) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "output",
        "Output",
        "Where finished files are written, how they are named, and what a name collision does.",
    );
    let mut folder = text_control(
        "outputFolder",
        "Output folder",
        "Finished files are written here. The queue selection sheet starts in this folder.",
        &settings.folder,
        "C:\\media\\finished",
    )
    .with_hint("The whole batch is refused if a title would write a file that is already there.");
    let folder_problem = validate(settings)
        .into_iter()
        .find(|problem| problem.starts_with("Output folder"))
        .unwrap_or_default();
    if !folder_problem.is_empty() {
        folder = folder.invalid(folder_problem);
    }
    tab.texts.push(folder);
    tab.texts.push(text_control(
        "outputNamePattern",
        "File name pattern",
        "Built from the title's name and number. {title} is the name, {n} the number.",
        &settings.name_pattern,
        "{title}",
    ));
    let mut replace = ToggleViewModel::new(
        "outputOverwrite",
        "Replace files that already exist",
        "A finished file is written over one that is already there.",
        settings.overwrite,
    );
    if !settings.overwrite && settings.collision == CollisionPreference::Replace {
        replace = replace
            .with_hint("Turned on, because the collision choice below says files are replaced.")
            .blocked("The collision choice is set to replace, so this is already on.");
    }
    tab.toggles.push(replace);
    let options: Vec<ChoiceOption> = CollisionPreference::ALL
        .iter()
        .map(|choice| ChoiceOption::new(choice.id(), choice.label()))
        .collect();
    tab.choices.push(
        ChoiceGroupViewModel::build(
            COLLISION,
            "When two jobs want the same file name",
            "Names are planned for the whole batch before anything is queued.",
            settings.collision.id(),
            &options,
            true,
            "",
        )
        .with_hint("Leaving this on 'ask' means a collision is refused rather than resolved."),
    );
    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

/// The collision policy the queue resolves a batch with.
///
/// This is the shape `windows::queue` wants, built here from the two stored
/// answers, so the queue never has to know how the preference is spelled.
pub fn policy(settings: &OutputSettings) -> crate::windows::queue::CollisionPolicy {
    crate::windows::queue::CollisionPolicy::of(settings.overwrite, settings.collision)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    /// The output-folder field as the page renders it.
    fn folder_field(settings: &OutputSettings) -> crate::tabs::common::TextViewModel {
        tab_view(settings).texts[0].clone()
    }

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    fn real_folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-output-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = OutputSettings::new();
        assert!(settings.folder.is_empty(), "there is no folder to guess");
        assert_eq!(settings.name_pattern, DEFAULT_NAME_PATTERN);
        assert!(!settings.overwrite);
        assert_eq!(settings.collision, CollisionPreference::Ask);
        assert!(settings.is_settled());
        assert_eq!(settings, OutputSettings::default());
    }

    #[test]
    fn an_empty_folder_is_refused_by_name_rather_than_stored() {
        let error = read_folder(&value("\"   \"")).expect_err("no folder");
        assert!(error.contains("Output folder"), "{error}");
        let error = read_folder(&value("7")).expect_err("not text");
        assert!(error.contains("text"), "{error}");
    }

    #[test]
    fn a_folder_that_is_not_there_is_refused_with_the_path_that_was_typed() {
        let error = read_folder(&value("\"Z:\\\\nowhere\\\\at\\\\all\"")).expect_err("no folder");
        assert!(error.contains("nowhere"), "{error}");
        assert!(error.contains("not a folder that exists"), "{error}");
    }

    #[test]
    fn a_folder_that_exists_is_accepted_and_trimmed() {
        let folder = real_folder("accept");
        let raw = serde_json::Value::String(format!("  {}  ", folder.display()));
        assert_eq!(
            read_folder(&raw).expect("folder"),
            folder.display().to_string()
        );
    }

    #[test]
    fn the_name_pattern_is_checked_on_a_name_it_would_actually_produce() {
        assert_eq!(
            read_pattern(&value("\"{title} {n}\"")).expect("tokens"),
            "{title} {n}"
        );
        let error = read_pattern(&value("\"\"")).expect_err("empty");
        assert!(error.contains("File name pattern"), "{error}");
        // Whitespace is a name that trims away to nothing, and it is caught here
        // rather than when a batch is queued.
        let error = read_pattern(&value("\"   \"")).expect_err("blank");
        assert!(error.contains("File name pattern"), "{error}");
        // A separator cannot appear in a file name, so a pattern that leaves one
        // is refused before it can plan a batch it cannot write.
        let error = read_pattern(&value("\"{title}/{n}\"")).expect_err("separator");
        assert!(error.contains('/'), "{error}");
        // A pattern long enough to push any name past the limit is refused too,
        // using the same rule the queue applies to a name it builds.
        let long = "x".repeat(crate::windows::queue::MAX_OUTPUT_NAME + 1);
        let error = read_pattern(&value(&serde_json::Value::String(long).to_string()))
            .expect_err("too long");
        assert!(error.contains("characters or fewer"), "{error}");
    }

    #[test]
    fn the_switch_and_the_collision_choice_are_settled_into_one_answer() {
        let mut settings = OutputSettings::new();
        settings.overwrite = true;
        settle_collision(&mut settings);
        assert_eq!(settings.collision, CollisionPreference::Replace);
        assert!(settings.is_settled());

        settings.overwrite = false;
        settings.collision = CollisionPreference::Renumber;
        settle_collision(&mut settings);
        assert!(
            !settings.overwrite,
            "choosing to renumber turns the switch off"
        );
        assert!(settings.is_settled());

        settings.collision = CollisionPreference::Replace;
        settle_collision(&mut settings);
        assert!(
            settings.overwrite,
            "choosing to replace turns the switch on"
        );
        assert!(settings.is_settled());
    }

    #[test]
    fn a_record_whose_two_answers_disagree_is_reported_and_the_queue_gets_one() {
        // The stored record can come from a file this build did not write, so the
        // disagreement is caught on the way in rather than trusted.
        let mut stored = OutputSettings {
            folder: real_folder("disagree").display().to_string(),
            overwrite: false,
            collision: CollisionPreference::Replace,
            ..OutputSettings::new()
        };
        assert!(!stored.is_settled());
        let problems = validate(&stored);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("cannot both be set")),
            "{problems:?}"
        );
        // Settling is what the setter does, so the queue is handed one answer.
        settle_collision(&mut stored);
        assert!(stored.is_settled());
        assert!(validate(&stored).is_empty());
        assert_eq!(
            policy(&stored).describe(),
            "an existing file is written over, so two titles may share a name"
        );
    }

    #[test]
    fn the_tab_shows_the_folder_as_a_problem_when_there_is_not_one() {
        let empty = folder_field(&OutputSettings::new());
        assert!(!empty.valid);
        assert!(empty.problem.contains("Output folder"), "{}", empty.problem);

        let ready = OutputSettings {
            folder: real_folder("ready").display().to_string(),
            ..OutputSettings::new()
        };
        let field = folder_field(&ready);
        assert!(field.valid, "{}", field.problem);
        let view = tab_view(&ready);
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.toggles.len(), 1);
        assert_eq!(view.choices.len(), 1);
        assert_eq!(view.choices[0].selected, "ask");
        assert_eq!(view.texts.len(), 2);
    }

    #[test]
    fn the_switch_that_the_collision_choice_already_decides_is_shown_closed() {
        let settings = OutputSettings {
            folder: real_folder("closed").display().to_string(),
            overwrite: false,
            collision: CollisionPreference::Replace,
            ..OutputSettings::new()
        };
        let toggle = tab_view(&settings).toggles[0].clone();
        assert!(!toggle.available);
        assert!(toggle.reason.contains("already on"), "{}", toggle.reason);
    }

    #[test]
    fn an_unknown_collision_is_refused_with_the_names_that_would_work() {
        let error = CollisionPreference::read(&value("\"clobber\"")).expect_err("unknown");
        assert!(error.contains("clobber"), "{error}");
        assert!(error.contains("renumber"), "{error}");
        assert_eq!(
            CollisionPreference::read(&value("\" SKIP \"")).expect("skip"),
            CollisionPreference::Skip
        );
    }
}
