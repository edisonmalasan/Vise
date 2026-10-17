//! The preset manager.
//!
//! Presets are stored through `vise_core`'s preset API, so the window is only
//! responsible for deciding what a user may do to them: a name has to be there,
//! two presets cannot share one, a category has to be one this build knows, and
//! only one preset can be the default. Everything else — reading, writing,
//! importing, exporting — is the core's job and is called as it is.
//!
//! The split matters because a preset that the window accepted but the core
//! would have rejected would appear in the list and then fail on the next
//! launch. The validation here is the same validation, expressed in the terms
//! the user is looking at.

use serde::{Deserialize, Serialize};

use vise_core::{AudioCodec, BuiltInPreset, ContainerFormat, PresetSpec, UserPreset, VideoCodec};

/// Identifier of the preset surface.
pub const SURFACE: &str = "presets";

/// The categories a user preset can be filed under.
pub const CATEGORIES: [(&str, &str); 4] = [
    ("general", "General"),
    ("web", "Web"),
    ("devices", "Devices"),
    ("professional", "Professional"),
];

/// The category label for `id`.
pub fn category_label(id: &str) -> &'static str {
    CATEGORIES
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .map(|(_, label)| *label)
        .unwrap_or("General")
}

/// The category named by `text`, refusing anything else.
pub fn parse_category(text: &str) -> Result<&'static str, String> {
    let wanted = text.trim().to_ascii_lowercase();
    CATEGORIES
        .iter()
        .find(|(id, _)| *id == wanted)
        .map(|(id, _)| *id)
        .ok_or_else(|| format!("'{wanted}' is not a preset category this build offers."))
}

/// What the window is doing with a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetAction {
    /// Store the settings on the tabs as a new preset.
    Create,
    /// Rename or re-describe an existing preset.
    Edit,
    /// Store a copy under another name.
    Clone,
    /// Remove a stored preset.
    Delete,
    /// Apply a preset to the tabs.
    Apply,
    /// Read a preset out of a file and store it alongside the others.
    Import,
    /// Write a preset to a file.
    Export,
    /// Make a preset the one that starts every new job.
    SetDefault,
    /// Stop using a preset as the default.
    ClearDefault,
}

impl PresetAction {
    /// Stable identifier the page sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            PresetAction::Create => "create",
            PresetAction::Edit => "edit",
            PresetAction::Clone => "clone",
            PresetAction::Delete => "delete",
            PresetAction::Apply => "apply",
            PresetAction::Import => "import",
            PresetAction::Export => "export",
            PresetAction::SetDefault => "set_default",
            PresetAction::ClearDefault => "clear_default",
        }
    }

    /// Whether this action destroys the stored preset.
    pub fn is_destructive(self) -> bool {
        matches!(self, PresetAction::Delete)
    }

    /// Whether the form needs a name before it can be submitted.
    pub fn needs_name(self) -> bool {
        !matches!(
            self,
            PresetAction::Delete
                | PresetAction::Apply
                | PresetAction::Import
                | PresetAction::Export
                | PresetAction::SetDefault
                | PresetAction::ClearDefault
        )
    }

    /// Whether the action reads or writes a file, and so needs a path.
    pub fn needs_path(self) -> bool {
        matches!(self, PresetAction::Import | PresetAction::Export)
    }
}

/// The edit as the page sends it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PresetInput {
    /// Which action to take.
    pub action: String,
    /// Which preset, for the actions that name one.
    pub id: String,
    /// The name.
    pub name: String,
    /// What the preset is for.
    pub description: String,
    /// Which category it is filed under.
    pub category: String,
    /// Whether this preset starts every new job.
    pub default: bool,
    /// The file an import reads or an export writes.
    pub path: String,
}

impl PresetInput {
    /// The action as an enum.
    pub fn action(&self) -> Result<PresetAction, String> {
        match self.action.trim().to_ascii_lowercase().as_str() {
            "" | "create" | "add" | "new" => Ok(PresetAction::Create),
            "edit" | "rename" | "update" => Ok(PresetAction::Edit),
            "clone" | "copy" | "duplicate" => Ok(PresetAction::Clone),
            "delete" | "remove" => Ok(PresetAction::Delete),
            "apply" | "use" => Ok(PresetAction::Apply),
            "import" | "load" => Ok(PresetAction::Import),
            "export" | "save_as" | "save as" => Ok(PresetAction::Export),
            "set_default" | "default" => Ok(PresetAction::SetDefault),
            "clear_default" | "unset_default" => Ok(PresetAction::ClearDefault),
            other => Err(format!(
                "'{other}' is not something this window can do with a preset."
            )),
        }
    }
}

/// One preset, as the list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetViewModel {
    /// Stable identifier; empty for the built-in presets.
    pub id: String,
    /// The name.
    pub name: String,
    /// What it is for, or a placeholder.
    pub description: String,
    /// Category label.
    pub category: String,
    /// Stable category identifier.
    pub category_id: String,
    /// Whether this is one of the presets that ships with the build.
    pub built_in: bool,
    /// Whether this preset is the default for new jobs.
    pub is_default: bool,
    /// Whether the page may change it, which built-in presets may not.
    pub editable: bool,
    /// Why it may not be changed, when it may not.
    pub edit_reason: String,
    /// One line describing what the preset sets.
    pub summary: String,
    /// Every control on the row, with its availability.
    pub actions: Vec<PresetActionViewModel>,
}

/// One control on a preset row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetActionViewModel {
    /// Stable identifier the page sends back.
    pub id: String,
    /// Label on the control.
    pub label: String,
    /// Whether it can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether using it asks for confirmation first.
    pub confirms: bool,
}

impl PresetActionViewModel {
    /// An action that is on offer.
    pub fn open(id: &str, label: &str, confirms: bool) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: true,
            reason: String::new(),
            confirms,
        }
    }

    /// The action, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// The form the page edits, as the window holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFormViewModel {
    /// Which action the form will perform.
    pub action: String,
    /// Whether the name field can be used.
    pub name_available: bool,
    /// Why the name field is closed, when it is.
    pub name_reason: String,
    /// What is in the name field.
    pub name: String,
    /// What is in the description field.
    pub description: String,
    /// Whether the description field can be used.
    pub description_available: bool,
    /// Why the description field is closed, when it is.
    pub description_reason: String,
    /// Every category, with the chosen one marked.
    pub categories: Vec<PresetOptionViewModel>,
    /// The chosen category.
    pub selected_category: String,
    /// Whether the default switch can be used.
    pub default_available: bool,
    /// Why the default switch is closed, when it is.
    pub default_reason: String,
    /// Whether the preset is the default.
    pub default: bool,
    /// The file an import reads or an export writes.
    pub path: String,
    /// Whether the file field can be used.
    pub path_available: bool,
    /// Why the file field is closed, when it is.
    pub path_reason: String,
    /// Whether the form may be submitted.
    pub can_submit: bool,
    /// Label on the control that submits the form.
    pub submit_label: String,
    /// Everything that has to be fixed first.
    pub problems: Vec<String>,
}

/// One option in the category picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetOptionViewModel {
    /// Stable identifier sent back.
    pub id: String,
    /// Label on the option.
    pub label: String,
    /// Whether this is the chosen option.
    pub selected: bool,
}

/// The preset manager, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    /// The presets that ship with the build.
    pub built_in: Vec<PresetViewModel>,
    /// The presets the user has stored.
    pub user: Vec<PresetViewModel>,
    /// The form.
    pub form: PresetFormViewModel,
    /// Whether any preset is the default.
    pub has_default: bool,
    /// The identifier of the default preset.
    pub default_id: String,
    /// The default preset's name, for the summary line.
    pub default_name: String,
    /// How many presets are listed.
    pub total: usize,
    /// One line summarising the library.
    pub summary: String,
    /// Whether there is nothing to show.
    pub empty: bool,
    /// Where stored presets are kept, so the user can find the file.
    pub store_path: String,
    /// The controls that act on the library as a whole.
    ///
    /// Importing and exporting are not row actions — they read and write files —
    /// so they sit at the top of the window with their reasons beside them.
    pub controls: Vec<PresetActionViewModel>,
}

/// The window's own bookkeeping of the preset library.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresetState {
    /// Where the stored presets live.
    pub path: std::path::PathBuf,
    /// Which preset starts every new job.
    pub default_id: String,
    /// Which category each stored preset is filed under.
    pub categories: std::collections::BTreeMap<String, String>,
}

impl PresetState {
    /// A library stored at `path`.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            default_id: String::new(),
            categories: std::collections::BTreeMap::new(),
        }
    }

    /// The category a stored preset is filed under, defaulting to general.
    pub fn category_of(&self, id: &str) -> &str {
        self.categories
            .get(id)
            .map(String::as_str)
            .unwrap_or("general")
    }

    /// Remember which preset is the default.
    pub fn set_default(&mut self, id: &str) {
        self.default_id = id.trim().to_string();
    }

    /// Forget the default.
    pub fn clear_default(&mut self) {
        self.default_id.clear();
    }

    /// Remember a preset's category.
    pub fn set_category(&mut self, id: &str, category: &str) {
        self.categories.insert(id.to_string(), category.to_string());
    }

    /// Forget a preset that has been deleted.
    pub fn forget(&mut self, id: &str) {
        if self.default_id == id {
            self.default_id.clear();
        }
        self.categories.remove(id);
    }

    /// The name of the default preset, when one is set.
    pub fn default_name(&self, user: &[UserPreset], built_in: &[BuiltInPreset]) -> String {
        if self.default_id.is_empty() {
            return String::new();
        }
        if let Some(found) = user.iter().find(|preset| preset.id == self.default_id) {
            return found.name.clone();
        }
        if let Some(found) = built_in.iter().find(|preset| preset.id == self.default_id) {
            return found.name.clone();
        }
        String::new()
    }

    /// Build the whole window for the stored `user` presets.
    pub fn view(
        &self,
        user: &[UserPreset],
        built_in: &[BuiltInPreset],
        form: PresetFormViewModel,
    ) -> PresetView {
        let built_in_rows: Vec<PresetViewModel> = built_in
            .iter()
            .map(|preset| PresetViewModel {
                id: preset.id.clone(),
                name: preset.name.clone(),
                description: preset
                    .description
                    .clone()
                    .unwrap_or_else(|| "A preset that ships with this build.".to_string()),
                category: category_label("general").to_string(),
                category_id: "general".to_string(),
                built_in: true,
                is_default: self.default_id == preset.id,
                editable: false,
                edit_reason:
                    "A preset that ships with the build cannot be renamed; copy it instead."
                        .to_string(),
                summary: preset_summary(&preset.preset),
                actions: vec![
                    PresetActionViewModel::open("apply", "Apply", false),
                    PresetActionViewModel::open("clone", "Copy", false),
                    PresetActionViewModel::open("export", "Export", false),
                    PresetActionViewModel::open("set_default", "Make default", false)
                        .blocked_or(self.default_id != preset.id, || {
                            "This preset already starts every new job.".to_string()
                        }),
                    PresetActionViewModel::open("delete", "Delete", true).blocked(
                        "A preset that ships with the build cannot be deleted; copy it first.",
                    ),
                ],
            })
            .collect();
        let user_rows: Vec<PresetViewModel> = user
            .iter()
            .map(|preset| {
                let category = self.category_of(&preset.id);
                let is_default = self.default_id == preset.id;
                PresetViewModel {
                    id: preset.id.clone(),
                    name: preset.name.clone(),
                    description: preset.description.clone().unwrap_or_default(),
                    category: category_label(category).to_string(),
                    category_id: category.to_string(),
                    built_in: false,
                    is_default,
                    editable: true,
                    edit_reason: String::new(),
                    summary: preset_summary(&preset.preset),
                    actions: vec![
                        PresetActionViewModel::open("apply", "Apply", false),
                        PresetActionViewModel::open("clone", "Copy", false),
                        PresetActionViewModel::open("set_default", "Make default", false)
                            .blocked_or(!is_default, || {
                                "This preset already starts every new job.".to_string()
                            }),
                        PresetActionViewModel::open(
                            "clear_default",
                            "Stop using as default",
                            false,
                        )
                        .blocked_or(is_default, || "This preset is not the default.".to_string()),
                        PresetActionViewModel::open("export", "Export", false),
                        PresetActionViewModel::open("edit", "Rename", false),
                        PresetActionViewModel::open("delete", "Delete", true),
                    ],
                }
            })
            .collect();
        let default_name = self.default_name(user, built_in);
        let total = built_in_rows.len() + user_rows.len();
        let controls = vec![
            PresetActionViewModel::open("import", "Import a preset file", false),
            PresetActionViewModel::open("export", "Export the chosen preset", false)
                .blocked_or(total > 0, || {
                    "There is no preset to export yet.".to_string()
                }),
            PresetActionViewModel::open("create", "Save the current settings", false),
        ];
        PresetView {
            summary: match (total, default_name.is_empty()) {
                (0, _) => "No presets are available.".to_string(),
                (_, true) => format!("{total} preset(s); none is the default."),
                (_, false) => format!("{total} preset(s); '{default_name}' is the default."),
            },
            default_name,
            has_default: !self.default_id.is_empty(),
            default_id: self.default_id.clone(),
            total,
            empty: total == 0,
            store_path: self.path.display().to_string(),
            built_in: built_in_rows,
            user: user_rows,
            controls,
            form,
        }
    }
}

impl PresetActionViewModel {
    /// The action when `available` holds, or blocked with the reason it does not.
    fn blocked_or(self, available: bool, reason: impl FnOnce() -> String) -> Self {
        if available {
            self
        } else {
            self.blocked(reason())
        }
    }
}

/// One line describing what a preset sets.
pub fn preset_summary(preset: &PresetSpec) -> String {
    let container = preset
        .container
        .or(preset.format)
        .map(container_label)
        .unwrap_or_else(|| "the chosen container".to_string());
    let video = preset
        .video_codec
        .or(preset.video)
        .map(video_label)
        .unwrap_or_else(|| "the chosen encoder".to_string());
    let audio = preset
        .audio_codec
        .or(preset.audio)
        .map(audio_label)
        .unwrap_or_else(|| "the chosen audio encoder".to_string());
    let quality = match preset.quality {
        Some(value) => format!(" quality {value}"),
        None => String::new(),
    };
    let subtitles = match preset.subtitle_mode.or(preset.subtitles) {
        Some(mode) => format!(", subtitles {mode:?}").to_ascii_lowercase(),
        None => String::new(),
    };
    format!("{container} · {video} · {audio}{quality}{subtitles}")
}

/// The label for a container.
pub fn container_label(container: ContainerFormat) -> String {
    format!("{container:?}").to_ascii_lowercase()
}

/// The label for a video encoder.
pub fn video_label(codec: VideoCodec) -> String {
    format!("{codec:?}").to_ascii_lowercase()
}

/// The label for an audio encoder.
pub fn audio_label(codec: AudioCodec) -> String {
    format!("{codec:?}").to_ascii_lowercase()
}

/// The file extension a preset exchange file carries.
///
/// The core reads and writes its preset files as JSON, so a name without this
/// extension is refused rather than written and then found unreadable.
pub const PRESET_FILE_EXTENSION: &str = "json";

/// Why a preset cannot be read from `path`, when it cannot.
///
/// A missing path and a path the media tools would not read are answered here,
/// where the page can put the reason under the field, rather than as a decoding
/// failure later on.
pub fn import_path_problem(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Some("Choose a preset file to import.".to_string());
    }
    let file = std::path::Path::new(trimmed);
    if file.is_dir() {
        return Some(format!("'{trimmed}' is a folder, not a preset file."));
    }
    if !trimmed
        .to_ascii_lowercase()
        .ends_with(&format!(".{PRESET_FILE_EXTENSION}"))
    {
        return Some(format!(
            "A preset file is a .{PRESET_FILE_EXTENSION} file; '{trimmed}' does not end in one."
        ));
    }
    if !file.is_file() {
        return Some(format!("There is no preset file at '{trimmed}'."));
    }
    None
}

/// Why a preset cannot be written to `path`, when it cannot.
pub fn export_path_problem(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Some("Choose where the preset should be written.".to_string());
    }
    let file = std::path::Path::new(trimmed);
    if file.is_dir() {
        return Some(format!("'{trimmed}' is a folder, not a preset file."));
    }
    if !trimmed
        .to_ascii_lowercase()
        .ends_with(&format!(".{PRESET_FILE_EXTENSION}"))
    {
        return Some(format!(
            "A preset file is a .{PRESET_FILE_EXTENSION} file; '{trimmed}' does not end in one."
        ));
    }
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && !parent.is_dir() => Some(format!(
            "'{}' is not a folder that exists.",
            parent.display()
        )),
        _ => None,
    }
}

/// Why a form cannot be submitted, as a list.
///
/// A name is required because a preset with no name cannot be picked from a
/// list; a duplicate name is a warning rather than a refusal, because two
/// differently-tuned presets may well want the same words.
pub fn form_problems(action: PresetAction, name: &str, existing: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let name = name.trim();
    if action.needs_name() && name.is_empty() {
        problems.push("Give the preset a name.".to_string());
    }
    if (action == PresetAction::Create || action == PresetAction::Clone)
        && !name.is_empty()
        && existing
            .iter()
            .any(|other| other.trim().eq_ignore_ascii_case(name))
    {
        problems.push(format!(
            "Another preset is already called '{name}'; storing this one anyway would make the list hard to read."
        ));
    }
    problems
}

/// Whether the form may be submitted.
///
/// A duplicate name is a warning rather than a refusal, so it does not close
/// this; every other problem does.
pub fn can_submit(action: PresetAction, name: &str, existing: &[String]) -> bool {
    form_problems(action, name, existing)
        .iter()
        .all(|problem| problem.starts_with("Another preset"))
}

/// Build the form the page edits.
pub fn form_view(
    input: &PresetInput,
    existing: &[String],
    found: Option<&UserPreset>,
) -> PresetFormViewModel {
    let action = input.action().unwrap_or(PresetAction::Create);
    let deleting = action == PresetAction::Delete;
    let name = if input.name.trim().is_empty() {
        found.map(|preset| preset.name.clone()).unwrap_or_default()
    } else {
        input.name.clone()
    };
    let problems = form_problems(action, &name, existing);
    let category = parse_category(&input.category).unwrap_or("general");
    let category_problem = if input.category.trim().is_empty() {
        String::new()
    } else {
        parse_category(&input.category).err().unwrap_or_default()
    };
    let mut all = problems.clone();
    let can_submit = can_submit(action, &name, existing)
        && category_problem.is_empty()
        && !problems
            .iter()
            .any(|problem| problem == "Give the preset a name.");
    if !category_problem.is_empty() {
        all.push(category_problem);
    }
    PresetFormViewModel {
        action: action.as_str().to_string(),
        name_available: action.needs_name(),
        name_reason: if action.needs_name() {
            String::new()
        } else {
            format!("{}ing a preset does not need a name.", action_word(action))
        },
        name,
        description: if input.description.trim().is_empty() {
            found
                .and_then(|preset| preset.description.clone())
                .unwrap_or_default()
        } else {
            input.description.clone()
        },
        description_available: action.needs_name(),
        description_reason: if action.needs_name() {
            String::new()
        } else {
            format!(
                "{}ing a preset does not need a description.",
                action_word(action)
            )
        },
        categories: CATEGORIES
            .iter()
            .map(|(id, label)| PresetOptionViewModel {
                id: (*id).to_string(),
                label: (*label).to_string(),
                selected: *id == category,
            })
            .collect(),
        selected_category: category.to_string(),
        default_available: !deleting,
        default_reason: if deleting {
            "A preset that is being deleted cannot be the default.".to_string()
        } else {
            String::new()
        },
        default: input.default,
        path: input.path.trim().to_string(),
        path_available: action.needs_path(),
        path_reason: if action.needs_path() {
            String::new()
        } else {
            format!(
                "{}ing a preset does not read or write a file.",
                action_word(action)
            )
        },
        can_submit,
        submit_label: submit_label(action).to_string(),
        problems: all,
    }
}

/// The label on the control that submits the form for `action`.
///
/// The page does not choose it: one window has one set of words for what it can
/// do with a preset, so the same action is never named two ways.
pub fn submit_label(action: PresetAction) -> &'static str {
    match action {
        PresetAction::Create => "Store the current settings",
        PresetAction::Edit => "Save the changes",
        PresetAction::Clone => "Store the copy",
        PresetAction::Delete => "Delete this preset",
        PresetAction::Apply => "Apply this preset",
        PresetAction::Import => "Import this file",
        PresetAction::Export => "Export this preset",
        PresetAction::SetDefault => "Use for every new job",
        PresetAction::ClearDefault => "Stop using this preset",
    }
}

/// The verb a person would use for `action`, for a sentence about it.
fn action_word(action: PresetAction) -> &'static str {
    match action {
        PresetAction::Delete => "delet",
        PresetAction::Apply => "apply",
        PresetAction::Import => "import",
        PresetAction::Export => "export",
        PresetAction::SetDefault => "setting a default for",
        PresetAction::ClearDefault => "clearing the default for",
        PresetAction::Create => "creat",
        PresetAction::Clone => "cop",
        PresetAction::Edit => "renam",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> PresetSpec {
        PresetSpec {
            container: Some(ContainerFormat::Mp4),
            video_codec: Some(VideoCodec::H264),
            audio_codec: Some(AudioCodec::Aac),
            quality: Some(22),
            ..Default::default()
        }
    }

    fn stored(id: &str, name: &str) -> UserPreset {
        UserPreset {
            id: id.to_string(),
            name: name.to_string(),
            description: Some("my own".to_string()),
            preset: spec(),
        }
    }

    fn input(action: &str) -> PresetInput {
        PresetInput {
            action: action.to_string(),
            id: String::new(),
            name: String::new(),
            description: String::new(),
            category: "general".to_string(),
            default: false,
            path: String::new(),
        }
    }

    #[test]
    fn a_preset_name_is_required_before_anything_is_stored() {
        let existing = vec!["Fast 1080p".to_string()];
        let problems = form_problems(PresetAction::Create, "   ", &existing);
        assert_eq!(problems, vec!["Give the preset a name."]);
        assert!(!can_submit(PresetAction::Create, "  ", &existing));
        assert!(form_problems(PresetAction::Delete, "", &existing).is_empty());
    }

    #[test]
    fn a_duplicate_name_is_a_warning_rather_than_a_refusal() {
        let existing = vec!["fast 1080p".to_string()];
        let problems = form_problems(PresetAction::Create, "Fast 1080p", &existing);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with("Another preset"));
        assert!(can_submit(PresetAction::Create, "Fast 1080p", &existing));
        assert!(
            can_submit(PresetAction::Create, "Fast 1080p plus", &existing),
            "a different name is fine"
        );
        // Renaming a preset onto its own name is not a duplicate.
        assert!(form_problems(PresetAction::Edit, "Fast 1080p", &existing).is_empty());
    }

    #[test]
    fn a_category_this_build_does_not_know_is_refused_by_name() {
        assert_eq!(parse_category(" Web ").expect("web"), "web");
        assert!(parse_category("hardware")
            .expect_err("unknown category")
            .contains("hardware"));
        assert_eq!(category_label("devices"), "Devices");
        assert_eq!(category_label("nonsense"), "General");
        assert_eq!(CATEGORIES.len(), 4);
    }

    #[test]
    fn the_form_reports_every_thing_that_has_to_be_fixed() {
        let form = form_view(&input("create"), &[], None);
        assert!(!form.can_submit);
        assert_eq!(form.problems, vec!["Give the preset a name."]);
        assert!(form.name_available);
        assert!(form.description_available);
        assert_eq!(form.action, "create");
        assert_eq!(form.categories.len(), 4);
        assert!(form.categories[0].selected);
        assert!(form.default_available);

        let ready = form_view(
            &PresetInput {
                name: "My preset".into(),
                category: "web".into(),
                default: true,
                ..input("create")
            },
            &[],
            None,
        );
        assert!(ready.can_submit);
        assert_eq!(ready.selected_category, "web");
        assert!(ready.default);
        assert!(ready.categories[1].selected);

        let mut bad = input("create");
        bad.category = "nonsense".into();
        let form = form_view(&bad, &[], None);
        assert!(!form.can_submit);
        assert!(
            form.problems.iter().any(|p| p.contains("nonsense")),
            "{:?}",
            form.problems
        );
    }

    #[test]
    fn the_form_closes_the_fields_a_delete_or_an_apply_does_not_need() {
        let found = stored("u1", "Mine");
        let delete = form_view(&input("delete"), &[], Some(&found));
        assert_eq!(delete.name, "Mine");
        assert!(!delete.name_available);
        assert!(delete.name_reason.contains("does not need a name"));
        assert!(!delete.description_available);
        assert!(delete
            .description_reason
            .contains("does not need a description"));
        assert!(!delete.default_available);
        assert!(delete.can_submit, "deleting needs nothing else");

        let apply = form_view(&input("apply"), &[], Some(&found));
        assert!(!apply.name_available);
        assert!(apply.can_submit);

        let edit = form_view(
            &PresetInput {
                action: "edit".into(),
                id: "u1".into(),
                name: String::new(),
                description: String::new(),
                category: "general".into(),
                default: false,
                path: String::new(),
            },
            &[],
            Some(&found),
        );
        assert_eq!(edit.name, "Mine", "the stored name fills an empty field");
        assert_eq!(edit.description, "my own");
        assert!(edit.can_submit);
    }

    #[test]
    fn an_action_this_window_cannot_do_is_refused_by_name() {
        assert!(input("create").action().expect("create") == PresetAction::Create);
        assert!(input("").action().expect("empty is create") == PresetAction::Create);
        assert!(input("clone").action().expect("clone") == PresetAction::Clone);
        assert!(input("apply").action().expect("apply") == PresetAction::Apply);
        assert!(input("import").action().expect("import") == PresetAction::Import);
        assert!(input("export").action().expect("export") == PresetAction::Export);
        let error = input("transmogrify").action().expect_err("unknown");
        assert!(error.contains("transmogrify"), "{error}");
        assert!(PresetAction::Delete.is_destructive());
        assert!(!PresetAction::Apply.is_destructive());
        assert!(!PresetAction::Import.is_destructive());
    }

    #[test]
    fn only_the_exchange_actions_read_or_write_a_file() {
        assert!(PresetAction::Import.needs_path());
        assert!(PresetAction::Export.needs_path());
        assert!(!PresetAction::Create.needs_path());
        assert!(!PresetAction::Delete.needs_path());
        // An imported preset brings its own name, so none is asked for.
        assert!(!PresetAction::Import.needs_name());
        assert!(!PresetAction::Export.needs_name());

        let form = form_view(
            &PresetInput {
                path: "C:\\out\\mine.json".to_string(),
                ..input("import")
            },
            &[],
            None,
        );
        assert!(form.path_available);
        assert_eq!(form.path, "C:\\out\\mine.json");
        assert!(
            form.can_submit,
            "the file itself is checked when it is read"
        );
        assert!(!form.name_available);
        assert!(form.name_reason.contains("does not need a name"));

        let create = form_view(&input("create"), &[], None);
        assert!(!create.path_available);
        assert!(create.path_reason.contains("does not read or write a file"));
    }

    #[test]
    fn the_submit_control_is_named_by_the_window_not_the_page() {
        assert_eq!(
            submit_label(PresetAction::Create),
            "Store the current settings"
        );
        assert_eq!(submit_label(PresetAction::Delete), "Delete this preset");
        assert_eq!(submit_label(PresetAction::Import), "Import this file");
        assert_eq!(submit_label(PresetAction::Export), "Export this preset");
        let form = form_view(&input("clone"), &[], None);
        assert_eq!(form.submit_label, "Store the copy");
        assert!(form
            .problems
            .contains(&"Give the preset a name.".to_string()));
    }

    #[test]
    fn a_preset_exchange_file_has_to_be_a_json_file_that_exists() {
        let dir = std::env::temp_dir().join("vise-desktop-preset-exchange");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("mine.json");
        std::fs::write(&file, b"{}").expect("file");

        assert!(import_path_problem("   ")
            .expect("blank")
            .contains("Choose"));
        assert!(import_path_problem("C:\\out\\mine.txt")
            .expect("extension")
            .contains(".json"));
        assert!(import_path_problem(dir.to_str().unwrap())
            .expect("folder")
            .contains("folder"));
        assert!(import_path_problem("C:\\definitely\\not\\here.json")
            .expect("missing")
            .contains("no preset file"));
        assert_eq!(import_path_problem(file.to_str().unwrap()), None);

        assert!(export_path_problem("").expect("blank").contains("written"));
        assert!(export_path_problem("mine.txt")
            .expect("extension")
            .contains(".json"));
        assert!(export_path_problem(dir.to_str().unwrap()).is_some());
        assert_eq!(export_path_problem(file.to_str().unwrap()), None);
        assert_eq!(
            export_path_problem(dir.join("new.json").to_str().unwrap()),
            None,
            "a file that is not there yet can still be written"
        );
        assert!(
            export_path_problem(&format!("{}\\missing\\new.json", dir.display()))
                .expect("missing folder")
                .contains("not a folder that exists")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_library_offers_import_and_export_above_the_list() {
        let mut state = PresetState::new("presets.json");
        let built_in = vise_core::builtin_presets();
        let empty = state.view(&[], &[], form_view(&input("create"), &[], None));
        let control = |view: &PresetView, id: &str| {
            view.controls
                .iter()
                .find(|control| control.id == id)
                .cloned()
                .expect("control")
        };
        assert!(control(&empty, "import").available);
        let export = control(&empty, "export");
        assert!(!export.available);
        assert!(export.reason.contains("no preset to export"));

        state.set_category("u1", "web");
        let filled = state.view(
            &[stored("u1", "Mine")],
            &built_in,
            form_view(&input("create"), &[], None),
        );
        assert!(control(&filled, "export").available);
        assert!(filled.user[0]
            .actions
            .iter()
            .any(|action| action.id == "export" && action.available));
        assert!(filled.built_in[0]
            .actions
            .iter()
            .any(|action| action.id == "export" && action.available));
    }

    #[test]
    fn the_library_lists_built_in_and_stored_presets_separately() {
        let mut state = PresetState::new("C:\\app\\presets.json");
        let built_in = vise_core::builtin_presets();
        let user = vec![stored("u1", "Mine"), stored("u2", "Yours")];
        state.set_category("u2", "professional");
        let view = state.view(&user, &built_in, form_view(&input("create"), &[], None));

        assert_eq!(view.built_in.len(), 3);
        assert_eq!(view.user.len(), 2);
        assert_eq!(view.total, 5);
        assert!(!view.empty);
        assert_eq!(view.user[0].category, "General");
        assert_eq!(view.user[1].category, "Professional");
        assert!(view.built_in.iter().all(|preset| preset.built_in));
        assert!(view.user.iter().all(|preset| !preset.built_in));
        assert!(view.store_path.contains("presets.json"));
        assert!(view.summary.contains("none is the default"));
        assert!(view.built_in[0].summary.contains("mp4"));
        assert!(view.built_in[0].summary.contains("h264"));
    }

    #[test]
    fn a_built_in_preset_cannot_be_renamed_or_deleted_but_can_be_copied() {
        let built_in = vise_core::builtin_presets();
        let view = PresetState::new("presets.json").view(
            &[],
            &built_in,
            form_view(&input("create"), &[], None),
        );
        let row = &view.built_in[0];
        let action = |id: &str| {
            row.actions
                .iter()
                .find(|action| action.id == id)
                .cloned()
                .expect("action")
        };
        assert!(action("apply").available);
        assert!(action("clone").available);
        assert!(action("set_default").available);
        assert!(!action("delete").available);
        assert!(action("delete").reason.contains("ships with the build"));
        assert!(action("delete").confirms);
        assert!(!row.editable);
    }

    #[test]
    fn exactly_one_preset_is_the_default() {
        let built_in = vise_core::builtin_presets();
        let user = vec![stored("u1", "Mine")];
        let mut state = PresetState::new("presets.json");
        state.set_default("u1");
        let view = state.view(&user, &built_in, form_view(&input("create"), &[], None));
        assert!(view.has_default);
        assert_eq!(view.default_name, "Mine");
        assert!(view.summary.contains("'Mine' is the default"));
        assert!(view.user[0].is_default);
        let set_default = view.user[0]
            .actions
            .iter()
            .find(|action| action.id == "set_default")
            .expect("action");
        assert!(!set_default.available);
        let clear = view.user[0]
            .actions
            .iter()
            .find(|action| action.id == "clear_default")
            .expect("action");
        assert!(clear.available);

        state.clear_default();
        let view = state.view(&user, &built_in, form_view(&input("create"), &[], None));
        assert!(!view.has_default);
        assert!(view.user[0]
            .actions
            .iter()
            .any(|action| action.id == "clear_default" && !action.available));
    }

    #[test]
    fn a_default_that_points_at_a_deleted_preset_is_reported_as_no_name() {
        let built_in = vise_core::builtin_presets();
        let mut state = PresetState::new("presets.json");
        state.set_default("gone");
        let view = state.view(&[], &built_in, form_view(&input("create"), &[], None));
        assert!(view.has_default);
        assert!(view.default_name.is_empty());

        state.forget("gone");
        let view = state.view(&[], &built_in, form_view(&input("create"), &[], None));
        assert!(!view.has_default);
        state.set_category("u1", "web");
        state.forget("u1");
        assert_eq!(state.category_of("u1"), "general");
    }

    #[test]
    fn a_preset_summary_names_what_it_leaves_alone() {
        let bare = PresetSpec::default();
        let summary = preset_summary(&bare);
        assert!(summary.contains("the chosen container"), "{summary}");
        assert!(summary.contains("the chosen encoder"), "{summary}");
        assert!(!summary.contains("quality"), "{summary}");

        let subtitles = PresetSpec {
            subtitle_mode: Some(vise_core::SubtitleMode::BurnIn),
            crop: Some((0, 0, 100, 100)),
            scale_to: Some((50, 50)),
            ..Default::default()
        };
        let summary = preset_summary(&subtitles);
        assert!(summary.contains("burnin"), "{summary}");
        assert_eq!(audio_label(AudioCodec::Aac), "aac");
        assert_eq!(video_label(VideoCodec::H265), "h265");
        assert_eq!(container_label(ContainerFormat::Webm), "webm");
    }
}
