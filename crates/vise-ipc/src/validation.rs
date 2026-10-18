//! Pre-flight job validation.
//!
//! A partially specified job is the single most common way to turn a media
//! request into a hard failure deep inside a subprocess, so validation is
//! deliberately exhaustive and reports every problem it finds at once instead
//! of stopping at the first.
//!
//! The rules are a **pure function** of the job and the machine facts handed to
//! them. Nothing in this module touches the filesystem, runs a subprocess, or
//! asks what the machine can do: [`probe_machine_facts`] does that part, once,
//! and its answer is injected. That split is what lets the window validate a
//! job before it is queued — with facts it already has — and lets the worker
//! validate the same job again as a defence in depth, with both sides running
//! the identical rule set and therefore reaching the identical answer.
//!
//! A rule that needs a fact nobody supplied is skipped rather than guessed. An
//! absent answer is not evidence of a problem, and a validator that invented
//! one would refuse jobs it cannot actually judge.

use std::fmt;
use std::path::Path;

use vise_core::{
    AudioCodec, AudioMixdown, ContainerFormat, HardwareCapabilities, JobSpec, ScanResult,
    StreamInfo, SubtitleMode, TrackKind, VideoCodec, VideoProcessingOptions,
};

/// One validation problem.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
///
/// Every problem is in here, not the first one, so the user can fix the whole
/// job in one pass instead of discovering the next problem after each retry.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// Whether the job is safe to run.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    /// Every problem on its own line, as a window reports a refusal.
    ///
    /// The field travels with the message so the page can put each problem next
    /// to the control it belongs to rather than in one paragraph.
    pub fn lines(&self) -> Vec<String> {
        self.issues.iter().map(|issue| issue.to_string()).collect()
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

/// What is known about one path.
///
/// These are answers, not conclusions: a path that was never probed has no
/// `PathFacts` at all, so the rules that would use them are skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathFacts {
    /// Something is there.
    pub exists: bool,
    /// What is there is a folder rather than a file.
    pub is_dir: bool,
    /// The file can be opened for reading.
    pub readable: bool,
    /// The destination can be written: the file itself when it is already
    /// there, or the folder it will be created in when it is not.
    pub writable: bool,
}

impl PathFacts {
    /// A file that is there and can be read and written.
    pub fn readable_file() -> Self {
        PathFacts {
            exists: true,
            is_dir: false,
            readable: true,
            writable: true,
        }
    }

    /// A file that is there and can be read but not written.
    pub fn read_only_file() -> Self {
        PathFacts {
            writable: false,
            ..PathFacts::readable_file()
        }
    }
}

/// The facts about the machine a job's validity depends on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MachineFacts {
    /// What is known about the source path.
    pub source: Option<PathFacts>,
    /// What is known about the destination path.
    pub destination: Option<PathFacts>,
    /// What the scan found, when the source has been scanned.
    pub scan: Option<ScanResult>,
}

impl MachineFacts {
    /// No facts at all: every check that needs one is skipped.
    pub fn unknown() -> Self {
        MachineFacts::default()
    }

    /// Facts with a scan result and nothing known about the paths.
    pub fn with_scan(scan: ScanResult) -> Self {
        MachineFacts {
            scan: Some(scan),
            ..MachineFacts::default()
        }
    }

    /// Attach a scan result to facts that already answer the path questions.
    pub fn and_scan(mut self, scan: ScanResult) -> Self {
        self.scan = Some(scan);
        self
    }

    /// The scan result, when one was injected.
    pub fn scan(&self) -> Option<&ScanResult> {
        self.scan.as_ref()
    }
}

/// Look the paths of `spec` up on this machine.
///
/// This is the only part of validation that touches the disk, and it is
/// deliberately outside the rules: the rules stay a pure function so they can
/// be exercised with facts a test chooses rather than files it has to create.
pub fn probe_machine_facts(spec: &JobSpec) -> MachineFacts {
    MachineFacts {
        source: probe_source(spec.input_path.trim()),
        destination: probe_destination(spec.output_path.trim()),
        scan: None,
    }
}

fn probe_source(path: &str) -> Option<PathFacts> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return None;
    }
    match std::fs::metadata(path) {
        Err(_) => Some(PathFacts::default()),
        Ok(meta) => {
            let is_dir = meta.is_dir();
            Some(PathFacts {
                exists: true,
                is_dir,
                // A folder cannot be opened as media even when opening it as a
                // path would succeed, so readability is only asked of a file.
                readable: !is_dir && std::fs::File::open(path).is_ok(),
                writable: false,
            })
        }
    }
}

fn probe_destination(path: &str) -> Option<PathFacts> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return None;
    }
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Some(PathFacts {
            exists: true,
            is_dir: true,
            readable: false,
            writable: false,
        }),
        Ok(_) => Some(PathFacts {
            exists: true,
            is_dir: false,
            readable: std::fs::File::open(path).is_ok(),
            writable: std::fs::OpenOptions::new().write(true).open(path).is_ok(),
        }),
        // The file is not there yet, which is the normal case. What decides
        // whether it can be written is the folder it will be created in; a
        // folder that is not there yet is created on the way, so it is not a
        // problem yet either.
        Err(_) => Some(PathFacts {
            exists: false,
            is_dir: false,
            readable: false,
            writable: match path.parent().filter(|p| !p.as_os_str().is_empty()) {
                Some(parent) => match std::fs::metadata(parent) {
                    Ok(meta) if meta.is_dir() => directory_accepts_files(parent),
                    Ok(_) => false,
                    Err(_) => true,
                },
                None => true,
            },
        }),
    }
}

/// Whether a new file can be created in `dir`.
fn directory_accepts_files(dir: &Path) -> bool {
    let probe = dir.join(".vise-write-probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
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
            AudioCodec::Pcm,
        ],
        ContainerFormat::Webm => &[AudioCodec::Opus, AudioCodec::Vorbis],
    }
}

/// Video codecs each container can carry in this build.
///
/// The professional codecs are Matroska-only here: they normally travel in a
/// quick-time or broadcast wrapper, and this build has no such container, so
/// they are offered where the muxer takes them rather than in a container that
/// cannot carry them at all.
fn container_video_support(container: ContainerFormat) -> &'static [VideoCodec] {
    match container {
        ContainerFormat::Mp4 => &[
            VideoCodec::H264,
            VideoCodec::H265,
            VideoCodec::Av1,
            VideoCodec::Vp9,
        ],
        ContainerFormat::Mkv => &[
            VideoCodec::H264,
            VideoCodec::H265,
            VideoCodec::Av1,
            VideoCodec::Vp9,
            VideoCodec::ProRes,
            VideoCodec::Dnxhr,
            VideoCodec::Ffv1,
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

/// The encoder whose filters a job's filter chain has to satisfy.
///
/// Resolved from the job alone: a named hardware encoder is what the job asked
/// for, and everything else is the software encoder this build reaches for. What
/// the machine would actually pick is a property of the machine, and asking the
/// machine here would make this a subprocess again.
fn filter_encoder(spec: &JobSpec) -> String {
    let hardware = spec.hardware_acceleration_options();
    if let Some(named) = hardware
        .encode
        .encoder
        .as_ref()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
    {
        return named.to_string();
    }
    // Everything else resolves to the software encoder: the pipeline reaches for
    // it on its own whenever no hardware encoder is usable, and a rule that
    // guessed a GPU encoder the machine may not have would refuse jobs that
    // would have run.
    HardwareCapabilities::default()
        .software_encoder(spec.video_codec)
        .to_string()
}

/// Whether an encoder codes every frame on its own.
///
/// This is what decides the filter rules below. Deblocking and telecine removal
/// both act on artefacts that only inter prediction leaves behind: a blocked
/// edge between two macroblocks in one picture, or a field-repeated cadence
/// across pictures. An intra-only codec produces neither, so on FFV1, ProRes,
/// and DNxHR those filters have nothing to work on — asking for them is not a
/// slower encode, it is a filter whose output is discarded.
fn encoder_is_intra_only(spec: &JobSpec) -> bool {
    if spec.video_codec.is_intra_only() {
        return true;
    }
    matches!(
        filter_encoder(spec).as_str(),
        "ffv1" | "prores_ks" | "dnxhd" | "prores" | "dnxhd_px"
    )
}

/// How many channels a mixdown target produces. Zero means "whatever the source
/// has", which is not a number to compare against.
fn mixdown_channels(mixdown: AudioMixdown) -> u32 {
    match mixdown {
        AudioMixdown::Auto => 0,
        AudioMixdown::Mono => 1,
        AudioMixdown::Stereo | AudioMixdown::Dolby => 2,
        AudioMixdown::Surround51 => 6,
        AudioMixdown::Surround71 => 8,
    }
}

/// The name a channel count is shown under.
fn layout_label(channels: u32) -> String {
    match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        other => format!("{other} channels"),
    }
}

/// The mixdown this job resolved to, as a channel count.
fn requested_mixdown(options: &vise_core::AudioProcessingOptions) -> u32 {
    if let Some(mixdown) = options.mixdown_enum {
        let known = mixdown_channels(mixdown);
        if known > 0 {
            return known;
        }
    }
    if let Some(named) = options.mixdown.as_ref() {
        return match named
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '_'], "")
            .as_str()
        {
            "mono" | "1.0" => 1,
            "stereo" | "2.0" | "dolby" => 2,
            "5.1" | "surround51" => 6,
            "7.1" | "surround71" => 8,
            _ => 0,
        };
    }
    0
}

/// Validate a job against the facts supplied about the machine.
///
/// Checks that need a fact nobody supplied are skipped, so a caller with no
/// facts still gets the rules that are about the job alone.
pub fn validate_job(spec: &JobSpec, facts: &MachineFacts) -> ValidationReport {
    let mut issues = Vec::new();

    validate_paths(spec, facts, &mut issues);
    validate_containers(spec, &mut issues);
    validate_rate_control(spec, facts, &mut issues);
    validate_video_filters(spec, facts, &mut issues);
    validate_audio(spec, facts, &mut issues);
    validate_subtitles(spec, facts, &mut issues);

    ValidationReport { issues }
}

/// Convenience wrapper for callers that only need the outcome.
pub fn ensure_valid(spec: &JobSpec, facts: &MachineFacts) -> Result<(), ValidationReport> {
    let report = validate_job(spec, facts);
    if report.is_valid() {
        Ok(())
    } else {
        Err(report)
    }
}

fn validate_paths(spec: &JobSpec, facts: &MachineFacts, issues: &mut Vec<ValidationIssue>) {
    if spec.input_path.trim().is_empty() {
        issues.push(ValidationIssue::new(
            "InputPath",
            "an input source must be supplied",
        ));
    } else {
        match facts.source {
            None => {}
            Some(source) => {
                if !source.exists {
                    issues.push(ValidationIssue::new(
                        "InputPath",
                        format!("input source does not exist: {}", spec.input_path.trim()),
                    ));
                } else if source.is_dir {
                    issues.push(ValidationIssue::new(
                        "InputPath",
                        format!(
                            "input source is a folder; scan it and submit a single title: {}",
                            spec.input_path.trim()
                        ),
                    ));
                } else if !source.readable {
                    issues.push(ValidationIssue::new(
                        "InputPath",
                        format!("input source cannot be read: {}", spec.input_path.trim()),
                    ));
                }
            }
        }
    }

    if spec.output_path.trim().is_empty() {
        issues.push(ValidationIssue::new(
            "OutputPath",
            "an output destination must be supplied",
        ));
        return;
    }

    if spec.output_path.trim() == spec.input_path.trim() {
        issues.push(ValidationIssue::new(
            "OutputPath",
            "the output destination must differ from the input source",
        ));
    }

    if let Some(destination) = facts.destination {
        if destination.is_dir {
            issues.push(ValidationIssue::new(
                "OutputPath",
                format!(
                    "the output destination is a folder, not a file: {}",
                    spec.output_path.trim()
                ),
            ));
        } else if !destination.writable {
            issues.push(ValidationIssue::new(
                "OutputPath",
                format!(
                    "the output destination cannot be written: {}",
                    spec.output_path.trim()
                ),
            ));
        }
    }

    match spec
        .output_path
        .trim()
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
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
            Some(container) if container != spec.container => issues.push(ValidationIssue::new(
                "OutputPath",
                format!(
                    "output extension '{extension}' does not match the {} container; use '.{}'",
                    container_name(spec.container),
                    container_extension(spec.container)
                ),
            )),
            Some(_) => {}
        },
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
                audio.label(),
                container_name(container),
                container_audio_support(container)
                    .iter()
                    .map(|c| c.label())
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
                video.label(),
                container_name(container),
                container_video_support(container)
                    .iter()
                    .map(|c| c.label())
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

/// Whether a mode has anything for a second pass to work on.
///
/// A two-pass encode exists to spend a first pass measuring the material so the
/// second pass can hit a rate or a size. A lossless write has no rate to hit and
/// an intra-only codec has no pass-based mode at all, so in both cases the first
/// pass produces a file nobody reads and the encoder runs once anyway.
fn two_pass_is_pointless(spec: &JobSpec) -> Option<String> {
    if spec.lossless_enabled() {
        return Some(
            "a lossless encode has nothing to gain from a second pass; switch two-pass off"
                .to_string(),
        );
    }
    if encoder_is_intra_only(spec) {
        return Some(format!(
            "{} codes every frame on its own and has no two-pass mode",
            spec.video_codec.label()
        ));
    }
    None
}

fn validate_rate_control(spec: &JobSpec, facts: &MachineFacts, issues: &mut Vec<ValidationIssue>) {
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
        match known_duration(facts.scan()) {
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

    // A lossless write takes no rate at all: the lossless encoders have no
    // quality or bitrate argument, so a value that came from a saved preset is
    // carried in the request and then refused by the encoder rather than
    // honoured. Refusing it here says so against the field that carries it.
    if spec.lossless_enabled() {
        if spec.effective_quality().is_some() {
            issues.push(ValidationIssue::new(
                "Quality",
                "a lossless encode has no quality value; clear the quality field",
            ));
        }
        if spec.effective_bitrate_kbps().is_some() {
            issues.push(ValidationIssue::new(
                "Bitrate",
                "a lossless encode has no bitrate; clear the bitrate field",
            ));
        }
    }

    if spec.two_pass_enabled() {
        if let Some(reason) = two_pass_is_pointless(spec) {
            issues.push(ValidationIssue::new("TwoPass", reason));
        }
    }
}

fn known_duration(scan: Option<&ScanResult>) -> Option<f64> {
    let scan = scan?;
    scan.format
        .duration_seconds
        .or_else(|| scan.titles.iter().find_map(|t| t.duration_seconds))
}

fn validate_video_filters(spec: &JobSpec, facts: &MachineFacts, issues: &mut Vec<ValidationIssue>) {
    let options = spec.video_processing_options();

    if let Some((x, y, w, h)) = options.crop {
        if w == 0 || h == 0 {
            issues.push(ValidationIssue::new(
                "Video.Crop",
                "crop width and height must be greater than zero",
            ));
        }
        if let Some((source_w, source_h)) = source_geometry(facts.scan()) {
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

    // Filters that act on artefacts left behind by inter prediction have
    // nothing to act on when the encoder is intra-only.
    let intra_only = encoder_is_intra_only(spec);
    if options.deblock && intra_only {
        issues.push(ValidationIssue::new(
            "Video.Deblock",
            format!(
                "{} codes every frame on its own, so it produces no blocking artefacts for deblocking to remove",
                spec.video_codec.label()
            ),
        ));
    }
    if options.detelecine && intra_only {
        issues.push(ValidationIssue::new(
            "Video.Detelecine",
            format!(
                "{} codes every frame on its own, so telecine cadence does not survive into the output",
                spec.video_codec.label()
            ),
        ));
    }
    // An absent block size is the filter's own default, which is a complete
    // request; a present one has to be inside the range the filter accepts, and
    // has to be wanted at all.
    match options.deblock_strength {
        Some(strength) if !(4..=31).contains(&strength) => issues.push(ValidationIssue::new(
            "Video.Deblock",
            format!("deblock block size {strength} is outside the supported range 4-31"),
        )),
        Some(_) if !options.deblock => issues.push(ValidationIssue::new(
            "Video.Deblock",
            "a deblock block size was set but deblocking is off, so the value is ignored",
        )),
        _ => {}
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

fn validate_audio(spec: &JobSpec, facts: &MachineFacts, issues: &mut Vec<ValidationIssue>) {
    let options = spec.audio_processing_options();
    let passthrough = options.passthrough || options.copy;
    let source_channels = source_audio_channels(facts.scan());

    if let Some(channels) = options.channels {
        if channels == 0 || channels > 8 {
            issues.push(ValidationIssue::new(
                "Audio.Channels",
                format!("channel count {channels} must be between 1 and 8"),
            ));
        } else if let Some(source_channels) = source_channels {
            if passthrough && channels != source_channels {
                issues.push(ValidationIssue::new(
                    "Audio.Channels",
                    format!(
                        "audio passthrough cannot change the layout from {source_channels} to {channels} channels"
                    ),
                ));
            } else if !passthrough && channels > source_channels {
                // Widening would have to invent speakers the recording does not
                // contain, so what comes out would not be the layout that was
                // asked for.
                issues.push(ValidationIssue::new(
                    "Audio.Channels",
                    format!(
                        "the source has {} channel(s), so {} output would have to invent speakers that are not in the recording",
                        source_channels,
                        layout_label(channels)
                    ),
                ));
            }
        }
    }

    let mixdown = requested_mixdown(&options);
    if mixdown > 0 {
        if let Some(channels) = options.channels {
            if channels != mixdown {
                issues.push(ValidationIssue::new(
                    "Audio.Mixdown",
                    format!(
                        "mixdown to {} implies {mixdown} channels but {channels} were requested",
                        layout_label(mixdown)
                    ),
                ));
            }
        }
        if let Some(source_channels) = source_channels {
            if mixdown > source_channels {
                issues.push(ValidationIssue::new(
                    "Audio.Mixdown",
                    format!(
                        "the source has {} channel(s), so a {} mixdown cannot be made from it",
                        source_channels,
                        layout_label(mixdown)
                    ),
                ));
            } else if mixdown == source_channels {
                // Mixing to the layout the track already has is not a mixdown:
                // the filter would be a copy of the input, and offering it as a
                // choice suggests it did something.
                issues.push(ValidationIssue::new(
                    "Audio.Mixdown",
                    format!(
                        "the source is already {}; there is nothing to mix down to that layout",
                        layout_label(mixdown)
                    ),
                ));
            }
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
        let needs_filter = mixdown > 0
            || options.mixdown.is_some()
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

fn validate_subtitles(spec: &JobSpec, facts: &MachineFacts, issues: &mut Vec<ValidationIssue>) {
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
        match facts.scan() {
            Some(scan) => {
                let has_embedded = scan.streams.iter().any(|s| s.kind == TrackKind::Subtitle);
                if !has_external && !has_embedded {
                    issues.push(ValidationIssue::new(
                        "Subtitle",
                        "subtitle burn-in requested but the source has no subtitle tracks and no subtitle file was supplied",
                    ));
                }
            }
            // Without scan facts the worker cannot confirm a subtitle source
            // exists; requiring the explicit path keeps the behaviour honest.
            None if !has_external => issues.push(ValidationIssue::new(
                "Subtitle",
                "subtitle burn-in requires either an embedded subtitle track or an explicit subtitle file",
            )),
            None => {}
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

    /// A Matroska job, for the rules that are about the container.
    fn mkv(input: &str, output: &str) -> JobSpec {
        JobSpec {
            container: ContainerFormat::Mkv,
            ..minimal_job(input, output)
        }
    }

    /// Facts for a source and a destination that are both fine, so a test can
    /// state the one thing it is about.
    fn usable() -> MachineFacts {
        MachineFacts {
            source: Some(PathFacts::readable_file()),
            destination: Some(PathFacts {
                exists: false,
                is_dir: false,
                readable: false,
                writable: true,
            }),
            scan: None,
        }
    }

    fn with_scan(channels: Option<u32>) -> MachineFacts {
        usable().and_scan(synthetic_scan(10.0, 640, 480, channels))
    }

    /// Assert a job is refused, and that the refusal names `field`.
    fn refused_for(job: &JobSpec, facts: &MachineFacts, field: &str) -> String {
        let report = validate_job(job, facts);
        assert!(
            report.issues.iter().any(|issue| issue.field == field),
            "expected a problem on {field}, got: {}",
            report.summary()
        );
        assert!(!report.is_valid());
        report.summary()
    }

    /// Assert a job is accepted.
    fn accepted(job: &JobSpec, facts: &MachineFacts) {
        let report = validate_job(job, facts);
        assert!(
            report.is_valid(),
            "unexpected problems: {}",
            report.summary()
        );
        assert!(ensure_valid(job, facts).is_ok());
    }

    #[test]
    fn reports_missing_paths() {
        let report = validate_job(&JobSpec::default(), &MachineFacts::unknown());
        assert!(!report.is_valid());
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"InputPath"));
        assert!(fields.contains(&"OutputPath"));
        assert!(report.summary().contains("InputPath"));
        assert_eq!(report.lines().len(), report.issues.len());
    }

    #[test]
    fn reports_every_problem_at_once() {
        // Six separate mistakes, all reported: a validator that stopped at the first
        // would cost the user six round trips instead of one.
        let mut job = JobSpec {
            input_path: "in.mkv".to_string(),
            container: ContainerFormat::Webm,
            video_codec: VideoCodec::H264,
            audio_codec: AudioCodec::Pcm,
            quality: Some(99),
            ..JobSpec::default()
        };
        job.audio_processing.channels = Some(6);
        let facts = MachineFacts {
            source: Some(PathFacts::default()),
            destination: Some(PathFacts {
                writable: false,
                ..PathFacts::default()
            }),
            scan: Some(synthetic_scan(10.0, 640, 480, Some(2))),
        };
        let report = validate_job(&job, &facts);
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        for expected in [
            "InputPath",
            "OutputPath",
            "VideoCodec",
            "AudioCodec",
            "Audio.Channels",
        ] {
            assert!(
                fields.contains(&expected),
                "{expected} missing from {fields:?}"
            );
        }
        assert!(
            report.issues.len() >= 6,
            "expected the whole list, got {}",
            report.summary()
        );
    }

    #[test]
    fn reports_a_missing_input_file_and_a_bad_extension() {
        let facts = MachineFacts {
            source: Some(PathFacts::default()),
            ..usable()
        };
        let report = validate_job(&spec("no-such-file.mkv", "out.txt"), &facts);
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"InputPath"));
        assert!(fields.contains(&"OutputPath"));
    }

    #[test]
    fn a_source_that_is_a_folder_is_not_a_source() {
        let facts = MachineFacts {
            source: Some(PathFacts {
                exists: true,
                is_dir: true,
                ..PathFacts::default()
            }),
            ..usable()
        };
        refused_for(&spec("C:\\media\\folder", "out.mp4"), &facts, "InputPath");
        // The same path read as a file is fine.
        let file = MachineFacts {
            source: Some(PathFacts::readable_file()),
            ..usable()
        };
        accepted(&spec("C:\\media\\movie.mp4", "out.mp4"), &file);
    }

    #[test]
    fn a_source_that_cannot_be_read_is_refused() {
        let facts = MachineFacts {
            source: Some(PathFacts {
                exists: true,
                is_dir: false,
                readable: false,
                writable: false,
            }),
            ..usable()
        };
        let summary = refused_for(&spec("locked.mp4", "out.mp4"), &facts, "InputPath");
        assert!(summary.contains("cannot be read"), "{summary}");
        accepted(&spec("locked.mp4", "out.mp4"), &usable());
    }

    #[test]
    fn a_destination_that_cannot_be_written_is_refused() {
        let facts = MachineFacts {
            destination: Some(PathFacts {
                exists: false,
                is_dir: false,
                readable: false,
                writable: false,
            }),
            ..usable()
        };
        let summary = refused_for(&spec("in.mp4", "C:\\out\\movie.mp4"), &facts, "OutputPath");
        assert!(summary.contains("cannot be written"), "{summary}");
        accepted(&spec("in.mp4", "C:\\out\\movie.mp4"), &usable());
    }

    #[test]
    fn a_destination_that_is_a_folder_is_refused() {
        let facts = MachineFacts {
            destination: Some(PathFacts {
                exists: true,
                is_dir: true,
                ..PathFacts::default()
            }),
            ..usable()
        };
        let summary = refused_for(&spec("in.mp4", "C:\\out\\movie.mp4"), &facts, "OutputPath");
        assert!(summary.contains("folder"), "{summary}");
    }

    #[test]
    fn the_probe_reports_what_is_really_on_disk() {
        let dir = std::env::temp_dir().join("vise-ipc-probe-facts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let source = dir.join("in.mkv");
        std::fs::write(&source, b"not real media").expect("write");
        let destination = dir.join("out.mkv");

        let facts = probe_machine_facts(&mkv(
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ));
        let probed_source = facts.source.expect("source probed");
        assert!(probed_source.exists && probed_source.readable && !probed_source.is_dir);
        let probed_destination = facts.destination.expect("destination probed");
        assert!(!probed_destination.exists);
        assert!(
            probed_destination.writable,
            "a new file beside one that exists can be written"
        );

        // A file that is not there, in a folder that is not either, is not a
        // problem yet: the folder is created on the way to the file.
        let nested = dir.join("made-on-demand").join("out.mkv");
        let facts = probe_machine_facts(&mkv(source.to_str().unwrap(), nested.to_str().unwrap()));
        assert!(facts.destination.expect("destination probed").writable);

        // A destination that is really a folder is refused rather than offered.
        let facts = probe_machine_facts(&mkv(source.to_str().unwrap(), dir.to_str().unwrap()));
        assert!(facts.destination.expect("probed").is_dir);
        let summary = refused_for(
            &mkv(
                source.to_str().unwrap(),
                dir.join("out.mkv").to_str().unwrap(),
            ),
            &facts,
            "OutputPath",
        );
        assert!(summary.contains("folder"), "{summary}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_container_extension_mismatch() {
        let summary = refused_for(&mkv("in.mkv", "out.mp4"), &usable(), "OutputPath");
        assert!(summary.contains("does not match"), "{summary}");
    }

    #[test]
    fn reports_container_codec_conflicts() {
        let mut job = spec("in.webm", "out.webm");
        job.container = ContainerFormat::Webm;
        job.video_codec = VideoCodec::H264;
        job.audio_codec = AudioCodec::Aac;
        let summary = refused_for(&job, &usable(), "VideoCodec");
        assert!(summary.contains("H.264"), "{summary}");
        let summary = refused_for(&job, &usable(), "AudioCodec");
        assert!(summary.contains("AAC"), "{summary}");
    }

    #[test]
    fn a_container_that_can_carry_the_chosen_codecs_is_accepted() {
        let mut job = mkv("in.mkv", "out.mkv");
        job.video_codec = VideoCodec::H265;
        job.audio_codec = AudioCodec::Opus;
        accepted(&job, &usable());
    }

    #[test]
    fn reports_lossless_container_constraint() {
        let mut job = spec("in.mp4", "out.mp4");
        job.lossless = true;
        let summary = refused_for(&job, &usable(), "Container");
        assert!(summary.contains("lossless"), "{summary}");
    }

    #[test]
    fn a_lossless_job_that_carries_no_rate_is_accepted() {
        let mut job = mkv("in.mkv", "out.mkv");
        job.video_codec = VideoCodec::Ffv1;
        job.audio_codec = AudioCodec::Flac;
        job.lossless = true;
        accepted(&job, &usable());
    }

    #[test]
    fn reports_rate_control_problems() {
        let mut job = spec("in.mp4", "out.mp4");
        job.quality = Some(99);
        assert!(validate_job(&job, &usable())
            .summary()
            .contains("outside the supported range"));

        // Constant quality is the default mode: an absent quality is valid.
        let job = spec("in.mp4", "out.mp4");
        accepted(&job, &usable());

        let mut job = spec("in.mp4", "out.mp4");
        job.encoding_mode = Some(vise_core::EncodingMode::AverageBitrate);
        refused_for(&job, &usable(), "Bitrate");

        let mut job = spec("in.mp4", "out.mp4");
        job.encoding_mode = Some(vise_core::EncodingMode::AverageBitrate);
        job.bitrate = Some(0);
        refused_for(&job, &usable(), "Bitrate");

        let mut job = spec("in.mp4", "out.mp4");
        job.encoding_mode = Some(vise_core::EncodingMode::TargetSize);
        job.target_size = Some(1024);
        refused_for(&job, &usable(), "TargetSize");
        let facts = usable().and_scan(synthetic_scan(10.0, 640, 480, Some(2)));
        accepted(&job, &facts);
    }

    #[test]
    fn a_lossless_request_may_not_carry_a_rate() {
        let mut job = mkv("in.mkv", "out.mkv");
        job.video_codec = VideoCodec::Ffv1;
        job.audio_codec = AudioCodec::Flac;
        job.lossless = true;
        job.quality = Some(20);
        let summary = refused_for(&job, &usable(), "Quality");
        assert!(
            summary.contains("lossless encode has no quality"),
            "{summary}"
        );

        job.quality = None;
        job.bitrate = Some(4000);
        refused_for(&job, &usable(), "Bitrate");

        // Without either, the same job is a complete lossless request.
        job.bitrate = None;
        accepted(&job, &usable());
    }

    #[test]
    fn a_two_pass_job_that_gains_nothing_from_it_is_refused() {
        let mut job = mkv("in.mkv", "out.mkv");
        job.two_pass = true;
        // A constant-quality second pass is a real thing and is left alone.
        job.quality = Some(22);
        accepted(&job, &usable());

        // A lossless write has no rate for the first pass to measure.
        job.lossless = true;
        let summary = refused_for(&job, &usable(), "TwoPass");
        assert!(summary.contains("lossless"), "{summary}");

        // An intra-only codec has no pass-based mode at all.
        job.lossless = false;
        job.quality = None;
        job.video_codec = VideoCodec::Ffv1;
        let summary = refused_for(&job, &usable(), "TwoPass");
        assert!(summary.contains("every frame on its own"), "{summary}");

        // An average-bitrate second pass is exactly what two passes are for.
        job.video_codec = VideoCodec::H264;
        job.encoding_mode = Some(vise_core::EncodingMode::AverageBitrate);
        job.bitrate = Some(2500);
        accepted(&job, &usable());
    }

    #[test]
    fn reports_crop_problems_against_source_geometry() {
        let facts = with_scan(Some(2));
        let mut job = spec("in.mp4", "out.mp4");
        job.crop = Some((600, 400, 320, 240));
        let summary = refused_for(&job, &facts, "Video.Crop");
        assert!(summary.contains("extends past"), "{summary}");

        job.crop = Some((0, 0, 0, 100));
        refused_for(&job, &usable(), "Video.Crop");

        job.crop = Some((0, 0, 320, 240));
        accepted(&job, &facts);
    }

    #[test]
    fn a_filter_the_encoder_cannot_apply_is_refused() {
        // Deblocking and telecine removal act on artefacts that only inter
        // prediction leaves behind, so an intra-only codec cannot be asked for
        // either of them.
        let mut job = mkv("in.mkv", "out.mkv");
        job.video_codec = VideoCodec::Ffv1;
        job.audio_codec = AudioCodec::Flac;
        job.video_processing.deblock = true;
        let summary = refused_for(&job, &usable(), "Video.Deblock");
        assert!(summary.contains("every frame on its own"), "{summary}");

        job.video_processing.deblock = false;
        accepted(&job, &usable());

        job.video_processing.detelecine = true;
        let summary = refused_for(&job, &usable(), "Video.Detelecine");
        assert!(summary.contains("telecine"), "{summary}");

        job.video_processing.detelecine = false;
        accepted(&job, &usable());

        // The same filters on a codec that does predict between frames are the
        // ordinary case and are left alone.
        job.video_codec = VideoCodec::H264;
        job.video_processing.deblock = true;
        job.video_processing.detelecine = true;
        accepted(&job, &usable());
    }

    #[test]
    fn a_deblock_strength_outside_the_filters_range_is_refused() {
        let mut job = spec("in.mp4", "out.mp4");
        job.video_processing.deblock = true;
        job.video_processing.deblock_strength = Some(64);
        let summary = refused_for(&job, &usable(), "Video.Deblock");
        assert!(
            summary.contains("between") || summary.contains("4-31"),
            "{summary}"
        );

        job.video_processing.deblock_strength = Some(4);
        accepted(&job, &usable());

        // A block size with deblocking off is a value nothing will read.
        job.video_processing.deblock = false;
        job.video_processing.deblock_strength = Some(8);
        refused_for(&job, &usable(), "Video.Deblock");
    }

    #[test]
    fn reports_audio_layout_conflicts() {
        let facts = with_scan(Some(2));
        let mut job = mkv("in.mkv", "out.mkv");
        job.audio_processing = AudioProcessingOptions {
            channels: Some(6),
            passthrough: true,
            ..Default::default()
        };
        let summary = refused_for(&job, &facts, "Audio.Channels");
        assert!(summary.contains("cannot change the layout"), "{summary}");

        let mut job = mkv("in.mkv", "out.mkv");
        job.audio_processing = AudioProcessingOptions {
            channels: Some(2),
            mixdown_enum: Some(vise_core::AudioMixdown::Surround51),
            ..Default::default()
        };
        let summary = refused_for(&job, &facts, "Audio.Mixdown");
        assert!(summary.contains("implies 6 channels"), "{summary}");

        let mut job = mkv("in.mkv", "out.mkv");
        job.audio_processing = AudioProcessingOptions {
            channels: Some(12),
            ..Default::default()
        };
        refused_for(&job, &facts, "Audio.Channels");
    }

    #[test]
    fn a_layout_the_source_cannot_produce_is_refused() {
        let stereo = with_scan(Some(2));
        let mut job = mkv("in.mkv", "out.mkv");

        // Asking for a surround track from a stereo recording would have to
        // invent the speakers that are not in it.
        job.audio_processing = AudioProcessingOptions {
            channels: Some(6),
            ..Default::default()
        };
        let summary = refused_for(&job, &stereo, "Audio.Channels");
        assert!(summary.contains("invent speakers"), "{summary}");

        // The same request as a named mixdown is refused the same way.
        job.audio_processing = AudioProcessingOptions {
            mixdown_enum: Some(vise_core::AudioMixdown::Surround51),
            ..Default::default()
        };
        let summary = refused_for(&job, &stereo, "Audio.Mixdown");
        assert!(summary.contains("cannot be made from it"), "{summary}");

        // Mixing a stereo source to mono is a real mixdown and is allowed.
        job.audio_processing = AudioProcessingOptions {
            mixdown_enum: Some(vise_core::AudioMixdown::Mono),
            ..Default::default()
        };
        accepted(&job, &stereo);
    }

    #[test]
    fn a_mixdown_to_the_layout_the_source_already_has_is_refused() {
        let mono = with_scan(Some(1));
        let mut job = mkv("in.mkv", "out.mkv");
        job.audio_processing = AudioProcessingOptions {
            mixdown_enum: Some(vise_core::AudioMixdown::Mono),
            ..Default::default()
        };
        let summary = refused_for(&job, &mono, "Audio.Mixdown");
        assert!(summary.contains("nothing to mix down"), "{summary}");

        // A surround source mixing down to stereo has something to do.
        let surround = with_scan(Some(6));
        job.audio_processing = AudioProcessingOptions {
            mixdown_enum: Some(vise_core::AudioMixdown::Stereo),
            ..Default::default()
        };
        accepted(&job, &surround);

        // Leaving the layout alone is always allowed.
        job.audio_processing = AudioProcessingOptions {
            mixdown_enum: Some(vise_core::AudioMixdown::Auto),
            ..Default::default()
        };
        accepted(&job, &surround);
    }

    #[test]
    fn reports_passthrough_conflicts() {
        let mut job = mkv("in.mkv", "out.mkv");
        job.audio_processing = AudioProcessingOptions {
            passthrough: true,
            gain: Some(2.0),
            ..Default::default()
        };
        refused_for(&job, &usable(), "Audio");
    }

    #[test]
    fn reports_subtitle_burn_in_without_a_source() {
        let mut job = spec("in.mp4", "out.mp4");
        job.subtitle_mode = SubtitleMode::BurnIn;
        refused_for(&job, &usable(), "Subtitle");
        refused_for(&job, &with_scan(Some(2)), "Subtitle");

        // A source that carries subtitles has something to burn in.
        let mut scan = synthetic_scan(10.0, 640, 480, Some(2));
        scan.streams.push(StreamInfo {
            index: 2,
            kind: TrackKind::Subtitle,
            ..Default::default()
        });
        accepted(&job, &MachineFacts::unknown().and_scan(scan));
    }

    #[test]
    fn accepts_a_well_formed_job() {
        let dir = std::env::temp_dir().join("vise-ipc-validation");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("input.mp4");
        let output = dir.join("output.mp4");
        std::fs::write(&input, b"not real media").expect("write input");

        let facts = probe_machine_facts(&spec(input.to_str().unwrap(), output.to_str().unwrap()))
            .and_scan(synthetic_scan(10.0, 640, 480, Some(2)));
        let mut job = spec(input.to_str().unwrap(), output.to_str().unwrap());
        job.quality = Some(22);
        accepted(&job, &facts);
    }
}
