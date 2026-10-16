//! Pre-flight job validation.
//!
//! The worker validates a job completely before the pipeline starts. A
//! partially specified job is the single most common way to turn a media
//! request into a hard failure deep inside a subprocess, so validation is
//! deliberately exhaustive and reports every problem it finds at once instead
//! of stopping at the first.

use std::fmt;

use vise_core::{
    AudioCodec, ContainerFormat, JobSpec, ScanResult, StreamInfo, SubtitleMode, TrackKind,
    VideoCodec, VideoProcessingOptions,
};

/// One validation problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// Dotted path of the offending field, e.g. `Video.Crop`.
    pub field: String,
    /// What is wrong and what would be acceptable.
    pub message: String,
}

impl ValidationIssue {
    /// Build an issue.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        ValidationIssue {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// A complete validation report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// Whether the job is safe to run.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    /// Single-line summary suitable for an error reply or a log line.
    pub fn summary(&self) -> String {
        self.issues
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            f.write_str("job is valid")
        } else {
            f.write_str(&self.summary())
        }
    }
}

/// Audio codecs each container can carry in this build.
fn container_audio_support(container: ContainerFormat) -> &'static [AudioCodec] {
    match container {
        ContainerFormat::Mp4 => &[
            AudioCodec::Aac,
            AudioCodec::Mp3,
            AudioCodec::Ac3,
            AudioCodec::Eac3,
        ],
        ContainerFormat::Mkv => &[
            AudioCodec::Aac,
            AudioCodec::Opus,
            AudioCodec::Vorbis,
            AudioCodec::Flac,
            AudioCodec::Mp3,
            AudioCodec::Ac3,
            AudioCodec::Eac3,
        ],
        ContainerFormat::Webm => &[AudioCodec::Opus, AudioCodec::Vorbis],
    }
}

/// Video codecs each container can carry in this build.
fn container_video_support(container: ContainerFormat) -> &'static [VideoCodec] {
    match container {
        ContainerFormat::Mp4 | ContainerFormat::Mkv => &[
            VideoCodec::H264,
            VideoCodec::H265,
            VideoCodec::Av1,
            VideoCodec::Vp9,
        ],
        ContainerFormat::Webm => &[VideoCodec::Vp9],
    }
}

/// Canonical file extension for a container.
fn container_extension(container: ContainerFormat) -> &'static str {
    match container {
        ContainerFormat::Mp4 => "mp4",
        ContainerFormat::Mkv => "mkv",
        ContainerFormat::Webm => "webm",
    }
}

/// Known extensions mapped to the container they imply.
fn extension_container(extension: &str) -> Option<ContainerFormat> {
    match extension {
        "mp4" | "m4v" | "m4a" => Some(ContainerFormat::Mp4),
        "mkv" => Some(ContainerFormat::Mkv),
        "webm" => Some(ContainerFormat::Webm),
        _ => None,
    }
}

/// Validate a job. When `scan` is supplied, checks that also use source facts
/// (geometry, available tracks, duration) are performed.
pub fn validate_job(spec: &JobSpec, scan: Option<&ScanResult>) -> ValidationReport {
    let mut issues = Vec::new();

    validate_paths(spec, &mut issues);
    validate_containers(spec, &mut issues);
    validate_rate_control(spec, scan, &mut issues);
    validate_video_filters(spec, scan, &mut issues);
    validate_audio(spec, scan, &mut issues);
    validate_subtitles(spec, scan, &mut issues);

    ValidationReport { issues }
}

/// Convenience wrapper for callers that only need the outcome.
pub fn ensure_valid(spec: &JobSpec, scan: Option<&ScanResult>) -> Result<(), ValidationReport> {
    let report = validate_job(spec, scan);
    if report.is_valid() {
        Ok(())
    } else {
        Err(report)
    }
}

fn validate_paths(spec: &JobSpec, issues: &mut Vec<ValidationIssue>) {
    if spec.input_path.trim().is_empty() {
        issues.push(ValidationIssue::new(
            "InputPath",
            "an input source must be supplied",
        ));
    } else {
        let input = std::path::Path::new(&spec.input_path);
        if !input.exists() {
            issues.push(ValidationIssue::new(
                "InputPath",
                format!("input source does not exist: {}", input.display()),
            ));
        } else if input.is_dir() {
            issues.push(ValidationIssue::new(
                "InputPath",
                format!(
                    "input source is a directory; scan it and submit a single title: {}",
                    input.display()
                ),
            ));
        }
    }

    if spec.output_path.trim().is_empty() {
        issues.push(ValidationIssue::new(
            "OutputPath",
            "an output destination must be supplied",
        ));
    } else {
        let output = std::path::Path::new(&spec.output_path);
        if spec.output_path.trim() == spec.input_path.trim() {
            issues.push(ValidationIssue::new(
                "OutputPath",
                "the output destination must differ from the input source",
            ));
        }
        match output
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
        {
            None => issues.push(ValidationIssue::new(
                "OutputPath",
                "the output destination needs a container file extension",
            )),
            Some(extension) => match extension_container(&extension) {
                None => issues.push(ValidationIssue::new(
                    "OutputPath",
                    format!("unknown output extension '{extension}'; use mp4, mkv, or webm"),
                )),
                Some(container) if container != spec.container => {
                    issues.push(ValidationIssue::new(
                        "OutputPath",
                        format!(
                        "output extension '{extension}' does not match the {} container; use '.{}'",
                        container_name(spec.container),
                        container_extension(spec.container)
                    ),
                    ))
                }
                Some(_) => {}
            },
        }
    }
}

fn validate_containers(spec: &JobSpec, issues: &mut Vec<ValidationIssue>) {
    let container = spec.container;
    let audio = spec.audio_codec;
    let video = spec.video_codec;
    let audio_opts = spec.audio_processing_options();
    let passthrough = audio_opts.passthrough || audio_opts.copy;

    if !passthrough && !container_audio_support(container).contains(&audio) {
        issues.push(ValidationIssue::new(
            "AudioCodec",
            format!(
                "{} audio cannot be stored in the {} container; use one of: {}",
                audio_codec_name(audio),
                container_name(container),
                container_audio_support(container)
                    .iter()
                    .map(|c| audio_codec_name(*c))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if !container_video_support(container).contains(&video) {
        issues.push(ValidationIssue::new(
            "VideoCodec",
            format!(
                "{} video cannot be stored in the {} container; use one of: {}",
                video_codec_name(video),
                container_name(container),
                container_video_support(container)
                    .iter()
                    .map(|c| video_codec_name(*c))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if spec.resolved_encoding_mode().is_lossless() && container != ContainerFormat::Mkv {
        issues.push(ValidationIssue::new(
            "Container",
            format!(
                "lossless encoding is only supported in the {} container in this build",
                container_name(ContainerFormat::Mkv)
            ),
        ));
    }
    if spec.lossless_enabled()
        && !passthrough
        && audio == AudioCodec::Vorbis
        && container == ContainerFormat::Mp4
    {
        issues.push(ValidationIssue::new(
            "AudioCodec",
            "lossless Vorbis audio requires the Matroska container",
        ));
    }
}

fn validate_rate_control(
    spec: &JobSpec,
    scan: Option<&ScanResult>,
    issues: &mut Vec<ValidationIssue>,
) {
    let mode = spec.resolved_encoding_mode();
    if mode.is_constant_quality() {
        // Constant quality is the default mode, so an absent quality means
        // "use the encoder default" and is a complete request. An out-of-range
        // value, however, is rejected rather than clamped.
        if let Some(q) = spec.effective_quality() {
            if q > 51 {
                issues.push(ValidationIssue::new(
                    "Quality",
                    format!("quality {q} is outside the supported range 0-51"),
                ));
            }
        }
    }
    if mode.is_average_bitrate() {
        match spec.effective_bitrate_kbps() {
            None => issues.push(ValidationIssue::new(
                "Bitrate",
                "average-bitrate encoding requires a bitrate in kbit/s",
            )),
            Some(0) => issues.push(ValidationIssue::new(
                "Bitrate",
                "average-bitrate encoding requires a bitrate greater than zero",
            )),
            Some(_) => {}
        }
    }
    if mode.is_target_size() {
        match spec.effective_target_size_bytes() {
            None => issues.push(ValidationIssue::new(
                "TargetSize",
                "target-size encoding requires a size in bytes",
            )),
            Some(0) => issues.push(ValidationIssue::new(
                "TargetSize",
                "target-size encoding requires a size greater than zero",
            )),
            Some(_) => {}
        }
        match known_duration(scan) {
            None => issues.push(ValidationIssue::new(
                "TargetSize",
                "target-size encoding needs a known source duration; scan the source first",
            )),
            Some(d) if d <= 0.0 => issues.push(ValidationIssue::new(
                "TargetSize",
                "target-size encoding needs a positive source duration",
            )),
            Some(_) => {}
        }
    }
}

fn known_duration(scan: Option<&ScanResult>) -> Option<f64> {
    let scan = scan?;
    scan.format
        .duration_seconds
        .or_else(|| scan.titles.iter().find_map(|t| t.duration_seconds))
}

fn validate_video_filters(
    spec: &JobSpec,
    scan: Option<&ScanResult>,
    issues: &mut Vec<ValidationIssue>,
) {
    let options = spec.video_processing_options();

    if let Some((x, y, w, h)) = options.crop {
        if w == 0 || h == 0 {
            issues.push(ValidationIssue::new(
                "Video.Crop",
                "crop width and height must be greater than zero",
            ));
        }
        if let Some((source_w, source_h)) = source_geometry(scan) {
            if w > source_w || h > source_h {
                issues.push(ValidationIssue::new(
                    "Video.Crop",
                    format!(
                        "crop {w}x{h} is larger than the source geometry {source_w}x{source_h}"
                    ),
                ));
            }
            if x.saturating_add(w) > source_w || y.saturating_add(h) > source_h {
                issues.push(ValidationIssue::new(
                    "Video.Crop",
                    format!(
                        "crop {x},{y} {w}x{h} extends past the source geometry {source_w}x{source_h}"
                    ),
                ));
            }
        }
    }

    for (field, value) in [
        ("Video.Scale", options.scale.or(options.scale_to)),
        ("Video.Pad", options.pad.or(options.pad_to)),
    ] {
        if let Some((w, h)) = value {
            if w == 0 || h == 0 {
                issues.push(ValidationIssue::new(
                    field,
                    "width and height must be greater than zero",
                ));
            }
        }
    }

    if let Some(fps) = options.constant_frame_rate {
        if fps <= 0.0 || fps > 1000.0 {
            issues.push(ValidationIssue::new(
                "Video.ConstantFrameRate",
                format!("frame rate {fps} must be between 0 and 1000"),
            ));
        }
    }
    if let Some(vise_core::FrameRateMode::Constant(fps)) = options.frame_rate {
        if fps <= 0.0 || fps > 1000.0 {
            issues.push(ValidationIssue::new(
                "Video.FrameRate",
                format!("frame rate {fps} must be between 0 and 1000"),
            ));
        }
    }
    if options.ten_bit && spec.container == ContainerFormat::Webm {
        issues.push(ValidationIssue::new(
            "Video.TenBit",
            "10-bit output is not supported in the WebM container",
        ));
    }
    if options.hdr && spec.container == ContainerFormat::Webm {
        issues.push(ValidationIssue::new(
            "Video.Hdr",
            "HDR metadata output is not supported in the WebM container",
        ));
    }
}

fn source_geometry(scan: Option<&ScanResult>) -> Option<(u32, u32)> {
    let scan = scan?;
    scan.streams
        .iter()
        .find(|s| s.kind == TrackKind::Video && s.width.is_some())
        .map(|s| (s.width.unwrap_or(0), s.height.unwrap_or(0)))
}

fn validate_audio(spec: &JobSpec, scan: Option<&ScanResult>, issues: &mut Vec<ValidationIssue>) {
    let options = spec.audio_processing_options();
    let passthrough = options.passthrough || options.copy;

    if let Some(channels) = options.channels {
        if channels == 0 || channels > 8 {
            issues.push(ValidationIssue::new(
                "Audio.Channels",
                format!("channel count {channels} must be between 1 and 8"),
            ));
        } else if let Some(source_channels) = source_audio_channels(scan) {
            if passthrough && channels != source_channels {
                issues.push(ValidationIssue::new(
                    "Audio.Channels",
                    format!(
                        "audio passthrough cannot change the layout from {source_channels} to {channels} channels"
                    ),
                ));
            }
        }
    }

    if let (Some(channels), Some(mixdown)) = (options.channels, options.mixdown_enum) {
        let expected = mixdown_channels(mixdown);
        if expected != 0 && expected != channels {
            issues.push(ValidationIssue::new(
                "Audio.Mixdown",
                format!(
                    "mixdown '{}' implies {expected} channels but {channels} were requested",
                    mixdown.as_str()
                ),
            ));
        }
    }

    if let Some(rate) = options.sample_rate {
        if !matches!(
            rate,
            8000 | 11025 | 16000 | 22050 | 24000 | 32000 | 44100 | 48000 | 96000 | 192_000
        ) {
            issues.push(ValidationIssue::new(
                "Audio.SampleRate",
                format!("sample rate {rate} is not a supported output rate"),
            ));
        }
    }

    if passthrough {
        let needs_filter = options.mixdown.is_some()
            || options.mixdown_enum.is_some()
            || options.dither
            || options.gain.is_some()
            || options.volume.is_some()
            || options.sample_rate.is_some();
        if needs_filter {
            issues.push(ValidationIssue::new(
                "Audio",
                "audio passthrough cannot be combined with gain, dither, resampling, or mixdown",
            ));
        }
    }

    if passthrough && spec.audio_codec == AudioCodec::Flac {
        issues.push(ValidationIssue::new(
            "AudioCodec",
            "lossless FLAC cannot be produced by passthrough; disable passthrough",
        ));
    }
}

fn source_audio_channels(scan: Option<&ScanResult>) -> Option<u32> {
    scan?
        .streams
        .iter()
        .find(|s| s.kind == TrackKind::Audio)
        .and_then(|s: &StreamInfo| s.channels)
}

fn mixdown_channels(mixdown: vise_core::AudioMixdown) -> u32 {
    match mixdown {
        vise_core::AudioMixdown::Auto => 0,
        vise_core::AudioMixdown::Mono => 1,
        vise_core::AudioMixdown::Stereo | vise_core::AudioMixdown::Dolby => 2,
        vise_core::AudioMixdown::Surround51 => 6,
        vise_core::AudioMixdown::Surround71 => 8,
    }
}

fn validate_subtitles(
    spec: &JobSpec,
    scan: Option<&ScanResult>,
    issues: &mut Vec<ValidationIssue>,
) {
    let options = spec.subtitle_processing_options();
    let burn_in = options.effective_burn_in() || spec.subtitle_mode == SubtitleMode::BurnIn;

    if burn_in {
        if spec.container == ContainerFormat::Webm {
            issues.push(ValidationIssue::new(
                "Subtitle",
                "subtitle burn-in is not supported in the WebM container",
            ));
        }
        let has_external = options
            .effective_path()
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false)
            || spec
                .subtitle_path
                .as_ref()
                .map(|p| !p.trim().is_empty())
                .unwrap_or(false);
        if let Some(scan) = scan {
            let has_embedded = scan.streams.iter().any(|s| s.kind == TrackKind::Subtitle);
            if !has_external && !has_embedded {
                issues.push(ValidationIssue::new(
                    "Subtitle",
                    "subtitle burn-in requested but the source has no subtitle tracks and no subtitle file was supplied",
                ));
            }
        } else if !has_external {
            // Without scan facts the worker cannot confirm a subtitle source
            // exists; requiring the explicit path keeps the behaviour honest.
            issues.push(ValidationIssue::new(
                "Subtitle",
                "subtitle burn-in requires either an embedded subtitle track or an explicit subtitle file",
            ));
        }
    }

    if spec.subtitle_mode == SubtitleMode::Passthrough
        && spec.container == ContainerFormat::Webm
        && options.srt_to_mp4
    {
        issues.push(ValidationIssue::new(
            "Subtitle",
            "SRT to MP4 text conversion targets MP4, not WebM",
        ));
    }
}

fn container_name(container: ContainerFormat) -> &'static str {
    match container {
        ContainerFormat::Mp4 => "MP4",
        ContainerFormat::Mkv => "Matroska",
        ContainerFormat::Webm => "WebM",
    }
}

fn audio_codec_name(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Aac => "AAC",
        AudioCodec::Opus => "Opus",
        AudioCodec::Vorbis => "Vorbis",
        AudioCodec::Flac => "FLAC",
        AudioCodec::Mp3 => "MP3",
        AudioCodec::Ac3 => "AC3",
        AudioCodec::Eac3 => "E-AC3",
    }
}

fn video_codec_name(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "H.264",
        VideoCodec::H265 => "H.265",
        VideoCodec::Av1 => "AV1",
        VideoCodec::Vp9 => "VP9",
    }
}

/// Build a synthetic scan result, useful for validation tests and for callers
/// that already know the source facts.
pub fn synthetic_scan(
    duration_seconds: f64,
    width: u32,
    height: u32,
    audio: Option<u32>,
) -> ScanResult {
    let mut streams = vec![StreamInfo {
        index: 0,
        kind: TrackKind::Video,
        width: Some(width),
        height: Some(height),
        ..Default::default()
    }];
    if let Some(channels) = audio {
        streams.push(StreamInfo {
            index: 1,
            kind: TrackKind::Audio,
            channels: Some(channels),
            ..Default::default()
        });
    }
    let stream_indices: Vec<usize> = (0..streams.len()).collect();
    ScanResult {
        format: vise_core::FormatInfo {
            duration_seconds: Some(duration_seconds),
            ..Default::default()
        },
        streams,
        titles: vec![vise_core::TitleInfo {
            index: 0,
            duration_seconds: Some(duration_seconds),
            stream_indices,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Convenience for tests: a job with only paths filled in.
pub fn minimal_job(input: &str, output: &str) -> JobSpec {
    JobSpec {
        input_path: input.to_string(),
        output_path: output.to_string(),
        video_processing: VideoProcessingOptions::default(),
        ..JobSpec::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::AudioProcessingOptions;

    fn spec(input: &str, output: &str) -> JobSpec {
        minimal_job(input, output)
    }

    #[test]
    fn reports_missing_paths() {
        let report = validate_job(&JobSpec::default(), None);
        assert!(!report.is_valid());
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"InputPath"));
        assert!(fields.contains(&"OutputPath"));
        assert!(report.summary().contains("InputPath"));
    }

    #[test]
    fn reports_missing_input_file_and_bad_extension() {
        let report = validate_job(&spec("no-such-file.mkv", "out.txt"), None);
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"InputPath"));
        assert!(fields.contains(&"OutputPath"));
    }

    #[test]
    fn reports_container_extension_mismatch() {
        let mut job = spec("in.mkv", "out.mp4");
        job.container = ContainerFormat::Mkv;
        let report = validate_job(&job, None);
        assert!(report.summary().contains("does not match"));
    }

    #[test]
    fn reports_container_codec_conflicts() {
        let mut job = spec("in.webm", "out.webm");
        job.container = ContainerFormat::Webm;
        job.video_codec = VideoCodec::H264;
        job.audio_codec = AudioCodec::Aac;
        let report = validate_job(&job, None);
        let summary = report.summary();
        assert!(summary.contains("H.264"));
        assert!(summary.contains("AAC"));
    }

    #[test]
    fn reports_lossless_container_constraint() {
        let mut job = spec("in.mp4", "out.mp4");
        job.lossless = true;
        let report = validate_job(&job, None);
        assert!(report.summary().contains("lossless"));
    }

    #[test]
    fn reports_rate_control_problems() {
        let mut job = spec("in.mp4", "out.mp4");
        job.quality = Some(99);
        assert!(validate_job(&job, None)
            .summary()
            .contains("outside the supported range"));

        // Constant quality is the default mode: an absent quality is valid.
        let job = spec("in.mp4", "out.mp4");
        assert!(!validate_job(&job, None).summary().contains("quality"));

        let mut job = spec("in.mp4", "out.mp4");
        job.encoding_mode = Some(vise_core::EncodingMode::AverageBitrate);
        assert!(validate_job(&job, None).summary().contains("bitrate"));

        let mut job = spec("in.mp4", "out.mp4");
        job.encoding_mode = Some(vise_core::EncodingMode::TargetSize);
        job.target_size = Some(1024);
        let report = validate_job(&job, None);
        assert!(report.summary().contains("known source duration"));
        let scan = synthetic_scan(10.0, 640, 480, Some(2));
        assert!(!validate_job(&job, Some(&scan))
            .summary()
            .contains("known source duration"));
    }

    #[test]
    fn reports_crop_problems_against_source_geometry() {
        let scan = synthetic_scan(10.0, 640, 480, Some(2));
        let mut job = spec("in.mp4", "out.mp4");
        job.crop = Some((600, 400, 320, 240));
        let report = validate_job(&job, Some(&scan));
        assert!(report.summary().contains("extends past"));

        job.crop = Some((0, 0, 0, 100));
        assert!(validate_job(&job, None)
            .summary()
            .contains("greater than zero"));
    }

    #[test]
    fn reports_audio_layout_conflicts() {
        let scan = synthetic_scan(10.0, 640, 480, Some(2));
        let mut job = spec("in.mp4", "out.mp4");
        job.audio_processing = AudioProcessingOptions {
            channels: Some(6),
            passthrough: true,
            ..Default::default()
        };
        assert!(validate_job(&job, Some(&scan))
            .summary()
            .contains("cannot change the layout"));

        let mut job = spec("in.mkv", "out.mkv");
        job.container = ContainerFormat::Mkv;
        job.audio_processing = AudioProcessingOptions {
            channels: Some(2),
            mixdown_enum: Some(vise_core::AudioMixdown::Surround51),
            ..Default::default()
        };
        assert!(validate_job(&job, None)
            .summary()
            .contains("implies 6 channels"));

        let mut job = spec("in.mkv", "out.mkv");
        job.container = ContainerFormat::Mkv;
        job.audio_processing = AudioProcessingOptions {
            channels: Some(12),
            ..Default::default()
        };
        assert!(validate_job(&job, None)
            .summary()
            .contains("between 1 and 8"));
    }

    #[test]
    fn reports_passthrough_conflicts() {
        let mut job = spec("in.mkv", "out.mkv");
        job.container = ContainerFormat::Mkv;
        job.audio_processing = AudioProcessingOptions {
            passthrough: true,
            gain: Some(2.0),
            ..Default::default()
        };
        assert!(validate_job(&job, None).summary().contains("passthrough"));
    }

    #[test]
    fn reports_subtitle_burn_in_without_a_source() {
        let mut job = spec("in.mp4", "out.mp4");
        job.subtitle_mode = SubtitleMode::BurnIn;
        assert!(validate_job(&job, None).summary().contains("burn-in"));

        let scan = synthetic_scan(10.0, 640, 480, Some(2));
        assert!(validate_job(&job, Some(&scan))
            .summary()
            .contains("no subtitle"));
    }

    #[test]
    fn accepts_a_well_formed_job() {
        let dir = std::env::temp_dir().join("vise-ipc-validation");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("input.mp4");
        let output = dir.join("output.mp4");
        std::fs::write(&input, b"not real media").expect("write input");

        let scan = synthetic_scan(10.0, 640, 480, Some(2));
        let mut job = spec(input.to_str().unwrap(), output.to_str().unwrap());
        job.quality = Some(22);
        let report = validate_job(&job, Some(&scan));
        assert!(report.is_valid(), "unexpected issues: {}", report.summary());
        assert!(ensure_valid(&job, Some(&scan)).is_ok());
    }
}
