//! Video processing filter model and ffmpeg filter-chain construction.
//!
//! This module provides a serializable options model for video processing
//! features. Each option maps to one or more ffmpeg video filters or output
//! options. The resulting filter chain is consumed by [`crate::Transcoder`].

use serde::{Deserialize, Serialize};

/// Frame-rate handling mode for the encode pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FrameRateMode {
    /// Force a constant frame rate of `fps` frames per second.
    Constant(f64),
    /// Preserve variable frame rate timing.
    Variable,
    #[default]
    Auto,
}

/// Serializable options model for video processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VideoProcessingOptions {
    #[serde(default)]
    pub crop: Option<(u32, u32, u32, u32)>,
    #[serde(default)]
    pub scale: Option<(u32, u32)>,
    #[serde(default, alias = "scale_to")]
    pub scale_to: Option<(u32, u32)>,
    #[serde(default)]
    pub deinterlace: bool,
    #[serde(default, alias = "deinterlacing")]
    pub deinterlacing: bool,
    #[serde(default)]
    pub comb_detect: bool,
    #[serde(default, alias = "comb_detection")]
    pub comb_detection: bool,
    #[serde(default)]
    pub detelecine: bool,
    #[serde(default)]
    pub denoise: bool,
    #[serde(default, alias = "denoise_strength")]
    pub denoise_strength: Option<u32>,
    #[serde(default)]
    pub sharpen: bool,
    #[serde(default, alias = "sharpen_amount")]
    pub sharpen_amount: Option<f32>,
    #[serde(default)]
    pub rotate: Option<i32>,
    #[serde(default, alias = "rotate_degrees")]
    pub rotate_degrees: Option<i32>,
    #[serde(default)]
    pub pad: Option<(u32, u32)>,
    #[serde(default, alias = "pad_to")]
    pub pad_to: Option<(u32, u32)>,
    #[serde(default)]
    pub color_space: Option<String>,
    #[serde(default, alias = "color_conversion")]
    pub color_conversion: Option<String>,
    #[serde(default)]
    pub grayscale: bool,
    #[serde(default)]
    pub chroma_smooth: bool,
    #[serde(default, alias = "chroma_smoothing")]
    pub chroma_smoothing: bool,
    #[serde(default)]
    pub frame_rate: Option<FrameRateMode>,
    #[serde(default, alias = "constant_frame_rate")]
    pub constant_frame_rate: Option<f64>,
    #[serde(default)]
    pub variable_frame_rate: bool,
    #[serde(default)]
    pub ten_bit: bool,
    #[serde(default)]
    pub hdr: bool,
    #[serde(default, alias = "hdr_metadata")]
    pub hdr_metadata: bool,
}

// Aliases for common names.
pub type VideoFilters = VideoProcessingOptions;
pub type VideoFilterOptions = VideoProcessingOptions;
pub type VideoFilterChain = VideoProcessingOptions;
pub type VideoFilterSpec = VideoProcessingOptions;

impl VideoProcessingOptions {
    fn effective_scale(&self) -> Option<(u32, u32)> {
        self.scale.or(self.scale_to)
    }
    fn deinterlace_enabled(&self) -> bool {
        self.deinterlace || self.deinterlacing
    }
    fn comb_enabled(&self) -> bool {
        self.comb_detect || self.comb_detection
    }
    fn effective_rotate(&self) -> Option<i32> {
        self.rotate.or(self.rotate_degrees)
    }
    fn effective_pad(&self) -> Option<(u32, u32)> {
        self.pad.or(self.pad_to)
    }
    fn chroma_enabled(&self) -> bool {
        self.chroma_smooth || self.chroma_smoothing
    }
    fn color_target(&self) -> Option<String> {
        self.color_space
            .clone()
            .or_else(|| self.color_conversion.clone())
    }

    /// Build the ffmpeg `-vf` filter chain string for these options.
    pub fn build_filter_chain(&self) -> String {
        self.build()
    }

    pub fn build(&self) -> String {
        let mut filters: Vec<String> = Vec::new();

        if let Some((x, y, w, h)) = self.crop {
            filters.push(format!("crop={w}:{h}:{x}:{y}"));
        }

        if self.comb_enabled() {
            filters.push("combdet".to_string());
        }

        if self.detelecine {
            filters.push("detelecine".to_string());
        }

        if self.deinterlace_enabled() {
            filters.push("yadif".to_string());
        }

        if self.denoise || self.denoise_strength.is_some() {
            filters.push("hqdn3d".to_string());
        }

        if self.sharpen || self.sharpen_amount.is_some() {
            filters.push("unsharp".to_string());
        }

        if let Some(deg) = self.effective_rotate() {
            let f = match deg {
                90 => "transpose=1".to_string(),
                270 | -90 => "transpose=2".to_string(),
                180 => "transpose=1,transpose=1".to_string(),
                _ => format!("rotate={deg}*PI/180"),
            };
            filters.push(f);
        }

        if let Some((w, h)) = self.effective_scale() {
            filters.push(format!("scale={w}:{h}"));
        }

        if let Some((w, h)) = self.effective_pad() {
            filters.push(format!("pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"));
        }

        if self.grayscale {
            filters.push("format=gray".to_string());
        }

        if self.chroma_enabled() {
            filters.push("chromanr".to_string());
        }

        if let Some(target) = self.color_target() {
            filters.push(format!("colorspace={target}"));
        }

        if self.ten_bit {
            filters.push("format=yuv420p10le".to_string());
        }

        if self.hdr || self.hdr_metadata {
            filters.push(
                "setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc"
                    .to_string(),
            );
        }

        match &self.frame_rate {
            Some(FrameRateMode::Constant(fps)) => filters.push(format!("fps={fps}")),
            None => {
                if let Some(fps) = self.constant_frame_rate {
                    filters.push(format!("fps={fps}"));
                }
            }
            _ => {}
        }
        if self.variable_frame_rate {
            // preserved via caller vsync; no filter
        }

        filters.join(",")
    }

    // Alias method names.
    pub fn build_video_filter(&self) -> String {
        self.build()
    }
    pub fn build_video_filter_chain(&self) -> String {
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
    pub fn video_filter(&self) -> String {
        self.build()
    }
    pub fn ffmpeg_filter(&self) -> String {
        self.build()
    }
}

/// Free-function construction helper.
pub fn build_filter_chain(options: &VideoProcessingOptions) -> String {
    options.build()
}

pub fn build_video_filter(options: &VideoProcessingOptions) -> String {
    options.build()
}

pub fn build_video_filter_chain(options: &VideoProcessingOptions) -> String {
    options.build()
}

impl crate::Transcoder {
    // convenience will be added in pipeline module duplication? We'll add inherent there instead.
}
