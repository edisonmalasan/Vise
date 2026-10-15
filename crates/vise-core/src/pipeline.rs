use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    AudioCodec, ContainerFormat, CoreError, FfprobeScanner, JobSpec, SubtitleMode, VideoCodec,
    VideoProcessingOptions,
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
        let ffmpeg = self.ffmpeg_bin()?;
        let input = PathBuf::from(&spec.input_path);
        if !input.is_file() {
            return Err(CoreError::InvalidMedia(format!(
                "input file does not exist: {}",
                input.display()
            )));
        }

        if let Some(parent) = Path::new(&spec.output_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
            }
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

        let mut cmd = Command::new(&ffmpeg);
        cmd.arg("-y").arg("-i").arg(&input);
        if !combined_filter.is_empty() {
            cmd.arg("-vf").arg(&combined_filter);
        }
        cmd.arg("-c:v").arg(video_encoder(video_codec));
        if audio_opts.passthrough || audio_opts.copy {
            cmd.arg("-c:a").arg("copy");
        } else {
            cmd.arg("-c:a").arg(audio_encoder(audio_codec));
        }
        if !audio_filter_chain.is_empty() {
            cmd.arg("-af").arg(&audio_filter_chain);
        }
        if let Some(rate) = audio_opts.sample_rate {
            cmd.arg("-ar").arg(rate.to_string());
        }
        if let Some(ch) = audio_opts.channels {
            cmd.arg("-ac").arg(ch.to_string());
        }
        // Keep broad compatibility for software baseline encoding.
        let pix_fmt = if video_opts.ten_bit {
            "yuv420p10le"
        } else {
            "yuv420p"
        };
        cmd.arg("-pix_fmt").arg(pix_fmt);
        // constant frame rate handling
        let cfr_fps = match &video_opts.frame_rate {
            Some(crate::FrameRateMode::Constant(fps)) => Some(*fps),
            _ => video_opts.constant_frame_rate,
        };
        if let Some(fps) = cfr_fps {
            cmd.arg("-r").arg(fps.to_string());
        }
        // variable frame rate handling
        if video_opts.variable_frame_rate
            || matches!(video_opts.frame_rate, Some(crate::FrameRateMode::Variable))
        {
            cmd.arg("-vsync").arg("vfr");
        }
        let chapter_opts = spec.chapter_processing_options();
        let metadata_opts = spec.metadata_processing_options();

        if container == ContainerFormat::Mp4 {
            cmd.arg("-movflags").arg("+faststart");
        }
        // HDR metadata preservation
        if video_opts.hdr || video_opts.hdr_metadata {
            cmd.arg("-color_primaries").arg("bt2020");
            cmd.arg("-color_trc").arg("smpte2084");
            cmd.arg("-colorspace").arg("bt2020nc");
            cmd.arg("-map_metadata").arg("0");
        }
        match spec.subtitle_mode {
            SubtitleMode::None => {
                cmd.arg("-sn");
            }
            SubtitleMode::BurnIn => {
                cmd.arg("-sn");
            }
            SubtitleMode::Passthrough => {
                cmd.arg("-map").arg("0");
                if sub_opts.srt_to_mp4 || matches!(container, ContainerFormat::Mp4) {
                    cmd.arg("-c:s").arg("mov_text");
                } else {
                    cmd.arg("-c:s").arg("copy");
                }
            }
        }
        // Chapter/metadata copy-edit handling through ffmpeg arguments.
        for arg in chapter_opts.build_ffmpeg_args() {
            cmd.arg(arg);
        }
        for arg in metadata_opts.build_metadata_args() {
            cmd.arg(arg);
        }

        // Encoding rate-control mode.
        let resolved_mode = spec.resolved_encoding_mode();
        let quality = spec.effective_quality();
        let bitrate = spec.effective_bitrate_kbps();
        let target = spec.effective_target_size_bytes();

        let input_duration = self.probe_duration(&input);
        if resolved_mode.is_lossless() {
            cmd.arg("-crf").arg("0");
        } else if resolved_mode.is_constant_quality() {
            if let Some(q) = quality {
                cmd.arg("-crf").arg(q.to_string());
            }
        } else if resolved_mode.is_average_bitrate() {
            if let Some(b) = bitrate {
                cmd.arg("-b:v").arg(format!("{}k", b));
            }
        } else if resolved_mode.is_target_size() {
            if let (Some(bytes), Some(dur)) = (target, input_duration) {
                if dur > 0.0 {
                    let kbps = ((bytes as f64) * 8.0 / 1000.0 / dur) as u64;
                    if kbps > 0 { cmd.arg("-b:v").arg(format!("{}k", kbps)); }
                }
            }
        }

        if spec.two_pass_enabled() {
            cmd.arg("-pass").arg("2");
        }

        if spec.two_pass_enabled() {
            let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
            let mut first = Command::new(&ffmpeg);
            first.arg("-y").arg("-i").arg(&input);
            if !combined_filter.is_empty() {
                first.arg("-vf").arg(&combined_filter);
            }
            first.arg("-c:v").arg(video_encoder(video_codec));
            first.arg("-an");
            first.arg("-pass").arg("1");
            first.arg("-f").arg("null");
            first.arg(null_device);
            let _ = first.output();
        }

        cmd.arg("-shortest");
        cmd.arg(&spec.output_path);

        let output = cmd
            .output()
            .map_err(|e| CoreError::CommandFailed(e.to_string()))?;

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

        let output_path = PathBuf::from(&spec.output_path);
        if !output_path.is_file() {
            return Err(CoreError::CommandFailed(
                "ffmpeg did not produce output file".into(),
            ));
        }

        // Best-effort metadata probe.
        let (duration_seconds, video_codec_name, audio_codec_name) =
            self.probe_output(&output_path);

        Ok(TranscodeResult {
            input_path: input,
            output_path,
            container,
            video_codec,
            audio_codec,
            duration_seconds,
            video_codec_name,
            audio_codec_name,
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
            .arg("-v").arg("error")
            .arg("-show_entries").arg("format=duration")
            .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
            .arg(path)
            .output().ok()?;
        if !output.status.success() { return None; }
        String::from_utf8_lossy(&output.stdout).trim().parse::<f64>().ok()
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

fn video_encoder(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "libx264",
        VideoCodec::H265 => "libx265",
        VideoCodec::Av1 => "libaom-av1",
        VideoCodec::Vp9 => "libvpx-vp9",
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
