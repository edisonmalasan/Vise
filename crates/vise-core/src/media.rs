use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    #[default]
    Video,
    Audio,
    Subtitle,
    Data,
}

/// A contiguous block of media data (decoded frame or muxed content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBuffer {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
    pub track: Option<usize>,
}

impl MediaBuffer {
    pub fn new(data: Vec<u8>, pts: i64, dts: i64) -> Self {
        Self {
            data,
            pts,
            dts,
            keyframe: false,
            track: None,
        }
    }

    pub fn with_keyframe(mut self, keyframe: bool) -> Self {
        self.keyframe = keyframe;
        self
    }

    pub fn with_track(mut self, track: usize) -> Self {
        self.track = Some(track);
        self
    }
}

/// An encoded unit ready for muxing or transmission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packet {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
    pub track: usize,
    pub track_kind: TrackKind,
}

impl Packet {
    pub fn new(data: Vec<u8>, pts: i64, dts: i64, track: usize, track_kind: TrackKind) -> Self {
        Self {
            data,
            pts,
            dts,
            keyframe: false,
            track,
            track_kind,
        }
    }
}

// ---------------------------------------------------------------------------
// Media scanning (subprocess-based via ffprobe/ffmpeg).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FormatInfo {
    pub format_name: Option<String>,
    pub format_long_name: Option<String>,
    pub duration_seconds: Option<f64>,
    pub size_bytes: Option<u64>,
    pub bit_rate: Option<u64>,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StreamInfo {
    pub index: usize,
    pub kind: TrackKind,
    pub codec: Option<String>,
    pub codec_long: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub bit_rate: Option<u64>,
    pub duration_seconds: Option<f64>,
    /// Dolby Vision profile reported in the stream's side data, when present.
    pub dolby_vision_profile: Option<u8>,
    /// Whether the stream carries a Dolby Vision RPU.
    pub dolby_vision_rpu: bool,
    /// Whether the stream carries HDR10+ dynamic metadata.
    pub hdr10_plus: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterInfo {
    pub index: usize,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TitleInfo {
    pub index: usize,
    pub duration_seconds: Option<f64>,
    pub chapters: Vec<ChapterInfo>,
    pub stream_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScanResult {
    pub path: PathBuf,
    pub format: FormatInfo,
    pub streams: Vec<StreamInfo>,
    pub chapters: Vec<ChapterInfo>,
    pub titles: Vec<TitleInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropDetection {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct FfprobeScanner {
    ffprobe: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
}

impl Default for FfprobeScanner {
    fn default() -> Self {
        Self {
            ffprobe: find_tool("ffprobe"),
            ffmpeg: find_tool("ffmpeg"),
        }
    }
}

impl FfprobeScanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tools(ffprobe: Option<PathBuf>, ffmpeg: Option<PathBuf>) -> Self {
        Self { ffprobe, ffmpeg }
    }

    pub fn ffprobe_path(&self) -> Option<&Path> {
        self.ffprobe.as_deref()
    }

    pub fn ffmpeg_path(&self) -> Option<&Path> {
        self.ffmpeg.as_deref()
    }

    fn ffprobe_bin(&self) -> Result<PathBuf, crate::CoreError> {
        self.ffprobe
            .clone()
            .or_else(|| find_tool("ffprobe"))
            .ok_or_else(|| crate::CoreError::MissingTool("ffprobe".into()))
    }

    fn ffmpeg_bin(&self) -> Result<PathBuf, crate::CoreError> {
        self.ffmpeg
            .clone()
            .or_else(|| find_tool("ffmpeg"))
            .ok_or_else(|| crate::CoreError::MissingTool("ffmpeg".into()))
    }

    pub fn scan_path(&self, path: &Path) -> Result<ScanResult, crate::CoreError> {
        self.scan(path)
    }

    pub fn extract_preview(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        time_seconds: f64,
    ) -> Result<(), crate::CoreError> {
        extract_preview_with(
            self.ffmpeg_bin()?,
            input.as_ref(),
            output.as_ref(),
            time_seconds,
        )
    }

    pub fn extract_preview_frame(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        time_seconds: f64,
    ) -> Result<(), crate::CoreError> {
        self.extract_preview(input, output, time_seconds)
    }

    pub fn detect_crop(&self, input: impl AsRef<Path>) -> Result<CropDetection, crate::CoreError> {
        detect_crop_with(self.ffmpeg_bin()?, input.as_ref())
    }

    pub fn crop_detect(&self, input: impl AsRef<Path>) -> Result<CropDetection, crate::CoreError> {
        self.detect_crop(input)
    }
}

pub trait MediaScanner {
    fn scan(&self, path: &Path) -> Result<ScanResult, crate::CoreError>;
}

impl MediaScanner for FfprobeScanner {
    fn scan(&self, path: &Path) -> Result<ScanResult, crate::CoreError> {
        let ffprobe = self.ffprobe_bin()?;
        let output = Command::new(&ffprobe)
            .arg("-v")
            .arg("error")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg("-show_chapters")
            .arg(path)
            .output()
            .map_err(|e| crate::CoreError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(crate::CoreError::InvalidMedia(
                if stderr.trim().is_empty() {
                    "ffprobe failed".into()
                } else {
                    stderr
                },
            ));
        }

        parse_ffprobe_output(&output.stdout, path)
    }
}

// Free functions for convenience / alternate expected call styles.
#[allow(dead_code)]
pub fn scan(path: &Path) -> Result<ScanResult, crate::CoreError> {
    FfprobeScanner::default().scan(path)
}

#[allow(dead_code)]
pub fn extract_preview(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    time_seconds: f64,
) -> Result<(), crate::CoreError> {
    let ffmpeg =
        find_tool("ffmpeg").ok_or_else(|| crate::CoreError::MissingTool("ffmpeg".into()))?;
    extract_preview_with(ffmpeg, input.as_ref(), output.as_ref(), time_seconds)
}

#[allow(dead_code)]
pub fn detect_crop(input: impl AsRef<Path>) -> Result<CropDetection, crate::CoreError> {
    let ffmpeg =
        find_tool("ffmpeg").ok_or_else(|| crate::CoreError::MissingTool("ffmpeg".into()))?;
    detect_crop_with(ffmpeg, input.as_ref())
}

fn extract_preview_with(
    ffmpeg: PathBuf,
    input: &Path,
    output: &Path,
    time_seconds: f64,
) -> Result<(), crate::CoreError> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::CoreError::Io(e.to_string()))?;
    }
    let status = Command::new(&ffmpeg)
        .arg("-y")
        .arg("-ss")
        .arg(format!("{:.3}", time_seconds.max(0.0)))
        .arg("-i")
        .arg(input)
        .arg("-frames:v")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg("-update")
        .arg("1")
        .arg(output)
        .status()
        .map_err(|e| crate::CoreError::CommandFailed(e.to_string()))?;

    if !status.success() || !output.exists() {
        return Err(crate::CoreError::CommandFailed(format!(
            "ffmpeg preview extraction failed (status {:?})",
            status.code()
        )));
    }
    Ok(())
}

fn detect_crop_with(ffmpeg: PathBuf, input: &Path) -> Result<CropDetection, crate::CoreError> {
    let output = Command::new(&ffmpeg)
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg("cropdetect=limit=24:round=2:reset=0")
        .arg("-frames:v")
        .arg("120")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .map_err(|e| crate::CoreError::CommandFailed(e.to_string()))?;

    // cropdetect writes to stderr
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // find last crop= specification
    let mut last: Option<CropDetection> = None;
    for line in text.lines() {
        if let Some(pos) = line.find("crop=") {
            let spec = &line[pos + 5..];
            let spec = spec
                .split_whitespace()
                .next()
                .unwrap_or(spec)
                .trim_end_matches([',', ']']);
            let parts: Vec<&str> = spec.split(':').collect();
            if parts.len() >= 4 {
                if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                    parts[0].parse::<u32>(),
                    parts[1].parse::<u32>(),
                    parts[2].parse::<u32>(),
                    parts[3].parse::<u32>(),
                ) {
                    last = Some(CropDetection {
                        x,
                        y,
                        width: w,
                        height: h,
                    });
                }
            }
        }
    }

    last.ok_or_else(|| crate::CoreError::InvalidMedia("no cropdetect output".into()))
}

// ---------------------------------------------------------------------------
// ffprobe JSON models
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct FfprobeResponse {
    #[serde(default)]
    format: Option<FfprobeFormat>,
    #[serde(default)]
    streams: Option<Vec<FfprobeStream>>,
    #[serde(default)]
    chapters: Option<Vec<FfprobeChapter>>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeFormat {
    #[serde(default)]
    format_name: Option<String>,
    #[serde(default)]
    format_long_name: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    duration: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    size: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    bit_rate: Option<String>,
    #[serde(default)]
    tags: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeStream {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    codec_long_name: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    avg_frame_rate: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    sample_rate: Option<String>,
    #[serde(default)]
    channels: Option<u32>,
    #[serde(default)]
    channel_layout: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    bit_rate: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    duration: Option<String>,
    #[serde(default)]
    tags: Option<BTreeMap<String, String>>,
    /// Per-frame side data, which is where Dolby Vision and HDR10+ travel.
    #[serde(default)]
    side_data_list: Option<Vec<FfprobeSideData>>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeSideData {
    #[serde(default)]
    side_data_type: Option<String>,
    #[serde(default)]
    dv_profile: Option<u8>,
    #[serde(default)]
    rpu_present_flag: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeChapter {
    #[serde(default, deserialize_with = "flex_string")]
    start_time: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    end_time: Option<String>,
    #[serde(default)]
    tags: Option<BTreeMap<String, String>>,
}

fn convert_format(f: FfprobeFormat) -> FormatInfo {
    FormatInfo {
        format_name: f.format_name,
        format_long_name: f.format_long_name,
        duration_seconds: f.duration.as_deref().and_then(parse_f64),
        size_bytes: f.size.as_deref().and_then(parse_u64),
        bit_rate: f.bit_rate.as_deref().and_then(parse_u64),
        tags: f.tags.unwrap_or_default(),
    }
}

fn convert_stream(s: FfprobeStream) -> StreamInfo {
    let kind = match s.codec_type.as_deref() {
        Some("video") => TrackKind::Video,
        Some("audio") => TrackKind::Audio,
        Some("subtitle") => TrackKind::Subtitle,
        _ => TrackKind::Data,
    };
    let (language, title) = if let Some(tags) = &s.tags {
        (tags.get("language").cloned(), tags.get("title").cloned())
    } else {
        (None, None)
    };
    let (dolby_vision_profile, dolby_vision_rpu, hdr10_plus) =
        read_hdr_side_data(s.side_data_list.as_deref());
    StreamInfo {
        index: s.index,
        kind,
        codec: s.codec_name,
        codec_long: s.codec_long_name,
        language,
        title,
        width: s.width,
        height: s.height,
        frame_rate: s.avg_frame_rate,
        sample_rate: s.sample_rate.as_deref().and_then(parse_u32),
        channels: s.channels,
        channel_layout: s.channel_layout,
        bit_rate: s.bit_rate.as_deref().and_then(parse_u64),
        duration_seconds: s.duration.as_deref().and_then(parse_f64),
        dolby_vision_profile,
        dolby_vision_rpu,
        hdr10_plus,
    }
}

/// Read the HDR side data ffprobe reports for a stream.
///
/// Dolby Vision announces itself as a `DOVI configuration record` with a
/// profile and an RPU-present flag; HDR10+ announces itself as SMPTE 2094-40
/// dynamic metadata. Both are side data rather than codec properties, which
/// is why a stream can be plain HEVC and still carry a Dolby Vision RPU.
fn read_hdr_side_data(
    side_data: Option<&[FfprobeSideData]>,
) -> (Option<u8>, bool, bool) {
    let mut profile = None;
    let mut rpu = false;
    let mut hdr10_plus = false;
    for entry in side_data.unwrap_or(&[]) {
        let kind = entry.side_data_type.as_deref().unwrap_or("");
        if kind.contains("DOVI") || kind.contains("Dolby Vision") {
            profile = entry.dv_profile.or(profile);
            rpu |= entry.rpu_present_flag.unwrap_or(0) != 0;
        } else if kind.contains("2094-40") || kind.contains("HDR10+") {
            hdr10_plus = true;
        }
    }
    (profile, rpu, hdr10_plus)
}

fn convert_chapter(idx: usize, c: FfprobeChapter) -> ChapterInfo {
    let title = c.tags.as_ref().and_then(|t| t.get("title").cloned());
    ChapterInfo {
        index: idx,
        start_seconds: c.start_time.as_deref().and_then(parse_f64),
        end_seconds: c.end_time.as_deref().and_then(parse_f64),
        title,
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

fn parse_u32(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.trim().parse::<u64>().ok()
}

fn flex_string<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_json::Value> = Option::deserialize(de)?;
    Ok(v.map(|x| match x {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }))
}

pub fn parse_ffprobe_output(bytes: &[u8], path: &Path) -> Result<ScanResult, crate::CoreError> {
    let parsed: FfprobeResponse =
        serde_json::from_slice(bytes).map_err(|e| crate::CoreError::Parse(e.to_string()))?;

    let mut streams = Vec::new();
    if let Some(raw_streams) = parsed.streams {
        for s in raw_streams {
            streams.push(convert_stream(s));
        }
    }

    let mut chapters = Vec::new();
    if let Some(raw_chapters) = parsed.chapters {
        for (idx, c) in raw_chapters.into_iter().enumerate() {
            chapters.push(convert_chapter(idx, c));
        }
    }

    let format = parsed.format.map(convert_format).unwrap_or_default();

    let title = TitleInfo {
        index: 0,
        duration_seconds: format.duration_seconds,
        chapters: chapters.clone(),
        stream_indices: (0..streams.len()).collect(),
    };

    Ok(ScanResult {
        path: path.to_path_buf(),
        format,
        streams,
        chapters,
        titles: vec![title],
    })
}

impl ScanResult {
    /// The first video stream's Dolby Vision profile and RPU presence, when
    /// the source carries them.
    pub fn dolby_vision(&self) -> Option<(u8, bool)> {
        self.streams
            .iter()
            .filter(|stream| stream.kind == TrackKind::Video)
            .find_map(|stream| stream.dolby_vision_profile.map(|profile| (profile, stream.dolby_vision_rpu)))
    }

    /// Whether any stream carries HDR10+ dynamic metadata.
    pub fn has_hdr10_plus(&self) -> bool {
        self.streams.iter().any(|stream| stream.hdr10_plus)
    }
}

// ---------------------------------------------------------------------------
// tool discovery
// ---------------------------------------------------------------------------

fn find_tool(name: &str) -> Option<PathBuf> {
    // PATH
    if let Some(path_env) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join(format!("{}.exe", name));
            if candidate.is_file() {
                return Some(candidate);
            }
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // WinGet fallback
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(local)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("Gyan.FFmpeg"))
                        .unwrap_or(false)
                    {
                        if let Ok(inner) = std::fs::read_dir(&path) {
                            for e2 in inner.flatten() {
                                let bin = e2.path().join("bin").join(format!("{}.exe", name));
                                if bin.is_file() {
                                    return Some(bin);
                                }
                            }
                        }
                    }
                }
            }
        }
        // hermes
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let hermes = PathBuf::from(local).join("hermes").join("tools");
            if let Ok(entries) = std::fs::read_dir(&hermes) {
                for e in entries.flatten() {
                    let bin = e.path().join("bin").join(format!("{}.exe", name));
                    if bin.is_file() {
                        return Some(bin);
                    }
                }
            }
        }
    }
    None
}
