//! The subtitles tab: what happens to each track, and how to find one.
//!
//! Three things decide a subtitle track's fate: whether it goes into the output
//! at all, whether it is drawn onto the picture or written as text beside it,
//! and whether the container can carry it. All three are checked here, because
//! a track that is accepted and then dropped by the encoder is worse than a
//! track the tab refuses with a reason.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vise_core::{SubtitleMode, SubtitleProcessingOptions, SubtitleTrackConfig};

use crate::tabs::common::{ChoiceGroup, ChoiceGroupViewModel, TextViewModel, ToggleViewModel};
use crate::tabs::compatibility::{burn_in_available, subtitle_passthrough};
use crate::tabs::source::{SubtitleFacts, TitleFacts};

/// What the job does with subtitle tracks as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleBehaviour {
    /// Nothing is written.
    None,
    /// The chosen tracks are written beside the picture.
    Passthrough,
    /// The chosen tracks are drawn onto the picture.
    BurnIn,
}

impl SubtitleBehaviour {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            SubtitleBehaviour::None => "none",
            SubtitleBehaviour::Passthrough => "passthrough",
            SubtitleBehaviour::BurnIn => "burn_in",
        }
    }

    /// The mode named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "none" | "drop" | "off" => Some(SubtitleBehaviour::None),
            "passthrough" | "pass_through" | "copy" | "keep" => {
                Some(SubtitleBehaviour::Passthrough)
            }
            "burn_in" | "burnin" | "burn" => Some(SubtitleBehaviour::BurnIn),
            _ => None,
        }
    }
}

/// One track's behaviour as the window holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleTrackSettings {
    /// Whether the track goes into the output.
    pub enabled: bool,
    /// What happens to it, or `inherit` for the job-wide behaviour.
    pub behaviour: String,
    /// Keep only the parts marked as forced dialogue.
    pub forced_only: bool,
}

impl Default for SubtitleTrackSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            behaviour: "inherit".to_string(),
            forced_only: false,
        }
    }
}

/// The subtitle choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubtitleSettings {
    /// What happens to subtitle tracks as a whole.
    pub behaviour: String,
    /// Keep only the parts marked as forced dialogue.
    pub forced_only: bool,
    /// Text typed into the language search.
    pub search: String,
    /// Path of an imported subtitle file.
    pub external_path: String,
    /// Name of the imported subtitle file, for display.
    pub external_name: String,
    /// Per-track choices, by track number.
    pub tracks: BTreeMap<u32, SubtitleTrackSettings>,
}

impl SubtitleSettings {
    /// Make sure every subtitle track of the title has an entry.
    pub fn sync(&mut self, facts: &TitleFacts) {
        self.tracks
            .retain(|number, _| facts.subtitle_track(*number).is_some());
        for track in &facts.subtitles {
            self.tracks.entry(track.number).or_default();
        }
    }

    /// Mark the chosen tracks and clear the rest.
    pub fn choose(&mut self, numbers: &[u32]) {
        let chosen: Vec<u32> = numbers.to_vec();
        for (number, track) in self.tracks.iter_mut() {
            track.enabled = chosen.contains(number);
        }
    }

    /// What one track will do, falling back to the job-wide behaviour.
    pub fn behaviour_of(&self, number: u32) -> SubtitleBehaviour {
        let chosen = self
            .tracks
            .get(&number)
            .map(|track| track.behaviour.as_str())
            .filter(|behaviour| *behaviour != "inherit" && !behaviour.is_empty())
            .unwrap_or(self.behaviour.as_str());
        SubtitleBehaviour::parse(chosen).unwrap_or(SubtitleBehaviour::Passthrough)
    }
}

/// Subtitle choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtitlesInput {
    /// What happens to subtitle tracks as a whole.
    pub behaviour: Option<String>,
    /// Keep only the parts marked as forced dialogue.
    pub forced_only: Option<bool>,
    /// Text typed into the language search.
    pub search: Option<String>,
}

impl SubtitlesInput {
    /// Apply the change to the job-wide choices.
    pub fn apply(self, settings: &mut SubtitleSettings) -> Result<(), String> {
        if let Some(behaviour) = self.behaviour {
            settings.behaviour = SubtitleBehaviour::parse(&behaviour)
                .ok_or_else(|| {
                    format!(
                        "'{}' is not a subtitle behaviour this build offers.",
                        behaviour.trim()
                    )
                })?
                .as_str()
                .to_string();
        }
        if let Some(forced) = self.forced_only {
            settings.forced_only = forced;
        }
        if let Some(search) = self.search {
            settings.search = search;
        }
        Ok(())
    }
}

/// One subtitle track's choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtitleTrackInput {
    /// Track number the change applies to.
    pub track: u32,
    /// Whether the track goes into the output.
    pub enabled: Option<bool>,
    /// What happens to it.
    pub behaviour: Option<String>,
    /// Keep only the parts marked as forced dialogue.
    pub forced_only: Option<bool>,
}

impl SubtitleTrackInput {
    /// Apply the change to one track.
    pub fn apply(self, settings: &mut SubtitleSettings) -> Result<(), String> {
        let entry = settings.tracks.entry(self.track).or_default();
        if let Some(enabled) = self.enabled {
            entry.enabled = enabled;
        }
        if let Some(behaviour) = self.behaviour {
            let value = behaviour.trim().to_ascii_lowercase();
            if value == "inherit" {
                entry.behaviour = value;
            } else {
                entry.behaviour = SubtitleBehaviour::parse(&value)
                    .ok_or_else(|| {
                        format!("'{value}' is not a subtitle behaviour this build offers.")
                    })?
                    .as_str()
                    .to_string();
            }
        }
        if let Some(forced) = self.forced_only {
            entry.forced_only = forced;
        }
        Ok(())
    }
}

/// One subtitle track as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrackViewModel {
    /// Track number inside the title.
    pub number: u32,
    /// Track label from the scan.
    pub label: String,
    /// Whether the track is chosen.
    pub enabled: bool,
    /// Whether this build can do what the track asks for.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// What happens to the track.
    pub behaviour: ChoiceGroupViewModel,
    /// Forced-dialogue switch.
    pub forced_only: ToggleViewModel,
    /// One line describing what will happen.
    pub summary: String,
}

/// The subtitles tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitlesTabViewModel {
    /// What happens to subtitle tracks as a whole.
    pub behaviour: ChoiceGroupViewModel,
    /// Forced-dialogue switch for every track.
    pub forced_only: ToggleViewModel,
    /// Language search field.
    pub search: TextViewModel,
    /// Languages the title carries, with how many tracks of each.
    pub languages: Vec<LanguageViewModel>,
    /// Tracks matching the search.
    pub tracks: Vec<SubtitleTrackViewModel>,
    /// Tracks hidden by the search.
    pub hidden: usize,
    /// The imported subtitle file.
    pub external: TextViewModel,
    /// The full path of the imported file, so the page can keep showing it.
    pub external_path: String,
    /// Whether an imported file can be used.
    pub external_available: bool,
    /// Why an imported file cannot be used, when it cannot.
    pub external_reason: String,
    /// One line describing the subtitle plan.
    pub summary: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// One language the title carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageViewModel {
    /// Language name.
    pub language: String,
    /// How many tracks carry it.
    pub count: usize,
    /// Whether the search text matches it.
    pub matched: bool,
}

/// Build the subtitles tab.
pub fn tab_view(
    settings: &SubtitleSettings,
    facts: &TitleFacts,
    container: &str,
) -> SubtitlesTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let behaviour =
        SubtitleBehaviour::parse(&settings.behaviour).unwrap_or(SubtitleBehaviour::Passthrough);

    // Without a chosen title there is nothing to write beside or draw onto.
    let needs_title = !facts.is_chosen();
    let behaviour_options = ChoiceGroup::of([
        ("none", "Leave them out"),
        ("passthrough", "Write them beside the picture"),
        ("burn_in", "Draw them onto the picture"),
    ]);
    let mut behaviour_view = ChoiceGroupViewModel::build(
        "subtitleBehaviour",
        "Subtitles",
        "What happens to the subtitle tracks that are chosen.",
        behaviour.as_str(),
        &behaviour_options.options,
        true,
        "",
    );
    if behaviour == SubtitleBehaviour::BurnIn {
        if let Err(reason) = burn_in_available(facts) {
            behaviour_view = behaviour_view.blocked(reason.clone());
            problems.push(reason);
        }
    }
    if behaviour == SubtitleBehaviour::Passthrough && needs_title {
        behaviour_view = behaviour_view.blocked("Choose a title first.");
    }

    let forced_only = ToggleViewModel::new(
        "forcedOnly",
        "Forced dialogue only",
        "Keeps only the parts the source marked as forced.",
        settings.forced_only,
    );

    let search = TextViewModel::new(
        "subtitleSearch",
        "Find a language",
        settings.search.trim(),
        "English",
    );

    let needle = settings.search.trim().to_ascii_lowercase();
    let languages: Vec<LanguageViewModel> = facts
        .subtitle_languages()
        .into_iter()
        .map(|(language, count)| LanguageViewModel {
            matched: needle.is_empty()
                || language.to_ascii_lowercase().contains(&needle)
                || facts
                    .subtitles
                    .iter()
                    .any(|track| track.language_code.eq_ignore_ascii_case(&needle)),
            language,
            count,
        })
        .collect();

    let mut tracks: Vec<SubtitleTrackViewModel> = Vec::new();
    let mut hidden = 0usize;
    for track in &facts.subtitles {
        let matches = needle.is_empty()
            || track.language.to_ascii_lowercase().contains(&needle)
            || track.language_code.to_ascii_lowercase().contains(&needle);
        if !matches {
            hidden += 1;
            continue;
        }
        tracks.push(track_view(track, settings, facts, container, behaviour));
    }
    for track in &tracks {
        if !track.available && track.enabled {
            problems.push(format!("Subtitle {}: {}", track.number, track.reason));
        }
    }

    let mut external = TextViewModel::new(
        "externalSubtitle",
        "Imported subtitle file",
        if settings.external_name.trim().is_empty() {
            settings.external_path.trim()
        } else {
            settings.external_name.trim()
        },
        "Choose a subtitle file to add it to the output",
    );
    let external_available =
        settings.external_path.trim().is_empty() || subtitle_passthrough(container, "srt").is_ok();
    if !external_available {
        external = external.invalid(subtitle_passthrough(container, "srt").unwrap_err());
    }

    let chosen = tracks.iter().filter(|track| track.enabled).count();
    let imported = settings.external_name.trim();
    let summary = match (facts.subtitles.is_empty(), imported.is_empty()) {
        (true, true) => "This title carries no subtitle tracks.".to_string(),
        (true, false) => format!("Using the imported subtitle file {imported}."),
        (false, true) => {
            format!(
                "{chosen} of {} subtitle track(s) will be written.",
                facts.subtitles.len()
            )
        }
        (false, false) => format!(
            "{chosen} of {} subtitle track(s) will be written, plus the imported file {imported}.",
            facts.subtitles.len()
        ),
    };

    SubtitlesTabViewModel {
        behaviour: behaviour_view,
        forced_only,
        search,
        languages,
        tracks,
        hidden,
        external,
        external_path: settings.external_path.trim().to_string(),
        external_available,
        external_reason: if external_available {
            String::new()
        } else {
            subtitle_passthrough(container, "srt")
                .err()
                .unwrap_or_default()
        },
        summary,
        ready: problems.is_empty(),
        problems,
    }
}

/// One track, with what it asks for checked against what is possible.
fn track_view(
    track: &SubtitleFacts,
    settings: &SubtitleSettings,
    facts: &TitleFacts,
    container: &str,
    job_behaviour: SubtitleBehaviour,
) -> SubtitleTrackViewModel {
    let behaviour = settings.behaviour_of(track.number);
    let enabled = settings
        .tracks
        .get(&track.number)
        .map(|entry| entry.enabled)
        .unwrap_or(false);

    let (available, reason) = match behaviour {
        SubtitleBehaviour::None => (true, String::new()),
        SubtitleBehaviour::BurnIn => match burn_in_available(facts) {
            Ok(()) => (true, String::new()),
            Err(reason) => (false, reason),
        },
        SubtitleBehaviour::Passthrough => match subtitle_passthrough(container, &track.codec) {
            Ok(()) => (true, String::new()),
            Err(reason) => (false, reason),
        },
    };

    let options: Vec<crate::tabs::common::ChoiceOption> = vec![
        crate::tabs::common::ChoiceOption::new("inherit", "Follow the job"),
        crate::tabs::common::ChoiceOption::new("none", "Leave it out"),
        crate::tabs::common::ChoiceOption::new("passthrough", "Write beside the picture"),
        crate::tabs::common::ChoiceOption::new("burn_in", "Draw onto the picture"),
    ];
    let chosen = settings
        .tracks
        .get(&track.number)
        .map(|entry| entry.behaviour.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("inherit");
    let mut behaviour_view = ChoiceGroupViewModel::build(
        &format!("subtitleBehaviourTrack{}", track.number),
        "What happens",
        "Whether this track follows the job or does something of its own.",
        chosen,
        &options,
        true,
        "",
    );
    let effective = if chosen == "inherit" || chosen.is_empty() {
        job_behaviour
    } else {
        behaviour
    };
    if !available {
        behaviour_view = behaviour_view.blocked(reason.clone());
    }

    let forced_only = ToggleViewModel::new(
        &format!("subtitleForcedTrack{}", track.number),
        "Forced dialogue only",
        "Keeps only the parts the source marked as forced.",
        settings
            .tracks
            .get(&track.number)
            .map(|entry| entry.forced_only)
            .unwrap_or(settings.forced_only),
    );

    let summary = if !enabled {
        "Not chosen for this output.".to_string()
    } else {
        match effective {
            SubtitleBehaviour::None => "Left out.".to_string(),
            SubtitleBehaviour::BurnIn => "Drawn onto the picture.".to_string(),
            SubtitleBehaviour::Passthrough => "Written beside the picture.".to_string(),
        }
    };

    SubtitleTrackViewModel {
        number: track.number,
        label: track.label.clone(),
        enabled,
        available,
        reason,
        behaviour: behaviour_view,
        forced_only,
        summary,
    }
}

/// The subtitle choices as the job model wants them.
pub fn to_job_options(
    settings: &SubtitleSettings,
    facts: &TitleFacts,
    container: &str,
) -> Result<(SubtitleMode, Vec<SubtitleTrackConfig>, Option<String>), String> {
    let mut tracks: Vec<SubtitleTrackConfig> = Vec::new();
    let mut burning = false;
    let mut writing = false;
    for track in &facts.subtitles {
        if !settings
            .tracks
            .get(&track.number)
            .map(|entry| entry.enabled)
            .unwrap_or(false)
        {
            continue;
        }
        let behaviour = settings.behaviour_of(track.number);
        match behaviour {
            SubtitleBehaviour::None => continue,
            SubtitleBehaviour::BurnIn => {
                burn_in_available(facts)?;
                burning = true;
            }
            SubtitleBehaviour::Passthrough => {
                subtitle_passthrough(container, &track.codec)?;
                writing = true;
            }
        }
        // A track may turn forced-only on for itself, but a track that says nothing
        // follows the job-wide switch.
        let forced_only = settings.forced_only
            || settings
                .tracks
                .get(&track.number)
                .map(|entry| entry.forced_only)
                .unwrap_or(false);
        tracks.push(SubtitleTrackConfig {
            source_track_id: Some(track.number),
            mode: Some(match behaviour {
                SubtitleBehaviour::BurnIn => SubtitleMode::BurnIn,
                SubtitleBehaviour::None => SubtitleMode::None,
                SubtitleBehaviour::Passthrough => SubtitleMode::Passthrough,
            }),
            copy: behaviour == SubtitleBehaviour::Passthrough,
            burn_in: behaviour == SubtitleBehaviour::BurnIn,
            forced_only,
            language: Some(track.language_code.trim().to_string()),
            ..Default::default()
        });
    }
    let external = match settings.external_path.trim() {
        "" => None,
        path => Some(path.to_string()),
    };
    if external.is_some() {
        writing = true;
    }
    let mode = match (burning, writing) {
        (true, _) => SubtitleMode::BurnIn,
        (false, true) => SubtitleMode::Passthrough,
        (false, false) => SubtitleMode::None,
    };
    Ok((mode, tracks, external))
}

/// The job-wide subtitle processing options.
pub fn to_job_processing(external: Option<String>) -> SubtitleProcessingOptions {
    SubtitleProcessingOptions {
        input_path: external,
        srt_to_mp4: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subtitle(number: u32, codec: &str, language: &str, code: &str) -> SubtitleFacts {
        SubtitleFacts {
            number,
            language: language.into(),
            language_code: code.into(),
            codec: codec.into(),
            description: String::new(),
            forced: false,
            label: format!("Subtitle {number} · {language} · {codec}"),
        }
    }

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            subtitles: vec![
                subtitle(1, "subrip", "English", "eng"),
                subtitle(2, "pgssub", "English", "eng"),
                subtitle(3, "subrip", "German", "deu"),
            ],
            ..Default::default()
        }
    }

    fn track(view: &SubtitlesTabViewModel, number: u32) -> &SubtitleTrackViewModel {
        view.tracks
            .iter()
            .find(|track| track.number == number)
            .unwrap_or_else(|| panic!("track {number}"))
    }

    #[test]
    fn the_defaults_pass_chosen_tracks_through() {
        let mut settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "mkv");
        assert!(view.ready, "{:?}", view.problems);
        assert!(track(&view, 1).enabled);
        assert!(track(&view, 1).available);
        assert_eq!(track(&view, 1).summary, "Written beside the picture.");
        assert!(view.summary.contains("1 of 3"));
    }

    #[test]
    fn a_bitmap_track_cannot_be_written_into_an_mp4() {
        let mut settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[2]);
        let view = tab_view(&settings, &facts(), "mp4");
        assert!(!view.ready);
        assert!(!track(&view, 2).available);
        assert!(
            track(&view, 2).reason.contains("bitmap"),
            "{}",
            track(&view, 2).reason
        );
        assert!(view.problems.iter().any(|p| p.contains("bitmap")));
        let error = to_job_options(&settings, &facts(), "mp4").expect_err("cannot be written");
        assert!(error.contains("bitmap"), "{error}");

        // Matroska carries it happily.
        let mkv = tab_view(&settings, &facts(), "mkv");
        assert!(track(&mkv, 2).available);
    }

    #[test]
    fn webm_only_takes_webvtt() {
        let mut settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "webm");
        assert!(!track(&view, 1).available);
        assert!(
            track(&view, 1).reason.contains("WebVTT"),
            "{}",
            track(&view, 1).reason
        );
    }

    #[test]
    fn burn_in_needs_a_picture_to_draw_on() {
        let mut settings = SubtitleSettings {
            behaviour: "burn_in".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "mp4");
        assert!(view.ready, "{:?}", view.problems);

        let silent = TitleFacts {
            number: 1,
            audio: vec![Default::default()],
            ..Default::default()
        };
        let view = tab_view(&settings, &silent, "mp4");
        assert!(!view.behaviour.available);
        assert!(
            view.behaviour.reason.contains("no picture"),
            "{}",
            view.behaviour.reason
        );
        assert!(!view.ready);

        let nothing = tab_view(&settings, &TitleFacts::default(), "mp4");
        assert!(nothing.behaviour.reason.contains("Choose a title"));
    }

    #[test]
    fn a_track_can_step_outside_the_job_behaviour() {
        let mut settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        settings.tracks.get_mut(&2).expect("track").behaviour = "none".into();
        let view = tab_view(&settings, &facts(), "mkv");
        assert_eq!(track(&view, 2).behaviour.selected, "none");
        assert_eq!(
            track(&view, 2).summary,
            "Left out.",
            "a track that steps out of the job behaviour says so"
        );
        let (mode, tracks, _) = to_job_options(&settings, &facts(), "mkv").expect("job");
        assert_eq!(mode, SubtitleMode::Passthrough);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].source_track_id, Some(1));
    }

    #[test]
    fn the_search_hides_tracks_without_matching_their_language() {
        let mut settings = SubtitleSettings {
            search: "german".into(),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts(), "mkv");
        assert_eq!(view.tracks.len(), 1);
        assert_eq!(view.hidden, 2);
        assert_eq!(view.tracks[0].number, 3);
        assert!(view
            .languages
            .iter()
            .any(|entry| entry.language == "German" && entry.matched));
        assert!(view
            .languages
            .iter()
            .any(|entry| entry.language == "English" && !entry.matched));

        settings.search = "eng".into();
        let by_code = tab_view(&settings, &facts(), "mkv");
        assert_eq!(by_code.tracks.len(), 2, "a code matches its tracks");
    }

    #[test]
    fn dropping_subtitles_writes_nothing() {
        let mut settings = SubtitleSettings {
            behaviour: "none".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1, 2, 3]);
        let view = tab_view(&settings, &facts(), "mp4");
        assert!(view.ready, "{:?}", view.problems);
        assert!(
            track(&view, 2).available,
            "a dropped track is always possible"
        );
        let (mode, tracks, _) = to_job_options(&settings, &facts(), "mp4").expect("job");
        assert_eq!(mode, SubtitleMode::None);
        assert!(tracks.is_empty());
    }

    #[test]
    fn an_imported_file_is_used_when_no_track_is_chosen() {
        let mut settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            external_path: "C:\\media\\extra.srt".into(),
            external_name: "extra.srt".into(),
            ..Default::default()
        };
        settings.sync(&facts());
        let view = tab_view(&settings, &facts(), "mp4");
        assert_eq!(view.external.text, "extra.srt");
        assert!(view.external_available);
        assert!(view.summary.contains("extra.srt"), "{}", view.summary);
        let (mode, tracks, external) = to_job_options(&settings, &facts(), "mp4").expect("job");
        assert_eq!(mode, SubtitleMode::Passthrough);
        assert!(tracks.is_empty());
        assert_eq!(external.as_deref(), Some("C:\\media\\extra.srt"));
        assert_eq!(
            to_job_processing(external.clone()).input_path.as_deref(),
            Some("C:\\media\\extra.srt")
        );
        assert!(to_job_processing(external).srt_to_mp4);
    }

    #[test]
    fn an_imported_file_is_refused_when_the_container_cannot_carry_it() {
        let settings = SubtitleSettings {
            behaviour: "passthrough".into(),
            external_path: "C:\\media\\extra.srt".into(),
            external_name: "extra.srt".into(),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts(), "webm");
        assert!(!view.external_available);
        assert!(!view.external.valid);
        assert!(
            view.external.problem.contains("WebVTT"),
            "{}",
            view.external.problem
        );
        assert!(view.external_reason.contains("WebVTT"));
    }

    #[test]
    fn a_burn_in_chosen_forced_track_reaches_the_job_model() {
        let mut settings = SubtitleSettings {
            behaviour: "burn_in".into(),
            forced_only: true,
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let (mode, tracks, _) = to_job_options(&settings, &facts(), "mp4").expect("job");
        assert_eq!(mode, SubtitleMode::BurnIn);
        assert!(tracks[0].burn_in);
        assert!(tracks[0].forced_only);
        assert_eq!(tracks[0].language.as_deref(), Some("eng"));
    }

    #[test]
    fn unknown_behaviours_are_refused_with_the_name() {
        let mut settings = SubtitleSettings::default();
        let job = SubtitlesInput {
            behaviour: Some("karaoke".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown behaviour");
        assert!(job.contains("karaoke"), "{job}");

        let track = SubtitleTrackInput {
            track: 1,
            behaviour: Some("hologram".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown track behaviour");
        assert!(track.contains("hologram"), "{track}");

        SubtitlesInput {
            behaviour: Some("Burn In".into()),
            forced_only: Some(true),
            search: Some("eng".into()),
        }
        .apply(&mut settings)
        .expect("valid change");
        assert_eq!(settings.behaviour, "burn_in");
        assert!(settings.forced_only);
        assert_eq!(settings.search, "eng");
    }

    #[test]
    fn syncing_drops_tracks_the_title_does_not_have() {
        let mut settings = SubtitleSettings::default();
        settings.sync(&facts());
        settings.choose(&[1, 2, 3]);
        let mut one = facts();
        one.subtitles.truncate(1);
        settings.sync(&one);
        assert_eq!(settings.tracks.len(), 1);
        assert!(settings.tracks.contains_key(&1));
    }
}
