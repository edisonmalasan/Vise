//! The metadata tab: the tags written into the finished file.
//!
//! Three things decide what ends up in the output's tags, and this tab is where
//! all three are made visible. The mode decides whether the source tags are
//! written through, written and then edited, or left out entirely. The key and
//! value rows let the user change one tag at a time, keeping every tag the scan
//! reported visible so a change is a change rather than a replacement. The
//! attachment switches cover art and other attachments, which are the two
//! things a tag list cannot express.
//!
//! The mode also decides the Summary tab's "keep the source tags" switch, and
//! it is the single place that value lives: two controls reading and writing
//! the same field cannot disagree, and the job specification is built from
//! whichever one the user touched.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vise_core::{MetadataMode, MetadataProcessingOptions};

use crate::tabs::common::{ChoiceGroup, ChoiceGroupViewModel, TextViewModel, ToggleViewModel};
use crate::tabs::source::TitleFacts;

/// The longest tag name the media tools accept.
pub const MAX_KEY_LENGTH: usize = 64;

/// What happens to the source tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TagMode {
    /// Write every source tag through unchanged.
    #[default]
    Copy,
    /// Write the source tags, then apply the edited values over them.
    Edit,
    /// Write nothing from the source.
    None,
}

impl TagMode {
    /// Every mode, in the order the picker lists them.
    pub fn all() -> [TagMode; 3] {
        [TagMode::Copy, TagMode::Edit, TagMode::None]
    }

    /// Stable identifier the page sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            TagMode::Copy => "copy",
            TagMode::Edit => "edit",
            TagMode::None => "none",
        }
    }

    /// Label beside the mode control.
    pub fn label(self) -> &'static str {
        match self {
            TagMode::Copy => "Keep the source tags",
            TagMode::Edit => "Keep and edit",
            TagMode::None => "Leave the source tags out",
        }
    }

    /// One line explaining what the mode does.
    pub fn hint(self) -> &'static str {
        match self {
            TagMode::Copy => {
                "Every tag the source carried is written to the finished file as it is."
            }
            TagMode::Edit => {
                "The source tags are written, then the values below are applied over them."
            }
            TagMode::None => "Nothing is taken from the source; only the values below are written.",
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
            "copy" | "keep" | "passthrough" | "pass_through" => Some(TagMode::Copy),
            "edit" | "keep_and_edit" => Some(TagMode::Edit),
            "none" | "off" | "drop" | "strip" => Some(TagMode::None),
            _ => None,
        }
    }

    /// Whether the source tags reach the output at all.
    pub fn copies_source(self) -> bool {
        !matches!(self, TagMode::None)
    }

    /// Whether the edited values reach the output.
    pub fn applies_edits(self) -> bool {
        matches!(self, TagMode::Edit)
    }

    /// The mode as the core model spells it.
    pub fn to_core(self) -> MetadataMode {
        match self {
            TagMode::Copy => MetadataMode::Copy,
            TagMode::Edit => MetadataMode::Edit,
            TagMode::None => MetadataMode::None,
        }
    }
}

/// Where a tag's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagOrigin {
    /// The value came from the scan.
    Source,
    /// The user wrote the value.
    Edited,
    /// The value was blanked out on purpose.
    Removed,
}

impl TagOrigin {
    /// Label shown beside the row.
    pub fn label(self) -> &'static str {
        match self {
            TagOrigin::Source => "From the source",
            TagOrigin::Edited => "Edited",
            TagOrigin::Removed => "Removed",
        }
    }
}

/// One tag row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TagEntry {
    /// The tag name.
    pub key: String,
    /// The value, exactly as typed.
    pub value: String,
    /// Where the value came from.
    pub origin: Option<TagOrigin>,
}

/// The metadata choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataSettings {
    /// What happens to the source tags.
    pub mode: String,
    /// One row per tag, sorted by name.
    pub entries: Vec<TagEntry>,
    /// Write the cover art into the output.
    pub cover_art: bool,
    /// Write other attachments into the output.
    pub attachments: bool,
    /// Blank out every source tag.
    pub clear: bool,
}

impl MetadataSettings {
    /// The mode as an enum.
    pub fn mode(&self) -> TagMode {
        TagMode::parse(&self.mode).unwrap_or_default()
    }

    /// Whether the source tags reach the output.
    ///
    /// This is the value the Summary tab's switch shows, so the two can never
    /// report different things for the same job.
    pub fn copies_source(&self) -> bool {
        !self.clear && self.mode().copies_source()
    }

    /// Bring the rows in line with the chosen title.
    ///
    /// A tag the new title does not carry is dropped along with anything the
    /// user wrote for it, because an edit to a tag that is not there has nothing
    /// to apply to. A tag that is there keeps its edited value.
    pub fn sync_with_title(&mut self, facts: &TitleFacts) {
        let source: BTreeMap<String, String> = facts
            .metadata
            .iter()
            .map(|fact| (fact.key.clone(), fact.value.clone()))
            .collect();
        let mut entries: Vec<TagEntry> = Vec::new();
        for (key, value) in &source {
            let kept = self
                .entries
                .iter()
                .find(|entry| entry.key.eq_ignore_ascii_case(key))
                .cloned();
            entries.push(match kept {
                Some(entry) if entry.origin == Some(TagOrigin::Removed) => TagEntry {
                    key: key.clone(),
                    value: String::new(),
                    origin: Some(TagOrigin::Removed),
                },
                Some(entry) if entry.origin == Some(TagOrigin::Edited) => TagEntry {
                    key: key.clone(),
                    value: entry.value,
                    origin: Some(TagOrigin::Edited),
                },
                _ => TagEntry {
                    key: key.clone(),
                    value: value.clone(),
                    origin: Some(TagOrigin::Source),
                },
            });
        }
        self.entries = entries;
    }

    /// Change the mode.
    ///
    /// The mode and the clear switch are separate on purpose: leaving the source
    /// tags out is not the same as blanking them, and a file that must still
    /// declare its tags needs the second without the first.
    pub fn set_mode(&mut self, text: &str) -> Result<(), String> {
        let mode = TagMode::parse(text).ok_or_else(|| {
            format!(
                "'{}' is not a metadata mode this build offers.",
                text.trim()
            )
        })?;
        self.mode = mode.as_str().to_string();
        Ok(())
    }

    /// Set the passthrough value the Summary tab shows.
    ///
    /// Switching passthrough on and off moves between "keep the source tags"
    /// and "leave them out"; it never touches the mode that says whether the
    /// user's own values are written over them, so an edit in progress is not
    /// lost by flipping a switch on another tab.
    pub fn set_passthrough(&mut self, wanted: bool) {
        let mode = if wanted {
            if self.mode() == TagMode::None {
                TagMode::Copy
            } else {
                self.mode()
            }
        } else {
            TagMode::None
        };
        self.mode = mode.as_str().to_string();
    }

    /// Validate a tag name.
    ///
    /// The media tools read tags as `name=value`, so a name carrying `=` would
    /// be written as two tags or none at all. A name that is empty or too long
    /// is refused for the same reason the other tabs refuse a size that no
    /// encoder accepts.
    pub fn check_key(key: &str) -> Result<String, String> {
        let key = key.trim();
        if key.is_empty() {
            return Err("Give the tag a name.".to_string());
        }
        if key.contains('=') {
            return Err(format!(
                "'{key}' cannot be a tag name; the media tools read '=' as the end of the name."
            ));
        }
        if key.chars().any(|c| (c as u32) < 0x20) {
            return Err(format!(
                "'{key}' cannot be a tag name; it holds a control character."
            ));
        }
        if key.chars().count() > MAX_KEY_LENGTH {
            return Err(format!(
                "A tag name has to be {MAX_KEY_LENGTH} characters or fewer; '{key}' is longer."
            ));
        }
        Ok(key.to_string())
    }

    /// Validate a tag value, keeping whitespace inside the text.
    pub fn check_value(value: &str) -> Result<String, String> {
        if value.chars().any(|c| (c as u32) < 0x20 && c != '\t') {
            return Err("A tag value cannot hold a line break or control character.".to_string());
        }
        Ok(value.trim().to_string())
    }

    /// Add a tag the source did not carry.
    ///
    /// A blank value is how a tag is added with no text yet, which is different
    /// from removing one; removal has its own action.
    pub fn add_tag(&mut self, key: &str, value: &str) -> Result<String, String> {
        let key = Self::check_key(key)?;
        let value = Self::check_value(value)?;
        if self
            .entries
            .iter()
            .any(|entry| entry.key.eq_ignore_ascii_case(&key))
        {
            return Err(format!("There is already a tag called '{key}'."));
        }
        self.entries.push(TagEntry {
            key: key.clone(),
            value,
            origin: Some(TagOrigin::Edited),
        });
        self.entries
            .sort_by_key(|entry| entry.key.to_ascii_lowercase());
        Ok(key)
    }

    /// Change one tag's value.
    ///
    /// A blank value removes the tag from the output rather than writing an
    /// empty one, which is what a user who clears a field means.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<String, String> {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.key.eq_ignore_ascii_case(key.trim()))
        else {
            return Err(format!("There is no tag called '{}'.", key.trim()));
        };
        let value = Self::check_value(value)?;
        entry.origin = Some(if value.is_empty() {
            TagOrigin::Removed
        } else {
            TagOrigin::Edited
        });
        entry.value = value.clone();
        Ok(value)
    }

    /// Restore a tag to the value the scan reported.
    pub fn restore(&mut self, key: &str, source_value: &str) -> Result<(), String> {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.key.eq_ignore_ascii_case(key.trim()))
        else {
            return Err(format!("There is no tag called '{}'.", key.trim()));
        };
        entry.value = source_value.to_string();
        entry.origin = Some(TagOrigin::Source);
        Ok(())
    }

    /// Drop a tag the user added.
    pub fn remove_tag(&mut self, key: &str) -> Result<(), String> {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !entry.key.eq_ignore_ascii_case(key.trim()));
        if self.entries.len() == before {
            return Err(format!("There is no tag called '{}'.", key.trim()));
        }
        Ok(())
    }

    /// Whether an edited value will reach the output.
    pub fn applies_edits(&self) -> bool {
        self.mode().applies_edits()
    }

    /// The metadata options the job specification carries.
    pub fn to_job_options(&self) -> MetadataProcessingOptions {
        let mode = self.mode();
        let mut metadata: BTreeMap<String, String> = BTreeMap::new();
        for entry in &self.entries {
            if entry.origin == Some(TagOrigin::Removed) {
                continue;
            }
            if entry.origin == Some(TagOrigin::Source) && !mode.copies_source() {
                continue;
            }
            if entry.value.trim().is_empty() {
                continue;
            }
            metadata.insert(entry.key.clone(), entry.value.clone());
        }
        MetadataProcessingOptions {
            mode: Some(mode.to_core()),
            copy: self.copies_source(),
            edit: mode.applies_edits(),
            metadata,
            clear_metadata: self.clear,
            cover_art: self.cover_art,
            attachments: self.attachments,
            ..Default::default()
        }
    }
}

/// The metadata change as the interface layer sends it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MetadataInput {
    /// `mode`, `add`, `edit`, `restore`, `remove`, `cover_art`, `attachments`,
    /// or `clear`.
    pub action: String,
    /// The tag name, for the per-tag actions.
    pub key: String,
    /// The tag value, for the add and edit actions.
    pub value: String,
    /// Whether the value a switch carries is on.
    pub checked: bool,
    /// The mode, for the mode action.
    pub mode: Option<String>,
}

impl MetadataInput {
    /// Apply the change, refusing names and keys this build does not use.
    pub fn apply(self, settings: &mut MetadataSettings, facts: &TitleFacts) -> Result<(), String> {
        match self.action.trim().to_ascii_lowercase().as_str() {
            "" | "mode" => {
                if let Some(mode) = self.mode {
                    settings.set_mode(&mode)?;
                }
            }
            "add" => {
                settings.add_tag(&self.key, &self.value)?;
            }
            "edit" | "set" => {
                settings.set_value(&self.key, &self.value)?;
            }
            "restore" => {
                let key = MetadataSettings::check_key(&self.key)?;
                let source = facts
                    .metadata
                    .iter()
                    .find(|fact| fact.key.eq_ignore_ascii_case(&key))
                    .map(|fact| fact.value.clone())
                    .ok_or_else(|| format!("The source does not carry a tag called '{key}'."))?;
                settings.restore(&key, &source)?;
            }
            "remove" => {
                settings.remove_tag(&self.key)?;
            }
            "cover_art" => {
                settings.cover_art = self.checked;
            }
            "attachments" => {
                settings.attachments = self.checked;
            }
            "clear" => {
                settings.clear = self.checked;
            }
            other => {
                return Err(format!(
                    "'{other}' is not something the metadata tab can do."
                ))
            }
        }
        Ok(())
    }
}

/// One tag row, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagViewModel {
    /// Stable identifier of the value field.
    pub id: String,
    /// The tag name.
    pub key: String,
    /// The value, exactly as it should appear in the field.
    pub text: String,
    /// Where the value came from.
    pub origin: String,
    /// Tone the page paints the row with.
    pub tone: String,
    /// Whether the value can be changed right now.
    pub editable: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether the value will reach the output.
    pub written: bool,
    /// Whether a restore control is on offer.
    pub restorable: bool,
    /// Whether a remove control is on offer.
    pub removable: bool,
}

/// The metadata tab, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataTabViewModel {
    /// The mode picker.
    pub mode: ChoiceGroupViewModel,
    /// One row per tag.
    pub tags: Vec<TagViewModel>,
    /// Whether the source carried any tags at all.
    pub has_source_tags: bool,
    /// The new-tag name field.
    pub new_key: TextViewModel,
    /// The new-tag value field.
    pub new_value: TextViewModel,
    /// The cover-art switch.
    pub cover_art: ToggleViewModel,
    /// The attachments switch.
    pub attachments: ToggleViewModel,
    /// The clear-everything switch.
    pub clear: ToggleViewModel,
    /// One line describing what the output will carry.
    pub summary: String,
    /// One line about what the source carried.
    pub source_note: String,
    /// Everything that has to be fixed.
    pub problems: Vec<String>,
    /// Whether nothing here needs fixing.
    pub ready: bool,
}

/// Build the metadata tab for the chosen title.
pub fn tab_view(settings: &MetadataSettings, facts: &TitleFacts) -> MetadataTabViewModel {
    let mode = settings.mode();
    let copies = settings.copies_source();
    let edits = settings.applies_edits();
    let chosen = facts.is_chosen();
    let tags: Vec<TagViewModel> = settings
        .entries
        .iter()
        .map(|entry| {
            let origin = entry.origin.unwrap_or(TagOrigin::Source);
            let written = !matches!(origin, TagOrigin::Removed)
                && !(matches!(origin, TagOrigin::Source) && !copies)
                && !entry.value.trim().is_empty();
            TagViewModel {
                id: format!("metadataValue{}", entry.key.to_ascii_lowercase()),
                key: entry.key.clone(),
                text: entry.value.clone(),
                origin: origin.label().to_string(),
                tone: match origin {
                    TagOrigin::Source => "muted",
                    TagOrigin::Edited => "info",
                    TagOrigin::Removed => "error",
                }
                .to_string(),
                editable: edits && !matches!(origin, TagOrigin::Removed),
                reason: if matches!(origin, TagOrigin::Removed) {
                    "This tag was removed; restore it before editing it.".to_string()
                } else if edits {
                    String::new()
                } else {
                    edit_closed_reason().to_string()
                },
                written,
                restorable: origin != TagOrigin::Source
                    && facts
                        .metadata
                        .iter()
                        .any(|fact| fact.key.eq_ignore_ascii_case(&entry.key)),
                removable: origin == TagOrigin::Edited,
            }
        })
        .collect();
    let source_count = facts.metadata.len();
    let edited = settings
        .entries
        .iter()
        .filter(|entry| entry.origin != Some(TagOrigin::Source))
        .count();
    let written = tags.iter().filter(|tag| tag.written).count();
    let mode_options = ChoiceGroup::of(
        TagMode::all()
            .into_iter()
            .map(|mode| (mode.as_str(), mode.label())),
    );
    let problems: Vec<String> = settings
        .entries
        .iter()
        .filter(|entry| MetadataSettings::check_key(&entry.key).is_err())
        .map(|entry| {
            if entry.key.trim().is_empty() {
                "A tag has no name.".to_string()
            } else {
                format!("'{}' cannot be used as a tag name.", entry.key)
            }
        })
        .collect();
    let no_title = "Choose a title to set its tags.";
    let add_reason = "Switch the mode to 'Keep and edit' before adding a tag.";
    let ready = problems.is_empty();
    MetadataTabViewModel {
        mode: ChoiceGroupViewModel::build(
            "metadataMode",
            "Source tags",
            mode.hint(),
            mode.as_str(),
            &mode_options.options,
            chosen,
            if chosen { "" } else { no_title },
        ),
        source_note: if source_count == 0 {
            if chosen {
                "This title carries no tags.".to_string()
            } else {
                "No title is chosen.".to_string()
            }
        } else {
            format!("The source carries {source_count} tag(s).")
        },
        new_key: closed_text(
            invalid_text(
                TextViewModel::new("metadataNewKey", "New tag name", "", "title"),
                problems.iter().any(|problem| problem == "A tag has no name."),
                "A tag has no name; fix it or remove it.",
            ),
            edits,
            add_reason,
        ),
        new_value: closed_text(
            TextViewModel::new("metadataNewValue", "Value", "", "what the tag says"),
            edits,
            add_reason,
        ),
        cover_art: closed_toggle(
            ToggleViewModel::new(
                "metadataCoverArt",
                "Cover art",
                "Written as a picture attached to the output.",
                settings.cover_art,
            ),
            chosen,
            no_title,
        ),
        attachments: closed_toggle(
            ToggleViewModel::new(
                "metadataAttachments",
                "Other attachments",
                "Written alongside the tags; fonts and subtitle pictures arrive this way.",
                settings.attachments,
            ),
            chosen,
            no_title,
        ),
        clear: closed_toggle(
            ToggleViewModel::new(
                "metadataClear",
                "Blank every source tag",
                "Clears the tags instead of leaving them out, for a file that must still declare them.",
                settings.clear,
            ),
            chosen,
            no_title,
        ),
        has_source_tags: source_count > 0,
        summary: if !chosen {
            no_title.to_string()
        } else if edited == 0 {
            format!(
                "{} of {} tag(s) will be written.",
                written,
                settings.entries.len()
            )
        } else {
            format!(
                "{} of {} tag(s) will be written; {edited} of them were changed here.",
                written,
                settings.entries.len()
            )
        },
        problems,
        ready,
        tags,
    }
}

/// The reason the per-tag value fields are closed in copy mode.
pub fn edit_closed_reason() -> &'static str {
    "Switch the mode to 'Keep and edit' before changing a value."
}

/// A text field, closed with `reason` when `available` does not hold.
fn closed_text(field: TextViewModel, available: bool, reason: &str) -> TextViewModel {
    if available {
        field
    } else {
        field.blocked(reason)
    }
}

/// A text field, marked unusable with `problem` when `invalid` holds.
fn invalid_text(field: TextViewModel, invalid: bool, problem: &str) -> TextViewModel {
    if invalid {
        field.invalid(problem)
    } else {
        field
    }
}

/// A switch, closed with `reason` when `available` does not hold.
fn closed_toggle(toggle: ToggleViewModel, available: bool, reason: &str) -> ToggleViewModel {
    if available {
        toggle
    } else {
        toggle.blocked(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::source::MetadataFact;

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            name: "Film".into(),
            metadata: vec![
                MetadataFact {
                    key: "title".into(),
                    value: "Original".into(),
                },
                MetadataFact {
                    key: "artist".into(),
                    value: "Someone".into(),
                },
            ],
            ..Default::default()
        }
    }

    fn edited() -> MetadataSettings {
        let mut settings = MetadataSettings::default();
        settings.sync_with_title(&facts());
        settings.set_mode("edit").expect("edit");
        settings
    }

    #[test]
    fn the_mode_reads_back_every_way_this_build_spells_it() {
        assert_eq!(TagMode::parse("Copy"), Some(TagMode::Copy));
        assert_eq!(TagMode::parse(" keep "), Some(TagMode::Copy));
        assert_eq!(TagMode::parse("keep-and-edit"), Some(TagMode::Edit));
        assert_eq!(TagMode::parse("strip"), Some(TagMode::None));
        assert_eq!(TagMode::parse("shred"), None);
        assert_eq!(TagMode::all().len(), 3);
        assert!(TagMode::Copy.copies_source());
        assert!(!TagMode::Copy.applies_edits());
        assert!(TagMode::Edit.applies_edits());
        assert!(!TagMode::None.copies_source());
        assert_eq!(TagMode::Edit.to_core(), MetadataMode::Edit);
        assert!(TagMode::None.hint().contains("only the values below"));
    }

    #[test]
    fn the_mode_refuses_a_name_it_does_not_use() {
        let mut settings = MetadataSettings::default();
        assert!(settings
            .set_mode("obliterate")
            .expect_err("unknown mode")
            .contains("obliterate"));
        assert_eq!(settings.mode(), TagMode::Copy, "the old mode stands");
    }

    #[test]
    fn the_mode_and_the_summary_switch_are_one_setting() {
        let mut settings = MetadataSettings::default();
        assert!(settings.copies_source());
        assert_eq!(settings.mode(), TagMode::Copy);

        settings.set_mode("edit").expect("edit");
        assert!(
            settings.copies_source(),
            "editing still keeps the source tags"
        );
        assert!(settings.applies_edits());

        settings.set_mode("none").expect("none");
        assert!(!settings.copies_source());
        assert!(
            !settings.clear,
            "leaving tags out is not the same as blanking them"
        );

        settings.set_passthrough(true);
        assert_eq!(settings.mode(), TagMode::Copy);
        settings.set_mode("edit").expect("edit");
        settings.set_passthrough(true);
        assert_eq!(settings.mode(), TagMode::Edit, "editing is not lost");
        settings.set_passthrough(false);
        assert_eq!(settings.mode(), TagMode::None);
        settings.set_passthrough(true);
        assert_eq!(
            settings.mode(),
            TagMode::Copy,
            "switching back resumes copying"
        );
        settings.set_passthrough(false);
        assert!(!settings.clear);
    }

    #[test]
    fn syncing_keeps_an_edited_value_and_drops_one_for_a_tag_that_is_gone() {
        let mut settings = edited();
        settings.set_value("title", "Renamed").expect("edit");
        settings.sync_with_title(&facts());
        let title = settings
            .entries
            .iter()
            .find(|entry| entry.key == "title")
            .expect("title");
        assert_eq!(title.value, "Renamed");
        assert_eq!(title.origin, Some(TagOrigin::Edited));

        let other = TitleFacts {
            number: 2,
            name: "Other".into(),
            metadata: vec![MetadataFact {
                key: "title".into(),
                value: "Different".into(),
            }],
            ..Default::default()
        };
        settings.add_tag("comment", "mine").expect("add");
        settings.sync_with_title(&other);
        assert_eq!(settings.entries.len(), 1);
        assert_eq!(settings.entries[0].key, "title");
        assert!(!settings.entries.iter().any(|entry| entry.key == "comment"));
    }

    #[test]
    fn a_removed_tag_stays_removed_across_a_change_of_title() {
        let mut settings = edited();
        settings.set_value("artist", "").expect("blank");
        settings.sync_with_title(&facts());
        let artist = settings
            .entries
            .iter()
            .find(|entry| entry.key == "artist")
            .expect("artist");
        assert_eq!(artist.origin, Some(TagOrigin::Removed));
        assert!(artist.value.is_empty());
        assert!(!settings.to_job_options().metadata.contains_key("artist"));
    }

    #[test]
    fn a_tag_name_the_media_tools_would_misread_is_refused() {
        let mut settings = edited();
        assert!(settings
            .add_tag("  ", "x")
            .expect_err("no name")
            .contains("name"));
        assert!(settings
            .add_tag("a=b", "x")
            .expect_err("separator")
            .contains("'a=b'"));
        assert!(settings
            .add_tag("with\ttab", "x")
            .expect_err("control character")
            .contains("control character"));
        let long = "k".repeat(MAX_KEY_LENGTH + 1);
        assert!(settings
            .add_tag(&long, "x")
            .expect_err("too long")
            .contains("64 characters"));
        assert!(
            settings.add_tag("title", "x").is_err(),
            "an existing tag cannot be added twice"
        );
    }

    #[test]
    fn a_tag_can_be_added_edited_restored_and_removed() {
        let mut settings = edited();
        settings.add_tag("Comment", "first").expect("add");
        let sorted: Vec<&str> = settings.entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(sorted, vec!["artist", "Comment", "title"], "sorted by name");

        assert_eq!(
            settings.set_value("comment", "second").expect("edit"),
            "second"
        );
        let entry = settings
            .entries
            .iter()
            .find(|entry| entry.key == "Comment")
            .expect("Comment");
        assert_eq!(entry.origin, Some(TagOrigin::Edited));

        settings.restore("Comment", "from source").expect("restore");
        assert_eq!(
            settings
                .entries
                .iter()
                .find(|entry| entry.key == "Comment")
                .expect("Comment")
                .origin,
            Some(TagOrigin::Source)
        );

        settings.remove_tag("Comment").expect("remove");
        assert!(!settings.entries.iter().any(|entry| entry.key == "Comment"));
        assert!(settings
            .remove_tag("Comment")
            .expect_err("gone")
            .contains("no tag called"));
        assert!(settings
            .set_value("Comment", "x")
            .expect_err("gone")
            .contains("no tag called"));
        assert!(settings
            .restore("Comment", "x")
            .expect_err("gone")
            .contains("no tag called"));
    }

    #[test]
    fn a_value_the_tools_cannot_write_is_refused() {
        let mut settings = edited();
        assert!(settings
            .set_value("title", "two\nlines")
            .expect_err("newline")
            .contains("line break"));
        assert!(MetadataSettings::check_value(" padded ").expect("value") == "padded");
    }

    #[test]
    fn the_job_options_follow_the_mode() {
        let mut settings = edited();
        settings.set_value("title", "Renamed").expect("edit");
        let copy = settings.to_job_options();
        assert_eq!(copy.mode, Some(MetadataMode::Edit));
        assert!(copy.copy);
        assert!(copy.edit);
        assert_eq!(
            copy.metadata.get("title").map(String::as_str),
            Some("Renamed")
        );
        assert_eq!(
            copy.metadata.get("artist").map(String::as_str),
            Some("Someone")
        );

        let copy_only = MetadataSettings {
            mode: "copy".into(),
            entries: settings.entries.clone(),
            ..Default::default()
        }
        .to_job_options();
        assert_eq!(copy_only.mode, Some(MetadataMode::Copy));
        assert!(!copy_only.edit, "a copy writes what it found");
        assert_eq!(
            copy_only.metadata.get("title").map(String::as_str),
            Some("Renamed"),
            "an edited value is what the tag now says, whichever mode writes it"
        );

        settings.clear = true;
        let cleared = settings.to_job_options();
        assert!(cleared.clear_metadata);
        assert!(!cleared.copy);

        settings.clear = false;
        settings.set_mode("none").expect("none");
        let none = settings.to_job_options();
        assert_eq!(none.mode, Some(MetadataMode::None));
        assert!(!none.copy);
        assert!(!none.clear_metadata);
        assert!(!none.metadata.contains_key("artist"));
    }

    #[test]
    fn the_attachment_switches_reach_the_job_options() {
        let settings = MetadataSettings {
            cover_art: true,
            attachments: true,
            ..Default::default()
        };
        let options = settings.to_job_options();
        assert!(options.cover_art);
        assert!(options.attachments);
    }

    #[test]
    fn the_tab_lists_every_source_tag_with_its_origin() {
        let mut settings = edited();
        settings.set_value("artist", "Me").expect("edit");
        let view = tab_view(&settings, &facts());
        assert!(view.has_source_tags);
        assert_eq!(view.tags.len(), 2);
        let artist = view
            .tags
            .iter()
            .find(|tag| tag.key == "artist")
            .expect("artist");
        assert_eq!(artist.origin, "Edited");
        assert_eq!(artist.tone, "info");
        assert!(artist.written);
        assert!(artist.editable);
        assert!(artist.id == "metadataValueartist");
        let title = view
            .tags
            .iter()
            .find(|tag| tag.key == "title")
            .expect("title");
        assert_eq!(title.origin, "From the source");
        assert_eq!(title.tone, "muted");
        assert!(!title.restorable, "an untouched tag has nothing to restore");
        assert!(!title.removable, "a source tag is not the user's to remove");
        assert!(view.mode.selected == "edit");
        assert_eq!(view.mode.options.len(), 3);
        assert!(view.summary.contains("2 of 2"));
        assert!(view.summary.contains("1 of them were changed"));
        assert!(view.source_note.contains("2 tag(s)"));
        assert!(view.ready);
        assert!(view.new_key.available);
        assert!(view.cover_art.available);
    }

    #[test]
    fn a_copied_value_cannot_be_changed_until_the_mode_says_so() {
        let mut settings = MetadataSettings::default();
        settings.sync_with_title(&facts());
        let view = tab_view(&settings, &facts());
        assert_eq!(view.mode.selected, "copy");
        assert!(view.tags.iter().all(|tag| !tag.editable));
        assert!(view
            .tags
            .iter()
            .all(|tag| tag.reason.contains("Keep and edit")));
        assert!(!view.new_key.available);
        assert!(!view.new_value.available);
        assert!(view.new_key.reason.contains("Keep and edit"));
        assert!(view.cover_art.available, "an attachment is not a tag edit");
    }

    #[test]
    fn a_removed_tag_is_shown_as_such_and_can_be_restored() {
        let mut settings = edited();
        settings.set_value("title", "").expect("blank");
        let view = tab_view(&settings, &facts());
        let title = view
            .tags
            .iter()
            .find(|tag| tag.key == "title")
            .expect("title");
        assert_eq!(title.origin, "Removed");
        assert_eq!(title.tone, "error");
        assert!(!title.written);
        assert!(title.restorable);
        assert!(title.reason.contains("restore"));
    }

    #[test]
    fn a_title_with_no_tags_says_so_rather_than_showing_an_empty_list() {
        let settings = MetadataSettings::default();
        let bare = TitleFacts {
            number: 1,
            name: "Bare".into(),
            ..Default::default()
        };
        let view = tab_view(&settings, &bare);
        assert!(!view.has_source_tags);
        assert!(view.tags.is_empty());
        assert_eq!(view.source_note, "This title carries no tags.");
        assert!(view.summary.contains("0 of 0"));
    }

    #[test]
    fn without_a_title_the_tab_is_closed_with_the_reason() {
        let settings = MetadataSettings::default();
        let view = tab_view(&settings, &TitleFacts::default());
        assert!(!view.mode.available);
        assert!(view.mode.reason.contains("Choose a title"));
        assert!(!view.cover_art.available);
        assert!(!view.clear.available);
        assert_eq!(view.source_note, "No title is chosen.");
        assert!(view.summary.contains("Choose a title"));
    }

    #[test]
    fn a_tag_row_with_no_name_is_reported_as_a_problem() {
        let mut settings = edited();
        settings.entries.push(TagEntry {
            key: String::new(),
            value: "orphan".into(),
            origin: Some(TagOrigin::Edited),
        });
        let view = tab_view(&settings, &facts());
        assert!(!view.ready);
        assert!(
            view.problems.iter().any(|p| p.contains("no name")),
            "{:?}",
            view.problems
        );
        assert!(!view.new_key.valid);
        assert!(view.new_key.problem.contains("no name"));
    }

    #[test]
    fn a_change_arrives_as_camel_case_and_is_applied() {
        let payload = r#"{"action":"edit","key":"title","value":"Renamed","checked":false}"#;
        let input: MetadataInput = serde_json::from_str(payload).expect("change");
        let mut settings = edited();
        input.apply(&mut settings, &facts()).expect("apply");
        assert_eq!(
            settings
                .entries
                .iter()
                .find(|entry| entry.key == "title")
                .expect("title")
                .value,
            "Renamed"
        );

        let add: MetadataInput =
            serde_json::from_str(r#"{"action":"add","key":"comment","value":"mine"}"#)
                .expect("add");
        let mut settings = edited();
        add.apply(&mut settings, &facts()).expect("add");
        assert!(settings.entries.iter().any(|entry| entry.key == "comment"));

        let cover: MetadataInput =
            serde_json::from_str(r#"{"action":"cover_art","checked":true}"#).expect("cover");
        cover.apply(&mut settings, &facts()).expect("cover");
        assert!(settings.cover_art);

        let restore: MetadataInput =
            serde_json::from_str(r#"{"action":"restore","key":"artist"}"#).expect("restore");
        restore.apply(&mut settings, &facts()).expect("restore");
        let missing: MetadataInput =
            serde_json::from_str(r#"{"action":"restore","key":"nope"}"#).expect("missing");
        assert!(missing
            .apply(&mut settings, &facts())
            .expect_err("not in the source")
            .contains("nope"));

        let unknown: MetadataInput =
            serde_json::from_str(r#"{"action":"incinerate"}"#).expect("unknown");
        assert!(unknown
            .apply(&mut settings, &facts())
            .expect_err("unknown action")
            .contains("incinerate"));
    }
}
