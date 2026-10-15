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

    /// Audio processing options applied during encoding.
    #[serde(default, alias = "audio_filters", alias = "audio_options", alias = "audio")]
    pub audio_processing: AudioProcessingOptions,

    /// Subtitle processing options applied during encoding.
    #[serde(default, alias = "subtitle_filters", alias = "subtitle_options", alias = "subtitles_options", alias = "subtitle", alias = "subtitles", alias = "subtitle_processing_options")]
    pub subtitle_processing: crate::SubtitleProcessingOptions,

    /// Subtitle tracks configuration.
    #[serde(default, alias = "subtitle_track_list", alias = "subtitle_tracks_config", alias = "subtitle_track", alias = "subtitles_tracks")]
    pub subtitle_tracks: Vec<crate::SubtitleTrackConfig>,
    #[serde(default)]
    pub subtitle_track_configs: Vec<crate::SubtitleTrackConfig>,
    #[serde(default)]
    pub subtitle_track_config: Option<crate::SubtitleTrackConfig>,

    /// External subtitle file path.
    #[serde(default, alias = "subtitles_path", alias = "subtitle_file", alias = "external_subtitle", alias = "subtitle_input")]
    pub subtitle_path: Option<String>,

    /// Chapter processing options.
    #[serde(default, alias = "chapter_options", alias = "chapters", alias = "chapter", alias = "chapter_processing_options", alias = "chapters_options")]
    pub chapter_processing: crate::ChapterProcessingOptions,

    /// Alias for chapter processing options.
    #[serde(default)]
    pub chapters_processing: Option<crate::ChapterProcessingOptions>,

    /// Metadata processing options.
    #[serde(default, alias = "metadata", alias = "metadata_processing_options", alias = "tags")]
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
    pub fn audio_processing_options(&self) -> AudioProcessingOptions {
        let mut opts = self.audio_processing.clone();
        if let Some(o) = &self.audio_track_config {
            if o.mixdown.is_some() { opts.mixdown = o.mixdown.clone(); }
            if o.mixdown_enum.is_some() { opts.mixdown_enum = o.mixdown_enum; }
            if o.sample_rate.is_some() { opts.sample_rate = o.sample_rate; }
            if o.dither { opts.dither = true; }
            if o.dither_mode.is_some() { opts.dither_mode = o.dither_mode.clone(); }
            if o.gain.is_some() { opts.gain = o.gain; }
            if o.volume.is_some() { opts.volume = o.volume; }
            if o.delay.is_some() { opts.delay = o.delay; }
            if o.passthrough { opts.passthrough = true; }
            if o.channel_count.is_some() { opts.channels = o.channel_count; }
            if o.encoder.is_some() { opts.encoder = o.encoder.clone(); }
            if o.bitrate.is_some() { opts.bitrate = o.bitrate; }
            if o.quality.is_some() { opts.quality = o.quality; }
        }
        opts
    }

    pub fn subtitle_processing_options(&self) -> crate::SubtitleProcessingOptions {
        let mut opts = self.subtitle_processing.clone();
        if let Some(t) = &self.subtitle_track_config {
            if t.source_track_id.is_some() { opts.source_track = t.source_track_id; }
            if t.burn_in { opts.burn_in = true; }
            if t.copy { opts.passthrough = true; opts.copy = true; }
            if t.external_path.is_some() { opts.input_path = t.external_path.clone(); }
            if t.closed_captions { opts.closed_captions = true; }
            if t.foreign_scan { opts.foreign_audio_scan = true; }
            if t.mode.is_some() { opts.mode = t.mode; }
        }
        for t in self.subtitle_tracks.iter().chain(self.subtitle_track_configs.iter()) {
            if opts.source_track.is_none() { opts.source_track = t.source_track_id; }
        }
        if let Some(p) = &self.subtitle_path {
            if opts.input_path.is_none() { opts.input_path = Some(p.clone()); }
        }
        if self.subtitle_mode == crate::SubtitleMode::BurnIn { opts.burn_in = true; }
        if self.subtitle_mode == crate::SubtitleMode::Passthrough { opts.passthrough = true; opts.copy = true; }
        if self.subtitle_mode == crate::SubtitleMode::None {
            opts.mode = Some(crate::SubtitleMode::None);
        }
        opts
    }

    pub fn chapter_processing_options(&self) -> crate::ChapterProcessingOptions {
        let mut opts = self.chapter_processing.clone();
        if let Some(c) = &self.chapters_processing {
            // merge fields
            if c.copy { opts.copy = true; }
            if c.edit { opts.edit = true; }
            if !c.chapters.is_empty() { opts.chapters = c.chapters.clone(); }
            if c.cover_art { opts.cover_art = true; }
            if c.attachments { opts.attachments = true; }
        }
        if self.chapter_copy { opts.copy = true; opts.chapter_copy = true; }
        if self.chapter_edit { opts.edit = true; opts.chapter_edit = true; }
        if self.chapter_editing { opts.chapter_editing = true; opts.edit = true; }
        if self.cover_art_handling { opts.cover_art = true; }
        if self.attachment_handling { opts.attachments = true; opts.attachment_copy = true; }
        opts
    }

    pub fn metadata_processing_options(&self) -> crate::MetadataProcessingOptions {
        let mut opts = self.metadata_processing.clone();
        if let Some(m) = &self.metadata_options {
            if m.copy { opts.copy = true; }
            if m.edit { opts.edit = true; }
            if m.title.is_some() { opts.title = m.title.clone(); }
            if !m.metadata.is_empty() { opts.metadata = m.metadata.clone(); }
        }
        if self.metadata_copy { opts.copy = true; opts.metadata_copy = true; }
        if self.metadata_edit { opts.edit = true; opts.metadata_edit = true; }
        if self.metadata_editing { opts.metadata_editing = true; opts.edit = true; }
        if self.cover_art_handling { opts.cover_art = true; }
        if self.attachment_handling { opts.attachments = true; opts.attachment_copy = true; }
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
