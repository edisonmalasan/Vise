use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    AudioCodec, ContainerFormat, CoreError, FfprobeScanner, HardwareAcceleration,
    HardwareCapabilities, JobSpec, SubtitleMode, VideoCodec, VideoProcessingOptions,
};

/// Baseline transcode planning and execution.
#[derive(Debug, Clone)]
pub struct Transcoder {
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
}

impl Default for Transcoder {
    fn default() -> Self {
        let scanner = FfprobeScanner::default();
        Self {
            ffmpeg: scanner.ffmpeg_path().map(PathBuf::from),
            ffprobe: scanner.ffprobe_path().map(PathBuf::from),
        }
    }
}

impl Transcoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tools(ffmpeg: Option<PathBuf>, ffprobe: Option<PathBuf>) -> Self {
        Self { ffmpeg, ffprobe }
    }

    fn ffmpeg_bin(&self) -> Result<PathBuf, CoreError> {
        self.ffmpeg
            .clone()
            .or_else(|| FfprobeScanner::default().ffmpeg_path().map(PathBuf::from))
            .ok_or_else(|| CoreError::MissingTool("ffmpeg".into()))
    }

    fn ffprobe_bin(&self) -> Result<PathBuf, CoreError> {
        self.ffprobe
            .clone()
            .or_else(|| FfprobeScanner::default().ffprobe_path().map(PathBuf::from))
            .ok_or_else(|| CoreError::MissingTool("ffprobe".into()))
    }

    pub fn transcode(&self, spec: &JobSpec) -> Result<TranscodeResult, CoreError> {
        self.transcode_job(spec)
    }

    pub fn run(&self, spec: &JobSpec) -> Result<TranscodeResult, CoreError> {
        self.transcode_job(spec)
    }

    pub fn transcode_job(&self, spec: &JobSpec) -> Result<TranscodeResult, CoreError> {
        match self.transcode_job_attempt(spec, true) {
            Ok(r) => Ok(r),
            Err(first_err) => {
                if spec.hardware_acceleration_options().is_enabled() {
                    match self.transcode_job_attempt(spec, false) {
                        Ok(r) => Ok(r),
                        Err(_) => Err(first_err),
                    }
                } else {
                    Err(first_err)
                }
            }
        }
    }

    fn transcode_job_attempt(
        &self,
        spec: &JobSpec,
        hardware_enabled: bool,
    ) -> Result<TranscodeResult, CoreError> {
        let plan = self.plan_with(spec, hardware_enabled)?;
        self.execute(&plan)
    }

    /// Resolve a job specification into a fully expanded encoder invocation.
    ///
    /// The plan is plain data, so callers that need process-level control (for
    /// example the background worker process, which must be able to pause,
    /// resume, and cancel) can run it themselves instead of going through
    /// [`Transcoder::transcode`].
    pub fn plan(&self, spec: &JobSpec) -> Result<TranscodePlan, CoreError> {
        self.plan_with(spec, spec.hardware_acceleration_options().is_enabled())
    }

    /// Run a plan produced by [`Transcoder::plan`] and probe the output.
    pub fn execute(&self, plan: &TranscodePlan) -> Result<TranscodeResult, CoreError> {
        if let Some(parent) = plan.output_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
            }
        }

        if let Some(first_pass) = &plan.first_pass_args {
            let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
            let _ = Command::new(&plan.program)
                .args(first_pass)
                .arg(null_device)
                .output();
        }

        let output = Command::new(&plan.program)
            .args(&plan.args)
            .output()
            .map_err(|e| CoreError::CommandFailed(e.to_string()))?;

        // The sidecar has served its purpose whether or not the encode worked;
        // leaving it behind would put a stray file next to the output.
        if let Some(path) = &plan.chapter_metadata_path {
            let _ = std::fs::remove_file(path);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(CoreError::CommandFailed(if stderr.trim().is_empty() {
                format!(
                    "ffmpeg transcode failed (status {:?})",
                    output.status.code()
                )
            } else {
                stderr
            }));
        }

        if !plan.output_path.is_file() {
            return Err(CoreError::CommandFailed(
                "ffmpeg did not produce output file".into(),
            ));
        }

        // Best-effort metadata probe.
        let (duration_seconds, video_codec_name, audio_codec_name) =
            self.probe_output(&plan.output_path);

        Ok(TranscodeResult {
            input_path: plan.input_path.clone(),
            output_path: plan.output_path.clone(),
            container: plan.container,
            video_codec: plan.video_codec,
            audio_codec: plan.audio_codec,
            duration_seconds,
            video_codec_name,
            audio_codec_name,
            lossless_claim: plan.lossless_claim.clone(),
            hdr_claims: plan.hdr_claims.clone(),
            attachment_claims: plan.attachment_claims.clone(),
        })
    }

    fn plan_with(
        &self,
        spec: &JobSpec,
        hardware_enabled: bool,
    ) -> Result<TranscodePlan, CoreError> {
        let ffmpeg = self.ffmpeg_bin()?;
        let input = PathBuf::from(&spec.input_path);
        if !input.is_file() {
            return Err(CoreError::InvalidMedia(format!(
                "input file does not exist: {}",
                input.display()
            )));
        }

        let container = spec.container;
        // WebM baseline uses VP9 + Opus; other containers honor requested codecs.
        let (video_codec, audio_codec) = match container {
            ContainerFormat::Webm => (VideoCodec::Vp9, AudioCodec::Opus),
            _ => (spec.video_codec, spec.audio_codec),
        };

        // Merge video processing options from all aliases and legacy fields.
        let mut video_opts = spec.video_processing.clone();
        for o in [
            spec.video_filters.as_ref(),
            spec.video_options.as_ref(),
            spec.filters.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            merge_video_options(&mut video_opts, o);
        }
        if let Some(crop) = spec.crop {
            if video_opts.crop.is_none() {
                video_opts.crop = Some(crop);
            }
        }
        if let Some(scale) = spec.scale_to {
            if video_opts.scale.is_none() && video_opts.scale_to.is_none() {
                video_opts.scale_to = Some(scale);
            }
        }

        let filter_chain = video_opts.build_filter_chain();

        let audio_opts = spec.audio_processing_options();
        let audio_filter_chain = audio_opts.build();

        let sub_opts = spec.subtitle_processing_options();
        let mut combined_filter = filter_chain.clone();
        if sub_opts.effective_burn_in() {
            let burn_path = sub_opts
                .effective_path()
                .map(|p| p.to_string())
                .or_else(|| spec.subtitle_path.clone())
                .unwrap_or_else(|| input.to_string_lossy().to_string());
            let burn = if sub_opts.ass_handling
                || sub_opts.ssa_handling
                || sub_opts.format == Some(crate::SubtitleFormat::Ass)
                || sub_opts.format == Some(crate::SubtitleFormat::Ssa)
            {
                format!("ass='{}'", burn_path)
            } else {
                format!("subtitles='{}'", burn_path)
            };
            if combined_filter.is_empty() {
                combined_filter = burn;
            } else {
                combined_filter = format!("{},{}", combined_filter, burn);
            }
        }

        let hw_opts = if hardware_enabled {
            spec.hardware_acceleration_options()
        } else {
            HardwareAcceleration::disabled()
        };
        let caps = HardwareCapabilities::detect_with(
            self.ffmpeg
                .clone()
                .or_else(|| FfprobeScanner::default().ffmpeg_path().map(PathBuf::from)),
        );
        let resolved_encoder = caps.preferred_encoder(video_codec, &hw_opts);
        let hw_decode = caps.preferred_decoder_arg(&hw_opts);

        // Lossless is a property of the whole job rather than of the video
        // encoder alone, so it is resolved once here and read by both the
        // encoder arguments and the finished result.
        let lossless_plan = crate::LosslessPlan::resolve(spec);
        let hdr_caps = crate::HdrCapabilities::detect(self.ffmpeg.as_deref());

        let chapter_opts = spec.chapter_processing_options();
        // An edited or replaced marker list becomes a metadata sidecar that
        // ffmpeg reads as a second input, so the markers the window shows are
        // the markers the finished file carries.
        let chapter_metadata_path = if chapter_opts.uses_edited_chapters() {
            let sidecar = chapter_sidecar_path(Path::new(&spec.output_path));
            std::fs::write(&sidecar, chapter_opts.render_ffmetadata())
                .map_err(|e| CoreError::Io(e.to_string()))?;
            Some(sidecar)
        } else {
            None
        };

        let mut args: Vec<String> = Vec::new();
        if let Some(dec) = &hw_decode {
            args.push("-hwaccel".into());
            args.push(dec.clone());
            let dev = hw_opts
                .device
                .clone()
                .or_else(|| hw_opts.decode.device.clone());
            if let Some(dev) = dev {
                args.push("-hwaccel_device".into());
                args.push(dev);
            }
        }
        args.push("-y".into());
        args.push("-i".into());
        args.push(input.to_string_lossy().to_string());
        if let Some(path) = &chapter_metadata_path {
            args.push("-i".into());
            args.push(path.to_string_lossy().to_string());
        }
        if !combined_filter.is_empty() {
            args.push("-vf".into());
            args.push(combined_filter.clone());
        }
        args.push("-c:v".into());
        args.push(resolved_encoder.clone());
        // Preset, profile, tune, level, and colour range shape how frames are
        // coded, so they follow the encoder name.
        args.extend(spec.video_encoding_options().encoder_args());
        // Dolby Vision RPU coding and HDR10+ SEI forwarding are private
        // options of the software HEVC encoder, so they are only added when
        // that encoder is the one that will run. The decision itself is still
        // reported to the caller even when this build cannot honour it.
        let hdr_claims: Vec<String> = if resolved_encoder == "libx265" {
            let vision = spec.dolby_vision_outcome(&hdr_caps);
            let plus = spec.hdr10_plus_outcome(&hdr_caps);
            args.extend(vision.args.clone());
            args.extend(plus.args.clone());
            [vision, plus]
                .iter()
                .filter(|outcome| !outcome.supported)
                .map(|outcome| outcome.claim.clone())
                .collect()
        } else {
            [
                spec.dolby_vision_outcome(&hdr_caps),
                spec.hdr10_plus_outcome(&hdr_caps),
            ]
            .iter()
            .filter(|outcome| !outcome.supported)
            .map(|outcome| outcome.claim.clone())
            .collect()
        };
        if audio_opts.passthrough || audio_opts.copy {
            args.push("-c:a".into());
            args.push("copy".into());
        } else {
            args.push("-c:a".into());
            args.push(audio_encoder(audio_codec).into());
        }
        let mut audio_filters = audio_filter_chain.clone();
        if spec.output_options.align_audio_video {
            // Nudge audio onto the video timeline when the streams start apart.
            if !audio_filters.is_empty() {
                audio_filters.push(',');
            }
            audio_filters.push_str("aresample=async=1:first_pts=0");
        }
        if !audio_filters.is_empty() {
            args.push("-af".into());
            args.push(audio_filters);
        }
        if let Some(rate) = audio_opts.sample_rate {
            args.push("-ar".into());
            args.push(rate.to_string());
        }
        if let Some(ch) = audio_opts.channels {
            args.push("-ac".into());
            args.push(ch.to_string());
        }
        // Keep broad compatibility for software baseline encoding, and pin the
        // lossless plan's pixel format when the job is a lossless write so the
        // encoder is not handed a format the plan did not choose.
        let pix_fmt = lossless_plan.pix_fmt.clone().unwrap_or_else(|| {
            if video_opts.ten_bit {
                "yuv420p10le".to_string()
            } else {
                "yuv420p".to_string()
            }
        });
        args.push("-pix_fmt".into());
        args.push(pix_fmt);
        // constant frame rate handling
        let cfr_fps = match &video_opts.frame_rate {
            Some(crate::FrameRateMode::Constant(fps)) => Some(*fps),
            _ => video_opts.constant_frame_rate,
        };
        if let Some(fps) = cfr_fps {
            args.push("-r".into());
            args.push(fps.to_string());
        }
        // variable frame rate handling
        if video_opts.variable_frame_rate
            || matches!(video_opts.frame_rate, Some(crate::FrameRateMode::Variable))
        {
            args.push("-vsync".into());
            args.push("vfr".into());
        }
        let metadata_opts = spec.metadata_processing_options();

        // The front-loaded index only exists in MP4; the other containers have
        // no equivalent flag, which is why the interface says so out loud.
        if container == ContainerFormat::Mp4 && spec.output_options.wants_web_optimized() {
            args.push("-movflags".into());
            args.push("+faststart".into());
        }
        // HDR metadata preservation
        if video_opts.hdr || video_opts.hdr_metadata {
            args.push("-color_primaries".into());
            args.push("bt2020".into());
            args.push("-color_trc".into());
            args.push("smpte2084".into());
            args.push("-colorspace".into());
            args.push("bt2020nc".into());
            if chapter_metadata_path.is_none() {
                args.push("-map_metadata".into());
                args.push("0".into());
            }
        }
        match spec.subtitle_mode {
            SubtitleMode::None => {
                args.push("-sn".into());
            }
            SubtitleMode::BurnIn => {
                args.push("-sn".into());
            }
            SubtitleMode::Passthrough => {
                args.push("-map".into());
                args.push("0".into());
                if sub_opts.srt_to_mp4 || matches!(container, ContainerFormat::Mp4) {
                    args.push("-c:s".into());
                    args.push("mov_text".into());
                } else {
                    args.push("-c:s".into());
                    args.push("copy".into());
                }
            }
        }
        // Chapter/metadata copy-edit handling through ffmpeg arguments.
        args.extend(chapter_opts.build_ffmpeg_args());
        args.extend(metadata_opts.build_metadata_args());
        // Cover art and attachments are extra streams, and which containers can
        // carry them is a property of the container rather than of the request,
        // so the decision is made here, where the container is known, and its
        // mapping arguments are the only ones that reach the encoder. A request
        // the container cannot honour is reported rather than mapped anyway.
        let attachment_decision = crate::attachments::resolve(
            container,
            metadata_opts.cover_art || chapter_opts.cover_art,
            metadata_opts.attachments
                || metadata_opts.attachment_copy
                || chapter_opts.attachments
                || chapter_opts.attachment_copy,
        );
        args.extend(attachment_decision.args.iter().cloned());
        let attachment_claims = match attachment_decision.reason() {
            Some(reason) => vec![reason],
            None => Vec::new(),
        };
        if chapter_metadata_path.is_some() {
            // The marker list the window holds wins over anything the source
            // carried, so it is mapped last.
            args.push("-map_metadata".into());
            args.push("1".into());
        }

        // Encoding rate-control mode.
        let resolved_mode = spec.resolved_encoding_mode();
        let quality = spec.effective_quality();
        let bitrate = spec.effective_bitrate_kbps();
        let target = spec.effective_target_size_bytes();

        let input_duration = self.probe_duration(&input);
        if resolved_mode.is_lossless() {
            // An intra lossless encoder takes no rate-control value at all;
            // handing it `-crf 0` would make the encoder refuse the job.
            if lossless_plan.video_encoder.is_none() {
                args.push("-crf".into());
                args.push("0".into());
            }
        } else if resolved_mode.is_constant_quality() {
            if let Some(q) = quality {
                args.push("-crf".into());
                args.push(q.to_string());
            }
        } else if resolved_mode.is_average_bitrate() {
            if let Some(b) = bitrate {
                args.push("-b:v".into());
                args.push(format!("{b}k"));
            }
        } else if resolved_mode.is_target_size() {
            if let (Some(bytes), Some(dur)) = (target, input_duration) {
                if dur > 0.0 {
                    let kbps = ((bytes as f64) * 8.0 / 1000.0 / dur) as u64;
                    if kbps > 0 {
                        args.push("-b:v".into());
                        args.push(format!("{kbps}k"));
                    }
                }
            }
        }

        let first_pass_args = if spec.two_pass_enabled() {
            args.push("-pass".into());
            args.push("2".into());
            let mut first = vec!["-y".to_string(), "-i".to_string()];
            first.push(input.to_string_lossy().to_string());
            if !combined_filter.is_empty() {
                first.push("-vf".into());
                first.push(combined_filter.clone());
            }
            first.push("-c:v".into());
            first.push(resolved_encoder.clone());
            first.push("-an".into());
            first.push("-pass".into());
            first.push("1".into());
            first.push("-f".into());
            first.push("null".into());
            Some(first)
        } else {
            None
        };

        args.push("-shortest".into());
        args.push(spec.output_path.clone());

        Ok(TranscodePlan {
            program: ffmpeg,
            args,
            first_pass_args,
            input_path: input,
            output_path: PathBuf::from(&spec.output_path),
            container,
            video_codec,
            audio_codec,
            video_encoder: resolved_encoder,
            filter_chain: combined_filter,
            audio_filter_chain,
            duration_seconds: input_duration,
            chapter_metadata_path,
            lossless_claim: if crate::is_lossless_request(spec) {
                Some(lossless_plan.claim.clone())
            } else {
                None
            },
            hdr_claims: hdr_claims.clone(),
            attachment_claims: attachment_claims.clone(),
        })
    }

    pub fn transcode_path(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        container: Option<ContainerFormat>,
    ) -> Result<TranscodeResult, CoreError> {
        let input = input.as_ref();
        let output = output.as_ref();
        let container = container.unwrap_or_else(|| infer_container(output));
        let spec = JobSpec {
            input_path: input.to_string_lossy().to_string(),
            output_path: output.to_string_lossy().to_string(),
            container,
            ..JobSpec::default()
        };
        self.transcode_job(&spec)
    }

    fn probe_duration(&self, path: &Path) -> Option<f64> {
        let ffprobe = self.ffprobe_bin().ok()?;
        let output = Command::new(&ffprobe)
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(path)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f64>()
            .ok()
    }
    fn probe_output(&self, path: &Path) -> (Option<f64>, Option<String>, Option<String>) {
        let ffprobe = match self.ffprobe_bin() {
            Ok(p) => p,
            Err(_) => return (None, None, None),
        };

        let output = Command::new(&ffprobe)
            .arg("-v")
            .arg("error")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg(path)
            .output();

        let bytes = match output {
            Ok(o) if o.status.success() => o.stdout,
            _ => return (None, None, None),
        };

        let parsed: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return (None, None, None),
        };

        let duration = parsed
            .get("format")
            .and_then(|f| f.get("duration"))
            .and_then(|d| match d {
                serde_json::Value::String(s) => s.parse::<f64>().ok(),
                serde_json::Value::Number(n) => n.as_f64(),
                _ => None,
            });

        let (video_codec, audio_codec) = parse_codecs(&parsed);
        (duration, video_codec, audio_codec)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscodeResult {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub container: ContainerFormat,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub duration_seconds: Option<f64>,
    pub video_codec_name: Option<String>,
    pub audio_codec_name: Option<String>,
    /// What the finished file can honestly be called, when the job is a
    /// lossless write; `None` when it is not.
    pub lossless_claim: Option<String>,
    /// Dolby Vision and HDR10+ requests this build could not honour, if any.
    pub hdr_claims: Vec<String>,
    /// Cover-art and attachment requests the container could not honour, if any.
    pub attachment_claims: Vec<String>,
}

/// A fully resolved encoder invocation.
///
/// A plan is nearly inert data: resolving it performs no media work beyond
/// validating the input, probing its duration, and writing the chapter
/// sidecar that an edited marker list needs. Frontends that need process-level
/// control (pause, resume, cancel, or crash isolation) can spawn
/// [`TranscodePlan::program`] with [`TranscodePlan::args`] directly.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscodePlan {
    /// Encoder executable resolved for this job.
    pub program: PathBuf,
    /// Full argument vector for the main encode pass.
    pub args: Vec<String>,
    /// First-pass argument vector for two-pass encodes; the null sink argument
    /// is appended by the executor.
    pub first_pass_args: Option<Vec<String>>,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub container: ContainerFormat,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    /// Concrete video encoder name selected after hardware capability probing.
    pub video_encoder: String,
    /// Video filter chain applied by the plan.
    pub filter_chain: String,
    /// Audio filter chain applied by the plan.
    pub audio_filter_chain: String,
    /// Probed input duration, when available.
    pub duration_seconds: Option<f64>,
    /// Chapter metadata sidecar written while planning, if the job carried an
    /// edited marker list. [`Transcoder::execute`] removes it afterwards.
    pub chapter_metadata_path: Option<PathBuf>,
    /// What the finished file can honestly be called, when the job is a
    /// lossless write; `None` when it is not.
    pub lossless_claim: Option<String>,
    /// Dolby Vision and HDR10+ requests this build could not honour, if any.
    pub hdr_claims: Vec<String>,
    /// Cover-art and attachment requests the container could not honour, if any.
    pub attachment_claims: Vec<String>,
}

impl TranscodePlan {
    /// Command-line rendering of the main pass, for logs and diagnostics.
    pub fn command_line(&self) -> String {
        let mut parts = vec![self.program.to_string_lossy().to_string()];
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }
}

/// Where the chapter sidecar for an output file lives.
///
/// The name sits beside the output so the executor can clean it up without
/// searching, and it is named after the output so two queued jobs writing to
/// different folders never collide.
fn chapter_sidecar_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    output.with_file_name(format!("{stem}.vise-chapters.ffmeta"))
}

fn infer_container(path: &Path) -> ContainerFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("mkv") => ContainerFormat::Mkv,
        Some("webm") => ContainerFormat::Webm,
        _ => ContainerFormat::Mp4,
    }
}

fn audio_encoder(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Aac => "aac",
        AudioCodec::Opus => "libopus",
        AudioCodec::Vorbis => "libvorbis",
        AudioCodec::Flac => "flac",
        AudioCodec::Mp3 => "libmp3lame",
        AudioCodec::Ac3 => "ac3",
        AudioCodec::Eac3 => "eac3",
        AudioCodec::Pcm => "pcm_s16le",
    }
}

fn parse_codecs(parsed: &serde_json::Value) -> (Option<String>, Option<String>) {
    let mut video = None;
    let mut audio = None;
    if let Some(streams) = parsed.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            let codec_type = stream.get("codec_type").and_then(|v| v.as_str());
            let codec = stream
                .get("codec_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match codec_type {
                Some("video") if video.is_none() => video = codec,
                Some("audio") if audio.is_none() => audio = codec,
                _ => {}
            }
        }
    }
    (video, audio)
}

impl Transcoder {
    /// Construct an ffmpeg filter chain string from video processing options.
    pub fn build_filter_chain(&self, options: &VideoProcessingOptions) -> String {
        options.build_filter_chain()
    }

    pub fn build_video_filter(&self, options: &VideoProcessingOptions) -> String {
        options.build_video_filter()
    }

    pub fn build_video_filter_chain(&self, options: &VideoProcessingOptions) -> String {
        options.build_video_filter_chain()
    }
}

fn merge_video_options(target: &mut VideoProcessingOptions, src: &VideoProcessingOptions) {
    if src.crop.is_some() {
        target.crop = src.crop;
    }
    if src.scale.is_some() {
        target.scale = src.scale;
    }
    if src.scale_to.is_some() {
        target.scale_to = src.scale_to;
    }
    if src.deinterlace || src.deinterlacing {
        target.deinterlace = true;
        target.deinterlacing = true;
    }
    if src.comb_detect || src.comb_detection {
        target.comb_detect = true;
        target.comb_detection = true;
    }
    if src.detelecine {
        target.detelecine = true;
    }
    if src.denoise {
        target.denoise = true;
    }
    if src.denoise_strength.is_some() {
        target.denoise_strength = src.denoise_strength;
    }
    if src.sharpen {
        target.sharpen = true;
    }
    if src.sharpen_amount.is_some() {
        target.sharpen_amount = src.sharpen_amount;
    }
    if src.deblock {
        target.deblock = true;
    }
    if src.deblock_strength.is_some() {
        target.deblock_strength = src.deblock_strength;
    }
    if src.rotate.is_some() {
        target.rotate = src.rotate;
    }
    if src.rotate_degrees.is_some() {
        target.rotate_degrees = src.rotate_degrees;
    }
    if src.pad.is_some() {
        target.pad = src.pad;
    }
    if src.pad_to.is_some() {
        target.pad_to = src.pad_to;
    }
    if src.color_space.is_some() {
        target.color_space = src.color_space.clone();
    }
    if src.color_conversion.is_some() {
        target.color_conversion = src.color_conversion.clone();
    }
    if src.grayscale {
        target.grayscale = true;
    }
    if src.chroma_smooth || src.chroma_smoothing {
        target.chroma_smooth = true;
        target.chroma_smoothing = true;
    }
    if src.frame_rate.is_some() {
        target.frame_rate = src.frame_rate.clone();
    }
    if src.constant_frame_rate.is_some() {
        target.constant_frame_rate = src.constant_frame_rate;
    }
    if src.variable_frame_rate {
        target.variable_frame_rate = true;
    }
    if src.ten_bit {
        target.ten_bit = true;
    }
    if src.hdr || src.hdr_metadata {
        target.hdr = true;
        target.hdr_metadata = true;
    }
}

// Free function convenience.
pub fn transcode_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<TranscodeResult, CoreError> {
    let container = infer_container(output.as_ref());
    let transcoder = Transcoder::default();
    transcoder.transcode_path(input, output, Some(container))
}

pub fn transcode(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<TranscodeResult, CoreError> {
    transcode_file(input, output)
}
