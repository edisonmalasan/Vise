use crate::{AudioProcessingOptions, VideoProcessingOptions};
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
    Ac3,
    Eac3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleMode {
    #[default]
    Passthrough,
    BurnIn,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EncodingMode {
    #[default]
    ConstantQuality,
    Cq,
    Crf,
    ConstantQualityMode,
    AverageBitrate,
    Abr,
    AverageBitrateMode,
    TargetSize,
    TargetFileSize,
    TargetSizeMode,
    TwoPass,
    TwoPassEncoding,
    TwoPassMode,
    Lossless,
    LosslessEncoding,
    LosslessMode,
}
impl EncodingMode {
    pub fn is_constant_quality(&self) -> bool {
        matches!(
            self,
            Self::ConstantQuality | Self::ConstantQualityMode | Self::Cq | Self::Crf
        )
    }
    pub fn is_average_bitrate(&self) -> bool {
        matches!(self, Self::AverageBitrate | Self::Abr)
    }
    pub fn is_target_size(&self) -> bool {
        matches!(self, Self::TargetSize | Self::TargetFileSize)
    }
    pub fn is_two_pass(&self) -> bool {
        matches!(self, Self::TwoPass | Self::TwoPassEncoding)
    }
    pub fn is_lossless(&self) -> bool {
        matches!(self, Self::Lossless | Self::LosslessEncoding)
    }
}
// Compatibility aliases removed.

/// Output framing and compatibility choices for the finished file.
///
/// These are properties of the finished container rather than of any one
/// stream, so they sit beside the codec choices on the job. Each flag is a
/// request the pipeline either honours for the chosen container or reports as
/// unavailable, never something that is accepted and then quietly dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OutputOptions {
    /// Move the index to the front of an MP4 so playback can start before the
    /// whole file has downloaded.
    ///
    /// Unset means "on", because that is what an MP4 has always been given and
    /// turning it off silently would change existing jobs.
    pub web_optimized: Option<bool>,
    /// Nudge audio timestamps so the first sound lines up with the first frame.
    pub align_audio_video: bool,
    /// Restrict an MP4 to the coding tools older portable players understand.
    pub ipod_compatible: bool,
    /// Copy source tags to the output instead of dropping them.
    pub passthrough_metadata: bool,
}

impl OutputOptions {
    /// Whether the finished MP4 carries a front-loaded index.
    pub fn wants_web_optimized(&self) -> bool {
        self.web_optimized.unwrap_or(true) || self.ipod_compatible
    }

    /// Whether this job asks for anything at all.
    pub fn is_empty(&self) -> bool {
        !self.wants_web_optimized()
            && !self.align_audio_video
            && !self.ipod_compatible
            && !self.passthrough_metadata
    }

    /// The MP4 profile and level that the portable-player flag pins, if any.
    pub fn mp4_compatibility(&self) -> Option<(&'static str, &'static str)> {
        self.ipod_compatible.then_some(("baseline", "3.0"))
    }
}

/// Canonical, serializable specification for a single transcode job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobSpec {
    #[serde(default)]
    pub input_path: String,
    #[serde(default)]
    pub output_path: String,
    #[serde(default)]
    pub container: ContainerFormat,
    #[serde(default)]
    pub video_codec: VideoCodec,
    #[serde(default)]
    pub audio_codec: AudioCodec,
    #[serde(default)]
    pub subtitle_mode: SubtitleMode,
    /// Source title index to process, if applicable.
    pub title_index: Option<u32>,
    /// Crop region as (x, y, width, height), if set.
    pub crop: Option<(u32, u32, u32, u32)>,
    /// Scale target (width, height), if set.
    pub scale_to: Option<(u32, u32)>,
    /// Optional constant-quality value.
    #[serde(default)]
    pub quality: Option<u32>,
    #[serde(default)]
    pub cq: Option<u32>,
    #[serde(default)]
    pub crf: Option<u32>,
    #[serde(default)]
    pub constant_quality_value: Option<u32>,

    /// Encoding rate-control mode.
    #[serde(
        default,
        alias = "rate_control",
        alias = "rate_control_mode",
        alias = "encode_mode",
        alias = "video_rate_control",
        alias = "encoding_rate_control"
    )]
    pub encoding_mode: Option<EncodingMode>,

    /// Encoder-side video choices: preset, profile, tune, level, colour range.
    #[serde(
        default,
        alias = "encoding_options",
        alias = "video_encoder_options",
        alias = "encoder_options"
    )]
    pub video_encoding: crate::VideoEncodingOptions,

    /// Output framing and compatibility flags for the finished container.
    #[serde(
        default,
        alias = "output",
        alias = "container_options",
        alias = "output_flags"
    )]
    pub output_options: OutputOptions,

    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub average_bitrate: Option<u32>,
    #[serde(default)]
    pub abr: Option<u32>,
    #[serde(default)]
    pub target_bitrate: Option<u32>,
    #[serde(default)]
    pub video_bitrate: Option<u32>,
    #[serde(default)]
    pub average_bitrate_kbps: Option<u32>,

    #[serde(default)]
    pub target_size: Option<u64>,
    #[serde(default)]
    pub target_file_size: Option<u64>,
    #[serde(default)]
    pub target_size_bytes: Option<u64>,

    #[serde(default)]
    pub two_pass: bool,
    #[serde(default)]
    pub lossless: bool,
    #[serde(default)]
    pub constant_quality: bool,
    #[serde(default)]
    pub average_bitrate_mode: bool,
    #[serde(default)]
    pub target_size_mode: bool,
    #[serde(default)]
    pub two_pass_encoding: bool,
    #[serde(default)]
    pub lossless_encoding: bool,
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

    /// Audio processing options applied during encoding.
    #[serde(
        default,
        alias = "audio_filters",
        alias = "audio_options",
        alias = "audio"
    )]
    pub audio_processing: AudioProcessingOptions,

    /// Subtitle processing options applied during encoding.
    #[serde(
        default,
        alias = "subtitle_filters",
        alias = "subtitle_options",
        alias = "subtitles_options",
        alias = "subtitle",
        alias = "subtitles",
        alias = "subtitle_processing_options"
    )]
    pub subtitle_processing: crate::SubtitleProcessingOptions,

    /// Subtitle tracks configuration.
    #[serde(
        default,
        alias = "subtitle_track_list",
        alias = "subtitle_tracks_config",
        alias = "subtitle_track",
        alias = "subtitles_tracks"
    )]
    pub subtitle_tracks: Vec<crate::SubtitleTrackConfig>,
    #[serde(default)]
    pub subtitle_track_configs: Vec<crate::SubtitleTrackConfig>,
    #[serde(default)]
    pub subtitle_track_config: Option<crate::SubtitleTrackConfig>,

    /// External subtitle file path.
    #[serde(
        default,
        alias = "subtitles_path",
        alias = "subtitle_file",
        alias = "external_subtitle",
        alias = "subtitle_input"
    )]
    pub subtitle_path: Option<String>,

    /// Chapter processing options.
    #[serde(
        default,
        alias = "chapter_options",
        alias = "chapters",
        alias = "chapter",
        alias = "chapter_processing_options",
        alias = "chapters_options"
    )]
    pub chapter_processing: crate::ChapterProcessingOptions,

    /// Alias for chapter processing options.
    #[serde(default)]
    pub chapters_processing: Option<crate::ChapterProcessingOptions>,

    /// Metadata processing options.
    #[serde(
        default,
        alias = "metadata",
        alias = "metadata_processing_options",
        alias = "tags"
    )]
    pub metadata_processing: crate::MetadataProcessingOptions,

    /// Alias for metadata processing options.
    #[serde(default)]
    pub metadata_options: Option<crate::MetadataProcessingOptions>,

    /// Top-level chapter copy/editing toggles.
    #[serde(default)]
    pub chapter_copy: bool,
    #[serde(default)]
    pub chapter_edit: bool,
    #[serde(default)]
    pub chapter_editing: bool,

    /// Top-level metadata copy/editing toggles.
    #[serde(default)]
    pub metadata_copy: bool,
    #[serde(default)]
    pub metadata_edit: bool,
    #[serde(default)]
    pub metadata_editing: bool,

    /// Cover art and attachment handling toggles.
    #[serde(default, alias = "cover_art")]
    pub cover_art_handling: bool,
    #[serde(default, alias = "attachment")]
    pub attachment_handling: bool,

    /// Audio tracks configuration.
    #[serde(default, alias = "audio_track_list", alias = "audio_tracks_config")]
    pub audio_tracks: Vec<crate::AudioTrackConfig>,
    #[serde(default)]
    pub audio_track_configs: Vec<crate::AudioTrackConfig>,
    #[serde(default)]
    pub audio_track_config: Option<crate::AudioTrackConfig>,

    /// Hardware acceleration options.
    #[serde(default, alias = "hardware_options", alias = "hw_acceleration")]
    pub hardware_acceleration: crate::HardwareAcceleration,

    #[serde(default)]
    pub hardware: Option<crate::HardwareAcceleration>,
    #[serde(default)]
    pub hwaccel: Option<crate::HardwareAcceleration>,
    #[serde(default)]
    pub hw_accel: Option<crate::HardwareAcceleration>,
    #[serde(default)]
    pub acceleration: Option<crate::HardwareAcceleration>,
    #[serde(default)]
    pub hwacceleration: Option<crate::HardwareAcceleration>,
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
            if x.flip.is_some() {
                opts.flip = x.flip;
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

    /// Encoder-side video options, with the portable-player profile folded in.
    ///
    /// The compatibility profile is a hard requirement of the output flags, so
    /// it wins over a profile the user picked while the flag is on.
    pub fn video_encoding_options(&self) -> crate::VideoEncodingOptions {
        let mut opts = self.video_encoding.clone();
        if let Some((profile, level)) = self.output_options.mp4_compatibility() {
            opts.profile = Some(profile.to_string());
            opts.level = Some(level.to_string());
        }
        opts
    }

    pub fn build_filter_chain(&self) -> String {
        self.video_processing_options().build_filter_chain()
    }
    pub fn effective_quality(&self) -> Option<u32> {
        self.quality
            .or(self.cq)
            .or(self.crf)
            .or(self.constant_quality_value)
    }
    pub fn effective_target_size_bytes(&self) -> Option<u64> {
        self.target_size
            .or(self.target_file_size)
            .or(self.target_size_bytes)
    }
    pub fn resolved_encoding_mode(&self) -> EncodingMode {
        if let Some(m) = self.encoding_mode {
            return m;
        }
        if self.lossless {
            return EncodingMode::Lossless;
        }
        if self.two_pass {
            return EncodingMode::TwoPass;
        }
        if self.effective_target_size_bytes().is_some() || self.target_size_mode {
            return EncodingMode::TargetSize;
        }
        if self.effective_bitrate_kbps().is_some() || self.average_bitrate_mode {
            return EncodingMode::AverageBitrate;
        }
        if self.effective_quality().is_some() || self.constant_quality {
            return EncodingMode::ConstantQuality;
        }
        EncodingMode::default()
    }
    pub fn encoding_mode(&self) -> EncodingMode {
        self.resolved_encoding_mode()
    }
    pub fn two_pass_enabled(&self) -> bool {
        self.two_pass || self.two_pass_encoding || self.resolved_encoding_mode().is_two_pass()
    }
    pub fn lossless_enabled(&self) -> bool {
        self.lossless || self.lossless_encoding || self.resolved_encoding_mode().is_lossless()
    }
    pub fn effective_bitrate_kbps(&self) -> Option<u32> {
        self.bitrate
            .or(self.average_bitrate)
            .or(self.abr)
            .or(self.target_bitrate)
            .or(self.video_bitrate)
            .or(self.average_bitrate_kbps)
    }
    pub fn audio_processing_options(&self) -> AudioProcessingOptions {
        let mut opts = self.audio_processing.clone();
        if let Some(o) = &self.audio_track_config {
            if o.mixdown.is_some() {
                opts.mixdown = o.mixdown.clone();
            }
            if o.mixdown_enum.is_some() {
                opts.mixdown_enum = o.mixdown_enum;
            }
            if o.sample_rate.is_some() {
                opts.sample_rate = o.sample_rate;
            }
            if o.dither {
                opts.dither = true;
            }
            if o.dither_mode.is_some() {
                opts.dither_mode = o.dither_mode.clone();
            }
            if o.gain.is_some() {
                opts.gain = o.gain;
            }
            if o.volume.is_some() {
                opts.volume = o.volume;
            }
            if o.delay.is_some() {
                opts.delay = o.delay;
            }
            if o.passthrough {
                opts.passthrough = true;
            }
            if o.channel_count.is_some() {
                opts.channels = o.channel_count;
            }
            if o.encoder.is_some() {
                opts.encoder = o.encoder.clone();
            }
            if o.bitrate.is_some() {
                opts.bitrate = o.bitrate;
            }
            if o.quality.is_some() {
                opts.quality = o.quality;
            }
        }
        opts
    }

    pub fn hardware_acceleration_options(&self) -> crate::HardwareAcceleration {
        let mut opts = self.hardware_acceleration.clone();
        for x in [
            self.hardware.as_ref(),
            self.hwaccel.as_ref(),
            self.hw_accel.as_ref(),
            self.acceleration.as_ref(),
            self.hwacceleration.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if x.enabled {
                opts.enabled = true;
            }
            if x.mode != crate::HardwareMode::Auto || opts.mode == crate::HardwareMode::Auto {
                // merge only if non-default
            }
            opts.decode.enabled |= x.decode.enabled;
            opts.encode.enabled |= x.encode.enabled;
            if x.encode.encoder.is_some() {
                opts.encode.encoder = x.encode.encoder.clone();
            }
            if x.decode.hwaccel.is_some() {
                opts.decode.hwaccel = x.decode.hwaccel.clone();
            }
            opts.fallback = opts.fallback || x.fallback;
        }
        opts
    }

    pub fn subtitle_processing_options(&self) -> crate::SubtitleProcessingOptions {
        let mut opts = self.subtitle_processing.clone();
        if let Some(t) = &self.subtitle_track_config {
            if t.source_track_id.is_some() {
                opts.source_track = t.source_track_id;
            }
            if t.burn_in {
                opts.burn_in = true;
            }
            if t.copy {
                opts.passthrough = true;
                opts.copy = true;
            }
            if t.external_path.is_some() {
                opts.input_path = t.external_path.clone();
            }
            if t.closed_captions {
                opts.closed_captions = true;
            }
            if t.foreign_scan {
                opts.foreign_audio_scan = true;
            }
            if t.mode.is_some() {
                opts.mode = t.mode;
            }
        }
        for t in self
            .subtitle_tracks
            .iter()
            .chain(self.subtitle_track_configs.iter())
        {
            if opts.source_track.is_none() {
                opts.source_track = t.source_track_id;
            }
            if t.forced_only {
                opts.forced_only = true;
            }
        }
        if let Some(p) = &self.subtitle_path {
            if opts.input_path.is_none() {
                opts.input_path = Some(p.clone());
            }
        }
        if self.subtitle_mode == crate::SubtitleMode::BurnIn {
            opts.burn_in = true;
        }
        if self.subtitle_mode == crate::SubtitleMode::Passthrough {
            opts.passthrough = true;
            opts.copy = true;
        }
        if self.subtitle_mode == crate::SubtitleMode::None {
            opts.mode = Some(crate::SubtitleMode::None);
        }
        opts
    }

    pub fn chapter_processing_options(&self) -> crate::ChapterProcessingOptions {
        let mut opts = self.chapter_processing.clone();
        if let Some(c) = &self.chapters_processing {
            // merge fields
            if c.copy {
                opts.copy = true;
            }
            if c.edit {
                opts.edit = true;
            }
            if !c.chapters.is_empty() {
                opts.chapters = c.chapters.clone();
            }
            if c.cover_art {
                opts.cover_art = true;
            }
            if c.attachments {
                opts.attachments = true;
            }
        }
        if self.chapter_copy {
            opts.copy = true;
            opts.chapter_copy = true;
        }
        if self.chapter_edit {
            opts.edit = true;
            opts.chapter_edit = true;
        }
        if self.chapter_editing {
            opts.chapter_editing = true;
            opts.edit = true;
        }
        if self.cover_art_handling {
            opts.cover_art = true;
        }
        if self.attachment_handling {
            opts.attachments = true;
            opts.attachment_copy = true;
        }
        opts
    }

    pub fn metadata_processing_options(&self) -> crate::MetadataProcessingOptions {
        let mut opts = self.metadata_processing.clone();
        if let Some(m) = &self.metadata_options {
            if m.copy {
                opts.copy = true;
            }
            if m.edit {
                opts.edit = true;
            }
            if m.title.is_some() {
                opts.title = m.title.clone();
            }
            if !m.metadata.is_empty() {
                opts.metadata = m.metadata.clone();
            }
        }
        if self.metadata_copy {
            opts.copy = true;
            opts.metadata_copy = true;
        }
        if self.metadata_edit {
            opts.edit = true;
            opts.metadata_edit = true;
        }
        if self.metadata_editing {
            opts.metadata_editing = true;
            opts.edit = true;
        }
        if self.output_options.passthrough_metadata {
            opts.copy = true;
        }
        if self.cover_art_handling {
            opts.cover_art = true;
        }
        if self.attachment_handling {
            opts.attachments = true;
            opts.attachment_copy = true;
        }
        opts
    }

    pub fn build_audio_filter_chain(&self) -> String {
        self.audio_processing_options().build()
    }
    pub fn build_audio_filter(&self) -> String {
        self.build_audio_filter_chain()
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
