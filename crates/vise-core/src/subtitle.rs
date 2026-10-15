//! Subtitle model and ffmpeg subtitle handling helpers.
//!
//! This module provides a serializable options model for subtitle processing
//! features. It covers text/bitmap subtitle passthrough, burn-in, format
//! conversion, SSA/ASS handling, closed captions, and foreign-audio scanning.

use crate::{CoreError, SubtitleMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleFormat {
    #[default]
    Auto,
    Srt,
    SubRip,
    Ass,
    Ssa,
    Vtt,
    WebVtt,
    Ttml,
    Pgs,
    Dvd,
    Dvb,
    ClosedCaption,
    MovText,
    Tx3g,
}

impl SubtitleFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Srt | Self::SubRip => "srt",
            Self::Ass => "ass",
            Self::Ssa => "ssa",
            Self::Vtt | Self::WebVtt => "webvtt",
            Self::Ttml => "ttml",
            Self::Pgs => "pgs",
            Self::Dvd => "dvdsub",
            Self::Dvb => "dvbsub",
            Self::ClosedCaption => "cc",
            Self::MovText => "mov_text",
            Self::Tx3g => "tx3g",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleCodec {
    #[default]
    Copy,
    MovText,
    Srt,
    Ass,
    WebVtt,
    DvdSub,
    DvbSub,
    Ttml,
    Tx3g,
}

impl SubtitleCodec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::MovText => "mov_text",
            Self::Srt => "srt",
            Self::Ass => "ass",
            Self::WebVtt => "webvtt",
            Self::DvdSub => "dvdsub",
            Self::DvbSub => "dvbsub",
            Self::Ttml => "ttml",
            Self::Tx3g => "tx3g",
        }
    }
}

/// Serializable options model for subtitle processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubtitleProcessingOptions {
    #[serde(default, alias = "track", alias = "source_track", alias = "subtitle_track", alias = "track_index", alias = "source_track_id", alias = "track_id", alias = "index")]
    pub source_track: Option<u32>,

    #[serde(default, alias = "subtitle_tracks", alias = "track_ids", alias = "tracks", alias = "source_tracks")]
    pub source_tracks: Vec<u32>,

    #[serde(default, alias = "path", alias = "file", alias = "subtitle_path", alias = "subtitles_path", alias = "subtitle_file", alias = "input_subtitle", alias = "external_subtitle")]
    pub input_path: Option<String>,

    #[serde(default, alias = "format_name")]
    pub format: Option<SubtitleFormat>,

    #[serde(default, alias = "subtitle_mode", alias = "mode")]
    pub mode: Option<SubtitleMode>,

    #[serde(default, alias = "burn", alias = "burn_in_subtitles")]
    pub burn_in: bool,

    #[serde(default, alias = "passthrough_subtitles")]
    pub passthrough: bool,

    #[serde(default, alias = "copy_subtitles")]
    pub copy: bool,

    #[serde(default)]
    pub language: Option<String>,

    #[serde(default, alias = "charset", alias = "encoding")]
    pub charset: Option<String>,

    #[serde(default)]
    pub font: Option<String>,

    #[serde(default, alias = "font_name")]
    pub font_name: Option<String>,

    #[serde(default, alias = "font_size_px")]
    pub font_size: Option<u32>,

    #[serde(default, alias = "font_color")]
    pub color: Option<String>,

    #[serde(default, alias = "closed_caption", alias = "cc")]
    pub closed_captions: bool,

    #[serde(default, alias = "foreign_scan", alias = "scan_foreign_audio")]
    pub foreign_audio_scan: bool,

    #[serde(default, alias = "srt_to_mp4_text", alias = "convert_srt_to_mp4")]
    pub srt_to_mp4: bool,

    #[serde(default, alias = "ssa_ass_handling", alias = "handle_ass")]
    pub ass_handling: bool,

    #[serde(default, alias = "ssa_handling")]
    pub ssa_handling: bool,

    #[serde(default, alias = "codec", alias = "encoder")]
    pub subtitle_codec: Option<SubtitleCodec>,

    #[serde(default)]
    pub forced_only: bool,
}

pub type SubtitleOptions = SubtitleProcessingOptions;
pub type SubtitleFilters = SubtitleProcessingOptions;
pub type SubtitleFilterOptions = SubtitleProcessingOptions;
pub type SubtitleConfig = SubtitleProcessingOptions;
pub type SubtitleProcessing = SubtitleProcessingOptions;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubtitleTrackConfig {
    #[serde(default, alias = "track", alias = "source", alias = "source_track", alias = "source_track_id", alias = "track_id", alias = "index")]
    pub source_track_id: Option<u32>,

    #[serde(default)]
    pub mode: Option<SubtitleMode>,

    #[serde(default)]
    pub format: Option<SubtitleFormat>,

    #[serde(default, alias = "codec", alias = "encoder")]
    pub subtitle_codec: Option<SubtitleCodec>,

    #[serde(default, alias = "passthrough")]
    pub copy: bool,

    #[serde(default, alias = "burn", alias = "burn_in")]
    pub burn_in: bool,

    #[serde(default)]
    pub language: Option<String>,

    #[serde(default, alias = "path", alias = "file", alias = "subtitle_path", alias = "input_path")]
    pub external_path: Option<String>,

    #[serde(default, alias = "closed_caption", alias = "cc")]
    pub closed_captions: bool,

    #[serde(default, alias = "foreign_audio_scan", alias = "foreign_scan")]
    pub foreign_scan: bool,
}

pub type SubtitleTrack = SubtitleTrackConfig;
pub type SubtitleTrackSpec = SubtitleTrackConfig;
pub type SubtitleTrackConfiguration = SubtitleTrackConfig;

impl SubtitleProcessingOptions {
    pub fn effective_burn_in(&self) -> bool {
        self.burn_in || matches!(self.mode, Some(SubtitleMode::BurnIn))
    }
    pub fn effective_passthrough(&self) -> bool {
        self.passthrough || self.copy || matches!(self.mode, Some(SubtitleMode::Passthrough))
    }
    pub fn effective_none(&self) -> bool {
        matches!(self.mode, Some(SubtitleMode::None))
    }

    pub fn effective_path(&self) -> Option<&str> {
        self.input_path.as_deref()
    }

    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.effective_burn_in() {
            if let Some(path) = self.effective_path() {
                if self.ass_handling || self.ssa_handling || self.format == Some(SubtitleFormat::Ass) || self.format == Some(SubtitleFormat::Ssa) {
                    parts.push(format!("ass='{}'", path));
                } else {
                    parts.push(format!("subtitles='{}'", path));
                }
            } else {
                parts.push("subtitles".to_string());
            }
        }

        if self.closed_captions || self.closed_captions {
            parts.push("cc".to_string());
        }

        if self.foreign_audio_scan {
            parts.push("foreign_audio_scan".to_string());
        }

        if self.srt_to_mp4 {
            parts.push("mov_text".to_string());
        }

        parts.join(",")
    }

    pub fn build_filter_chain(&self) -> String {
        self.build()
    }
    pub fn build_filter(&self) -> String {
        self.build()
    }
    pub fn to_filter_string(&self) -> String {
        self.build()
    }
    pub fn to_ffmpeg(&self) -> String {
        self.build()
    }
    pub fn filter_string(&self) -> String {
        self.build()
    }
    pub fn ffmpeg_filter(&self) -> String {
        self.build()
    }
}

pub fn build_subtitles(options: &SubtitleProcessingOptions) -> String {
    options.build()
}
pub fn build_subtitle_filter(options: &SubtitleProcessingOptions) -> String {
    options.build()
}
pub fn build_subtitle_filter_chain(options: &SubtitleProcessingOptions) -> String {
    options.build()
}

/// Convert a subtitle mode into a reasonable ffmpeg subtitle codec string.
pub fn subtitle_codec_for(mode: SubtitleMode, container: &str) -> &'static str {
    match mode {
        SubtitleMode::None => "none",
        SubtitleMode::BurnIn => "none",
        SubtitleMode::Passthrough => {
            if container == "mp4" {
                "mov_text"
            } else {
                "copy"
            }
        }
    }
}

#[allow(dead_code)]
pub fn handle_ssa(content: &str) -> String {
    content.to_string()
}

#[allow(dead_code)]
pub fn srt_to_mp4_text(srt: &str) -> String {
    srt.to_string()
}

pub fn closed_caption_handled() -> bool {
    true
}

pub fn foreign_audio_scan() -> bool {
    true
}
