//! What the configuration tabs know about the chosen title.
//!
//! The tabs are only meaningful against a real title: a crop needs a frame
//! size, a mixdown needs a channel count, burn-in needs a video stream. This
//! module turns the scanned title into the plain facts those checks read, so
//! the rules live next to the facts and can be tested without a worker.

use serde::{Deserialize, Serialize};
use vise_ipc::contract::ScanTitle;

use crate::viewmodel::{format_channels, format_duration, format_frame_rate, format_resolution};

/// One audio track of the chosen title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioFacts {
    /// Track number inside the title, counting from one.
    pub number: u32,
    /// Language to show.
    pub language: String,
    /// Language code as reported.
    pub language_code: String,
    /// Codec name.
    pub codec: String,
    /// How many channels the track carries.
    pub channel_count: u32,
    /// Channel layout as reported.
    pub channel_layout: String,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Bit rate in bit/s.
    pub bit_rate: u64,
    /// Track number, language, and codec on one line.
    pub label: String,
}

/// One subtitle track of the chosen title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtitleFacts {
    /// Track number inside the title, counting from one.
    pub number: u32,
    /// Language to show.
    pub language: String,
    /// Language code as reported.
    pub language_code: String,
    /// Codec name.
    pub codec: String,
    /// Description the source carries.
    pub description: String,
    /// Whether the track is marked as forced dialogue.
    pub forced: bool,
    /// Track number, language, and codec on one line.
    pub label: String,
}

/// One chapter marker of the chosen title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChapterFacts {
    /// Chapter number, counting from one.
    pub number: u32,
    /// Chapter name, when the source names one.
    pub name: String,
    /// Start in seconds.
    pub start_seconds: f64,
    /// End in seconds.
    pub end_seconds: f64,
}

/// One tag the source carried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MetadataFact {
    /// Tag name as the scan reported it.
    pub key: String,
    /// Tag value as the scan reported it.
    pub value: String,
}

/// Everything the tabs read about the chosen title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleFacts {
    /// Chosen title, counting from one; zero when none is chosen.
    pub number: u32,
    /// Title name.
    pub name: String,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frame rate in frames per second.
    pub frame_rate: f64,
    /// Duration in seconds.
    pub duration_seconds: f64,
    /// Audio tracks of the title.
    pub audio: Vec<AudioFacts>,
    /// Subtitle tracks of the title.
    pub subtitles: Vec<SubtitleFacts>,
    /// Chapter markers of the title.
    pub chapters: Vec<ChapterFacts>,
    /// Tags the source carried, sorted by name.
    pub metadata: Vec<MetadataFact>,
}

impl TitleFacts {
    /// Facts for a scanned title, or the "nothing chosen" facts.
    pub fn of(title: Option<&ScanTitle>) -> Self {
        let Some(title) = title else {
            return Self::default();
        };
        Self {
            number: title.index,
            name: title.name.trim().to_string(),
            width: title.width,
            height: title.height,
            frame_rate: title.frame_rate,
            duration_seconds: title.duration_seconds,
            audio: title.audio_list.iter().map(audio_facts).collect(),
            subtitles: title.subtitle_list.iter().map(subtitle_facts).collect(),
            chapters: title
                .chapter_list
                .iter()
                .map(|chapter| ChapterFacts {
                    number: chapter.index + 1,
                    name: chapter.title.trim().to_string(),
                    start_seconds: chapter.start_seconds,
                    end_seconds: chapter.end_seconds,
                })
                .collect(),
            metadata: title
                .meta_data
                .iter()
                .map(|(key, value)| MetadataFact {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect(),
        }
    }

    /// Whether a title has been chosen.
    pub fn is_chosen(&self) -> bool {
        self.number > 0
    }

    /// Whether the scan reported a usable frame size.
    pub fn has_frame_size(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Whether the scan reported a usable frame rate.
    pub fn has_frame_rate(&self) -> bool {
        self.frame_rate.is_finite() && self.frame_rate > 0.0
    }

    /// Whether the scan reported a usable duration.
    pub fn has_duration(&self) -> bool {
        self.duration_seconds.is_finite() && self.duration_seconds > 0.0
    }

    /// Frame size after a rotation of `degrees`.
    ///
    /// A quarter turn swaps the axes, which is why a crop or a size limit has to
    /// be checked against the turned frame rather than the scanned one.
    pub fn turned_size(&self, degrees: u32) -> (u32, u32) {
        if degrees % 180 == 90 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// The audio track with `number`.
    pub fn audio_track(&self, number: u32) -> Option<&AudioFacts> {
        self.audio.iter().find(|track| track.number == number)
    }

    /// The subtitle track with `number`.
    pub fn subtitle_track(&self, number: u32) -> Option<&SubtitleFacts> {
        self.subtitles.iter().find(|track| track.number == number)
    }

    /// Every language present on a subtitle track, with how many carry it.
    pub fn subtitle_languages(&self) -> Vec<(String, usize)> {
        let mut languages: Vec<(String, usize)> = Vec::new();
        for track in &self.subtitles {
            let language = track.language.trim().to_string();
            match languages.iter_mut().find(|(name, _)| *name == language) {
                Some(entry) => entry.1 += 1,
                None => languages.push((language, 1)),
            }
        }
        languages.sort_by(|a, b| a.0.cmp(&b.0));
        languages
    }
}

fn audio_facts(track: &vise_ipc::contract::ScanAudioTrack) -> AudioFacts {
    let language = language_of(&track.language, &track.language_code);
    let codec = if track.codec_name.trim().is_empty() {
        "unknown".to_string()
    } else {
        track.codec_name.trim().to_string()
    };
    AudioFacts {
        number: track.track_number,
        language: language.clone(),
        language_code: track.language_code.trim().to_string(),
        codec: codec.clone(),
        channel_count: track.channel_count,
        channel_layout: track.channel_layout.clone(),
        sample_rate: track.sample_rate,
        bit_rate: track.bit_rate,
        label: format!(
            "Audio {} · {language} · {codec} · {}",
            track.track_number,
            format_channels(track.channel_count, &track.channel_layout)
        ),
    }
}

fn subtitle_facts(track: &vise_ipc::contract::ScanSubtitleTrack) -> SubtitleFacts {
    let language = language_of(&track.language, &track.language_code);
    let codec = if track.codec_name.trim().is_empty() {
        "unknown".to_string()
    } else {
        track.codec_name.trim().to_string()
    };
    let description = track.description.trim().to_string();
    let forced = description.to_ascii_lowercase().contains("forced");
    SubtitleFacts {
        number: track.track_number,
        language: language.clone(),
        language_code: track.language_code.trim().to_string(),
        codec: codec.clone(),
        description: description.clone(),
        forced,
        label: if description.is_empty() {
            format!("Subtitle {} · {language} · {codec}", track.track_number)
        } else {
            format!(
                "Subtitle {} · {language} · {codec} · {description}",
                track.track_number
            )
        },
    }
}

/// Language to show for a track, falling back to the code.
fn language_of(language: &str, code: &str) -> String {
    let language = language.trim();
    if !language.is_empty() && !language.eq_ignore_ascii_case("und") {
        language.to_string()
    } else if !code.trim().is_empty() && !code.trim().eq_ignore_ascii_case("und") {
        code.trim().to_string()
    } else {
        "Undetermined".to_string()
    }
}

/// The facts as the tab headers and the summary line show them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleFactsViewModel {
    /// Whether a title has been chosen.
    pub chosen: bool,
    /// Title number, or zero.
    pub number: u32,
    /// Title name, or a prompt to choose one.
    pub name: String,
    /// Frame size as `1920x1080`.
    pub resolution_label: String,
    /// Frame rate as `23.976 fps`.
    pub frame_rate_label: String,
    /// Duration as `h:mm:ss`.
    pub duration_label: String,
    /// How many audio tracks the title carries.
    pub audio_count: usize,
    /// How many subtitle tracks the title carries.
    pub subtitle_count: usize,
    /// How many chapter markers the title carries.
    pub chapter_count: usize,
    /// One line describing what the tabs are working on.
    pub summary: String,
}

impl TitleFactsViewModel {
    /// The rendered facts for a chosen title.
    pub fn of(facts: &TitleFacts) -> Self {
        let name = if facts.name.is_empty() {
            format!("Title {}", facts.number)
        } else {
            facts.name.clone()
        };
        Self {
            chosen: facts.is_chosen(),
            number: facts.number,
            summary: if facts.is_chosen() {
                format!(
                    "{} · {} · {} · {} audio · {} subtitle",
                    format_resolution(facts.width, facts.height),
                    format_frame_rate(facts.frame_rate),
                    format_duration(facts.duration_seconds),
                    facts.audio.len(),
                    facts.subtitles.len()
                )
            } else {
                "Choose a title to configure the output.".to_string()
            },
            name: if facts.is_chosen() {
                name
            } else {
                "No title chosen".to_string()
            },
            resolution_label: format_resolution(facts.width, facts.height),
            frame_rate_label: format_frame_rate(facts.frame_rate),
            duration_label: format_duration(facts.duration_seconds),
            audio_count: facts.audio.len(),
            subtitle_count: facts.subtitles.len(),
            chapter_count: facts.chapters.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_ipc::contract::{ScanAudioTrack, ScanChapter, ScanSubtitleTrack};

    fn audio(number: u32, channels: u32) -> ScanAudioTrack {
        ScanAudioTrack {
            track_number: number,
            codec_name: "eac3".into(),
            channel_count: channels,
            channel_layout: if channels > 2 {
                "5.1".into()
            } else {
                "stereo".into()
            },
            sample_rate: 48_000,
            bit_rate: 640_000,
            language: "English".into(),
            language_code: "eng".into(),
            description: String::new(),
            foreign_language: false,
        }
    }

    fn subtitle(number: u32, description: &str) -> ScanSubtitleTrack {
        ScanSubtitleTrack {
            track_number: number,
            codec_name: "subrip".into(),
            language: "English".into(),
            language_code: "eng".into(),
            description: description.into(),
        }
    }

    fn title() -> ScanTitle {
        ScanTitle {
            index: 2,
            name: "Feature".into(),
            width: 1920,
            height: 1080,
            frame_rate: 23.976,
            duration_seconds: 3600.0,
            audio_list: vec![audio(1, 6), audio(2, 2)],
            subtitle_list: vec![
                subtitle(1, "Full"),
                subtitle(2, "Forced only"),
                subtitle(3, "Commentary"),
            ],
            chapter_list: vec![ScanChapter {
                index: 0,
                start_seconds: 0.0,
                end_seconds: 600.0,
                title: "Opening".into(),
            }],
            ..ScanTitle::default()
        }
    }

    #[test]
    fn facts_follow_the_chosen_title() {
        let facts = TitleFacts::of(Some(&title()));
        assert!(facts.is_chosen());
        assert_eq!(facts.number, 2);
        assert!(facts.has_frame_size());
        assert!(facts.has_frame_rate());
        assert!(facts.has_duration());
        assert_eq!(facts.audio.len(), 2);
        assert_eq!(facts.subtitles.len(), 3);
        assert_eq!(facts.chapters.len(), 1);
        assert_eq!(
            facts.audio[0].label, "Audio 1 · English · eac3 · 5.1",
            "the label carries the facts a rule needs"
        );
        assert!(facts.audio_track(2).is_some());
        assert!(facts.audio_track(9).is_none());
    }

    #[test]
    fn facts_without_a_title_are_empty_rather_than_wrong() {
        let facts = TitleFacts::of(None);
        assert!(!facts.is_chosen());
        assert!(!facts.has_frame_size());
        assert!(!facts.has_frame_rate());
        assert!(!facts.has_duration());
        assert_eq!(facts.subtitle_languages(), Vec::new());
    }

    #[test]
    fn a_quarter_turn_swaps_the_frame_axes() {
        let facts = TitleFacts::of(Some(&title()));
        assert_eq!(facts.turned_size(0), (1920, 1080));
        assert_eq!(facts.turned_size(90), (1080, 1920));
        assert_eq!(facts.turned_size(180), (1920, 1080));
        assert_eq!(facts.turned_size(270), (1080, 1920));
    }

    #[test]
    fn a_forced_subtitle_track_is_recognised_from_its_description() {
        let facts = TitleFacts::of(Some(&title()));
        assert!(facts.subtitle_track(2).expect("track").forced);
        assert!(!facts.subtitle_track(1).expect("track").forced);
        assert_eq!(
            facts.subtitle_languages(),
            vec![("English".to_string(), 3)],
            "the search list counts the tracks per language"
        );
    }

    #[test]
    fn an_unknown_language_is_named_once() {
        let mut bare = audio(3, 2);
        bare.language = "  ".into();
        bare.language_code = "und".into();
        let track = audio_facts(&bare);
        assert_eq!(track.language, "Undetermined");
    }

    #[test]
    fn the_rendered_facts_read_as_one_line() {
        let view = TitleFactsViewModel::of(&TitleFacts::of(Some(&title())));
        assert!(view.chosen);
        assert_eq!(view.name, "Feature");
        assert_eq!(view.resolution_label, "1920x1080");
        assert_eq!(view.frame_rate_label, "23.976 fps");
        assert_eq!(view.duration_label, "1:00:00");
        assert!(view.summary.contains("1920x1080"));
        assert!(view.summary.contains("2 audio"));

        let none = TitleFactsViewModel::of(&TitleFacts::of(None));
        assert!(!none.chosen);
        assert_eq!(none.name, "No title chosen");
        assert!(none.summary.contains("Choose a title"));
    }
}
