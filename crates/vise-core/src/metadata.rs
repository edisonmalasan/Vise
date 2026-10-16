//! Metadata model and ffmpeg metadata handling helpers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MetadataMode {
    #[default]
    Copy,
    Edit,
    None,
}

impl MetadataMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Edit => "edit",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MetadataProcessingOptions {
    #[serde(default, alias = "metadata_mode", alias = "mode")]
    pub mode: Option<MetadataMode>,

    #[serde(default, alias = "copy_metadata", alias = "metadata_copies")]
    pub copy: bool,
    #[serde(default, alias = "edit_metadata", alias = "metadata_edits")]
    pub edit: bool,
    #[serde(default)]
    pub metadata_copy: bool,
    #[serde(default)]
    pub metadata_edit: bool,
    #[serde(default)]
    pub metadata_editing: bool,

    #[serde(default, alias = "title_name", alias = "name")]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default, alias = "album_artist")]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub track: Option<String>,
    #[serde(default, alias = "disc")]
    pub disc_number: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default, alias = "year")]
    pub recording_date: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub composer: Option<String>,
    #[serde(default)]
    pub encoder: Option<String>,

    #[serde(
        default,
        alias = "tags",
        alias = "metadata_tags",
        alias = "custom_tags"
    )]
    pub metadata: BTreeMap<String, String>,

    #[serde(default)]
    pub clear_metadata: bool,

    #[serde(default, alias = "copy_cover_art", alias = "cover_art_copy")]
    pub cover_art: bool,
    #[serde(default, alias = "copy_attachments", alias = "attachments_copy")]
    pub attachments: bool,
    #[serde(default)]
    pub attachment_copy: bool,
}

pub type MetadataOptions = MetadataProcessingOptions;
pub type MetadataFilters = MetadataProcessingOptions;
pub type MetadataFilterOptions = MetadataProcessingOptions;
pub type MetadataConfig = MetadataProcessingOptions;
pub type MetadataProcessing = MetadataProcessingOptions;
pub type Metadata = MetadataProcessingOptions;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MetadataTrackConfig {
    #[serde(default, alias = "source", alias = "source_track")]
    pub source_track_id: Option<u32>,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

pub type MetadataTrack = MetadataTrackConfig;
pub type MetadataTrackSpec = MetadataTrackConfig;
pub type MetadataTrackConfiguration = MetadataTrackConfig;

impl MetadataProcessingOptions {
    pub fn effective_copy(&self) -> bool {
        self.copy
            || self.metadata_copy
            || self.mode == Some(MetadataMode::Copy)
            || self.mode.is_none()
    }
    pub fn effective_edit(&self) -> bool {
        self.edit
            || self.metadata_edit
            || self.metadata_editing
            || self.mode == Some(MetadataMode::Edit)
            || self.title.is_some()
            || self.artist.is_some()
            || !self.metadata.is_empty()
    }
    pub fn effective_none(&self) -> bool {
        self.mode == Some(MetadataMode::None)
    }
    pub fn user_tags(&self) -> BTreeMap<String, String> {
        let mut out = self.metadata.clone();
        if let Some(v) = &self.title {
            out.insert("title".into(), v.clone());
        }
        if let Some(v) = &self.artist {
            out.insert("artist".into(), v.clone());
        }
        if let Some(v) = &self.album_artist {
            out.insert("album_artist".into(), v.clone());
        }
        if let Some(v) = &self.album {
            out.insert("album".into(), v.clone());
        }
        if let Some(v) = &self.track {
            out.insert("track".into(), v.clone());
        }
        if let Some(v) = &self.disc_number {
            out.insert("disc_number".into(), v.clone());
        }
        if let Some(v) = &self.date {
            out.insert("date".into(), v.clone());
        }
        if let Some(v) = &self.recording_date {
            out.insert("recording_date".into(), v.clone());
        }
        if let Some(v) = &self.genre {
            out.insert("genre".into(), v.clone());
        }
        if let Some(v) = &self.comment {
            out.insert("comment".into(), v.clone());
        }
        if let Some(v) = &self.description {
            out.insert("description".into(), v.clone());
        }
        if let Some(v) = &self.copyright {
            out.insert("copyright".into(), v.clone());
        }
        if let Some(v) = &self.composer {
            out.insert("composer".into(), v.clone());
        }
        if let Some(v) = &self.encoder {
            out.insert("encoder".into(), v.clone());
        }
        out
    }

    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.effective_copy() && !self.effective_none() {
            parts.push("-map_metadata 0".to_string());
        }
        if self.cover_art {
            parts.push("-map 0:v?".to_string());
        }
        if self.attachments || self.attachment_copy {
            parts.push("-map 0:t?".to_string());
        }
        for (k, v) in self.user_tags() {
            parts.push(format!("-metadata {}={}", k, v));
        }
        if self.clear_metadata {
            parts.push("-map_metadata -1".to_string());
        }
        parts.join(" ")
    }

    pub fn build_filter_chain(&self) -> String {
        self.build()
    }
    pub fn build_ffmpeg_args(&self) -> Vec<String> {
        self.build().split_whitespace().map(String::from).collect()
    }
    pub fn build_metadata_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.effective_copy() && !self.effective_none() {
            args.push("-map_metadata".into());
            args.push("0".into());
        }
        if self.cover_art {
            args.push("-map".into());
            args.push("0:v?".into());
        }
        if self.attachments || self.attachment_copy {
            args.push("-map".into());
            args.push("0:t?".into());
        }
        for (k, v) in self.user_tags() {
            args.push("-metadata".into());
            args.push(format!("{}={}", k, v));
        }
        if self.clear_metadata {
            args.push("-map_metadata".into());
            args.push("-1".into());
        }
        args
    }
    pub fn to_ffmpeg(&self) -> String {
        self.build()
    }
    pub fn metadata_args(&self) -> Vec<String> {
        self.build_metadata_args()
    }
    pub fn filter_string(&self) -> String {
        self.build()
    }
}

pub fn build_metadata(options: &MetadataProcessingOptions) -> String {
    options.build()
}
pub fn build_metadata_filter(options: &MetadataProcessingOptions) -> String {
    options.build()
}
pub fn build_metadata_filter_chain(options: &MetadataProcessingOptions) -> String {
    options.build()
}

pub fn metadata_copy() -> bool {
    true
}
pub fn metadata_editing() -> bool {
    true
}
// cover_art_handling and attachment_handling are provided by the chapter module.
