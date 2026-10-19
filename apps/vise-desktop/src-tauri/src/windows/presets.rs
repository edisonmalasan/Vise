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

/// The categories a preset can be filed under.
///
/// These are the catalog's six, read from the catalog rather than written here
/// twice: a list of four categories in this window and six in the catalog would
/// mean the same word filing two different things.
pub fn categories() -> Vec<(&'static str, &'static str)> {
    vise_core::CATEGORIES
        .iter()
        .map(|category| (category.id(), category.label()))
        .collect()
}

/// The category named by `text`, refusing anything else.
pub fn parse_category(text: &str) -> Result<&'static str, String> {
    let wanted = text.trim().to_ascii_lowercase();
    vise_core::PresetCategory::parse(&wanted)
        .map(|category| category.id())
        .ok_or_else(|| {
            format!(
                "'{wanted}' is not a preset category this build offers; choose {}.",
                category_names()
            )
        })
}

/// The category names, in one sentence, for a refusal.
pub fn category_names() -> String {
    categories()
        .iter()
        .map(|(_, label)| *label)
        .collect::<Vec<_>>()
        .join(", ")
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
    /// Which players or platforms it is meant for, or nothing.
    ///
    /// This is the guidance a person needs before choosing a preset, so it is on
    /// the row rather than in a file they would have to open.
    pub compatibility: String,
    /// The encoder that will run, named the way the media tools name it.
    pub encoder: String,
    /// The same encoder, in words.
    pub encoder_label: String,
    /// Whether that encoder runs on a chip.
    pub hardware: bool,
    /// Whether the preset was written for a chip, whatever this machine does
    /// with it.
    ///
    /// This is what a person reads to understand why a preset is in the hardware
    /// list at all, and it does not change with the machine: a preset written for
    /// a chip is written for a chip even where the encoder turns out to be
    /// missing.
    pub needs_hardware: bool,
    /// The chip it was written for, in words, when it needs one.
    pub family: String,
    /// Whether it is the software stand-in for the encoder it was written for.
    pub fallback: bool,
    /// Whether this preset can be used on this machine at all.
    pub available: bool,
    /// Why it cannot be, when it cannot.
    pub available_reason: String,
    /// What is happening instead, in words, when something was substituted.
    pub available_note: String,
    /// Whether this is the entry the browser has open.
    pub selected: bool,
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

/// The catalog browser, as the page renders it.
///
/// The six categories are the strip; the entries in the chosen category are the
/// list; the entry that is open is read out in full beside it. Nothing here is a
/// decision the page could have made: which categories exist, which entries they
/// hold, which encoder each one will actually run, and why anything is closed are
/// all answered by the core and sent as finished text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogViewModel {
    /// The schema version the shipped catalog is written in.
    pub version: String,
    /// Every category, with its count and the chosen one marked.
    pub categories: Vec<PresetCategoryViewModel>,
    /// The chosen category.
    pub active_category: String,
    /// The entries in the chosen category, in catalog order.
    pub entries: Vec<PresetViewModel>,
    /// The entry that is open, or nothing when the category is empty.
    pub detail: Option<PresetViewModel>,
    /// How many presets the whole catalog holds.
    pub total: usize,
    /// How many of them this machine can use.
    pub usable: usize,
    /// How many will run in software rather than on the chip they were written
    /// for.
    pub fallback: usize,
    /// Whether the media tools were found, and so whether anything has been
    /// checked at all.
    pub checked: bool,
    /// One line describing the whole catalog on this machine.
    pub summary: String,
    /// What has to be true before the answers below mean anything.
    pub note: String,
    /// Whether there is nothing in the chosen category.
    pub empty: bool,
}

/// One category on the strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCategoryViewModel {
    /// Stable identifier the page sends back.
    pub id: String,
    /// The name on the strip.
    pub label: String,
    /// What belongs in it.
    pub description: String,
    /// How many presets it holds.
    pub count: usize,
    /// Whether the strip has it open.
    pub selected: bool,
}

/// The preset manager, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    /// The presets that ship with the build, browsed by category.
    pub catalog: CatalogViewModel,
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

/// What the page sends when it moves around the catalog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogInput {
    /// The category to open.
    pub category: String,
    /// The entry to open inside it.
    pub entry: String,
}

/// One built-in preset's description, in the language in force.
///
/// Three answers rather than one, because the catalog carries the base sentence
/// for every preset it ships and a locale carries translations of a subset of
/// them: a translated sentence where there is one, the catalog's own words where
/// there is not, and the preset's name where the catalog has neither. A user who
/// cannot read the base sentence gets the base sentence and a language picker
/// rather than an empty row.
fn described_in(
    id: &str,
    locale: &str,
    entry: &vise_core::PresetEntry,
    preset: &PresetSpec,
) -> String {
    if let Some(found) = vise_core::localized_description(id, locale) {
        if found.translated {
            return found.text;
        }
        if !found.text.trim().is_empty() {
            return found.text;
        }
    }
    if entry.description.trim().is_empty() {
        preset
            .name
            .clone()
            .unwrap_or_else(|| "A preset that ships with this build.".to_string())
    } else {
        entry.description.clone()
    }
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
    /// Which category the browser has open.
    pub catalog_category: String,
    /// Which entry the browser has open inside it.
    pub catalog_entry: String,
    /// What the media tools were found to offer.
    ///
    /// Empty until [`PresetState::observe_hardware`] has run, which is what makes
    /// "not checked" different from "checked and found nothing".
    pub capabilities: vise_core::HardwareCapabilities,
    /// What the user has asked the machine to use.
    pub acceleration: vise_core::HardwareAcceleration,
    /// The language the catalog's descriptions are shown in.
    ///
    /// Empty until the language module says which one, and empty means the base
    /// language: the fallback chain answers for a tag nothing is known about, which
    /// is the right answer for a state that has not been told anything. Held here
    /// rather than read from a global so a window has one owner for what it shows
    /// and a second path cannot disagree with it.
    pub locale: String,
}

impl PresetState {
    /// A library stored at `path`.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            default_id: String::new(),
            categories: std::collections::BTreeMap::new(),
            catalog_category: default_category().to_string(),
            catalog_entry: String::new(),
            capabilities: vise_core::HardwareCapabilities::default(),
            acceleration: vise_core::HardwareAcceleration::auto(),
            locale: vise_core::i18n::BASE_LOCALE.to_string(),
        }
    }

    /// Show the catalog's descriptions in another language.
    ///
    /// Reports whether the language moved, so a caller that repaints on every
    /// change is not repainting a browser that did not change.
    pub fn use_language(&mut self, tag: &str) -> bool {
        let resolved = vise_core::resolve_locale(tag);
        let wanted = resolved.locale.tag().to_string();
        if self.locale == wanted {
            return false;
        }
        self.locale = wanted;
        true
    }

    /// Record what the media tools were found to offer.
    pub fn observe_hardware(
        &mut self,
        capabilities: &vise_core::HardwareCapabilities,
        acceleration: &vise_core::HardwareAcceleration,
    ) {
        self.capabilities = capabilities.clone();
        self.acceleration = acceleration.clone();
    }

    /// Whether the media tools have been looked for yet.
    pub fn is_checked(&self) -> bool {
        self.capabilities.ffmpeg_path.is_some()
    }

    /// Show `category` in the browser, refusing anything this build does not
    /// file.
    pub fn set_catalog_category(&mut self, category: &str) -> Result<(), String> {
        let wanted = category.trim();
        if wanted.is_empty() {
            self.catalog_category = default_category().to_string();
            self.catalog_entry.clear();
            return Ok(());
        }
        if vise_core::PresetCategory::parse(wanted).is_none() {
            return Err(format!(
                "'{wanted}' is not a preset category this build offers; choose {}.",
                category_names()
            ));
        }
        if self.catalog_category != wanted {
            // The open entry belongs to the category it was chosen in; opening
            // another category opens that category's first entry rather than a
            // selection the user cannot see.
            self.catalog_entry.clear();
        }
        self.catalog_category = wanted.to_string();
        Ok(())
    }

    /// Open `entry` in the browser, refusing an identifier that is not a preset
    /// this build carries.
    pub fn set_catalog_entry(&mut self, entry: &str) -> Result<(), String> {
        let wanted = entry.trim();
        if wanted.is_empty() {
            self.catalog_entry.clear();
            return Ok(());
        }
        let found = vise_core::builtin_entry(wanted)
            .ok_or_else(|| format!("There is no preset called '{wanted}' in this build."))?;
        self.catalog_category = found.category.id().to_string();
        self.catalog_entry = found.id.clone();
        Ok(())
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
    ///
    /// The presets that ship with the build are not listed here as a flat block:
    /// they are browsed by category, with the availability of each one resolved
    /// against what this machine was found to have, so a preset that cannot run
    /// here is shown closed with the reason beside it rather than as a button
    /// that would fail at encode time.
    pub fn view(
        &self,
        user: &[UserPreset],
        built_in: &[BuiltInPreset],
        form: PresetFormViewModel,
    ) -> PresetView {
        let catalog = self.catalog_view();
        let user_rows: Vec<PresetViewModel> = user
            .iter()
            .map(|preset| {
                let category = self.category_of(&preset.id);
                let is_default = self.default_id == preset.id;
                let entry = vise_core::user_preset_to_entry(
                    preset,
                    vise_core::PresetCategory::parse(category)
                        .unwrap_or(vise_core::PresetCategory::General),
                );
                let resolution = entry.resolve(&self.capabilities, &self.acceleration);
                let mut row = self.row(&entry, &preset.preset, &resolution, is_default, false);
                row.actions = vec![
                    PresetActionViewModel::open("apply", "Apply", false).blocked_or(
                        resolution.available,
                        || {
                            format!(
                                "This preset cannot run on this machine. {}",
                                resolution.reason
                            )
                        },
                    ),
                    PresetActionViewModel::open("clone", "Copy", false),
                    PresetActionViewModel::open("set_default", "Make default", false)
                        .blocked_or(!is_default, || {
                            "This preset already starts every new job.".to_string()
                        }),
                    PresetActionViewModel::open("clear_default", "Stop using as default", false)
                        .blocked_or(is_default, || "This preset is not the default.".to_string()),
                    PresetActionViewModel::open("export", "Export", false),
                    PresetActionViewModel::open("edit", "Rename", false),
                    PresetActionViewModel::open("delete", "Delete", true),
                ];
                row
            })
            .collect();
        let default_name = self.default_name(user, built_in);
        let total = catalog.total + user_rows.len();
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
            catalog,
            user: user_rows,
            controls,
            form,
        }
    }

    /// One row, whatever kind of preset it is.
    ///
    /// Whether the preset ships with the build is passed in rather than worked
    /// out from the identifier: a stored preset that happens to carry an
    /// identifier the catalog also uses is still a stored preset, and marking it
    /// unchangeable because of a name collision would be wrong.
    fn row(
        &self,
        entry: &vise_core::PresetEntry,
        preset: &PresetSpec,
        resolution: &vise_core::PresetResolution,
        is_default: bool,
        built_in: bool,
    ) -> PresetViewModel {
        PresetViewModel {
            id: entry.id.clone(),
            name: entry.name.clone(),
            description: if built_in {
                described_in(&entry.id, &self.locale, entry, preset)
            } else if entry.description.trim().is_empty() {
                preset
                    .name
                    .clone()
                    .unwrap_or_else(|| "A preset that ships with this build.".to_string())
            } else {
                // A stored preset's description is the user's own words, so there
                // is nothing here to translate and nothing to fall back from.
                entry.description.clone()
            },
            category: entry.category.label().to_string(),
            category_id: entry.category.id().to_string(),
            built_in,
            is_default,
            editable: !built_in,
            edit_reason: if built_in {
                "A preset that ships with the build cannot be renamed; copy it instead.".to_string()
            } else {
                String::new()
            },
            summary: preset_summary(preset),
            compatibility: entry.compatibility.clone().unwrap_or_default(),
            encoder: resolution.encoder.identifier().to_string(),
            encoder_label: resolution.encoder.label().to_string(),
            hardware: resolution.hardware,
            needs_hardware: entry
                .family
                .map(|family| family != vise_core::HardwareFamily::Software)
                .unwrap_or(false)
                || entry
                    .encoder
                    .map(|encoder| encoder.is_hardware())
                    .unwrap_or(false),
            family: entry
                .family
                .map(|family| family.label().to_string())
                .unwrap_or_default(),
            fallback: resolution.fallback,
            available: resolution.available,
            available_reason: resolution.reason.clone(),
            available_note: resolution.note.clone(),
            // Only the browser selects a row; the stored list is not part of it.
            selected: false,
            actions: Vec::new(),
        }
    }

    /// The catalog browser, resolved against this machine.
    pub fn catalog_view(&self) -> CatalogViewModel {
        let category =
            vise_core::PresetCategory::parse(&self.catalog_category).unwrap_or_else(|| {
                vise_core::PresetCategory::parse(default_category()).expect("a category")
            });
        let mut entries: Vec<PresetViewModel> = Vec::new();
        for entry in vise_core::builtin_entries_in(category) {
            let resolution = entry.resolve(&self.capabilities, &self.acceleration);
            let is_default = self.default_id == entry.id;
            let mut row = self.row(entry, &entry.spec, &resolution, is_default, true);
            row.actions = vec![
                PresetActionViewModel::open("apply", "Apply", false).blocked_or(
                    resolution.available,
                    || {
                        format!(
                            "This preset cannot run on this machine. {}",
                            resolution.reason
                        )
                    },
                ),
                PresetActionViewModel::open("clone", "Copy", false),
                PresetActionViewModel::open("export", "Export", false),
                PresetActionViewModel::open("set_default", "Make default", false)
                    .blocked_or(!is_default, || {
                        "This preset already starts every new job.".to_string()
                    }),
                PresetActionViewModel::open("delete", "Delete", true).blocked(
                    "A preset that ships with the build cannot be deleted; copy it first.",
                ),
            ];
            entries.push(row);
        }
        // The open entry is the one the user asked for, or the first in the
        // category: a browser that opened on nothing would be a list with no way
        // to read any of it. The answer is derived rather than stored, so a
        // category that changed underneath a stale selection still opens on
        // something the user can see.
        let selected_id = if entries.iter().any(|row| row.id == self.catalog_entry) {
            self.catalog_entry.clone()
        } else {
            entries
                .first()
                .map(|row| row.id.clone())
                .unwrap_or_default()
        };
        for row in entries.iter_mut() {
            row.selected = row.id == selected_id;
        }
        let detail = entries.iter().find(|row| row.selected).cloned();
        let all = vise_core::builtin_entries();
        let usable = all
            .iter()
            .filter(|entry| {
                entry
                    .resolve(&self.capabilities, &self.acceleration)
                    .available
            })
            .count();
        let fallback = all
            .iter()
            .filter(|entry| {
                entry
                    .resolve(&self.capabilities, &self.acceleration)
                    .fallback
            })
            .count();
        let checked = self.is_checked();
        CatalogViewModel {
            version: vise_core::builtin_catalog().version.label(),
            categories: vise_core::CATEGORIES
                .iter()
                .map(|each| PresetCategoryViewModel {
                    id: each.id().to_string(),
                    label: each.label().to_string(),
                    description: each.description().to_string(),
                    count: vise_core::builtin_entries_in(*each).len(),
                    selected: *each == category,
                })
                .collect(),
            active_category: category.id().to_string(),
            total: all.len(),
            usable,
            fallback,
            checked,
            summary: match (checked, fallback) {
                (false, _) => format!(
                    "{} presets; this machine has not been checked yet, so nothing below has \
                     been resolved yet.",
                    all.len()
                ),
                (true, 0) if usable == all.len() => format!(
                    "{} presets; every one of them runs here as written.",
                    all.len()
                ),
                (true, 0) => format!(
                    "{} presets; {usable} run here as written and {} need something this machine \
                     does not have.",
                    all.len(),
                    all.len() - usable
                ),
                (true, fallback) => format!(
                    "{} presets; {usable} run here, {fallback} of them in software because the \
                     chip they were written for is not here.",
                    all.len()
                ),
            },
            note: if checked {
                if self.acceleration.is_enabled()
                    && self.acceleration.mode != vise_core::HardwareMode::Software
                    && self.acceleration.preference != vise_core::HardwarePreference::Disable
                {
                    "Hardware encoding is on, so a preset written for a chip runs on it when the \
                     media tools offer it."
                        .to_string()
                } else {
                    "Hardware encoding is switched off, so every hardware preset will run its \
                     software stand-in instead."
                        .to_string()
                }
            } else {
                "The media tools have not been found, so no preset has been checked against this \
                 machine. Every preset is left open rather than closed on the strength of a probe \
                 that never ran."
                    .to_string()
            },
            empty: entries.is_empty(),
            entries,
            detail,
        }
    }
}

/// The category the browser opens on.
pub fn default_category() -> &'static str {
    vise_core::CATEGORIES
        .first()
        .map(|category| category.id())
        .unwrap_or("general")
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
        .container_of()
        .map(container_label)
        .unwrap_or_else(|| "the chosen container".to_string());
    let video = preset
        .video_codec_of()
        .map(video_label)
        .unwrap_or_else(|| "the chosen encoder".to_string());
    let audio = preset
        .audio_codec_of()
        .map(audio_label)
        .unwrap_or_else(|| "the chosen audio encoder".to_string());
    let quality = match preset.quality {
        Some(value) => format!(" quality {value}"),
        None => String::new(),
    };
    let subtitles = match preset.subtitle_mode_of() {
        Some(mode) => format!(", subtitles {mode:?}").to_ascii_lowercase(),
        None => String::new(),
    };
    // A size-targeted preset is a different thing from a quality-targeted one, so
    // the size it aims for is part of what it sets rather than a field the user
    // has to find on another tab.
    let size = match (preset.encoding_mode, preset.target_size) {
        (Some(mode), Some(bytes)) if mode.is_target_size() => {
            format!(" aiming at {} MB", bytes / 1_000_000)
        }
        _ => String::new(),
    };
    let cap = match preset.duration_cap_seconds {
        Some(seconds) => format!(", made for titles up to {seconds} s"),
        None => String::new(),
    };
    format!("{container} · {video} · {audio}{quality}{subtitles}{size}{cap}")
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
        categories: categories()
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
        // Every category the catalog files under is one this window offers, so
        // the strip and the store cannot mean different things by the same word.
        for category in vise_core::CATEGORIES {
            let id = category.id();
            assert_eq!(parse_category(id).unwrap_or_else(|_| panic!("{id}")), id);
        }
        assert!(parse_category("nonsense")
            .expect_err("unknown category")
            .contains("nonsense"));
        assert_eq!(categories().len(), 6);
    }

    #[test]
    fn the_form_reports_every_thing_that_has_to_be_fixed() {
        let form = form_view(&input("create"), &[], None);
        assert!(!form.can_submit);
        assert_eq!(form.problems, vec!["Give the preset a name."]);
        assert!(form.name_available);
        assert!(form.description_available);
        assert_eq!(form.action, "create");
        assert_eq!(form.categories.len(), 6);
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

    // This test is about Windows path shapes: a drive letter and a backslash
    // name one file on Windows and one odd filename on Linux, so the
    // behaviour it checks only has one answer on one platform.
    #[cfg(windows)]
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
        // Export is about the library as a whole, and the build ships a catalog,
        // so there is always something to export even before the user stores one.
        assert!(control(&empty, "export").available);
        assert_eq!(empty.user.len(), 0);

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
        assert!(filled.catalog.entries[0]
            .actions
            .iter()
            .any(|action| action.id == "export" && action.available));
    }

    #[test]
    fn the_library_lists_the_catalog_and_the_stored_presets_separately() {
        let mut state = PresetState::new("C:\\app\\presets.json");
        let built_in = vise_core::builtin_presets();
        let user = vec![stored("u1", "Mine"), stored("u2", "Yours")];
        state.set_category("u2", "professional");
        let view = state.view(&user, &built_in, form_view(&input("create"), &[], None));

        assert_eq!(view.catalog.categories.len(), 6);
        assert_eq!(
            view.catalog.total,
            built_in.len(),
            "the browser lists every preset that ships with the build"
        );
        assert_eq!(view.user.len(), 2);
        assert_eq!(view.total, built_in.len() + 2);
        assert!(!view.empty);
        assert_eq!(view.user[0].category, "General");
        assert_eq!(view.user[1].category, "Professional");
        assert!(view.catalog.entries.iter().all(|preset| preset.built_in));
        assert!(view.user.iter().all(|preset| !preset.built_in));
        assert!(view.store_path.contains("presets.json"));
        assert!(view.summary.contains("none is the default"));
        assert!(view.catalog.entries[0].summary.contains("mp4"));
        assert!(view.catalog.entries[0].summary.contains("h264"));
        // The browser opens on the general ladder, and every rung of it is there
        // rather than a flat list of every preset in the catalog.
        assert_eq!(view.catalog.active_category, "general");
        assert!(view.catalog.entries.len() > 50);
        assert_eq!(
            view.catalog
                .entries
                .iter()
                .filter(|entry| entry.summary.contains("h264"))
                .count(),
            20,
            "four quality families by five rungs"
        );
    }

    #[test]
    fn a_built_in_preset_cannot_be_renamed_or_deleted_but_can_be_copied() {
        let built_in = vise_core::builtin_presets();
        let view = PresetState::new("presets.json").view(
            &[],
            &built_in,
            form_view(&input("create"), &[], None),
        );
        let row = &view.catalog.entries[0];
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

        // A size-targeted preset says so on the line that describes it, because
        // the size it aims for is part of what it sets.
        let targeted = PresetSpec {
            encoding_mode: Some(vise_core::EncodingMode::TargetSize),
            target_size: Some(10_000_000),
            duration_cap_seconds: Some(60),
            ..Default::default()
        };
        let summary = preset_summary(&targeted);
        assert!(summary.contains("aiming at 10 MB"), "{summary}");
        assert!(summary.contains("up to 60 s"), "{summary}");
    }

    /// A machine that has every encoder the catalog asks for.
    fn everything() -> vise_core::HardwareCapabilities {
        vise_core::HardwareCapabilities {
            devices: vec![
                "cuda".to_string(),
                "qsv".to_string(),
                "vaapi".to_string(),
                "amf".to_string(),
                "videotoolbox".to_string(),
                "d3d11va".to_string(),
            ],
            ffmpeg_path: Some(std::path::PathBuf::from("ffmpeg")),
            hwaccels: vec!["qsv".to_string(), "cuda".to_string()],
            decoders: vec!["h264".to_string()],
            encoders: vise_core::EVERY_ENCODER
                .iter()
                .map(|encoder| encoder.identifier().to_string())
                .collect(),
        }
    }

    /// A machine with the software encoders only.
    fn software_only() -> vise_core::HardwareCapabilities {
        let mut caps = everything();
        caps.hwaccels.clear();
        caps.devices.clear();
        caps.encoders
            .retain(|name| name.starts_with("lib") || name == "ffv1");
        caps
    }

    #[test]
    fn a_translated_description_is_used_and_the_rest_fall_back_to_the_catalog() {
        // The catalog ships 138 descriptions and the locale files translate a
        // subset of them. Both answers have to be readable: a row is never empty
        // because nobody has translated it yet, and never shows an identifier
        // because it has no text at all.
        let base = {
            let mut state = PresetState::new("presets.json");
            state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
            state.catalog_view()
        };
        let translated_id = "general-very-fast-480-30-h264";
        let untranslated_id = "general-super-hq-2160-60-h264";

        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
        assert_eq!(state.locale, "en", "the base language to begin with");
        assert!(state.use_language("es"), "the first change moves");
        assert!(!state.use_language("es"), "the second one does not");
        assert_eq!(state.locale, "es");
        let spanish = state.catalog_view();

        // The translated row is in Spanish and is still the same preset.
        let row = spanish
            .entries
            .iter()
            .find(|entry| entry.id == translated_id)
            .expect("the preset is in the browser");
        let same = base
            .entries
            .iter()
            .find(|entry| entry.id == translated_id)
            .expect("the preset is in the browser");
        assert_ne!(row.description, same.description, "it was translated");
        assert!(
            row.description.starts_with("La codificación"),
            "{}",
            row.description
        );
        assert!(
            !row.description.contains("quickest encode"),
            "{}",
            row.description
        );

        // The untranslated one is the catalog's own sentence, unchanged.
        let row = spanish
            .entries
            .iter()
            .find(|entry| entry.id == untranslated_id)
            .expect("the preset is in the browser");
        let same = base
            .entries
            .iter()
            .find(|entry| entry.id == untranslated_id)
            .expect("the preset is in the browser");
        assert_eq!(row.description, same.description);
        assert!(!row.description.trim().is_empty());
        assert_eq!(
            same.description,
            vise_core::builtin_entry(untranslated_id)
                .expect("the preset")
                .description
        );

        // Every row has a sentence in every shipped locale.
        for tag in ["en", "es", "de", "fr", "ja", "ar", "kl"] {
            let mut state = PresetState::new("presets.json");
            state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
            state.use_language(tag);
            for entry in state.catalog_view().entries {
                assert!(!entry.description.trim().is_empty(), "{tag}: {}", entry.id);
                assert!(
                    !entry.description.contains(&entry.id),
                    "{} {}",
                    tag,
                    entry.id
                );
            }
        }
    }

    #[test]
    fn a_user_presets_own_description_is_left_alone() {
        // It is the user's words, so there is nothing here to translate and
        // nothing to fall back from.
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
        state.use_language("ja");
        let mine = vise_core::UserPreset {
            id: "general-very-fast-480-30-h264".to_string(),
            name: "Mine".to_string(),
            description: Some("what I want".to_string()),
            preset: vise_core::PresetSpec::default(),
        };
        let view = state.view(
            &[mine],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        let row = view
            .user
            .iter()
            .find(|entry| entry.name == "Mine")
            .expect("the stored preset is listed");
        assert_eq!(row.description, "what I want");
    }

    #[test]
    fn the_browser_offers_the_six_categories_with_what_each_one_holds() {
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        assert_eq!(view.catalog.categories.len(), 6);
        let labels: Vec<&str> = view
            .catalog
            .categories
            .iter()
            .map(|category| category.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "General",
                "Web",
                "Devices",
                "Matroska",
                "Hardware",
                "Professional"
            ]
        );
        assert!(view
            .catalog
            .categories
            .iter()
            .all(|category| category.count > 0 && !category.description.is_empty()));
        assert!(view.catalog.categories[0].selected);
        assert_eq!(
            view.catalog
                .categories
                .iter()
                .map(|c| c.count)
                .sum::<usize>(),
            view.catalog.total
        );
        assert_eq!(
            view.catalog.version,
            vise_core::PRESET_SCHEMA_VERSION.label()
        );
        assert!(view.catalog.checked);
        assert!(
            view.catalog.summary.contains("as written"),
            "{}",
            view.catalog.summary
        );
    }

    #[test]
    fn moving_to_a_category_opens_its_first_entry_and_forgets_the_last_one() {
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
        state
            .set_catalog_entry("general-super-hq-2160-60-av1")
            .expect("an entry");
        state
            .set_catalog_category("professional")
            .expect("a category");
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        assert_eq!(view.catalog.active_category, "professional");
        let detail = view
            .catalog
            .detail
            .expect("the professional presets open on one");
        assert_eq!(detail.id, "professional-prores-422-proxy");
        assert!(view
            .catalog
            .entries
            .iter()
            .all(|entry| entry.selected == (entry.id == detail.id)));

        // Opening an entry moves to the category that holds it, so a page that
        // sends only an identifier still lands somewhere the user can see.
        state
            .set_catalog_entry("device-roku-1080-30-h264")
            .expect("an entry");
        assert_eq!(state.catalog_category, "devices");
    }

    #[test]
    fn a_category_and_an_entry_this_build_does_not_have_are_refused_by_name() {
        let mut state = PresetState::new("presets.json");
        let error = state
            .set_catalog_category("nonsense")
            .expect_err("no such category");
        assert!(error.contains("nonsense"), "{error}");
        assert!(error.contains("General"), "{error}");
        let error = state
            .set_catalog_entry("no-such-preset")
            .expect_err("no such preset");
        assert!(error.contains("no-such-preset"), "{error}");
        // Nothing was changed by the refusals.
        assert_eq!(state.catalog_category, "general");
        assert!(state.catalog_entry.is_empty());
    }

    #[test]
    fn a_hardware_preset_is_closed_on_a_machine_without_the_chip() {
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&software_only(), &vise_core::HardwareAcceleration::auto());
        state.set_catalog_category("hardware").expect("a category");
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        let row = |id: &str| {
            view.catalog
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .cloned()
                .expect("a hardware preset")
        };
        // A chip that is not here becomes the software encoder of the same codec,
        // and the row says so rather than hiding it.
        let nvenc = row("hardware-nvenc-hevc-1080-30");
        assert!(nvenc.available);
        assert!(nvenc.fallback);
        assert!(!nvenc.hardware);
        assert_eq!(nvenc.encoder, "libx265");
        assert!(
            nvenc.available_note.contains("hevc_nvenc"),
            "{}",
            nvenc.available_note
        );
        assert!(
            nvenc.available_note.contains("libx265"),
            "{}",
            nvenc.available_note
        );
        let apply = |row: &PresetViewModel| {
            row.actions
                .iter()
                .find(|action| action.id == "apply")
                .cloned()
                .expect("apply")
        };
        assert!(
            apply(&nvenc).available,
            "a fallback is still a job this machine can do"
        );
        assert!(view.catalog.fallback > 0);

        // A machine with the chip runs it, and says which one.
        let mut with_chip = PresetState::new("presets.json");
        with_chip.observe_hardware(&everything(), &vise_core::HardwareAcceleration::auto());
        with_chip
            .set_catalog_category("hardware")
            .expect("a category");
        let view = with_chip.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        let nvenc = view
            .catalog
            .entries
            .iter()
            .find(|entry| entry.id == "hardware-nvenc-hevc-1080-30")
            .expect("a hardware preset");
        assert!(nvenc.hardware);
        assert!(!nvenc.fallback);
        assert_eq!(nvenc.encoder, "hevc_nvenc");
    }

    #[test]
    fn a_preset_with_no_software_stand_in_is_closed_with_its_reason() {
        let mut state = PresetState::new("presets.json");
        let mut caps = software_only();
        caps.encoders.retain(|name| name != "prores_ks");
        state.observe_hardware(&caps, &vise_core::HardwareAcceleration::auto());
        state
            .set_catalog_category("professional")
            .expect("a category");
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        let row = view
            .catalog
            .entries
            .iter()
            .find(|entry| entry.id == "professional-prores-422-hq")
            .expect("a ProRes preset");
        assert!(!row.available, "a proxy is not an interchange master");
        assert!(
            row.available_reason.contains("prores_ks"),
            "{}",
            row.available_reason
        );
        let apply = row
            .actions
            .iter()
            .find(|action| action.id == "apply")
            .expect("apply");
        assert!(!apply.available);
        assert!(
            apply.reason.contains("cannot run on this machine"),
            "{}",
            apply.reason
        );
        // Copying and exporting are still on offer: reading a preset is not the
        // same as running it.
        assert!(row.actions.iter().any(|a| a.id == "clone" && a.available));
        assert!(row.actions.iter().any(|a| a.id == "export" && a.available));
    }

    #[test]
    fn a_machine_that_was_never_probed_closes_nothing_and_says_so() {
        let mut state = PresetState::new("presets.json");
        state.set_catalog_category("hardware").expect("a category");
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        assert!(!view.catalog.checked);
        assert!(view.catalog.entries.iter().all(|entry| entry.available));
        assert!(
            view.catalog.note.contains("not been found"),
            "{}",
            view.catalog.note
        );
        assert!(
            view.catalog.summary.contains("not been checked"),
            "{}",
            view.catalog.summary
        );
    }

    #[test]
    fn switching_hardware_off_is_shown_on_every_hardware_row() {
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(
            &everything(),
            &crate::preferences::hardware::to_hardware_options(
                &crate::preferences::hardware::HardwareSettings {
                    acceleration: crate::preferences::hardware::AccelerationPreference::Never,
                    ..Default::default()
                },
            ),
        );
        state.set_catalog_category("hardware").expect("a category");
        let view = state.view(
            &[],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        assert!(
            view.catalog
                .entries
                .iter()
                // The Media Foundation rows are in this list for decoding and
                // name a software encoder, so they are not the ones that fall
                // back; the rest are written for a chip and are.
                .filter(|entry| {
                    vise_core::builtin_entry(&entry.id)
                        .and_then(|found| found.encoder)
                        .map(|encoder| encoder.is_hardware())
                        .unwrap_or(false)
                })
                .all(|entry| entry.fallback && !entry.hardware),
            "every hardware row falls back when hardware encoding is off"
        );
        assert!(view
            .catalog
            .entries
            .iter()
            .any(|entry| entry.needs_hardware));
        assert!(
            view.catalog.note.contains("switched off"),
            "{}",
            view.catalog.note
        );
    }

    #[test]
    fn a_stored_preset_is_resolved_too_rather_than_assumed_runnable() {
        let mut state = PresetState::new("presets.json");
        state.observe_hardware(&software_only(), &vise_core::HardwareAcceleration::auto());
        let mut mine = stored("u1", "Mine");
        mine.preset.encoder = Some(vise_core::EncoderId::NvencAv1);
        mine.preset.video_codec = Some(VideoCodec::Av1);
        let view = state.view(
            &[mine],
            &vise_core::builtin_presets(),
            form_view(&input("create"), &[], None),
        );
        let row = &view.user[0];
        assert!(row.fallback);
        assert_eq!(row.encoder, "libaom-av1");
        assert_eq!(row.encoder_label, "software AV1");
        assert!(
            row.compatibility.is_empty(),
            "a stored preset carries no guidance"
        );
    }
}
