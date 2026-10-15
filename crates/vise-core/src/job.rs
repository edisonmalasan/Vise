use crate::VideoProcessingOptions;
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
    /// Video processing options applied during encoding.
    #[serde(default)]
    pub video_processing: VideoProcessingOptions,
    /// Alias for video processing options.
    #[serde(default)]
    pub video_filters: Option<VideoProcessingOptions>,
    /// Alias for video processing options.
    #[serde(default)]
    pub video_options: Option<VideoProcessingOptions>,
    /// Alias for video processing options.
    #[serde(default)]
    pub filters: Option<VideoProcessingOptions>,
}

impl JobSpec {
    pub fn video_processing_options(&self) -> VideoProcessingOptions {
        let mut opts = self.video_processing.clone();
        for x in [
            self.video_filters.as_ref(),
            self.video_options.as_ref(),
            self.filters.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if x.crop.is_some() {
                opts.crop = x.crop;
            }
            if x.scale.is_some() {
                opts.scale = x.scale;
            }
            if x.scale_to.is_some() {
                opts.scale_to = x.scale_to;
            }
            opts.deinterlace |= x.deinterlace || x.deinterlacing;
            opts.comb_detect |= x.comb_detect || x.comb_detection;
            opts.detelecine |= x.detelecine;
            opts.denoise |= x.denoise;
            opts.sharpen |= x.sharpen;
            opts.grayscale |= x.grayscale;
            opts.chroma_smooth |= x.chroma_smooth || x.chroma_smoothing;
            opts.ten_bit |= x.ten_bit;
            opts.hdr |= x.hdr || x.hdr_metadata;
            opts.hdr_metadata |= x.hdr_metadata || x.hdr;
            if x.rotate.is_some() {
                opts.rotate = x.rotate;
            }
            if x.rotate_degrees.is_some() {
                opts.rotate_degrees = x.rotate_degrees;
            }
            if x.pad.is_some() {
                opts.pad = x.pad;
            }
            if x.pad_to.is_some() {
                opts.pad_to = x.pad_to;
            }
            if x.color_space.is_some() {
                opts.color_space = x.color_space.clone();
            }
            if x.color_conversion.is_some() {
                opts.color_conversion = x.color_conversion.clone();
            }
            if x.constant_frame_rate.is_some() {
                opts.constant_frame_rate = x.constant_frame_rate;
            }
            if x.frame_rate.is_some() {
                opts.frame_rate = x.frame_rate.clone();
            }
            if x.variable_frame_rate {
                opts.variable_frame_rate = true;
            }
            if x.denoise_strength.is_some() {
                opts.denoise_strength = x.denoise_strength;
            }
            if x.sharpen_amount.is_some() {
                opts.sharpen_amount = x.sharpen_amount;
            }
        }
        if let Some(c) = self.crop {
            if opts.crop.is_none() {
                opts.crop = Some(c);
            }
        }
        if let Some(s) = self.scale_to {
            if opts.scale.is_none() && opts.scale_to.is_none() {
                opts.scale_to = Some(s);
            }
        }
        opts
    }

    pub fn build_filter_chain(&self) -> String {
        self.video_processing_options().build_filter_chain()
    }
    pub fn build_video_filter(&self) -> String {
        self.build_filter_chain()
    }
    pub fn build_video_filter_chain(&self) -> String {
        self.build_filter_chain()
    }
    pub fn to_filter_string(&self) -> String {
        self.build_filter_chain()
    }
}
