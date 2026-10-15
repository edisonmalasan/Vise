use std::fs;
use std::path::Path;

use vise_core::{AudioCodec, ContainerFormat, JobSpec, VideoCodec};

#[derive(Debug, serde::Deserialize, serde::Serialize, Default, Clone)]
pub struct Preset {
    pub format: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub title: Option<u32>,
    pub quality: Option<u32>,
}

pub fn load(path: &Path) -> Result<Preset, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("reading preset: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parsing preset: {e}"))
}

pub fn apply(spec: &mut JobSpec, preset: &Preset) {
    if let Some(format) = &preset.format {
        if let Some(c) = parse_container(format) {
            spec.container = c;
        }
    }
    if let Some(v) = &preset.video_codec {
        if let Some(c) = parse_video(v) {
            spec.video_codec = c;
        }
    }
    if let Some(a) = &preset.audio_codec {
        if let Some(c) = parse_audio(a) {
            spec.audio_codec = c;
        }
    }
    if let Some(t) = preset.title {
        spec.title_index = Some(t);
    }
    if let Some(q) = preset.quality {
        spec.quality = Some(q);
    }
}

pub fn parse_container(s: &str) -> Option<ContainerFormat> {
    match s.to_lowercase().as_str() {
        "mp4" => Some(ContainerFormat::Mp4),
        "mkv" => Some(ContainerFormat::Mkv),
        "webm" => Some(ContainerFormat::Webm),
        _ => None,
    }
}

pub fn parse_video(s: &str) -> Option<VideoCodec> {
    match s.to_lowercase().as_str() {
        "h264" => Some(VideoCodec::H264),
        "h265" | "hevc" => Some(VideoCodec::H265),
        "av1" => Some(VideoCodec::Av1),
        "vp9" => Some(VideoCodec::Vp9),
        _ => None,
    }
}

pub fn parse_audio(s: &str) -> Option<AudioCodec> {
    match s.to_lowercase().as_str() {
        "aac" => Some(AudioCodec::Aac),
        "opus" => Some(AudioCodec::Opus),
        "vorbis" => Some(AudioCodec::Vorbis),
        "flac" => Some(AudioCodec::Flac),
        "mp3" => Some(AudioCodec::Mp3),
        _ => None,
    }
}
