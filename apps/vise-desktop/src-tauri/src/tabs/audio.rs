//! The audio tab: per-track behaviour, defaults, and what they work out to.
//!
//! Two ideas keep this tab small. First, a set of rules is written once — the
//! defaults — and a track that says nothing of its own inherits them, so a job
//! with four tracks does not need four copies of the same bit rate. Second, a
//! track that is passed through cannot also be re-encoded, mixed, or adjusted,
//! and every control that would do one of those says so instead of sitting
//! there looking like it works.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vise_core::{AudioCodec, AudioMixdown, AudioMode, AudioProcessingOptions, AudioTrackConfig};

use crate::tabs::common::{ChoiceGroupViewModel, NumField, NumberViewModel, ToggleViewModel};
use crate::tabs::compatibility::{
    audio_codec_of, audio_encoder_options, dither_modes, mixdown_is_possible, mixdowns,
    sample_rates,
};
use crate::tabs::source::{AudioFacts, TitleFacts};

/// The rules that decide what happens to an audio track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioRules {
    /// Encoder, or `job` to follow the container's choice.
    pub encoder: String,
    /// Bit rate in kb/s.
    pub bitrate: NumField,
    /// Quality value for the encoders that take one.
    pub quality: NumField,
    /// Mixdown target.
    pub mixdown: String,
    /// Sample rate, or `keep`.
    pub sample_rate: String,
    /// Copy the track instead of encoding it.
    pub passthrough: bool,
    /// Dither after resampling.
    pub dither: bool,
    /// Which dither to use.
    pub dither_mode: String,
    /// Gain in decibels.
    pub gain: NumField,
    /// Delay in milliseconds.
    pub delay: NumField,
    /// Even out the loudness.
    pub normalize: bool,
}

impl Default for AudioRules {
    fn default() -> Self {
        Self {
            encoder: "job".to_string(),
            // A lossy encoder needs a number, and 192 kb/s is a sane stereo
            // starting point, so a job is queueable before the user touches
            // anything.
            bitrate: NumField::of("192"),
            // Opus, Vorbis, and FLAC take a quality value instead of a rate,
            // and they get one for the same reason the bit rate has one: a job
            // is queueable before the user touches anything. Without it a
            // catalog preset that names one of these codecs could not be
            // applied at all, because the tab would refuse a job it cannot
            // describe rather than carry a number it would have had to invent.
            // The two fields are not both sent — the tab sends whichever the
            // chosen encoder takes and blocks the other.
            quality: NumField::of("5"),
            mixdown: "auto".to_string(),
            sample_rate: "keep".to_string(),
            passthrough: false,
            dither: false,
            dither_mode: "none".to_string(),
            gain: NumField::blank(),
            delay: NumField::blank(),
            normalize: false,
        }
    }
}

impl AudioRules {
    /// Whether this set of rules says anything of its own.
    pub fn is_empty(&self) -> bool {
        self == &AudioRules::default()
    }

    /// The track's own value, or the default when the track says nothing.
    ///
    /// A blank field is how a track says "whatever the defaults say", which is
    /// why inheriting is a plain fallback rather than a flag the user sets.
    pub fn inherit(&self, defaults: &AudioRules) -> AudioRules {
        AudioRules {
            encoder: if self.encoder.trim().is_empty() || self.encoder == "job" {
                defaults.encoder.clone()
            } else {
                self.encoder.clone()
            },
            bitrate: if self.bitrate.is_blank() {
                defaults.bitrate.clone()
            } else {
                self.bitrate.clone()
            },
            quality: if self.quality.is_blank() {
                defaults.quality.clone()
            } else {
                self.quality.clone()
            },
            mixdown: if self.mixdown.trim().is_empty() || self.mixdown == "auto" {
                defaults.mixdown.clone()
            } else {
                self.mixdown.clone()
            },
            sample_rate: if self.sample_rate.trim().is_empty() || self.sample_rate == "keep" {
                defaults.sample_rate.clone()
            } else {
                self.sample_rate.clone()
            },
            passthrough: self.passthrough || defaults.passthrough,
            dither: self.dither || defaults.dither,
            dither_mode: if self.dither_mode.trim().is_empty() || self.dither_mode == "none" {
                defaults.dither_mode.clone()
            } else {
                self.dither_mode.clone()
            },
            gain: if self.gain.is_blank() {
                defaults.gain.clone()
            } else {
                self.gain.clone()
            },
            delay: if self.delay.is_blank() {
                defaults.delay.clone()
            } else {
                self.delay.clone()
            },
            // A switch cannot be turned off again by a track once the defaults
            // have turned it on, so a track opts into these by asking for them.
            normalize: self.normalize,
        }
    }

    /// The encoder a track will use, given the job's own audio codec.
    pub fn resolved_encoder(&self, job_codec: &str) -> Option<AudioCodec> {
        if self.passthrough {
            return None;
        }
        if self.encoder.trim().is_empty() || self.encoder == "job" {
            return audio_codec_of(job_codec);
        }
        audio_codec_of(&self.encoder)
    }
}

/// One track's rules as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AudioTrackSettings {
    /// Whether the track goes into the output.
    pub enabled: bool,
    /// The track's own rules; empty means "use the defaults".
    pub rules: AudioRules,
}

/// The audio choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioSettings {
    /// The rules every track starts from.
    pub defaults: AudioRules,
    /// Per-track overrides, by track number.
    pub tracks: BTreeMap<u32, AudioTrackSettings>,
}

impl AudioSettings {
    /// The rules for one track, with the defaults folded in.
    pub fn resolved(&self, number: u32) -> AudioRules {
        self.tracks
            .get(&number)
            .map(|track| track.rules.inherit(&self.defaults))
            .unwrap_or_else(|| self.defaults.clone())
    }

    /// Whether a track goes into the output.
    pub fn enabled(&self, number: u32) -> bool {
        self.tracks
            .get(&number)
            .map(|track| track.enabled)
            .unwrap_or(false)
    }

    /// Make sure every track of the title has an entry, keeping the ones the
    /// user has already touched.
    ///
    /// A new title has different tracks, and an entry that no longer matches
    /// anything would sit in the map forever being counted in nothing.
    pub fn sync(&mut self, facts: &TitleFacts) {
        self.tracks
            .retain(|number, _| facts.audio_track(*number).is_some());
        for track in &facts.audio {
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
}

/// Audio choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioDefaultsInput {
    /// Encoder for every track.
    pub encoder: Option<String>,
    /// Bit rate for every track.
    pub bitrate: Option<String>,
    /// Quality for every track.
    pub quality: Option<String>,
    /// Mixdown for every track.
    pub mixdown: Option<String>,
    /// Sample rate for every track.
    pub sample_rate: Option<String>,
    /// Copy every track instead of encoding it.
    pub passthrough: Option<bool>,
    /// Dither after resampling.
    pub dither: Option<bool>,
    /// Which dither to use.
    pub dither_mode: Option<String>,
    /// Gain in decibels.
    pub gain: Option<String>,
    /// Delay in milliseconds.
    pub delay: Option<String>,
    /// Even out the loudness.
    pub normalize: Option<bool>,
}

impl AudioDefaultsInput {
    /// Apply the change to the default rule set.
    pub fn apply(self, settings: &mut AudioRules, container: &str) -> Result<(), String> {
        apply_common(self.into(), settings, container)
    }
}

/// One track's choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioTrackInput {
    /// Track number the change applies to.
    pub track: u32,
    /// Whether the track goes into the output.
    pub enabled: Option<bool>,
    /// Encoder for this track.
    pub encoder: Option<String>,
    /// Bit rate for this track.
    pub bitrate: Option<String>,
    /// Quality for this track.
    pub quality: Option<String>,
    /// Mixdown for this track.
    pub mixdown: Option<String>,
    /// Sample rate for this track.
    pub sample_rate: Option<String>,
    /// Copy this track instead of encoding it.
    pub passthrough: Option<bool>,
    /// Dither after resampling.
    pub dither: Option<bool>,
    /// Which dither to use.
    pub dither_mode: Option<String>,
    /// Gain in decibels.
    pub gain: Option<String>,
    /// Delay in milliseconds.
    pub delay: Option<String>,
    /// Even out the loudness.
    pub normalize: Option<bool>,
}

impl AudioTrackInput {
    /// Apply the change to one track.
    pub fn apply(self, settings: &mut AudioSettings, container: &str) -> Result<(), String> {
        let entry = settings.tracks.entry(self.track).or_default();
        if let Some(enabled) = self.enabled {
            entry.enabled = enabled;
        }
        let mut rules = entry.rules.clone();
        apply_common(
            (
                self.encoder,
                self.bitrate,
                self.quality,
                self.mixdown,
                self.sample_rate,
                self.passthrough,
                self.dither,
                self.dither_mode,
                self.gain,
                self.delay,
                self.normalize,
            ),
            &mut rules,
            container,
        )?;
        entry.rules = rules;
        Ok(())
    }
}

/// The parts of a change, whichever control sent them.
#[allow(clippy::type_complexity)]
type Change = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
    Option<bool>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
);

#[allow(clippy::too_many_arguments)]
fn apply_common(change: Change, rules: &mut AudioRules, container: &str) -> Result<(), String> {
    let (
        encoder,
        bitrate,
        quality,
        mixdown,
        sample_rate,
        passthrough,
        dither,
        dither_mode,
        gain,
        delay,
        normalize,
    ) = change;
    if let Some(encoder) = encoder {
        let encoder = encoder.trim().to_ascii_lowercase();
        if encoder != "job"
            && !audio_encoder_options(container)
                .iter()
                .any(|option| option.id == encoder && option.available)
        {
            return Err(format!(
                "'{encoder}' is not an audio encoder this build offers."
            ));
        }
        rules.encoder = encoder;
    }
    if let Some(value) = bitrate {
        rules.bitrate = NumField { text: value };
    }
    if let Some(value) = quality {
        rules.quality = NumField { text: value };
    }
    if let Some(value) = mixdown {
        let value = value.trim().to_ascii_lowercase();
        if !mixdowns().iter().any(|option| option.id == value) {
            return Err(format!("'{value}' is not a mixdown this build offers."));
        }
        rules.mixdown = value;
    }
    if let Some(value) = sample_rate {
        let value = value.trim().to_ascii_lowercase();
        if !sample_rates().iter().any(|option| option.id == value) {
            return Err(format!("'{value}' is not a sample rate this build offers."));
        }
        rules.sample_rate = value;
    }
    if let Some(value) = passthrough {
        rules.passthrough = value;
    }
    if let Some(value) = dither {
        rules.dither = value;
    }
    if let Some(value) = dither_mode {
        let value = value.trim().to_ascii_lowercase();
        if !dither_modes().iter().any(|option| option.id == value) {
            return Err(format!("'{value}' is not a dither this build offers."));
        }
        rules.dither_mode = value;
    }
    if let Some(value) = gain {
        rules.gain = NumField { text: value };
    }
    if let Some(value) = delay {
        rules.delay = NumField { text: value };
    }
    if let Some(value) = normalize {
        rules.normalize = value;
    }
    Ok(())
}

impl From<AudioDefaultsInput> for Change {
    fn from(input: AudioDefaultsInput) -> Self {
        (
            input.encoder,
            input.bitrate,
            input.quality,
            input.mixdown,
            input.sample_rate,
            input.passthrough,
            input.dither,
            input.dither_mode,
            input.gain,
            input.delay,
            input.normalize,
        )
    }
}

/// The rules as the page renders them, for the defaults row and for one track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRulesViewModel {
    /// Encoder picker.
    pub encoder: ChoiceGroupViewModel,
    /// Bit-rate field.
    pub bitrate: NumberViewModel,
    /// Quality field.
    pub quality: NumberViewModel,
    /// Mixdown picker.
    pub mixdown: ChoiceGroupViewModel,
    /// Sample-rate picker.
    pub sample_rate: ChoiceGroupViewModel,
    /// Copy-the-track switch.
    pub passthrough: ToggleViewModel,
    /// Dither switch.
    pub dither: ToggleViewModel,
    /// Dither picker.
    pub dither_mode: ChoiceGroupViewModel,
    /// Gain field.
    pub gain: NumberViewModel,
    /// Delay field.
    pub delay: NumberViewModel,
    /// Loudness-evening switch.
    pub normalize: ToggleViewModel,
    /// What the rules will do.
    pub summary: String,
    /// What has to be fixed before the track can be used.
    pub problems: Vec<String>,
    /// Whether the rules can be used.
    pub ready: bool,
}

/// One audio track as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrackViewModel {
    /// Track number inside the title.
    pub number: u32,
    /// Track label from the scan.
    pub label: String,
    /// Whether the track goes into the output.
    pub enabled: bool,
    /// Which rules the track uses.
    pub rules: AudioRulesViewModel,
    /// Rule names the track has set itself rather than inheriting.
    pub own: Vec<String>,
}

/// The audio tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTabViewModel {
    /// The rules every track starts from.
    pub defaults: AudioRulesViewModel,
    /// Every audio track of the title.
    pub tracks: Vec<AudioTrackViewModel>,
    /// One line describing how many tracks will be encoded.
    pub summary: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Build the audio tab.
pub fn tab_view(
    settings: &AudioSettings,
    facts: &TitleFacts,
    container: &str,
    job_codec: &str,
) -> AudioTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let defaults = rules_view(
        &settings.defaults,
        None,
        container,
        job_codec,
        "Default",
        "default",
    );
    problems.extend(defaults.problems.iter().cloned());

    let tracks: Vec<AudioTrackViewModel> = facts
        .audio
        .iter()
        .map(|track| {
            let settings_for_track = settings.tracks.get(&track.number);
            let resolved = settings.resolved(track.number);
            let rules = rules_view(
                &resolved,
                Some(track),
                container,
                job_codec,
                &format!("Track {}", track.number),
                &format!("track{}", track.number),
            );
            problems.extend(rules.problems.iter().cloned());
            AudioTrackViewModel {
                number: track.number,
                label: track.label.clone(),
                enabled: settings.enabled(track.number),
                own: own_rules(settings_for_track.map(|entry| &entry.rules)),
                rules,
            }
        })
        .collect();

    let chosen = tracks.iter().filter(|track| track.enabled).count();
    let summary = if facts.audio.is_empty() {
        "This title carries no audio tracks.".to_string()
    } else {
        format!(
            "{chosen} of {} track(s) will be encoded.",
            facts.audio.len()
        )
    };

    AudioTabViewModel {
        defaults,
        tracks,
        summary,
        ready: problems.is_empty(),
        problems,
    }
}

/// The rule names a track sets itself.
///
/// A track that says nothing has the same rules as a fresh set, so only a
/// difference from that counts as the track's own choice.
fn own_rules(rules: Option<&AudioRules>) -> Vec<String> {
    let Some(rules) = rules else {
        return Vec::new();
    };
    let plain = AudioRules::default();
    let mut own: Vec<String> = Vec::new();
    if rules.encoder != plain.encoder {
        own.push("encoder".to_string());
    }
    if rules.bitrate != plain.bitrate {
        own.push("bit rate".to_string());
    }
    if rules.quality != plain.quality {
        own.push("quality".to_string());
    }
    if rules.mixdown != plain.mixdown {
        own.push("mixdown".to_string());
    }
    if rules.sample_rate != plain.sample_rate {
        own.push("sample rate".to_string());
    }
    if rules.passthrough {
        own.push("passthrough".to_string());
    }
    if rules.dither {
        own.push("dither".to_string());
    }
    if rules.dither_mode != plain.dither_mode {
        own.push("dither kind".to_string());
    }
    if rules.gain != plain.gain {
        own.push("gain".to_string());
    }
    if rules.delay != plain.delay {
        own.push("delay".to_string());
    }
    if rules.normalize {
        own.push("loudness".to_string());
    }
    own
}

/// Render one rule set.
///
/// `label` names the rule set in any problem, so a message about track 2 does
/// not read as though it were about the whole job, while `owner` is the same
/// name without spaces so it can be used in element identifiers.
fn rules_view(
    rules: &AudioRules,
    track: Option<&AudioFacts>,
    container: &str,
    job_codec: &str,
    label: &str,
    owner: &str,
) -> AudioRulesViewModel {
    let mut problems: Vec<String> = Vec::new();
    let channels = track.map(|track| track.channel_count).unwrap_or(0);
    let copied = rules.passthrough;
    let copy_reason =
        "A copied track is written straight across, so it cannot also be re-encoded or adjusted.";
    let track_name = track
        .map(|track| format!("track {}", track.number))
        .unwrap_or_else(|| "a track".to_string());

    let encoder = ChoiceGroupViewModel::build(
        &format!("audioEncoder{owner}"),
        "Encoder",
        "Which audio coding this track uses.",
        &rules.encoder,
        &audio_encoder_options(container),
        !copied,
        if copied { copy_reason } else { "" },
    );

    let encoder_codec = rules.resolved_encoder(job_codec).unwrap_or(AudioCodec::Aac);
    // The two encoders that are asked for a quality value instead of a rate.
    let quality_needed = matches!(encoder_codec, AudioCodec::Vorbis | AudioCodec::Opus);
    let lossless = matches!(encoder_codec, AudioCodec::Flac);

    let mut bitrate = NumberViewModel::new(
        &format!("audioBitrate{owner}"),
        "Bit rate",
        rules.bitrate.text.trim(),
        "kb/s",
    )
    .ranged(8.0, 1536.0);
    let mut quality = NumberViewModel::new(
        &format!("audioQuality{owner}"),
        "Quality",
        rules.quality.text.trim(),
        "",
    )
    .ranged(-1.0, 10.0);

    if copied {
        for field in [&mut bitrate, &mut quality] {
            *field = field.clone().blocked(copy_reason);
        }
    } else if lossless {
        for field in [&mut bitrate, &mut quality] {
            *field = field
                .clone()
                .blocked("A lossless encoder does not take a bit rate or a quality value.");
        }
    } else if quality_needed {
        bitrate =
            bitrate.blocked("The chosen encoder takes a quality value instead of a bit rate.");
        if rules.quality.is_blank() {
            problems.push(format!(
                "{label} uses an encoder that takes a quality value, so one is needed."
            ));
        } else if let Ok(Some(value)) = rules.quality.number("Quality") {
            if !(-1.0..=10.0).contains(&value) {
                problems.push(format!("{label}: quality must be between -1 and 10."));
            }
        }
    } else {
        quality =
            quality.blocked("The chosen encoder takes a bit rate instead of a quality value.");
        if rules.bitrate.is_blank() {
            problems.push(format!("{label} needs a bit rate for the chosen encoder."));
        } else if let Ok(Some(value)) = rules.bitrate.whole("Bit rate") {
            if !(8..=1536).contains(&value) {
                problems.push(format!(
                    "{label}: bit rate must be between 8 and 1536 kb/s."
                ));
            }
        }
    }
    if let Err(problem) = rules.bitrate.whole("Bit rate") {
        problems.push(format!("{label}: {problem}"));
    }
    if let Err(problem) = rules.quality.number("Quality") {
        problems.push(format!("{label}: {problem}"));
    }

    // A mixdown is only meaningful when the track is being encoded.
    let mut mixdown = ChoiceGroupViewModel::build(
        &format!("audioMixdown{owner}"),
        "Mixdown",
        "How many channels the output track carries.",
        &rules.mixdown,
        &mixdowns(),
        !copied,
        if copied { copy_reason } else { "" },
    );
    if !copied {
        if let Err(problem) = mixdown_is_possible(&rules.mixdown, channels) {
            // A track that simply says nothing is told why it cannot answer;
            // a track that asked for an impossible mixdown is refused.
            if channels > 0 && !rules.mixdown.eq_ignore_ascii_case("auto") {
                problems.push(format!("{label}: {problem}"));
            } else {
                mixdown = mixdown.blocked(problem.clone());
            }
        }
    }

    let sample_rate = ChoiceGroupViewModel::build(
        &format!("audioSampleRate{owner}"),
        "Sample rate",
        "How many samples a second the output carries.",
        &rules.sample_rate,
        &sample_rates(),
        !copied,
        if copied { copy_reason } else { "" },
    );

    let mut dither = ToggleViewModel::new(
        &format!("audioDither{owner}"),
        "Dither",
        "Rounds the noise away when the sample rate changes.",
        rules.dither,
    );
    let dither_mode = ChoiceGroupViewModel::build(
        &format!("audioDitherMode{owner}"),
        "Dither kind",
        "Which shape of noise the dither adds.",
        &rules.dither_mode,
        &dither_modes(),
        rules.dither && !copied,
        if copied {
            copy_reason
        } else if !rules.dither {
            "Turn dither on to choose its shape."
        } else {
            ""
        },
    );
    if copied {
        dither = dither.blocked(copy_reason);
    }
    if rules.dither && rules.dither_mode == "none" {
        // Dither is on but no shape is chosen, which would round the same way as
        // no dither at all.
        dither = dither.blocked("Choose a dither shape as well as turning dither on.");
        problems.push(format!(
            "{label}: dither is on but no dither shape is chosen."
        ));
    }

    let mut gain = NumberViewModel::new(
        &format!("audioGain{owner}"),
        "Gain",
        rules.gain.text.trim(),
        "dB",
    )
    .ranged(-60.0, 24.0);
    let mut delay = NumberViewModel::new(
        &format!("audioDelay{owner}"),
        "Delay",
        rules.delay.text.trim(),
        "ms",
    )
    .ranged(-10_000.0, 10_000.0);
    for field in [&mut gain, &mut delay] {
        if copied {
            *field = field.clone().blocked(copy_reason);
        }
    }
    if let Err(problem) = rules.gain.number("Gain") {
        gain = gain.invalid(problem.clone());
        problems.push(format!("{label}: {problem}"));
    }
    if let Err(problem) = rules.delay.number("Delay") {
        delay = delay.invalid(problem.clone());
        problems.push(format!("{label}: {problem}"));
    }
    let normalize = ToggleViewModel::new(
        &format!("audioNormalize{owner}"),
        "Even out the loudness",
        "Levels the track to a broadcast-style target.",
        rules.normalize,
    );

    let summary = if copied {
        format!("{track_name} is copied into the output as it is.")
    } else {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!(
            "encoded as {}",
            rules
                .resolved_encoder(job_codec)
                .map(|codec| format!("{codec:?}").to_lowercase())
                .unwrap_or_else(|| "nothing".to_string())
        ));
        if !rules.bitrate.is_blank() {
            parts.push(format!("{} kb/s", rules.bitrate.text.trim()));
        }
        if !rules.quality.is_blank() {
            parts.push(format!("quality {}", rules.quality.text.trim()));
        }
        if !rules.mixdown.eq_ignore_ascii_case("auto") {
            parts.push(format!("mixed to {}", rules.mixdown));
        }
        if !rules.sample_rate.eq_ignore_ascii_case("keep") {
            parts.push(format!("at {} Hz", rules.sample_rate));
        }
        if !rules.gain.is_blank() {
            parts.push(format!("gain {} dB", rules.gain.text.trim()));
        }
        if !rules.delay.is_blank() {
            parts.push(format!("delay {} ms", rules.delay.text.trim()));
        }
        if rules.dither {
            parts.push(format!("{} dither", rules.dither_mode));
        }
        if rules.normalize {
            parts.push("loudness evened out".to_string());
        }
        format!("{track_name} is {}.", parts.join(", "))
    };

    let ready = problems.is_empty();
    AudioRulesViewModel {
        encoder,
        bitrate,
        quality,
        mixdown,
        sample_rate,
        passthrough: ToggleViewModel::new(
            &format!("audioPassthrough{owner}"),
            "Copy instead of encode",
            "Writes the track across without touching it.",
            copied,
        ),
        dither,
        dither_mode,
        gain,
        delay,
        normalize,
        summary,
        problems,
        ready,
    }
}

/// The audio choices as the job model wants them.
///
/// The first track's rules also become the job-wide audio options, which is what
/// the worker reads when it builds the audio filter chain.
pub fn to_job_options(
    settings: &AudioSettings,
    facts: &TitleFacts,
    job_codec: &str,
) -> Result<(AudioProcessingOptions, Vec<AudioTrackConfig>), String> {
    let mut tracks: Vec<AudioTrackConfig> = Vec::new();
    let mut first: Option<AudioProcessingOptions> = None;
    for track in &facts.audio {
        if !settings.enabled(track.number) {
            continue;
        }
        let rules = settings.resolved(track.number);
        let channels = mixdown_is_possible(&rules.mixdown, track.channel_count)?;
        let sample_rate = if rules.sample_rate == "keep" {
            None
        } else {
            rules.sample_rate.parse::<u32>().ok()
        };
        let config = AudioTrackConfig {
            source_track_id: Some(track.number),
            mode: Some(if rules.passthrough {
                AudioMode::Passthrough
            } else {
                AudioMode::Encode
            }),
            encoder: rules
                .resolved_encoder(job_codec)
                .map(|codec| format!("{codec:?}").to_lowercase()),
            bitrate: rules.bitrate.whole("Bit rate")?,
            quality: rules.quality.number("Quality")?,
            mixdown: match &rules.mixdown {
                mixdown if mixdown.eq_ignore_ascii_case("auto") => None,
                other => Some(other.clone()),
            },
            mixdown_enum: Some(match rules.mixdown.as_str() {
                "mono" => AudioMixdown::Mono,
                "stereo" => AudioMixdown::Stereo,
                "5.1" => AudioMixdown::Surround51,
                "7.1" => AudioMixdown::Surround71,
                _ => AudioMixdown::Auto,
            }),
            sample_rate,
            dither: rules.dither,
            dither_mode: if rules.dither {
                Some(rules.dither_mode.clone())
            } else {
                None
            },
            gain: None,
            volume: rules.gain.number("Gain")?,
            delay: rules.delay.number("Delay")?,
            passthrough: rules.passthrough,
            channel_count: channels,
        };
        if first.is_none() {
            first = Some(AudioProcessingOptions {
                source_track: Some(track.number),
                source_tracks: vec![track.number],
                mixdown: config.mixdown.clone(),
                mixdown_enum: config.mixdown_enum,
                channels,
                layout: None,
                sample_rate,
                dither: rules.dither,
                dither_mode: config.dither_mode.clone(),
                gain: None,
                volume: config.volume,
                delay: config.delay,
                delay_seconds: None,
                passthrough: rules.passthrough,
                encoder: config.encoder.clone(),
                bitrate: config.bitrate,
                quality: config.quality,
                normalize: rules.normalize,
                copy: false,
            });
        }
        tracks.push(config);
    }
    Ok((first.unwrap_or_default(), tracks))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(number: u32, channels: u32, layout: &str) -> AudioFacts {
        AudioFacts {
            number,
            language: "English".into(),
            language_code: "eng".into(),
            codec: "eac3".into(),
            channel_count: channels,
            channel_layout: layout.into(),
            sample_rate: 48_000,
            bit_rate: 640_000,
            label: format!("Audio {number} · English · eac3 · {layout}"),
            foreign_language: false,
        }
    }

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            audio: vec![audio(1, 6, "5.1"), audio(2, 2, "stereo")],
            ..Default::default()
        }
    }

    fn track(view: &AudioTabViewModel, number: u32) -> &AudioTrackViewModel {
        view.tracks
            .iter()
            .find(|track| track.number == number)
            .unwrap_or_else(|| panic!("track {number}"))
    }

    #[test]
    fn the_defaults_encode_every_track_at_the_jobs_codec() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.summary, "1 of 2 track(s) will be encoded.");
        assert!(track(&view, 1).enabled);
        assert!(!track(&view, 2).enabled);
        let rules = &track(&view, 1).rules;
        assert!(rules.ready, "{:?}", rules.problems);
        assert!(rules.summary.contains("aac"), "{}", rules.summary);
        assert_eq!(rules.encoder.selected, "job");
    }

    #[test]
    fn a_track_inherits_the_defaults_until_it_says_otherwise() {
        let defaults = AudioRules {
            bitrate: NumField::of("192"),
            mixdown: "stereo".into(),
            sample_rate: "48000".into(),
            ..Default::default()
        };
        let mut settings = AudioSettings {
            defaults,
            tracks: BTreeMap::new(),
        };
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        settings.tracks.get_mut(&1).expect("track").rules.bitrate = NumField::of("320");
        settings.tracks.get_mut(&2).expect("track").rules.mixdown = "mono".into();

        let view = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(track(&view, 1).rules.bitrate.text == "320");
        assert!(
            track(&view, 1).rules.mixdown.selected == "stereo",
            "inherited"
        );
        assert!(track(&view, 2).rules.mixdown.selected == "mono");
        assert!(track(&view, 2).rules.bitrate.text == "192", "inherited");
        assert_eq!(track(&view, 1).own, vec!["bit rate".to_string()]);
        assert_eq!(track(&view, 2).own, vec!["mixdown".to_string()]);
    }

    #[test]
    fn a_copied_track_closes_every_control_that_would_re_encode_it() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        settings.choose(&[1]);
        settings
            .tracks
            .get_mut(&1)
            .expect("track")
            .rules
            .passthrough = true;
        let view = tab_view(&settings, &facts(), "mp4", "aac");
        let rules = &track(&view, 1).rules;
        assert!(rules.passthrough.checked);
        for (closed, label) in [
            (rules.encoder.available, "encoder"),
            (rules.bitrate.available, "bit rate"),
            (rules.quality.available, "quality"),
            (rules.mixdown.available, "mixdown"),
            (rules.sample_rate.available, "sample rate"),
            (rules.dither.available, "dither"),
            (rules.gain.available, "gain"),
            (rules.delay.available, "delay"),
        ] {
            assert!(!closed, "{label} should be closed");
        }
        assert!(
            rules.encoder.reason.contains("copied"),
            "{}",
            rules.encoder.reason
        );
        assert!(
            rules.mixdown.reason.contains("copied"),
            "{}",
            rules.mixdown.reason
        );
        assert!(rules.summary.contains("copied"), "{}", rules.summary);
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_mixdown_cannot_invent_speakers_the_track_does_not_have() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        settings.tracks.get_mut(&2).expect("track").rules.mixdown = "7.1".into();
        let view = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(!view.ready);
        assert!(
            track(&view, 2)
                .rules
                .problems
                .iter()
                .any(|p| p.contains("cannot be mixed")),
            "{:?}",
            track(&view, 2).rules.problems
        );
        // Mixing down is still fine.
        settings.tracks.get_mut(&1).expect("track").rules.mixdown = "stereo".into();
        settings.tracks.get_mut(&2).expect("track").rules.mixdown = "auto".into();
        let view = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn the_encoder_decides_whether_a_bit_rate_or_a_quality_is_wanted() {
        let mut settings = AudioSettings {
            defaults: AudioRules {
                bitrate: NumField::of("192"),
                ..Default::default()
            },
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let aac = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(track(&aac, 1).rules.bitrate.available);
        assert!(!track(&aac, 1).rules.quality.available);
        assert!(track(&aac, 1).rules.quality.reason.contains("bit rate"));

        settings.defaults.encoder = "opus".into();
        settings.defaults.quality = NumField::of("5");
        settings.defaults.bitrate = NumField::blank();
        let opus = tab_view(&settings, &facts(), "mkv", "aac");
        assert!(track(&opus, 1).rules.quality.available);
        assert!(!track(&opus, 1).rules.bitrate.available);
        assert!(track(&opus, 1).rules.summary.contains("opus"));

        settings.defaults.encoder = "flac".into();
        let flac = tab_view(&settings, &facts(), "mkv", "aac");
        assert!(!track(&flac, 1).rules.bitrate.available);
        assert!(track(&flac, 1).rules.bitrate.reason.contains("lossless"));
    }

    #[test]
    fn an_encoder_with_a_quality_value_needs_one() {
        let mut settings = AudioSettings {
            defaults: AudioRules {
                encoder: "vorbis".into(),
                // The rules start with a quality value so a job is queueable
                // before anything is touched, so this asks about the case that
                // default does not cover: the user clearing the field.
                quality: NumField::blank(),
                ..Default::default()
            },
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "mkv", "aac");
        assert!(!view.ready);
        assert!(
            view.problems.iter().any(|p| p.contains("quality value")),
            "{:?}",
            view.problems
        );
    }

    #[test]
    fn numbers_outside_their_range_are_named_in_the_message() {
        let mut settings = AudioSettings {
            defaults: AudioRules {
                bitrate: NumField::of("5000"),
                gain: NumField::of("loud"),
                ..Default::default()
            },
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1]);
        let view = tab_view(&settings, &facts(), "mp4", "aac");
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("between 8 and 1536")));
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("Gain must be a number")));
    }

    #[test]
    fn unknown_names_are_refused() {
        let mut settings = AudioSettings::default();
        let encoder = AudioDefaultsInput {
            encoder: Some("dts".into()),
            ..Default::default()
        }
        .apply(&mut settings.defaults, "mkv")
        .expect_err("unknown encoder");
        assert!(encoder.contains("dts"), "{encoder}");

        let mixdown = AudioDefaultsInput {
            mixdown: Some("quad".into()),
            ..Default::default()
        }
        .apply(&mut settings.defaults, "mkv")
        .expect_err("unknown mixdown");
        assert!(mixdown.contains("quad"), "{mixdown}");

        let rate = AudioDefaultsInput {
            sample_rate: Some("96000".into()),
            ..Default::default()
        }
        .apply(&mut settings.defaults, "mkv")
        .expect_err("unknown sample rate");
        assert!(rate.contains("96000"), "{rate}");

        let dither = AudioTrackInput {
            track: 1,
            dither_mode: Some("blue".into()),
            ..Default::default()
        }
        .apply(&mut settings, "mkv")
        .expect_err("unknown dither");
        assert!(dither.contains("blue"), "{dither}");

        AudioDefaultsInput {
            encoder: Some("job".into()),
            bitrate: Some("256".into()),
            ..Default::default()
        }
        .apply(&mut settings.defaults, "mp4")
        .expect("valid change");
        assert_eq!(settings.defaults.bitrate, NumField::of("256"));
    }

    #[test]
    fn a_track_change_only_touches_that_track() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        AudioTrackInput {
            track: 2,
            enabled: Some(true),
            bitrate: Some("128".into()),
            ..Default::default()
        }
        .apply(&mut settings, "mp4")
        .expect("track change");
        assert!(settings.enabled(2));
        assert!(!settings.enabled(1));
        assert_eq!(settings.tracks[&2].rules.bitrate, NumField::of("128"));
        assert!(settings.tracks[&1].rules.is_empty());
    }

    #[test]
    fn the_job_model_gets_a_track_per_enabled_track_and_no_others() {
        let mut settings = AudioSettings {
            defaults: AudioRules {
                bitrate: NumField::of("192"),
                ..Default::default()
            },
            ..Default::default()
        };
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        settings.tracks.get_mut(&2).expect("track").rules.mixdown = "mono".into();
        settings.tracks.get_mut(&2).expect("track").rules.delay = NumField::of("250");
        let (options, tracks) = to_job_options(&settings, &facts(), "aac").expect("job options");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].source_track_id, Some(1));
        assert_eq!(tracks[0].bitrate, Some(192));
        assert_eq!(tracks[0].encoder.as_deref(), Some("aac"));
        assert_eq!(tracks[1].mixdown.as_deref(), Some("mono"));
        assert_eq!(tracks[1].mixdown_enum, Some(AudioMixdown::Mono));
        assert_eq!(
            tracks[1].channel_count,
            Some(1),
            "a mono mixdown is one channel"
        );
        assert_eq!(tracks[1].delay, Some(250.0));
        assert_eq!(options.source_track, Some(1));
        assert_eq!(options.bitrate, Some(192));

        settings.choose(&[2]);
        let (_, only) = to_job_options(&settings, &facts(), "aac").expect("job options");
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].source_track_id, Some(2));
    }

    #[test]
    fn a_track_that_cannot_be_mixed_refuses_the_whole_job() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        settings.tracks.get_mut(&2).expect("track").rules.mixdown = "7.1".into();
        let error = to_job_options(&settings, &facts(), "aac").expect_err("cannot be mixed");
        assert!(error.contains("cannot be mixed"), "{error}");
    }

    #[test]
    fn syncing_drops_tracks_the_title_does_not_have() {
        let mut settings = AudioSettings::default();
        settings.sync(&facts());
        settings.choose(&[1, 2]);
        let mut one_track = facts();
        one_track.audio.truncate(1);
        settings.sync(&one_track);
        assert_eq!(settings.tracks.len(), 1);
        assert!(settings.tracks.contains_key(&1));
    }
}
