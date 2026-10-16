//! View models the desktop interface renders.
//!
//! Everything the window draws is assembled here, on the Rust side, from the
//! worker contract types. The page receives finished, plain objects and holds
//! no media logic of its own, which keeps the formatting rules that a user
//! actually sees — durations, resolutions, language labels, range validity —
//! covered by ordinary unit tests.
//!
//! The models answer three questions the source area asks: what is being
//! scanned, which titles did the scan find, and what is selected inside the
//! chosen title (points, angles, and tracks).

use std::fmt;

use serde::{Deserialize, Serialize};
use vise_ipc::contract::{
    ScanAudioTrack, ScanChapter, ScanSubtitleTrack, ScanTitle, TitlesResult, WorkState,
};

use crate::sources::SourceViewModel;

/// Where a scan currently stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScanPhase {
    /// No source has been scanned yet.
    #[default]
    Idle,
    /// A source was accepted and the worker is probing it.
    Scanning,
    /// Titles are available.
    Ready,
    /// The scan finished with a problem.
    Failed,
}

impl ScanPhase {
    /// Stable identifier used by the interface layer.
    pub fn as_str(self) -> &'static str {
        match self {
            ScanPhase::Idle => "idle",
            ScanPhase::Scanning => "scanning",
            ScanPhase::Ready => "ready",
            ScanPhase::Failed => "failed",
        }
    }
}

/// What part of a title an encode should cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RangeMode {
    /// From the first frame to the last.
    #[default]
    Whole,
    /// A span aligned to chapter boundaries.
    Chapters,
    /// An explicit start and end point.
    PointToPoint,
}

impl RangeMode {
    /// Every mode, in the order the picker lists them.
    pub fn all() -> [RangeMode; 3] {
        [
            RangeMode::Whole,
            RangeMode::Chapters,
            RangeMode::PointToPoint,
        ]
    }

    /// Stable identifier used by the interface layer.
    pub fn as_str(self) -> &'static str {
        match self {
            RangeMode::Whole => "whole",
            RangeMode::Chapters => "chapters",
            RangeMode::PointToPoint => "point_to_point",
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            RangeMode::Whole => "Whole title",
            RangeMode::Chapters => "Chapters",
            RangeMode::PointToPoint => "Point to point",
        }
    }

    /// Whether this mode needs chapter boundaries to exist.
    pub fn needs_chapters(self) -> bool {
        matches!(self, RangeMode::Chapters)
    }

    /// The mode named by `text`, if it is one this build offers.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match wanted.as_str() {
            "whole" | "whole_title" | "entire" => Some(RangeMode::Whole),
            "chapters" | "chapter" => Some(RangeMode::Chapters),
            "point_to_point" | "pointtopoint" | "points" => Some(RangeMode::PointToPoint),
            _ => None,
        }
    }
}

/// Which kind of stream a track view describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKindTag {
    Audio,
    Subtitle,
}

/// Scan progress, shaped for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanProgressViewModel {
    /// Fraction complete, 0.0 to 1.0.
    pub fraction: f64,
    /// Progress as a whole percentage, clamped to 0-100.
    pub percent: u32,
    /// Whether a progress bar is worth showing at all.
    pub visible: bool,
    /// Sources already probed.
    pub completed: usize,
    /// Sources submitted.
    pub total: usize,
    /// Name of the source being probed.
    pub current_file: String,
    /// Titles found so far.
    pub title_count: usize,
    /// One line summarising the progress.
    pub status_line: String,
}

/// One scanned title, as the title list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleViewModel {
    /// Reported title index, counting from one.
    pub index: u32,
    /// Title name reported by the scan.
    pub name: String,
    /// `Title 2 · 1:42:10 · main feature`.
    pub label: String,
    /// `1920x1080 · matroska · 1:42:10 · 2 audio · 1 subtitle · 12 chapters`.
    pub facts: String,
    /// Duration in seconds.
    pub duration_seconds: f64,
    /// Duration as `h:mm:ss`.
    pub duration_label: String,
    /// Frame size as `1920x1080`, or a placeholder when unknown.
    pub resolution_label: String,
    /// Container name.
    pub container: String,
    /// Video codec name.
    pub video_codec: String,
    /// Frame rate as `23.976 fps`, or a placeholder when unknown.
    pub frame_rate_label: String,
    /// How many audio tracks the title carries.
    pub audio_count: usize,
    /// How many subtitle tracks the title carries.
    pub subtitle_count: usize,
    /// How many chapters the title carries.
    pub chapter_count: usize,
    /// How many angles the source exposes for this title.
    pub angle_count: u32,
    /// Whether the scan marked this title as the main feature.
    pub is_main_feature: bool,
}

/// One audio or subtitle track inside a title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackViewModel {
    /// Audio or subtitle.
    pub kind: TrackKindTag,
    /// Track number inside the title, counting from one.
    pub number: u32,
    /// Language as reported, human-readable when known.
    pub language: String,
    /// Language code as reported, `und` when unknown.
    pub language_code: String,
    /// Codec name.
    pub codec: String,
    /// Codec and shape, such as `E-AC3 · 5.1`.
    pub detail: String,
    /// Track number, language, and codec on one line.
    pub label: String,
    /// Whether the user chose this track.
    pub selected: bool,
}

/// Chapter boundaries offered as range points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterViewModel {
    /// Chapter number, counting from one.
    pub index: u32,
    /// Chapter title, when the source names one.
    pub name: String,
    /// Start in seconds.
    pub start_seconds: f64,
    /// End in seconds.
    pub end_seconds: f64,
    /// `Chapter 2 · 00:12:34 – 00:45:00`.
    pub label: String,
    /// `00:12:34 – 00:45:00`, used by the point pickers.
    pub span_label: String,
}

/// One selectable camera angle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AngleViewModel {
    /// Angle number, counting from one.
    pub index: u32,
    /// `Angle 2`.
    pub label: String,
}

/// One selectable range point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangePointViewModel {
    /// Point position in seconds.
    pub seconds: f64,
    /// `00:12:34`, or a chapter name when the point is a chapter start.
    pub label: String,
}

/// One selectable range mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeModeViewModel {
    /// Stable identifier used by the interface layer.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the selected title can use this mode.
    pub available: bool,
    /// Why the mode cannot be used, when it cannot.
    pub reason: String,
}

/// The range and angle selection for one title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeViewModel {
    /// Selected mode.
    pub mode: RangeMode,
    /// Every mode with its availability for this title.
    pub modes: Vec<RangeModeViewModel>,
    /// Whether the title has chapter boundaries.
    pub has_chapters: bool,
    /// Whether the source exposes more than one angle.
    pub has_angles: bool,
    /// Selectable angles.
    pub angles: Vec<AngleViewModel>,
    /// Selected angle, counting from one.
    pub angle: u32,
    /// Resolved start in seconds.
    pub start_seconds: f64,
    /// Resolved end in seconds.
    pub end_seconds: f64,
    /// Resolved start as `h:mm:ss`.
    pub start_label: String,
    /// Resolved end as `h:mm:ss`.
    pub end_label: String,
    /// Resolved span as `h:mm:ss`.
    pub duration_label: String,
    /// Points the start and end pickers offer.
    pub points: Vec<RangePointViewModel>,
    /// Chapters inside the title.
    pub chapters: Vec<ChapterViewModel>,
    /// Whether the selection can be used.
    pub valid: bool,
    /// Why the selection cannot be used, when it cannot.
    pub problem: String,
}

impl RangeViewModel {
    fn unusable(mode: RangeMode, problem: impl Into<String>) -> Self {
        let problem = problem.into();
        Self {
            mode,
            modes: mode_choices(&problem),
            has_chapters: false,
            has_angles: false,
            angles: Vec::new(),
            angle: 1,
            start_seconds: 0.0,
            end_seconds: 0.0,
            start_label: format_duration(0.0),
            end_label: format_duration(0.0),
            duration_label: format_duration(0.0),
            points: Vec::new(),
            chapters: Vec::new(),
            valid: false,
            problem,
        }
    }
}

/// One entry per mode, with why a mode cannot be used for this title.
fn mode_choices(chapters_reason: &str) -> Vec<RangeModeViewModel> {
    RangeMode::all()
        .into_iter()
        .map(|mode| {
            let available = !mode.needs_chapters() || chapters_reason.is_empty();
            RangeModeViewModel {
                id: mode.as_str().to_string(),
                label: mode.label().to_string(),
                available,
                reason: if available {
                    String::new()
                } else {
                    chapters_reason.to_string()
                },
            }
        })
        .collect()
}

/// Audio and subtitle tracks of the selected title, with the user's choice
/// marked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackSelectionViewModel {
    /// Audio tracks available for the title.
    pub audio: Vec<TrackViewModel>,
    /// Subtitle tracks available for the title.
    pub subtitle: Vec<TrackViewModel>,
    /// Chosen audio track numbers.
    pub selected_audio: Vec<u32>,
    /// Chosen subtitle track numbers.
    pub selected_subtitle: Vec<u32>,
}

/// The full source area state the window renders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SelectionViewModel {
    /// Source currently loaded, when one is.
    pub source: Option<SourceViewModel>,
    /// Where the scan stands.
    pub phase: ScanPhase,
    /// Stable identifier for the phase.
    pub phase_name: String,
    /// Scan progress, when a scan ran.
    pub progress: Option<ScanProgressViewModel>,
    /// Titles found by the last scan.
    pub titles: Vec<TitleViewModel>,
    /// Index the scan marked as the main feature; zero when there is none.
    pub main_title_index: u32,
    /// Title the user chose; zero when none is chosen.
    pub selected_title_index: u32,
    /// Tracks of the chosen title.
    pub tracks: TrackSelectionViewModel,
    /// Range and angle selection for the chosen title.
    pub range: Option<RangeViewModel>,
    /// Why the scan or the current selection cannot be used.
    pub problem: String,
}

impl SelectionViewModel {
    /// Whether the window can queue work from this state.
    pub fn is_usable(&self) -> bool {
        self.phase == ScanPhase::Ready && self.selected_title_index > 0
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format a duration as `h:mm:ss`, or `m:ss` below an hour.
pub fn format_duration(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds.round() as u64
    } else {
        0
    };
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// Format a frame size, with a placeholder when the scan reported none.
pub fn format_resolution(width: u32, height: u32) -> String {
    if width == 0 || height == 0 {
        "unknown size".to_string()
    } else {
        format!("{width}x{height}")
    }
}

/// Format a frame rate, with a placeholder when the scan reported none.
pub fn format_frame_rate(rate: f64) -> String {
    if !rate.is_finite() || rate <= 0.0 {
        "unknown frame rate".to_string()
    } else if (rate - rate.round()).abs() < 0.01 {
        format!("{} fps", rate.round() as u32)
    } else {
        format!("{rate:.3} fps")
    }
}

/// Describe an audio channel layout, falling back to a channel count.
pub fn format_channels(channel_count: u32, layout: &str) -> String {
    let layout = layout.trim();
    if !layout.is_empty() && !layout.eq_ignore_ascii_case("unknown") {
        layout.to_string()
    } else if channel_count > 0 {
        format!("{channel_count} channels")
    } else {
        "unknown layout".to_string()
    }
}

/// Describe a bit rate in kbit/s, with a placeholder when unknown.
pub fn format_bit_rate(bit_rate: u64) -> String {
    if bit_rate == 0 {
        "unknown bit rate".to_string()
    } else {
        format!("{} kb/s", bit_rate / 1000)
    }
}

/// Describe a sample rate in kHz, with a placeholder when unknown.
pub fn format_sample_rate(sample_rate: u32) -> String {
    if sample_rate == 0 {
        "unknown sample rate".to_string()
    } else {
        format!("{} kHz", sample_rate as f64 / 1000.0)
    }
}

/// Language to show for a track, falling back to the code.
fn language_label(language: &str, code: &str) -> String {
    let language = language.trim();
    if !language.is_empty() && !language.eq_ignore_ascii_case("und") {
        language.to_string()
    } else if !code.trim().is_empty() && !code.trim().eq_ignore_ascii_case("und") {
        code.trim().to_string()
    } else {
        "Undetermined".to_string()
    }
}

// ---------------------------------------------------------------------------
// Projections
// ---------------------------------------------------------------------------

/// Chapter view for one chapter.
pub fn chapter_view(chapter: &ScanChapter) -> ChapterViewModel {
    let name = chapter.title.trim().to_string();
    let span_label = format!(
        "{} – {}",
        format_duration(chapter.start_seconds),
        format_duration(chapter.end_seconds)
    );
    let label = if name.is_empty() {
        format!("Chapter {} · {span_label}", chapter.index + 1)
    } else {
        format!("Chapter {} · {name} · {span_label}", chapter.index + 1)
    };
    ChapterViewModel {
        index: chapter.index + 1,
        name,
        start_seconds: chapter.start_seconds,
        end_seconds: chapter.end_seconds,
        label,
        span_label,
    }
}

/// Every chapter of a title, as view models.
pub fn chapter_views(chapters: &[ScanChapter]) -> Vec<ChapterViewModel> {
    chapters.iter().map(chapter_view).collect()
}

/// One title, as the title list shows it.
pub fn title_view(title: &ScanTitle, main_feature: u32) -> TitleViewModel {
    let name = title.name.trim().to_string();
    let name = if name.is_empty() {
        format!("Title {}", title.index)
    } else {
        name
    };
    let is_main_feature = main_feature != 0 && title.index == main_feature;
    let index_text = format!("Title {}", title.index);
    let label = match (title.name.trim().is_empty(), is_main_feature) {
        (true, true) => format!("{index_text} · main feature"),
        (true, false) => index_text.clone(),
        (false, true) => format!("{index_text} · {name} · main feature"),
        (false, false) => format!("{index_text} · {name}"),
    };
    let duration_label = format_duration(title.duration_seconds);
    let resolution_label = format_resolution(title.width, title.height);
    let container = if title.container.trim().is_empty() {
        "unknown container".to_string()
    } else {
        title.container.clone()
    };
    let chapter_count = title.chapter_list.len();
    let facts = format!(
        "{resolution_label} · {container} · {duration_label} · {} audio · {} subtitle{}",
        title.audio_list.len(),
        title.subtitle_list.len(),
        if chapter_count > 0 {
            format!(" · {chapter_count} chapters")
        } else {
            String::new()
        }
    );
    TitleViewModel {
        index: title.index,
        name,
        label,
        facts,
        duration_seconds: title.duration_seconds,
        duration_label,
        resolution_label,
        container,
        video_codec: if title.video_codec.trim().is_empty() {
            "unknown".to_string()
        } else {
            title.video_codec.clone()
        },
        frame_rate_label: format_frame_rate(title.frame_rate),
        audio_count: title.audio_list.len(),
        subtitle_count: title.subtitle_list.len(),
        chapter_count,
        angle_count: title.angle_count.max(1),
        is_main_feature,
    }
}

/// Every title of a scan, as the title list shows them.
pub fn title_views(titles: &TitlesResult) -> Vec<TitleViewModel> {
    titles
        .title_list
        .iter()
        .map(|title| title_view(title, titles.main_feature))
        .collect()
}

/// One audio track, as the track list shows it.
pub fn audio_track_view(track: &ScanAudioTrack, selected: &[u32]) -> TrackViewModel {
    let language = language_label(&track.language, &track.language_code);
    let codec = if track.codec_name.trim().is_empty() {
        "unknown codec".to_string()
    } else {
        track.codec_name.clone()
    };
    let detail = format!(
        "{codec} · {} · {} · {}",
        format_channels(track.channel_count, &track.channel_layout),
        format_sample_rate(track.sample_rate),
        format_bit_rate(track.bit_rate)
    );
    let label = format!(
        "Audio {} · {} ({}) · {}",
        track.track_number,
        language,
        track.language_code.trim(),
        detail
    );
    TrackViewModel {
        kind: TrackKindTag::Audio,
        number: track.track_number,
        language,
        language_code: track.language_code.trim().to_string(),
        codec,
        detail,
        label,
        selected: selected.contains(&track.track_number),
    }
}

/// One subtitle track, as the track list shows it.
pub fn subtitle_track_view(track: &ScanSubtitleTrack, selected: &[u32]) -> TrackViewModel {
    let language = language_label(&track.language, &track.language_code);
    let codec = if track.codec_name.trim().is_empty() {
        "unknown codec".to_string()
    } else {
        track.codec_name.trim().to_string()
    };
    let mut detail = codec.clone();
    if !track.description.trim().is_empty() {
        detail = format!("{codec} · {}", track.description.trim());
    }
    let label = format!(
        "Subtitle {} · {} ({}) · {}",
        track.track_number,
        language,
        track.language_code.trim(),
        detail
    );
    TrackViewModel {
        kind: TrackKindTag::Subtitle,
        number: track.track_number,
        language,
        language_code: track.language_code.trim().to_string(),
        codec,
        detail,
        label,
        selected: selected.contains(&track.track_number),
    }
}

/// The tracks of one title, with the user's choice marked.
///
/// The default is the first audio track and no subtitle tracks, which is what
/// a user expects before touching anything.
pub fn track_selection(
    title: Option<&ScanTitle>,
    selected_audio: &[u32],
    selected_subtitle: &[u32],
) -> TrackSelectionViewModel {
    let Some(title) = title else {
        return TrackSelectionViewModel::default();
    };
    TrackSelectionViewModel {
        audio: title
            .audio_list
            .iter()
            .map(|track| audio_track_view(track, selected_audio))
            .collect(),
        subtitle: title
            .subtitle_list
            .iter()
            .map(|track| subtitle_track_view(track, selected_subtitle))
            .collect(),
        selected_audio: selected_audio.to_vec(),
        selected_subtitle: selected_subtitle.to_vec(),
    }
}

/// The audio track numbers that are selected, defaulting to the first one.
pub fn default_audio_selection(title: &ScanTitle) -> Vec<u32> {
    title
        .audio_list
        .first()
        .map(|track| vec![track.track_number])
        .unwrap_or_default()
}

/// Range and angle selection for one title.
///
/// The resolution happens here rather than in the interface so that an
/// impossible selection is refused with a reason the user can act on, instead
/// of being discovered later by the encoder.
pub fn range_view(title: &ScanTitle, selection: &RangeSelection) -> RangeViewModel {
    let duration = title.duration_seconds.max(0.0);
    let chapters = chapter_views(&title.chapter_list);
    let has_chapters = !chapters.is_empty();
    let angle_count = title.angle_count.max(1);
    let angles: Vec<AngleViewModel> = (1..=angle_count)
        .map(|index| AngleViewModel {
            index,
            label: format!("Angle {index}"),
        })
        .collect();

    let mode = selection.mode;
    let chapter_reason = if has_chapters {
        String::new()
    } else {
        "This title has no chapters to align to.".to_string()
    };

    let (start, end, problem) = match mode {
        RangeMode::Whole => (0.0, duration, String::new()),
        RangeMode::Chapters => {
            if !has_chapters {
                (0.0, duration, chapter_reason.clone())
            } else {
                match (selection.start_seconds, selection.end_seconds) {
                    (Some(start), Some(end)) if valid_span(start, end, duration) => {
                        (start, end, String::new())
                    }
                    (Some(_), Some(_)) => (
                        0.0,
                        duration,
                        "Choose chapter boundaries inside the title.".to_string(),
                    ),
                    // Nothing chosen yet means the whole title, which is a
                    // usable starting point rather than a problem.
                    _ => (0.0, duration, String::new()),
                }
            }
        }
        RangeMode::PointToPoint => match (selection.start_seconds, selection.end_seconds) {
            (Some(start), Some(end)) if valid_span(start, end, duration) => {
                (start, end, String::new())
            }
            _ => (
                0.0,
                duration,
                "Choose a start point before the end point.".to_string(),
            ),
        },
    };

    let angle = match selection.angle {
        Some(angle) if angle >= 1 && angle <= angle_count => angle,
        Some(_) => {
            return RangeViewModel::unusable(
                mode,
                format!("This title has {angle_count} angle(s)."),
            )
        }
        None => 1,
    };

    let mut points: Vec<RangePointViewModel> = vec![RangePointViewModel {
        seconds: 0.0,
        label: format_duration(0.0),
    }];
    for chapter in &chapters {
        // A chapter that starts where the previous point already sits adds
        // nothing to the picker, so it is not offered twice.
        if points
            .last()
            .is_some_and(|point| (point.seconds - chapter.start_seconds).abs() < 0.001)
        {
            continue;
        }
        points.push(RangePointViewModel {
            seconds: chapter.start_seconds,
            label: if chapter.name.is_empty() {
                format_duration(chapter.start_seconds)
            } else {
                format!(
                    "{} · {}",
                    format_duration(chapter.start_seconds),
                    chapter.name
                )
            },
        });
    }
    if duration > 0.0
        && points
            .last()
            .is_none_or(|point| (point.seconds - duration).abs() >= 0.001)
    {
        points.push(RangePointViewModel {
            seconds: duration,
            label: format_duration(duration),
        });
    }

    RangeViewModel {
        mode,
        modes: mode_choices(&chapter_reason),
        has_chapters,
        has_angles: angle_count > 1,
        angles,
        angle,
        start_seconds: start,
        end_seconds: end,
        start_label: format_duration(start),
        end_label: format_duration(end),
        duration_label: format_duration((end - start).max(0.0)),
        points,
        chapters,
        valid: problem.is_empty(),
        problem,
    }
}

/// Whether `start` and `end` describe a usable span inside `duration`.
fn valid_span(start: f64, end: f64, duration: f64) -> bool {
    start.is_finite() && end.is_finite() && start >= 0.0 && end > start && end <= duration + 0.5
}

/// Range and angle selection as the interface layer sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RangeSelection {
    /// Selected mode.
    pub mode: RangeMode,
    /// Explicit start, when the mode needs one.
    pub start_seconds: Option<f64>,
    /// Explicit end, when the mode needs one.
    pub end_seconds: Option<f64>,
    /// Explicit angle, when the source exposes angles.
    pub angle: Option<u32>,
}

impl RangeSelection {
    /// Validate the mode name that arrived from the interface layer.
    pub fn with_mode_name(mut self, name: &str) -> Result<Self, String> {
        self.mode = RangeMode::parse(name)
            .ok_or_else(|| format!("'{}' is not a range mode this build offers.", name.trim()))?;
        Ok(self)
    }
}

/// Tracks the user chose, as they arrived from the interface layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackSelectionInput {
    /// Chosen audio track numbers.
    pub audio: Vec<u32>,
    /// Chosen subtitle track numbers.
    pub subtitle: Vec<u32>,
}

impl TrackSelectionInput {
    /// Keep the chosen tracks only when the title actually has them.
    pub fn reconcile(self, title: &ScanTitle) -> Self {
        let audio: Vec<u32> = self
            .audio
            .into_iter()
            .filter(|number| title.audio_list.iter().any(|t| t.track_number == *number))
            .collect();
        let subtitle: Vec<u32> = self
            .subtitle
            .into_iter()
            .filter(|number| {
                title
                    .subtitle_list
                    .iter()
                    .any(|t| t.track_number == *number)
            })
            .collect();
        Self {
            audio: dedupe_sorted(audio),
            subtitle: dedupe_sorted(subtitle),
        }
    }
}

fn dedupe_sorted(mut numbers: Vec<u32>) -> Vec<u32> {
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

/// Map a worker progress report onto the phases the interface shows.
pub fn phase_of(state: WorkState, successful: bool) -> ScanPhase {
    match state {
        WorkState::Unknown => ScanPhase::Idle,
        WorkState::Working => ScanPhase::Scanning,
        // A finished scan is only ready once the titles have arrived.
        WorkState::ScanDone | WorkState::WorkDone => {
            if successful {
                ScanPhase::Ready
            } else {
                ScanPhase::Failed
            }
        }
    }
}

/// Build the progress view from a worker progress report.
pub fn progress_view(scan: Option<&vise_ipc::contract::ScanProgress>) -> ScanProgressViewModel {
    match scan {
        None => ScanProgressViewModel::default(),
        Some(scan) => {
            let current_file = path_display_name(&scan.current_path);
            let status_line = if scan.total == 0 {
                "Starting scan…".to_string()
            } else {
                let done = scan.completed.min(scan.total);
                let shown = if done >= scan.total {
                    scan.total
                } else {
                    done + 1
                };
                format!(
                    "Scanning {shown} of {} · {current_file}{}",
                    scan.total,
                    if scan.title_count > 0 {
                        format!(" · {} title(s) so far", scan.title_count)
                    } else {
                        String::new()
                    }
                )
            };
            ScanProgressViewModel {
                fraction: scan.progress.clamp(0.0, 1.0),
                percent: (scan.progress.clamp(0.0, 1.0) * 100.0).round() as u32,
                visible: scan.total > 0,
                completed: scan.completed,
                total: scan.total,
                current_file,
                title_count: scan.title_count,
                status_line,
            }
        }
    }
}

/// Last path component of a path string, for compact progress text.
fn path_display_name(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    match trimmed.rsplit(['/', '\\']).next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => trimmed.to_string(),
    }
}

impl fmt::Display for ScanPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(track_number: u32, language: &str, code: &str) -> ScanAudioTrack {
        ScanAudioTrack {
            track_number,
            codec_name: "eac3".into(),
            channel_count: 6,
            channel_layout: "5.1".into(),
            sample_rate: 48_000,
            bit_rate: 640_000,
            language: language.into(),
            language_code: code.into(),
            description: String::new(),
        }
    }

    fn subtitle(
        track_number: u32,
        language: &str,
        code: &str,
        description: &str,
    ) -> ScanSubtitleTrack {
        ScanSubtitleTrack {
            track_number,
            codec_name: "subrip".into(),
            language: language.into(),
            language_code: code.into(),
            description: description.into(),
        }
    }

    fn chapter(index: u32, start: f64, end: f64, title: &str) -> ScanChapter {
        ScanChapter {
            index,
            start_seconds: start,
            end_seconds: end,
            title: title.into(),
        }
    }

    fn title() -> ScanTitle {
        ScanTitle {
            index: 2,
            name: "Feature".into(),
            container: "matroska".into(),
            duration_seconds: 3600.0,
            video_codec: "hevc".into(),
            width: 1920,
            height: 1080,
            frame_rate: 23.976,
            angle_count: 3,
            audio_list: vec![
                audio(1, "English", "eng"),
                audio(2, "German", "deu"),
                audio(3, "Undetermined", "und"),
            ],
            subtitle_list: vec![
                subtitle(1, "English", "eng", "Full"),
                subtitle(2, "Forced", "eng", "Forced only"),
            ],
            chapter_list: vec![
                chapter(0, 0.0, 600.0, "Opening"),
                chapter(1, 600.0, 1800.0, "Middle"),
                chapter(2, 1800.0, 3600.0, "End"),
            ],
            ..ScanTitle::default()
        }
    }

    #[test]
    fn formats_durations_resolutions_and_rates() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(59.0), "0:59");
        assert_eq!(format_duration(3661.0), "1:01:01");
        assert_eq!(format_duration(-5.0), "0:00");
        assert_eq!(format_duration(f64::NAN), "0:00");
        assert_eq!(format_resolution(1920, 1080), "1920x1080");
        assert_eq!(format_resolution(0, 0), "unknown size");
        assert_eq!(format_frame_rate(25.0), "25 fps");
        assert_eq!(format_frame_rate(23.976), "23.976 fps");
        assert_eq!(format_frame_rate(0.0), "unknown frame rate");
        assert_eq!(format_channels(6, "5.1"), "5.1");
        assert_eq!(format_channels(6, "unknown"), "6 channels");
        assert_eq!(format_bit_rate(640_000), "640 kb/s");
        assert_eq!(format_bit_rate(0), "unknown bit rate");
        assert_eq!(format_sample_rate(48_000), "48 kHz");
    }

    #[test]
    fn title_view_reports_duration_resolution_and_counts() {
        let view = title_view(&title(), 2);
        assert_eq!(view.index, 2);
        assert_eq!(view.label, "Title 2 · Feature · main feature");
        assert_eq!(view.duration_label, "1:00:00");
        assert_eq!(view.resolution_label, "1920x1080");
        assert_eq!(view.container, "matroska");
        assert_eq!(view.frame_rate_label, "23.976 fps");
        assert_eq!(view.audio_count, 3);
        assert_eq!(view.subtitle_count, 2);
        assert_eq!(view.chapter_count, 3);
        assert_eq!(view.angle_count, 3);
        assert!(view.is_main_feature);
        assert!(view.facts.contains("1920x1080"));
        assert!(view.facts.contains("3 chapters"));
    }

    #[test]
    fn title_view_falls_back_when_the_scan_reports_nothing() {
        let bare = ScanTitle {
            index: 1,
            name: "   ".into(),
            duration_seconds: 0.0,
            ..ScanTitle::default()
        };
        let view = title_view(&bare, 0);
        assert_eq!(view.name, "Title 1");
        assert_eq!(view.label, "Title 1");
        assert_eq!(view.duration_label, "0:00");
        assert_eq!(view.resolution_label, "unknown size");
        assert_eq!(view.container, "unknown container");
        assert_eq!(view.angle_count, 1);
        assert!(!view.is_main_feature);
        assert!(!view.facts.contains("chapters"));
    }

    #[test]
    fn title_list_marks_only_the_main_feature() {
        let mut first = title();
        first.index = 1;
        let result = TitlesResult {
            main_feature: 2,
            title_list: vec![first, title()],
        };
        let views = title_views(&result);
        assert_eq!(views.len(), 2);
        assert!(!views[0].is_main_feature);
        assert!(views[1].is_main_feature);
    }

    #[test]
    fn track_projection_shows_language_and_codec() {
        let chosen = vec![2];
        let tracks = track_selection(Some(&title()), &chosen, &[]);
        assert_eq!(tracks.audio.len(), 3);
        assert_eq!(
            tracks.audio[0].label,
            "Audio 1 · English (eng) · eac3 · 5.1 · 48 kHz · 640 kb/s"
        );
        assert!(!tracks.audio[0].selected);
        assert!(tracks.audio[1].selected);
        assert_eq!(tracks.audio[2].language, "Undetermined");
        assert_eq!(tracks.audio[2].detail, "eac3 · 5.1 · 48 kHz · 640 kb/s");
        assert_eq!(
            tracks.subtitle[1].label,
            "Subtitle 2 · Forced (eng) · subrip · Forced only"
        );
        assert_eq!(tracks.selected_audio, vec![2]);
    }

    #[test]
    fn track_projection_without_a_title_is_empty() {
        let tracks = track_selection(None, &[1], &[1]);
        assert!(tracks.audio.is_empty());
        assert!(tracks.subtitle.is_empty());
    }

    #[test]
    fn default_audio_selection_picks_the_first_track() {
        assert_eq!(default_audio_selection(&title()), vec![1]);
        let silent = ScanTitle::default();
        assert!(default_audio_selection(&silent).is_empty());
    }

    #[test]
    fn track_input_drops_tracks_the_title_does_not_have() {
        let input = TrackSelectionInput {
            audio: vec![3, 1, 1, 9],
            subtitle: vec![2, 5],
        };
        let reconciled = input.reconcile(&title());
        assert_eq!(reconciled.audio, vec![1, 3]);
        assert_eq!(reconciled.subtitle, vec![2]);
    }

    #[test]
    fn range_modes_are_parsed_and_described() {
        assert_eq!(RangeMode::parse("Whole"), Some(RangeMode::Whole));
        assert_eq!(
            RangeMode::parse("point-to-point"),
            Some(RangeMode::PointToPoint)
        );
        assert_eq!(RangeMode::parse(" chapters "), Some(RangeMode::Chapters));
        assert_eq!(RangeMode::parse("nonsense"), None);
        assert!(RangeMode::Chapters.needs_chapters());
        assert!(!RangeMode::Whole.needs_chapters());
        assert!(RangeSelection::default()
            .with_mode_name("chapters")
            .expect("mode")
            .mode
            .needs_chapters());
        assert!(RangeSelection::default()
            .with_mode_name("sideways")
            .is_err());
    }

    #[test]
    fn whole_range_covers_the_title() {
        let view = range_view(&title(), &RangeSelection::default());
        assert!(view.valid);
        assert_eq!(view.start_seconds, 0.0);
        assert_eq!(view.end_seconds, 3600.0);
        assert_eq!(view.duration_label, "1:00:00");
        assert_eq!(view.angle, 1);
        assert!(view.has_angles);
        assert_eq!(view.angles.len(), 3);
        assert_eq!(view.modes.len(), 3);
        assert!(view.modes.iter().all(|mode| mode.available));
        // Points are the distinct chapter boundaries plus the title edges; the first
        // chapter starts at zero, so it does not add a second point there.
        assert_eq!(view.points.len(), 4);
        assert_eq!(view.points[1].label, "10:00 · Middle");
        assert_eq!(view.points.last().map(|p| p.seconds), Some(3600.0));
        assert_eq!(view.chapters.len(), 3);
        assert_eq!(view.chapters[1].label, "Chapter 2 · Middle · 10:00 – 30:00");
    }

    #[test]
    fn chapter_range_uses_the_chosen_boundaries() {
        let selection = RangeSelection {
            mode: RangeMode::Chapters,
            start_seconds: Some(600.0),
            end_seconds: Some(1800.0),
            angle: None,
        };
        let view = range_view(&title(), &selection);
        assert!(view.valid);
        assert_eq!(view.start_label, "10:00");
        assert_eq!(view.end_label, "30:00");
        assert_eq!(view.duration_label, "20:00");
    }

    #[test]
    fn chapter_range_without_chapters_is_refused_with_a_reason() {
        let bare = ScanTitle {
            index: 1,
            duration_seconds: 120.0,
            ..ScanTitle::default()
        };
        let selection = RangeSelection {
            mode: RangeMode::Chapters,
            ..Default::default()
        };
        let view = range_view(&bare, &selection);
        assert!(!view.valid);
        assert!(view.problem.contains("no chapters"));
        assert_eq!(view.points.len(), 2, "only the title edges are offered");
        let chapters_mode = view
            .modes
            .iter()
            .find(|mode| mode.id == RangeMode::Chapters.as_str())
            .expect("chapters entry");
        assert!(!chapters_mode.available);
        assert!(chapters_mode.reason.contains("no chapters"));
        assert!(
            view.modes
                .iter()
                .any(|mode| mode.available && mode.id == "point_to_point"),
            "point to point stays available without chapters"
        );
    }

    #[test]
    fn point_to_point_range_requires_an_ordered_span() {
        let missing = RangeSelection {
            mode: RangeMode::PointToPoint,
            ..Default::default()
        };
        assert!(!range_view(&title(), &missing).valid);

        let reversed = RangeSelection {
            mode: RangeMode::PointToPoint,
            start_seconds: Some(900.0),
            end_seconds: Some(300.0),
            angle: None,
        };
        assert!(!range_view(&title(), &reversed).valid);

        let good = RangeSelection {
            mode: RangeMode::PointToPoint,
            start_seconds: Some(90.0),
            end_seconds: Some(120.0),
            angle: Some(2),
        };
        let view = range_view(&title(), &good);
        assert!(view.valid);
        assert_eq!(view.start_label, "1:30");
        assert_eq!(view.end_label, "2:00");
        assert_eq!(view.duration_label, "0:30");
        assert_eq!(view.angle, 2);
    }

    #[test]
    fn an_angle_beyond_the_source_is_refused() {
        let selection = RangeSelection {
            mode: RangeMode::Whole,
            angle: Some(9),
            ..Default::default()
        };
        let view = range_view(&title(), &selection);
        assert!(!view.valid);
        assert!(view.problem.contains("3 angle"));
    }

    #[test]
    fn a_source_without_angles_reports_a_single_one() {
        let single = ScanTitle {
            index: 1,
            duration_seconds: 60.0,
            angle_count: 0,
            ..ScanTitle::default()
        };
        let view = range_view(&single, &RangeSelection::default());
        assert!(!view.has_angles);
        assert_eq!(view.angles.len(), 1);
        assert!(view.valid);
    }

    #[test]
    fn progress_view_summarises_the_scan() {
        let report = vise_ipc::contract::ScanProgress {
            progress: 0.5,
            completed: 2,
            total: 4,
            current_path: "C:\\media\\clips\\scene2.mkv".into(),
            title_count: 3,
        };
        let view = progress_view(Some(&report));
        assert!(view.visible);
        assert_eq!(view.percent, 50);
        assert_eq!(view.current_file, "scene2.mkv");
        assert!(view.status_line.contains("Scanning 3 of 4"));
        assert!(view.status_line.contains("3 title(s) so far"));

        let empty = progress_view(None);
        assert!(!empty.visible);
        assert_eq!(empty.status_line, "");
    }

    #[test]
    fn phases_follow_the_worker_state() {
        assert_eq!(phase_of(WorkState::Unknown, true), ScanPhase::Idle);
        assert_eq!(phase_of(WorkState::Working, true), ScanPhase::Scanning);
        assert_eq!(phase_of(WorkState::ScanDone, true), ScanPhase::Ready);
        assert_eq!(phase_of(WorkState::WorkDone, false), ScanPhase::Failed);
        assert_eq!(ScanPhase::Ready.as_str(), "ready");
        assert_eq!(ScanPhase::Scanning.to_string(), "scanning");
    }

    #[test]
    fn selection_serialises_for_the_interface() {
        let view = SelectionViewModel {
            source: None,
            phase: ScanPhase::Ready,
            phase_name: ScanPhase::Ready.as_str().to_string(),
            progress: Some(progress_view(Some(&vise_ipc::contract::ScanProgress {
                progress: 1.0,
                completed: 1,
                total: 1,
                ..Default::default()
            }))),
            titles: vec![title_view(&title(), 2)],
            main_title_index: 2,
            selected_title_index: 2,
            tracks: track_selection(Some(&title()), &[1], &[2]),
            range: Some(range_view(&title(), &RangeSelection::default())),
            problem: String::new(),
        };
        assert!(view.is_usable());
        let json = serde_json::to_value(&view).expect("serialise");
        assert_eq!(json["phase"], "ready");
        assert_eq!(json["selectedTitleIndex"], 2);
        assert_eq!(json["titles"][0]["durationLabel"], "1:00:00");
        assert_eq!(json["tracks"]["audio"][0]["languageCode"], "eng");
        assert_eq!(json["range"]["mode"], "whole");
        assert_eq!(json["range"]["startLabel"], "0:00");
    }

    #[test]
    fn a_selection_carries_the_source_label_the_picker_shows() {
        let source = crate::sources::SourceViewModel {
            path: "C:\\media\\movie.mkv".into(),
            name: "movie.mkv".into(),
            location: "C:\\media".into(),
            kind: crate::sources::SourceKind::File,
            label: "movie.mkv · File".into(),
        };
        let view = SelectionViewModel {
            source: Some(source),
            phase: ScanPhase::Idle,
            phase_name: ScanPhase::Idle.as_str().to_string(),
            ..Default::default()
        };
        let json = serde_json::to_value(&view).expect("serialise");
        assert_eq!(json["source"]["label"], "movie.mkv · File");
        assert_eq!(json["source"]["kind"], "file");
    }
}
