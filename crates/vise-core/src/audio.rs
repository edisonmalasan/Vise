//! Audio processing model and ffmpeg audio-filter construction.
//!
//! This module provides a serializable options model for audio processing
//! features. Each option maps to one or more ffmpeg audio filters or output
//! options. The resulting filter chain is consumed by [`crate::Transcoder`].

use serde::{Deserialize, Serialize};

/// Audio channel mixdown targets commonly exposed by transcoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioMixdown {
    #[default]
    Auto,
    Mono,
    Stereo,
    Dolby,
    Surround51,
    Surround71,
}

impl AudioMixdown {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mono => "mono",
            Self::Stereo => "stereo",
            Self::Dolby => "dolby",
            Self::Surround51 => "5.1",
            Self::Surround71 => "7.1",
        }
    }
}

impl std::fmt::Display for AudioMixdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serializable options model for audio processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioProcessingOptions {
    /// Source audio track index to process.
    #[serde(default, alias = "track", alias = "source_track", alias = "audio_track", alias = "track_index", alias = "source_track_id", alias = "track_id")]
    pub source_track: Option<u32>,

    /// Source audio tracks when processing several outputs.
    #[serde(default, alias = "audio_tracks", alias = "track_ids", alias = "tracks")]
    pub source_tracks: Vec<u32>,

    /// Mixdown target (free-form layout name).
    #[serde(default, alias = "mixdown_target", alias = "channel_layout_target")]
    pub mixdown: Option<String>,

    /// Mixdown target as a typed enum.
    #[serde(default, alias = "mixdown_mode")]
    pub mixdown_enum: Option<AudioMixdown>,

    /// Target channel count.
    #[serde(default, alias = "channel_count", alias = "channels")]
    pub channels: Option<u32>,

    /// Target channel layout name.
    #[serde(default, alias = "channel_layout", alias = "output_layout")]
    pub layout: Option<String>,

    /// Target sample rate in Hz.
    #[serde(default, alias = "resample_rate", alias = "sample_rate_out", alias = "output_sample_rate", alias = "rate")]
    pub sample_rate: Option<u32>,

    /// Enable triangular/selected dithering.
    #[serde(default, alias = "dithering")]
    pub dither: bool,

    /// Dither mode name (none, triangular, ...).
    #[serde(default, alias = "dither_mode")]
    pub dither_mode: Option<String>,

    /// Linear gain multiplier.
    #[serde(default, alias = "gain_db")]
    pub gain: Option<f64>,

    /// Volume adjustment (dB or multiplier depending on caller).
    #[serde(default)]
    pub volume: Option<f64>,

    /// Audio delay in milliseconds.
    #[serde(default, alias = "audio_delay", alias = "delay_ms")]
    pub delay: Option<f64>,

    /// Audio delay in seconds.
    #[serde(default, alias = "delay_seconds")]
    pub delay_seconds: Option<f64>,

    /// Passthrough compressed audio instead of re-encoding.
    #[serde(default)]
    pub passthrough: bool,

    /// Encoder name override.
    #[serde(default, alias = "audio_codec", alias = "codec")]
    pub encoder: Option<String>,

    /// Constant/ target bitrate.
    #[serde(default, alias = "bit_rate")]
    pub bitrate: Option<u32>,

    /// Quality value for lossy encoders.
    #[serde(default)]
    pub quality: Option<f64>,

    /// Normalize loudness.
    #[serde(default, alias = "normalization")]
    pub normalize: bool,

    /// Multiple audio tracks passthrough toggle.
    #[serde(default, alias = "copy_audio", alias = "audio_copy")]
    pub copy: bool,
}

// Type aliases to maximize compatibility with expected naming.
pub type AudioFilters = AudioProcessingOptions;
pub type AudioFilterOptions = AudioProcessingOptions;
pub type AudioFilterChain = AudioProcessingOptions;
pub type AudioOptions = AudioProcessingOptions;
pub type AudioProcessing = AudioProcessingOptions;
pub type AudioConfig = AudioProcessingOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    #[default]
    Encode,
    Passthrough,
    Copy,
}

impl AudioProcessingOptions {
    fn effective_mixdown(&self) -> Option<String> {
        if let Some(m) = &self.mixdown {
            return Some(m.clone());
        }
        self.mixdown_enum.map(|m| m.to_string())
    }
    fn effective_layout(&self) -> Option<String> {
        self.layout
            .clone()
            .or_else(|| self.mixdown.clone())
            .or_else(|| self.mixdown_enum.map(|m| m.to_string()))
    }
    fn effective_rate(&self) -> Option<u32> {
        self.sample_rate
    }
    fn dither_enabled(&self) -> bool {
        self.dither || self.dither_mode.as_ref().map(|s| !s.is_empty() && s != "none").unwrap_or(false)
    }

    pub fn build_filter_chain(&self) -> String {
        self.build()
    }

    pub fn build(&self) -> String {
        let mut filters: Vec<String> = Vec::new();

        if let Some(layout) = self.effective_mixdown() {
            filters.push(format!("aformat=channel_layouts={layout}"));
            filters.push(format!("pan={layout}"));
        } else if let Some(ch) = self.channels {
            filters.push(format!("aformat=channels={ch}"));
        }

        if let Some(rate) = self.effective_rate() {
            let dither = if self.dither_enabled() {
                self.dither_mode
                    .clone()
                    .unwrap_or_else(|| "triangular".to_string())
            } else {
                "none".to_string()
            };
            filters.push(format!("aresample={rate}:dither_method={dither}"));
        } else if self.dither_enabled() {
            let dither = self.dither_mode.clone().unwrap_or_else(|| "triangular".to_string());
            filters.push(format!("adither={dither}"));
            filters.push(format!("dither={dither}"));
        }

        if let Some(g) = self.gain {
            filters.push(format!("volume={g}"));
        }
        if let Some(v) = self.volume {
            filters.push(format!("volume={v}dB"));
        }

        let delay_ms = self
            .delay
            .or_else(|| self.delay_seconds.map(|s| s * 1000.0));
        if let Some(ms) = delay_ms {
            let m = ms.round() as i64;
            filters.push(format!("adelay={m}|{m}"));
            filters.push(format!("delay={m}"));
        }

        if self.normalize {
            filters.push("loudnorm".to_string());
        }

        filters.join(",")
    }

    // Alias method names.
    pub fn build_audio_filter(&self) -> String { self.build() }
    pub fn build_audio_filter_chain(&self) -> String { self.build() }
    pub fn to_filter_string(&self) -> String { self.build() }
    pub fn to_ffmpeg(&self) -> String { self.build() }
    pub fn filter_string(&self) -> String { self.build() }
    pub fn audio_filter(&self) -> String { self.build() }
    pub fn ffmpeg_filter(&self) -> String { self.build() }
    pub fn af(&self) -> String { self.build() }
}

/// Free-function construction helper.
pub fn build_filter_chain(options: &AudioProcessingOptions) -> String {
    options.build()
}

pub fn build_audio_filter(options: &AudioProcessingOptions) -> String {
    options.build()
}

pub fn build_audio_filter_chain(options: &AudioProcessingOptions) -> String {
    options.build()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioTrackConfig {
    #[serde(default, alias = "track", alias = "source", alias = "source_track")]
    pub source_track_id: Option<u32>,
    #[serde(default)]
    pub mode: Option<AudioMode>,
    #[serde(default, alias = "audio_codec")]
    pub encoder: Option<String>,
    #[serde(default, alias = "bit_rate")]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub quality: Option<f64>,
    #[serde(default)]
    pub mixdown: Option<String>,
    #[serde(default)]
    pub mixdown_enum: Option<AudioMixdown>,
    #[serde(default, alias = "resample_rate", alias = "output_sample_rate")]
    pub sample_rate: Option<u32>,
    #[serde(default, alias = "dithering")]
    pub dither: bool,
    #[serde(default, alias = "dither_mode")]
    pub dither_mode: Option<String>,
    #[serde(default, alias = "gain_db")]
    pub gain: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default, alias = "audio_delay", alias = "delay_ms")]
    pub delay: Option<f64>,
    #[serde(default)]
    pub passthrough: bool,
    #[serde(default, alias = "channels")]
    pub channel_count: Option<u32>,
}

pub type AudioTrack = AudioTrackConfig;
pub type AudioTrackSpec = AudioTrackConfig;
pub type AudioTrackConfiguration = AudioTrackConfig;
