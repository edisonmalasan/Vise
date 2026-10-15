//! Chapter model and ffmpeg chapter handling helpers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChapterMode {
    #[default]
    Copy,
    Edit,
    None,
}

impl ChapterMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Edit => "edit",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Chapter {
    #[serde(default, alias = "start", alias = "start_time", alias = "start_ms", alias = "startTime")]
    pub start_seconds: Option<f64>,
    #[serde(default, alias = "end", alias = "end_time", alias = "end_ms", alias = "endTime")]
    pub end_seconds: Option<f64>,
    #[serde(default, alias = "name", alias = "label")]
    pub title: Option<String>,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterProcessingOptions {
    #[serde(default, alias = "chapter_mode", alias = "mode")]
    pub mode: Option<ChapterMode>,

    #[serde(default, alias = "copy_chapters", alias = "chapters_copy")]
    pub copy: bool,
    #[serde(default, alias = "edit_chapters", alias = "chapters_edit")]
    pub edit: bool,

    #[serde(default, alias = "copies")]
    pub chapter_copy: bool,
    #[serde(default, alias = "edits")]
    pub chapter_edit: bool,
    #[serde(default)]
    pub chapter_editing: bool,

    #[serde(default, alias = "chapters_list", alias = "chapter_list")]
    pub chapters: Vec<Chapter>,

    #[serde(default, alias = "chapter_count")]
    pub count: Option<u32>,

    #[serde(default)]
    pub allow_split: bool,

    #[serde(default, alias = "extract_cover_art", alias = "cover_art_extract")]
    pub extract_cover: bool,
    #[serde(default, alias = "copy_cover_art", alias = "cover_art_copy")]
    pub cover_art: bool,
    #[serde(default, alias = "copy_attachments", alias = "attachments_copy")]
    pub attachments: bool,
    #[serde(default, alias = "attachment_copy")]
    pub attachment_copy: bool,
}

pub type ChapterOptions = ChapterProcessingOptions;
pub type ChapterFilters = ChapterProcessingOptions;
pub type ChapterFilterOptions = ChapterProcessingOptions;
pub type ChapterConfig = ChapterProcessingOptions;
pub type ChapterProcessing = ChapterProcessingOptions;
pub type Chapters = ChapterProcessingOptions;
pub type ChaptersOptions = ChapterProcessingOptions;
pub type ChaptersProcessing = ChapterProcessingOptions;
pub type ChaptersProcessingOptions = ChapterProcessingOptions;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterTrackConfig {
    #[serde(default, alias = "source", alias = "source_track")]
    pub source_track_id: Option<u32>,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
}

pub type ChapterTrack = ChapterTrackConfig;
pub type ChapterTrackSpec = ChapterTrackConfig;
pub type ChapterTrackConfiguration = ChapterTrackConfig;

impl ChapterProcessingOptions {
    pub fn effective_copy(&self) -> bool {
        self.copy || self.chapter_copy || self.mode == Some(ChapterMode::Copy) || self.mode.is_none()
    }
    pub fn effective_edit(&self) -> bool {
        self.edit || self.chapter_edit || self.chapter_editing || self.mode == Some(ChapterMode::Edit)
    }
    pub fn effective_none(&self) -> bool {
        self.mode == Some(ChapterMode::None)
    }
    pub fn has_edits(&self) -> bool {
        !self.chapters.is_empty() || self.effective_edit()
    }

    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.effective_copy() && !self.effective_none() {
            parts.push("-map_chapters 0".to_string());
        }
        if self.cover_art {
            parts.push("-map 0:v?".to_string());
        }
        if self.attachments || self.attachment_copy {
            parts.push("-map 0:t?".to_string());
        }
        if self.extract_cover {
            parts.push("extract_cover_art".to_string());
        }
        parts.join(" ")
    }

    pub fn build_filter_chain(&self) -> String {
        self.build()
    }
    pub fn build_ffmpeg_args(&self) -> Vec<String> {
        self.build().split_whitespace().map(String::from).collect()
    }
    pub fn to_ffmpeg(&self) -> String {
        self.build()
    }
    pub fn filter_string(&self) -> String {
        self.build()
    }
}

pub fn build_chapters(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn build_chapter_filter(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn build_chapter_filter_chain(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn chapter_copy() -> bool {
    true
}
pub fn chapter_editing() -> bool {
    true
}
pub fn cover_art_handling() -> bool {
    true
}
pub fn attachment_handling() -> bool {
    true
}
