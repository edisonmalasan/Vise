//! The configuration tabs.
//!
//! Every tab is a view model built here, on the Rust side, from one set of
//! settings. The page asks for the whole set, sends back the change for the one
//! control the user touched, and gets the whole set again: there is no partial
//! state in the window that the page could get out of step with, and every rule
//! — a crop that has to fit inside the frame, a mixdown that cannot invent
//! speakers, a subtitle codec the container will not carry — is ordinary Rust
//! that a unit test can pin down.
//!
//! The settings also build the job specification, so what the tabs show and what
//! the worker is asked to do come from the same place. A tab that reports a
//! problem blocks the queue button rather than letting the encoder discover it.

pub mod audio;
pub mod chapters;
pub mod common;
pub mod compatibility;
pub mod dimensions;
pub mod filters;
pub mod source;
pub mod subtitles;
pub mod summary;
pub mod video;

use serde::{Deserialize, Serialize};
use vise_core::{ColourRange, EncodingMode, JobSpec, OutputOptions, VideoEncodingOptions};

use audio::AudioSettings;
pub use audio::{AudioDefaultsInput, AudioTrackInput};
pub use chapters::{ChapterSettings, ChaptersInput, MarkerInput};
pub use dimensions::DimensionsInput;
pub use filters::{FilterInput, FilterSettings};
pub use source::{TitleFacts, TitleFactsViewModel};
use subtitles::SubtitleSettings;
pub use subtitles::{SubtitleTrackInput, SubtitlesInput};
pub use summary::{SummaryInput, SummarySettings};
pub use video::{VideoInput, VideoSettings};

/// One tab in the strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabDescriptor {
    /// Stable identifier the page uses to pick the panel.
    pub id: String,
    /// Label on the tab.
    pub label: String,
    /// One line the page shows as the tab's hint.
    pub hint: String,
    /// Whether anything on the tab needs fixing.
    pub needs_attention: bool,
}

/// Every tab, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSetViewModel {
    /// The tab strip, in order.
    pub tabs: Vec<TabDescriptor>,
    /// Which tab the page shows.
    pub active: String,
    /// What the tabs are working on.
    pub source: TitleFactsViewModel,
    /// The summary tab.
    pub summary: summary::SummaryTabViewModel,
    /// The dimensions tab.
    pub dimensions: dimensions::DimensionsTabViewModel,
    /// The filters tab.
    pub filters: filters::FiltersTabViewModel,
    /// The video tab.
    pub video: video::VideoTabViewModel,
    /// The audio tab.
    pub audio: audio::AudioTabViewModel,
    /// The subtitles tab.
    pub subtitles: subtitles::SubtitlesTabViewModel,
    /// The chapters tab.
    pub chapters: chapters::ChaptersTabViewModel,
    /// Everything that has to be fixed before a job can be queued.
    pub problems: Vec<String>,
    /// Whether the job can be queued from these choices.
    pub can_queue: bool,
}

/// Every choice the tabs hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TabSettings {
    /// Which tab the page shows.
    pub active: String,
    /// Output container and flags.
    pub summary: SummarySettings,
    /// Crop, scale, and orientation.
    pub dimensions: dimensions::DimensionsSettings,
    /// Filter choices.
    pub filters: FilterSettings,
    /// Encoder and rate control.
    pub video: VideoSettings,
    /// Audio rules and per-track overrides.
    pub audio: AudioSettings,
    /// Subtitle behaviour.
    pub subtitles: SubtitleSettings,
    /// Chapter markers.
    pub chapters: ChapterSettings,
}

/// The tab order, with the label and the hint the strip shows.
const TABS: [(&str, &str, &str); 7] = [
    (
        "summary",
        "Summary",
        "Container, codecs, and the flags that shape the finished file.",
    ),
    (
        "dimensions",
        "Dimensions",
        "Crop, scaling, size limits, and which way the picture ends up facing.",
    ),
    (
        "filters",
        "Filters",
        "Every picture repair and effect, with its strength.",
    ),
    (
        "video",
        "Video",
        "Encoder, rate control, and the encoder-side choices.",
    ),
    (
        "audio",
        "Audio",
        "Defaults for every track, and the tracks that step outside them.",
    ),
    (
        "subtitles",
        "Subtitles",
        "What happens to each subtitle track, and how to find one.",
    ),
    (
        "chapters",
        "Chapters",
        "Which chapter markers are written, and editing them.",
    ),
];

impl TabSettings {
    /// Settings that make a usable default job.
    pub fn new() -> Self {
        Self {
            active: TABS[0].0.to_string(),
            ..Default::default()
        }
    }

    /// Bring the per-track settings in line with the chosen title.
    ///
    /// A title has its own tracks, so entries for tracks that are no longer there
    /// are dropped and the ones that are new start from the defaults.
    pub fn sync_with_title(&mut self, facts: &TitleFacts) {
        self.audio.sync(facts);
        self.subtitles.sync(facts);
        self.chapters.sync(facts);
    }

    /// Mark the tracks the source area chose.
    pub fn choose_tracks(&mut self, audio: &[u32], subtitle: &[u32]) {
        self.audio.choose(audio);
        self.subtitles.choose(subtitle);
    }

    /// Whether the page may show `tab`.
    pub fn is_tab(&self, tab: &str) -> bool {
        TABS.iter().any(|(id, _, _)| *id == tab.trim())
    }

    /// Build every tab for the chosen title.
    pub fn view(&self, facts: &TitleFacts) -> TabSetViewModel {
        let container = self.summary.container.as_str();
        let summary = summary::tab_view(&self.summary, &self.video);
        let dimensions = dimensions::tab_view(&self.dimensions, facts);
        let filters = filters::tab_view(&self.filters, &self.dimensions, facts);
        let video = video::tab_view(&self.video, facts);
        let audio = audio::tab_view(&self.audio, facts, container, &self.summary.audio_codec);
        let subtitles = subtitles::tab_view(&self.subtitles, facts, container);
        let chapters = chapters::tab_view(&self.chapters, facts);

        // Each tab is listed in one place: its identifier, its label, and whether
        // anything on it still has to be fixed.
        let sections: Vec<(&str, bool)> = vec![
            ("summary", summary.ready),
            ("dimensions", dimensions.ready),
            ("filters", filters.ready),
            ("video", video.ready),
            ("audio", audio.ready),
            ("subtitles", subtitles.ready),
            ("chapters", chapters.ready),
        ];
        let ready_of = |id: &str| {
            sections
                .iter()
                .find(|(section, _)| *section == id)
                .map(|(_, ready)| *ready)
                .unwrap_or(true)
        };
        let mut problems: Vec<String> = Vec::new();
        for (id, label, _) in TABS {
            if ready_of(id) {
                continue;
            }
            for problem in tab_problems(
                id,
                &summary,
                &dimensions,
                &filters,
                &video,
                &audio,
                &subtitles,
                &chapters,
            ) {
                problems.push(format!("{label}: {problem}"));
            }
        }
        let tabs = TABS
            .iter()
            .map(|(id, label, hint)| TabDescriptor {
                id: (*id).to_string(),
                label: (*label).to_string(),
                hint: (*hint).to_string(),
                needs_attention: !ready_of(id),
            })
            .collect();

        let active = if self.is_tab(&self.active) {
            self.active.clone()
        } else {
            TABS[0].0.to_string()
        };

        TabSetViewModel {
            tabs,
            active,
            source: TitleFactsViewModel::of(facts),
            summary,
            dimensions,
            filters,
            video,
            audio,
            subtitles,
            chapters,
            can_queue: problems.is_empty(),
            problems,
        }
    }

    /// Build the job specification these choices describe.
    ///
    /// Every tab's rules run again here, so a job cannot be queued with a
    /// combination the tabs would have refused.
    pub fn to_job_spec(
        &self,
        input_path: &str,
        output_path: &str,
        facts: &TitleFacts,
    ) -> Result<JobSpec, String> {
        let container = compatibility::container_of(&self.summary.container).ok_or_else(|| {
            format!(
                "'{}' is not a container this build offers.",
                self.summary.container
            )
        })?;
        let video_codec =
            compatibility::video_codec_of(&self.summary.video_codec).ok_or_else(|| {
                format!(
                    "'{}' is not a video encoder this build offers.",
                    self.summary.video_codec
                )
            })?;
        let audio_codec =
            compatibility::audio_codec_of(&self.summary.audio_codec).ok_or_else(|| {
                format!(
                    "'{}' is not an audio encoder this build offers.",
                    self.summary.audio_codec
                )
            })?;

        let (crop, scale) = dimensions::to_job_geometry(&self.dimensions, facts)?;
        let mut video_processing = filters::to_job_options(&self.filters, &self.dimensions);
        video_processing.crop = crop;
        video_processing.scale = scale;

        let (rate_mode, quality, bitrate, target_size, two_pass, lossless) =
            rate_control(&self.video, facts)?;
        video_processing.frame_rate = frame_rate(&self.video)?;

        let (audio_processing, audio_tracks) =
            audio::to_job_options(&self.audio, facts, &self.summary.audio_codec)?;
        let (subtitle_mode, subtitle_tracks, external) =
            subtitles::to_job_options(&self.subtitles, facts, &self.summary.container)?;
        let chapter_processing = chapters::to_job_options(&self.chapters);

        let video_encoding = VideoEncodingOptions {
            preset: named(&self.video.preset),
            profile: named(&self.video.profile),
            tune: named(&self.video.tune),
            level: named(&self.video.level),
            colour_range: match self.video.colour_range.as_str() {
                "limited" => Some(ColourRange::Limited),
                "full" => Some(ColourRange::Full),
                _ => None,
            },
        };

        Ok(JobSpec {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            container,
            video_codec,
            audio_codec,
            subtitle_mode,
            video_encoding,
            output_options: OutputOptions {
                // `None` would mean "on", which is not what a switched-off
                // switch means.
                web_optimized: Some(self.summary.web_optimized),
                align_audio_video: self.summary.align_audio_video,
                ipod_compatible: self.summary.ipod_compatible,
                passthrough_metadata: self.summary.passthrough_metadata,
            },
            encoding_mode: Some(rate_mode),
            quality,
            bitrate,
            target_size,
            two_pass,
            lossless,
            video_processing,
            audio_processing,
            subtitle_tracks,
            subtitle_path: external.clone(),
            subtitle_processing: subtitles::to_job_processing(external),
            audio_tracks,
            chapter_processing,
            metadata_processing: vise_core::MetadataProcessingOptions {
                copy: self.summary.passthrough_metadata,
                ..Default::default()
            },
            ..Default::default()
        })
    }
}

/// A named encoder choice, or nothing for "encoder default".
fn named(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "default" => None,
        other => Some(other.to_string()),
    }
}

/// The rate control, as the job model wants it.
type RateControl = (
    EncodingMode,
    Option<u32>,
    Option<u32>,
    Option<u64>,
    bool,
    bool,
);

fn rate_control(settings: &VideoSettings, facts: &TitleFacts) -> Result<RateControl, String> {
    let mode = match settings.rate() {
        video::RateControl::Quality => EncodingMode::ConstantQuality,
        video::RateControl::Bitrate => EncodingMode::AverageBitrate,
        video::RateControl::TargetSize => EncodingMode::TargetSize,
        video::RateControl::Lossless => EncodingMode::Lossless,
    };
    let quality = match settings.rate() {
        video::RateControl::Quality => Some(
            settings
                .quality
                .whole("Quality")?
                .ok_or_else(|| "Choose a quality value.".to_string())?,
        ),
        _ => None,
    };
    let bitrate = match settings.rate() {
        video::RateControl::Bitrate => Some(
            settings
                .bitrate
                .whole("Bit rate")?
                .ok_or_else(|| "Choose a bit rate.".to_string())?,
        ),
        _ => None,
    };
    let target_size = match settings.rate() {
        video::RateControl::TargetSize => {
            let megabytes = settings
                .target_size
                .whole("File size")?
                .ok_or_else(|| "Choose a target file size.".to_string())?;
            if !facts.has_duration() {
                return Err(
                    "A target file size needs the source duration, which this title does not report."
                        .to_string(),
                );
            }
            Some(megabytes as u64 * 1_000_000)
        }
        _ => None,
    };
    let two_pass = settings.two_pass && settings.rate().benefits_from_two_pass();
    Ok((mode, quality, bitrate, target_size, two_pass, false))
}

/// The frame-rate choice, as the filter model wants it.
fn frame_rate(settings: &VideoSettings) -> Result<Option<vise_core::FrameRateMode>, String> {
    match settings.frame_rate_kind() {
        video::FrameRateMode::Source => Ok(None),
        video::FrameRateMode::Variable => Ok(Some(vise_core::FrameRateMode::Variable)),
        video::FrameRateMode::Constant => {
            let value = settings
                .frame_rate
                .number("Frames per second")?
                .ok_or_else(|| "Choose a frame rate for the fixed-rate mode.".to_string())?;
            if !(1.0..=240.0).contains(&value) {
                return Err("Frames per second must be between 1 and 240.".to_string());
            }
            Ok(Some(vise_core::FrameRateMode::Constant(value)))
        }
    }
}

/// The problems of one tab, for the summary line above the strip.
#[allow(clippy::too_many_arguments)]
fn tab_problems(
    id: &str,
    summary: &summary::SummaryTabViewModel,
    dimensions: &dimensions::DimensionsTabViewModel,
    filters: &filters::FiltersTabViewModel,
    video: &video::VideoTabViewModel,
    audio: &audio::AudioTabViewModel,
    subtitles: &subtitles::SubtitlesTabViewModel,
    chapters: &chapters::ChaptersTabViewModel,
) -> Vec<String> {
    match id {
        "summary" => summary.problems.clone(),
        "dimensions" => dimensions.problems.clone(),
        "filters" => filters.problems.clone(),
        "video" => video.problems.clone(),
        "audio" => audio.problems.clone(),
        "subtitles" => subtitles.problems.clone(),
        _ => chapters.problems.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::common::NumField;
    use crate::tabs::source::{AudioFacts, ChapterFacts, SubtitleFacts};
    use vise_core::{ContainerFormat, VideoCodec};

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            frame_rate: 23.976,
            duration_seconds: 3600.0,
            audio: vec![AudioFacts {
                number: 1,
                language: "English".into(),
                language_code: "eng".into(),
                codec: "eac3".into(),
                channel_count: 6,
                channel_layout: "5.1".into(),
                sample_rate: 48_000,
                bit_rate: 640_000,
                label: "Audio 1 · English".into(),
            }],
            subtitles: vec![SubtitleFacts {
                number: 1,
                language: "English".into(),
                language_code: "eng".into(),
                codec: "subrip".into(),
                description: String::new(),
                forced: false,
                label: "Subtitle 1 · English".into(),
            }],
            chapters: vec![ChapterFacts {
                number: 1,
                name: "Opening".into(),
                start_seconds: 0.0,
                end_seconds: 600.0,
            }],
            ..Default::default()
        }
    }

    fn ready_settings() -> TabSettings {
        let mut settings = TabSettings::new();
        settings.sync_with_title(&facts());
        settings.choose_tracks(&[1], &[1]);
        settings.audio.defaults.bitrate = NumField::of("192");
        settings
    }

    #[test]
    fn the_tabs_report_the_title_they_are_working_on() {
        let settings = ready_settings();
        let view = settings.view(&facts());
        assert_eq!(view.tabs.len(), 7);
        assert_eq!(view.tabs[0].id, "summary");
        assert_eq!(view.active, "summary");
        assert!(view.source.chosen);
        assert_eq!(view.source.resolution_label, "1920x1080");
        assert!(settings.is_tab("chapters"));
        assert!(!settings.is_tab("metadata"));
    }

    #[test]
    fn the_active_tab_falls_back_when_the_page_asks_for_one_that_is_not_there() {
        let mut settings = ready_settings();
        settings.active = "metadata".into();
        assert_eq!(settings.view(&facts()).active, "summary");
        settings.active = "video".into();
        assert_eq!(settings.view(&facts()).active, "video");
    }

    #[test]
    fn a_default_set_of_choices_can_be_queued() {
        let settings = ready_settings();
        let view = settings.view(&facts());
        assert!(view.can_queue, "{:?}", view.problems);
        assert!(view.tabs.iter().all(|tab| !tab.needs_attention));
        let spec = settings
            .to_job_spec("in.mkv", "out.mp4", &facts())
            .expect("job spec");
        assert_eq!(spec.container, ContainerFormat::Mp4);
        assert_eq!(spec.video_codec, VideoCodec::H264);
        assert_eq!(spec.encoding_mode, Some(EncodingMode::ConstantQuality));
        assert_eq!(spec.quality, Some(20));
        assert_eq!(spec.audio_tracks.len(), 1);
        assert_eq!(spec.subtitle_tracks.len(), 1);
        assert_eq!(
            spec.chapter_processing.mode,
            Some(vise_core::ChapterMode::Copy)
        );
        assert!(spec.output_options.wants_web_optimized());
        assert!(spec.metadata_processing.copy);
    }

    #[test]
    fn every_choice_the_tabs_show_reaches_the_job_spec() {
        let mut settings = ready_settings();
        settings.dimensions.crop_mode = "manual".into();
        settings.dimensions.crop_width = NumField::of("1900");
        settings.dimensions.crop_height = NumField::of("1000");
        settings.dimensions.scale_mode = "exact".into();
        settings.dimensions.scale_width = NumField::of("950");
        settings.dimensions.scale_height = NumField::of("500");
        settings.dimensions.rotate = "90".into();
        settings.dimensions.flip = "horizontal".into();
        settings.filters.denoise = true;
        settings.filters.sharpen = true;
        settings.filters.chroma_smooth = true;
        settings.video.rate_control = "bitrate".into();
        settings.video.bitrate = NumField::of("2500");
        settings.video.two_pass = true;
        settings.video.preset = "slow".into();
        settings.video.profile = "main".into();
        settings.video.tune = "film".into();
        settings.video.level = "4.1".into();
        settings.video.colour_range = "full".into();
        settings.video.frame_rate_mode = "constant".into();
        settings.video.frame_rate = NumField::of("24");
        settings.summary.ipod_compatible = true;
        settings.summary.align_audio_video = true;
        settings.chapters.mode = "edit".into();

        let view = settings.view(&facts());
        assert!(view.can_queue, "{:?}", view.problems);
        let spec = settings
            .to_job_spec("in.mkv", "out.mp4", &facts())
            .expect("job spec");
        assert_eq!(spec.video_processing.crop, Some((0, 0, 1900, 1000)));
        assert_eq!(spec.video_processing.scale, Some((950, 500)));
        assert_eq!(spec.video_processing.rotate, Some(90));
        assert_eq!(
            spec.video_processing.flip,
            Some(vise_core::FlipMode::Horizontal)
        );
        assert!(spec.video_processing.denoise);
        assert!(spec.video_processing.sharpen);
        assert!(spec.video_processing.chroma_smooth);
        assert_eq!(spec.encoding_mode, Some(EncodingMode::AverageBitrate));
        assert_eq!(spec.bitrate, Some(2500));
        assert!(spec.two_pass);
        assert_eq!(spec.video_encoding.preset.as_deref(), Some("slow"));
        assert_eq!(spec.video_encoding.tune.as_deref(), Some("film"));
        assert_eq!(spec.video_encoding.level.as_deref(), Some("4.1"));
        assert_eq!(spec.video_encoding.colour_range, Some(ColourRange::Full));
        assert_eq!(
            spec.video_processing.frame_rate,
            Some(vise_core::FrameRateMode::Constant(24.0))
        );
        assert!(spec.output_options.ipod_compatible);
        assert!(spec.output_options.align_audio_video);
        // The portable-player profile is pinned by the flag.
        assert_eq!(
            spec.video_encoding_options().profile.as_deref(),
            Some("baseline")
        );
        assert_eq!(
            spec.chapter_processing.mode,
            Some(vise_core::ChapterMode::Edit)
        );
        assert_eq!(spec.chapter_processing.chapters.len(), 1);
    }

    #[test]
    fn a_turn_moves_the_filter_before_the_scale() {
        let mut settings = ready_settings();
        settings.dimensions.rotate = "90".into();
        settings.filters.deinterlace = true;
        let spec = settings
            .to_job_spec("in.mkv", "out.mp4", &facts())
            .expect("job spec");
        let chain = spec.video_processing.build_filter_chain();
        assert!(chain.contains("yadif"), "{chain}");
        assert!(chain.contains("transpose=1"), "{chain}");
    }

    #[test]
    fn a_problem_in_any_tab_blocks_the_queue() {
        let mut settings = ready_settings();
        settings.video.rate_control = "bitrate".into();
        settings.video.bitrate = NumField::blank();
        let view = settings.view(&facts());
        assert!(!view.can_queue);
        assert!(view
            .tabs
            .iter()
            .any(|tab| tab.id == "video" && tab.needs_attention));
        assert!(
            view.problems.iter().any(|p| p.starts_with("Video: ")),
            "{:?}",
            view.problems
        );
        let error = settings
            .to_job_spec("in.mkv", "out.mp4", &facts())
            .expect_err("no bit rate");
        assert!(error.contains("bit rate"), "{error}");
    }

    #[test]
    fn a_target_size_without_a_duration_refuses_the_job() {
        let mut settings = ready_settings();
        settings.video.rate_control = "target_size".into();
        settings.video.target_size = NumField::of("700");
        let silent = TitleFacts {
            number: 1,
            width: facts().width,
            height: facts().height,
            frame_rate: facts().frame_rate,
            duration_seconds: 0.0,
            audio: facts().audio,
            subtitles: facts().subtitles,
            chapters: facts().chapters,
            ..Default::default()
        };
        let view = settings.view(&silent);
        assert!(!view.can_queue);
        let error = settings
            .to_job_spec("in.mkv", "out.mp4", &silent)
            .expect_err("no duration");
        assert!(error.contains("source duration"), "{error}");
    }

    #[test]
    fn syncing_with_a_title_drops_settings_for_tracks_it_does_not_have() {
        let mut settings = ready_settings();
        settings.choose_tracks(&[1], &[1]);
        assert_eq!(settings.audio.tracks.len(), 1);
        let other = TitleFacts {
            number: 2,
            width: 1280,
            height: 720,
            ..Default::default()
        };
        settings.sync_with_title(&other);
        assert!(settings.audio.tracks.is_empty());
        assert!(settings.subtitles.tracks.is_empty());
        assert_eq!(settings.chapters.markers.len(), 0);
    }
}
