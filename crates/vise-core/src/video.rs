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

/// Which way a frame is mirrored.
///
/// Rotation and mirroring are separate choices in the interface: a user who
/// wants a sideways phone video often wants it turned, not mirrored, so neither
/// value implies the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FlipMode {
    /// Leave the frame as it is.
    #[default]
    None,
    /// Mirror left to right.
    Horizontal,
    /// Mirror top to bottom.
    Vertical,
    /// Mirror both ways.
    Both,
}

impl FlipMode {
    /// Stable identifier used by the interface layer.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Both => "both",
        }
    }

    /// The filter chain fragment this choice produces.
    pub fn filter(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Horizontal => Some("hflip"),
            Self::Vertical => Some("vflip"),
            Self::Both => Some("hflip,vflip"),
        }
    }
}

/// Colour range the encoder should signal and fill.
///
/// Video decoded outside a colour-tagged range assumes limited range, so a
/// full-range source stays washed out unless the job says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColourRange {
    /// Television range, 16-235.
    #[default]
    Limited,
    /// Full range, 0-255.
    Full,
}

impl ColourRange {
    /// The encoder argument value for this range.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Limited => "tv",
            Self::Full => "pc",
        }
    }
}

/// Encoder-side choices that are passed to the video encoder rather than
/// applied as filters.
///
/// Preset, profile, tune, level, and colour range change how frames are coded,
/// not what is in them, so they travel beside the filter model instead of
/// inside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VideoEncodingOptions {
    /// Encoder speed preset, such as `slow` or `veryfast`.
    pub preset: Option<String>,
    /// Coding profile, such as `high` or `main`.
    pub profile: Option<String>,
    /// Content tune, such as `film` or `animation`.
    pub tune: Option<String>,
    /// Coding level, such as `4.1`.
    pub level: Option<String>,
    /// Colour range to signal and fill.
    pub colour_range: Option<ColourRange>,
}

impl VideoEncodingOptions {
    /// Whether this job asks for anything at all.
    pub fn is_empty(&self) -> bool {
        self.preset.is_none()
            && self.profile.is_none()
            && self.tune.is_none()
            && self.level.is_none()
            && self.colour_range.is_none()
    }

    /// Encoder arguments for these options, in a stable order.
    ///
    /// An empty or blank value is skipped rather than forwarded, because an
    /// empty `-profile:v` would make the encoder refuse the whole job.
    pub fn encoder_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        let mut push_value = |flag: &str, value: &Option<String>| {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                args.push(flag.to_string());
                args.push(value.to_string());
            }
        };
        push_value("-preset", &self.preset);
        push_value("-profile:v", &self.profile);
        push_value("-tune", &self.tune);
        push_value("-level", &self.level);
        if let Some(range) = self.colour_range {
            args.push("-color_range".to_string());
            args.push(range.as_str().to_string());
        }
        args
    }
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
    /// Smooth the block edges a compressed picture leaves behind.
    ///
    /// Off by default: deblocking is a judgement about how much of a source's
    /// compression to remove, and applying it unasked for changes every encode.
    #[serde(default)]
    pub deblock: bool,
    /// Block size for [`Self::deblock`], 4 to 31.
    ///
    /// `None` lets the filter pick its default, which is the right answer for a
    /// user who only asked for the edges to be smoothed.
    #[serde(default, alias = "deblock_block", alias = "deblock_block_size")]
    pub deblock_strength: Option<u8>,
    #[serde(default)]
    pub rotate: Option<i32>,
    #[serde(default, alias = "rotate_degrees")]
    pub rotate_degrees: Option<i32>,
    #[serde(default, alias = "flip_mode", alias = "mirror")]
    pub flip: Option<FlipMode>,
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

    /// The deblock filter for the chosen strength.
    ///
    /// The filter's own `block` option is the block size it works on, and a
    /// value outside its 4-to-31 range would make ffmpeg refuse the job. A value
    /// that is present but out of range is left to the filter rather than
    /// clamping it into the middle of the range, which would silently change
    /// what the user asked for.
    fn deblock_filter(&self) -> String {
        match self.deblock_strength {
            Some(strength) => format!("deblock=filter=strong:block={strength}"),
            None => "deblock".to_string(),
        }
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

        if let Some(strength) = self.denoise_strength.filter(|s| *s > 0) {
            // One strength for the whole filter: spatial smoothing for luma and
            // chroma, with the temporal terms kept a little higher so noise does
            // not shimmer between frames.
            let temporal = strength * 3 / 2;
            filters.push(format!(
                "hqdn3d={strength}:{strength}:{temporal}:{temporal}"
            ));
        } else if self.denoise {
            filters.push("hqdn3d".to_string());
        }

        if let Some(amount) = self.sharpen_amount.filter(|a| *a != 0.0) {
            filters.push(format!("unsharp=5:5:{amount}:5:5:0"));
        } else if self.sharpen {
            filters.push("unsharp".to_string());
        }

        if self.deblock {
            filters.push(self.deblock_filter());
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

        if let Some(flip) = self.flip.and_then(|flip| flip.filter()) {
            filters.push(flip.to_string());
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

pub fn build_video_filter_chain(options: &VideoProcessingOptions) -> String {
    options.build()
}

impl crate::Transcoder {
    // convenience will be added in pipeline module duplication? We'll add inherent there instead.
}
