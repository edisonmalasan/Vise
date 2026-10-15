use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContainerFormat {
    #[default]
    Mp4,
    Mkv,
    Webm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    #[default]
    H264,
    H265,
    Av1,
    Vp9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioCodec {
    #[default]
    Aac,
    Opus,
    Vorbis,
    Flac,
    Mp3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleMode {
    #[default]
    Passthrough,
    BurnIn,
    None,
}

/// Canonical, serializable specification for a single transcode job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobSpec {
    pub input_path: String,
    pub output_path: String,
    pub container: ContainerFormat,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub subtitle_mode: SubtitleMode,
    /// Source title index to process, if applicable.
    pub title_index: Option<u32>,
    /// Crop region as (x, y, width, height), if set.
    pub crop: Option<(u32, u32, u32, u32)>,
    /// Scale target (width, height), if set.
    pub scale_to: Option<(u32, u32)>,
    /// Optional constant-quality value.
    pub quality: Option<u32>,
}
