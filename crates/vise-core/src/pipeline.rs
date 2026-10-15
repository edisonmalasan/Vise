use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{AudioCodec, ContainerFormat, CoreError, FfprobeScanner, JobSpec, VideoCodec};

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

        let mut cmd = Command::new(&ffmpeg);
        cmd.arg("-y").arg("-i").arg(&input);
        cmd.arg("-c:v").arg(video_encoder(video_codec));
        cmd.arg("-c:a").arg(audio_encoder(audio_codec));
        // Keep broad compatibility for software baseline encoding.
        cmd.arg("-pix_fmt").arg("yuv420p");
        if container == ContainerFormat::Mp4 {
            cmd.arg("-movflags").arg("+faststart");
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
