//! The video tab: encoder, rate control, and encoder-side choices.
//!
//! Rate control is the part of this tab that has real rules. Quality wants a
//! number in a known range, a bitrate wants a number, a target size wants a
//! duration to work it out from, and a second pass only means anything for a
//! bitrate or a size. Those rules are resolved here so the tab can show the
//! consequence of a choice before the job is queued.

use serde::{Deserialize, Serialize};
use vise_core::{DolbyVisionAction, Hdr10PlusAction};

use crate::tabs::common::{
    number_in_range, ChoiceGroup, ChoiceGroupViewModel, ChoiceOption, NumField, NumberViewModel,
    ToggleViewModel,
};
use crate::tabs::compatibility::{
    encoder_levels, encoder_presets, encoder_profiles, encoder_tunes, video_codec_options,
};
use crate::tabs::source::TitleFacts;

/// How the encoder spends its bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControl {
    /// One value that trades size against quality.
    Quality,
    /// A target average bit rate.
    Bitrate,
    /// A target file size.
    TargetSize,
    /// No loss at all.
    Lossless,
}

impl RateControl {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            RateControl::Quality => "quality",
            RateControl::Bitrate => "bitrate",
            RateControl::TargetSize => "target_size",
            RateControl::Lossless => "lossless",
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
            "quality" | "crf" | "cq" | "constant_quality" => Some(RateControl::Quality),
            "bitrate" | "abr" | "average_bitrate" => Some(RateControl::Bitrate),
            "target_size" | "size" | "target_file_size" => Some(RateControl::TargetSize),
            "lossless" => Some(RateControl::Lossless),
            _ => None,
        }
    }

    /// Whether a second pass changes anything for this mode.
    pub fn benefits_from_two_pass(self) -> bool {
        matches!(self, RateControl::Bitrate | RateControl::TargetSize)
    }

    /// The name the picker and every message use.
    pub fn label(self) -> &'static str {
        match self {
            RateControl::Quality => "constant quality",
            RateControl::Bitrate => "average bit rate",
            RateControl::TargetSize => "target file size",
            RateControl::Lossless => "lossless",
        }
    }
}

/// Whether the frame rate is left alone, pinned, or freed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRateMode {
    /// Whatever the source has.
    Source,
    /// A fixed rate the user types.
    Constant,
    /// Timings follow the source, frames are not duplicated.
    Variable,
}

impl FrameRateMode {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameRateMode::Source => "source",
            FrameRateMode::Constant => "constant",
            FrameRateMode::Variable => "variable",
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
            "source" | "keep" | "auto" => Some(FrameRateMode::Source),
            "constant" | "cfr" | "fixed" => Some(FrameRateMode::Constant),
            "variable" | "vfr" => Some(FrameRateMode::Variable),
            _ => None,
        }
    }
}

/// The video choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// Chosen video codec.
    pub encoder: String,
    /// Rate-control mode.
    pub rate_control: String,
    /// Constant-quality value.
    pub quality: NumField,
    /// Target bit rate in kb/s.
    pub bitrate: NumField,
    /// Target file size in megabytes.
    pub target_size: NumField,
    /// Run a first analysis pass.
    pub two_pass: bool,
    /// Encode without loss.
    pub lossless: bool,
    /// Encoder speed preset.
    pub preset: String,
    /// Coding profile.
    pub profile: String,
    /// Content tune.
    pub tune: String,
    /// Coding level.
    pub level: String,
    /// Colour range.
    pub colour_range: String,
    /// Frame-rate handling.
    pub frame_rate_mode: String,
    /// Frame rate used by the constant mode.
    pub frame_rate: NumField,
    /// What to do with a Dolby Vision RPU.
    pub dolby_vision: String,
    /// What to do with HDR10+ dynamic metadata.
    pub hdr10_plus: String,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            encoder: "h264".to_string(),
            rate_control: RateControl::Quality.as_str().to_string(),
            quality: NumField::of(20),
            bitrate: NumField::blank(),
            target_size: NumField::blank(),
            two_pass: false,
            lossless: false,
            preset: "default".to_string(),
            profile: "default".to_string(),
            tune: "default".to_string(),
            level: "default".to_string(),
            colour_range: "default".to_string(),
            frame_rate_mode: FrameRateMode::Source.as_str().to_string(),
            frame_rate: NumField::of("24"),
            // HDR metadata is carried through unless the user asks otherwise:
            // dropping a Dolby Vision RPU or HDR10+ dynamic metadata is a change
            // to the picture, not a neutral default.
            dolby_vision: DolbyVisionAction::PassThrough.as_str().to_string(),
            hdr10_plus: Hdr10PlusAction::PassThrough.as_str().to_string(),
        }
    }
}

impl VideoSettings {
    /// The rate-control mode as an enum.
    pub fn rate(&self) -> RateControl {
        RateControl::parse(&self.rate_control).unwrap_or(RateControl::Quality)
    }

    /// The frame-rate mode as an enum.
    pub fn frame_rate_kind(&self) -> FrameRateMode {
        FrameRateMode::parse(&self.frame_rate_mode).unwrap_or(FrameRateMode::Source)
    }

    /// One line describing the rate control, used by the summary tab.
    pub fn quality_label(&self) -> String {
        match self.rate() {
            RateControl::Quality => {
                let value = self.quality.text.trim();
                if value.is_empty() {
                    "constant quality (no value chosen)".to_string()
                } else {
                    format!("constant quality {value} (lower is better looking)")
                }
            }
            RateControl::Bitrate => {
                let value = self.bitrate.text.trim();
                if value.is_empty() {
                    "target bit rate (no value chosen)".to_string()
                } else {
                    format!("target bit rate {value} kb/s")
                }
            }
            RateControl::TargetSize => {
                let value = self.target_size.text.trim();
                if value.is_empty() {
                    "target file size (no value chosen)".to_string()
                } else {
                    format!("target file size {value} MB")
                }
            }
            RateControl::Lossless => "lossless".to_string(),
        }
    }
}

/// Video choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoInput {
    /// Chosen video codec.
    pub encoder: Option<String>,
    /// Rate-control mode.
    pub rate_control: Option<String>,
    /// Constant-quality value.
    pub quality: Option<String>,
    /// Target bit rate in kb/s.
    pub bitrate: Option<String>,
    /// Target file size in megabytes.
    pub target_size: Option<String>,
    /// Run a first analysis pass.
    pub two_pass: Option<bool>,
    /// Encode without loss.
    pub lossless: Option<bool>,
    /// Encoder speed preset.
    pub preset: Option<String>,
    /// Coding profile.
    pub profile: Option<String>,
    /// Content tune.
    pub tune: Option<String>,
    /// Coding level.
    pub level: Option<String>,
    /// Colour range.
    pub colour_range: Option<String>,
    /// Frame-rate handling.
    pub frame_rate_mode: Option<String>,
    /// Frame rate used by the constant mode.
    pub frame_rate: Option<String>,
    /// What to do with a Dolby Vision RPU.
    pub dolby_vision: Option<String>,
    /// What to do with HDR10+ dynamic metadata.
    pub hdr10_plus: Option<String>,
}

impl VideoInput {
    /// Apply the change, refusing names this build does not offer.
    pub fn apply(self, settings: &mut VideoSettings) -> Result<(), String> {
        if let Some(encoder) = self.encoder {
            let encoder = encoder.trim().to_ascii_lowercase();
            if !video_codec_options()
                .iter()
                .any(|option| option.id == encoder)
            {
                return Err(format!(
                    "'{encoder}' is not a video encoder this build offers."
                ));
            }
            settings.encoder = encoder;
        }
        if let Some(mode) = self.rate_control {
            settings.rate_control = RateControl::parse(&mode)
                .ok_or_else(|| {
                    format!("'{}' is not a rate control this build offers.", mode.trim())
                })?
                .as_str()
                .to_string();
        }
        if let Some(value) = self.quality {
            settings.quality = NumField { text: value };
        }
        if let Some(value) = self.bitrate {
            settings.bitrate = NumField { text: value };
        }
        if let Some(value) = self.target_size {
            settings.target_size = NumField { text: value };
        }
        if let Some(value) = self.two_pass {
            settings.two_pass = value;
        }
        if let Some(value) = self.lossless {
            settings.lossless = value;
        }
        for (name, value) in [
            ("preset", self.preset),
            ("profile", self.profile),
            ("tune", self.tune),
            ("level", self.level),
            ("colour_range", self.colour_range),
        ] {
            if let Some(value) = value {
                let value = value.trim().to_ascii_lowercase();
                if value.is_empty() {
                    continue;
                }
                let offered = match name {
                    "preset" => encoder_presets(&settings.encoder),
                    "profile" => encoder_profiles(&settings.encoder),
                    "tune" => encoder_tunes(&settings.encoder),
                    "level" => encoder_levels(&settings.encoder),
                    _ => colour_range_options(),
                };
                if !offered.iter().any(|option| option.id == value) {
                    return Err(format!("'{value}' is not a {name} this build offers."));
                }
                match name {
                    "preset" => settings.preset = value,
                    "profile" => settings.profile = value,
                    "tune" => settings.tune = value,
                    "level" => settings.level = value,
                    _ => settings.colour_range = value,
                }
            }
        }
        if let Some(mode) = self.frame_rate_mode {
            settings.frame_rate_mode = FrameRateMode::parse(&mode)
                .ok_or_else(|| {
                    format!(
                        "'{}' is not a frame-rate mode this build offers.",
                        mode.trim()
                    )
                })?
                .as_str()
                .to_string();
        }
        // The HDR rows refuse a name they do not offer, the same as every other
        // picker: an unrecognised action must not be stored and then resolved
        // into whatever the core's default happens to be.
        if let Some(action) = self.dolby_vision {
            settings.dolby_vision = DolbyVisionAction::parse(&action)
                .ok_or_else(|| {
                    format!(
                        "'{}' is not a Dolby Vision action this build offers.",
                        action.trim()
                    )
                })?
                .as_str()
                .to_string();
        }
        if let Some(action) = self.hdr10_plus {
            settings.hdr10_plus = Hdr10PlusAction::parse(&action)
                .ok_or_else(|| {
                    format!(
                        "'{}' is not an HDR10+ action this build offers.",
                        action.trim()
                    )
                })?
                .as_str()
                .to_string();
        }
        if let Some(value) = self.frame_rate {
            settings.frame_rate = NumField { text: value };
        }
        Ok(())
    }
}

/// Colour ranges the video tab offers.
pub fn colour_range_options() -> Vec<crate::tabs::common::ChoiceOption> {
    use crate::tabs::common::ChoiceOption;
    vec![
        ChoiceOption::new("default", "Leave it to the encoder"),
        ChoiceOption::new("limited", "Limited (16-235)"),
        ChoiceOption::new("full", "Full (0-255)"),
    ]
}

/// The video tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTabViewModel {
    /// Video encoder picker.
    pub encoder: ChoiceGroupViewModel,
    /// Rate-control picker.
    pub rate_control: ChoiceGroupViewModel,
    /// Constant-quality field.
    pub quality: NumberViewModel,
    /// Target bit-rate field.
    pub bitrate: NumberViewModel,
    /// Target file-size field.
    pub target_size: NumberViewModel,
    /// Second-pass switch.
    pub two_pass: ToggleViewModel,
    /// Lossless switch.
    pub lossless: ToggleViewModel,
    /// Encoder speed preset picker.
    pub preset: ChoiceGroupViewModel,
    /// Coding profile picker.
    pub profile: ChoiceGroupViewModel,
    /// Content tune picker.
    pub tune: ChoiceGroupViewModel,
    /// Coding level picker.
    pub level: ChoiceGroupViewModel,
    /// Colour range picker.
    pub colour_range: ChoiceGroupViewModel,
    /// Frame-rate mode picker.
    pub frame_rate_mode: ChoiceGroupViewModel,
    /// Frame-rate field for the constant mode.
    pub frame_rate: NumberViewModel,
    /// What happens to a Dolby Vision RPU the source carries.
    pub dolby_vision: ChoiceGroupViewModel,
    /// What happens to HDR10+ dynamic metadata the source carries.
    pub hdr10_plus: ChoiceGroupViewModel,
    /// One line describing the rate control.
    pub plan_label: String,
    /// Worth knowing but not blocking.
    pub notes: Vec<String>,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Read a number the encoder needs, collecting anything wrong with it.
///
/// The field is handed back too, so a value the encoder would refuse is marked
/// unusable where the user typed it rather than only in the problem list.
fn read_number(
    field: &NumField,
    label: &str,
    min: u32,
    max: u32,
    problems: &mut Vec<String>,
    view: &mut NumberViewModel,
) -> Option<u32> {
    let value = match field.whole(label) {
        Ok(value) => value,
        Err(e) => {
            *view = view.clone().invalid(e.clone());
            problems.push(e);
            None
        }
    };
    if value.is_some() {
        if let Err(e) = number_in_range(value, min, max, label) {
            *view = view.clone().invalid(e.clone());
            problems.push(e);
            return None;
        }
    }
    value
}

/// The two HDR metadata rows, resolved against what the source carries and what
/// this machine's encoder can do.
///
/// The rows are always present, because a user who cannot see what will happen
/// to the RPU has no way to ask for it to be kept. What varies is whether the
/// choice is open: a source with no Dolby Vision metadata, or an encoder without
/// RPU coding, closes the row with the reason rather than hiding the question.
fn hdr_rows(
    settings: &VideoSettings,
    facts: &TitleFacts,
    notes: &mut Vec<String>,
    problems: &mut Vec<String>,
) -> (ChoiceGroupViewModel, ChoiceGroupViewModel) {
    let caps = vise_core::HdrCapabilities::detect(
        vise_core::FfprobeScanner::default()
            .ffmpeg_path()
            .map(std::path::PathBuf::from)
            .as_deref(),
    );
    let has_dv = facts.dolby_vision_profile > 0 || facts.dolby_vision_rpu;
    let has_hdr10_plus = facts.hdr10_plus;

    let dv_options = vec![
        ChoiceOption::new(
            DolbyVisionAction::PassThrough.as_str(),
            "Keep the metadata in the video",
        ),
        ChoiceOption::new(
            DolbyVisionAction::ExtractRpu.as_str(),
            "Save it beside the file instead",
        ),
        ChoiceOption::new(
            DolbyVisionAction::Strip.as_str(),
            "Remove it, leaving plain HDR10",
        ),
    ];
    let hdr_options = vec![
        ChoiceOption::new(
            Hdr10PlusAction::PassThrough.as_str(),
            "Keep the scene changes",
        ),
        ChoiceOption::new(
            Hdr10PlusAction::Strip.as_str(),
            "Remove them, leaving static HDR10",
        ),
    ];

    let dv_available = has_dv;
    let dv_reason = if has_dv {
        String::new()
    } else if facts.number == 0 {
        "Choose a title to see whether it carries Dolby Vision metadata.".to_string()
    } else {
        "This title carries no Dolby Vision metadata, so there is nothing to keep or remove."
            .to_string()
    };
    let dv = ChoiceGroupViewModel::build(
        "videoDolbyVision",
        "Dolby Vision metadata",
        "What happens to the dynamic metadata the source carries.",
        &settings.dolby_vision,
        &dv_options,
        dv_available,
        &dv_reason,
    );

    let hdr_available = has_hdr10_plus;
    let hdr_reason = if has_hdr10_plus {
        String::new()
    } else if facts.number == 0 {
        "Choose a title to see whether it carries HDR10+ metadata.".to_string()
    } else {
        "This title carries no HDR10+ metadata, so there is nothing to keep or remove.".to_string()
    };
    let hdr10 = ChoiceGroupViewModel::build(
        "videoHdr10Plus",
        "HDR10+ metadata",
        "What happens to the per-scene brightness data the source carries.",
        &settings.hdr10_plus,
        &hdr_options,
        hdr_available,
        &hdr_reason,
    );

    // A request the machine cannot carry is worth knowing about even when the
    // source has nothing to carry: the row stays closed with its own reason, and
    // the machine's limitation is stated once so it is not a surprise at encode
    // time on a different machine.
    if has_dv && !caps.dolby_vision_rpu {
        notes.push(
            "This machine's HEVC encoder cannot code Dolby Vision metadata, so the request will be reported rather than carried."
                .to_string(),
        );
    }
    if has_hdr10_plus && !caps.hdr10_plus_sei {
        notes.push(
            "This machine's HEVC encoder cannot forward HDR10+ metadata, so the request will be reported rather than carried."
                .to_string(),
        );
    }
    let _ = problems;
    (dv, hdr10)
}

/// Build the video tab.
pub fn tab_view(settings: &VideoSettings, facts: &TitleFacts) -> VideoTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let rate = settings.rate();

    let encoder = ChoiceGroupViewModel::build(
        "videoEncoder",
        "Encoder",
        "Which video coding the output uses.",
        &settings.encoder,
        &video_codec_options(),
        true,
        "",
    );

    let rate_options = ChoiceGroup::of([
        ("quality", "Constant quality"),
        ("bitrate", "Average bit rate"),
        ("target_size", "Target file size"),
        ("lossless", "Lossless"),
    ]);
    let rate_control = ChoiceGroupViewModel::build(
        "rateControl",
        "Rate control",
        "How the encoder spends the bits it has.",
        rate.as_str(),
        &rate_options.options,
        true,
        "",
    );

    let mut quality =
        NumberViewModel::new("videoQuality", "Quality", settings.quality.text.trim(), "")
            .ranged(0.0, 51.0);
    let mut bitrate = NumberViewModel::new(
        "videoBitrate",
        "Bit rate",
        settings.bitrate.text.trim(),
        "kb/s",
    )
    .ranged(1.0, 500_000.0);
    let mut target_size = NumberViewModel::new(
        "videoTargetSize",
        "File size",
        settings.target_size.text.trim(),
        "MB",
    )
    .ranged(1.0, 1_000_000.0);

    // Only the field the chosen mode reads is on offer; the others say what they
    // would need instead of sitting there looking ignored.
    let other = match rate {
        RateControl::Quality => ("a bit rate", "a target size"),
        RateControl::Bitrate => ("a quality value", "a target size"),
        RateControl::TargetSize => ("a quality value", "a bit rate"),
        RateControl::Lossless => ("a quality value", "a bit rate or size"),
    };
    for field in [&mut quality, &mut bitrate, &mut target_size] {
        let wanted = matches!(
            (field.id.as_str(), rate),
            ("videoQuality", RateControl::Quality)
                | ("videoBitrate", RateControl::Bitrate)
                | ("videoTargetSize", RateControl::TargetSize)
        );
        if !wanted {
            *field = field.clone().blocked(format!(
                "The {} rate control reads {} instead.",
                rate.label(),
                if field.id == "videoQuality" {
                    other.0
                } else {
                    other.1
                }
            ));
        }
    }

    match rate {
        RateControl::Quality => {
            let value = read_number(
                &settings.quality,
                "Quality",
                0,
                51,
                &mut problems,
                &mut quality,
            );
            if value.is_none() && problems.is_empty() {
                problems.push(
                    "Choose a quality value between 0 and 51 for the constant-quality rate control."
                        .to_string(),
                );
            }
        }
        RateControl::Bitrate => {
            let value = read_number(
                &settings.bitrate,
                "Bit rate",
                1,
                500_000,
                &mut problems,
                &mut bitrate,
            );
            if value.is_none() && problems.is_empty() {
                problems
                    .push("Choose a bit rate for the average-bit-rate rate control.".to_string());
            }
        }
        RateControl::TargetSize => {
            let mb = read_number(
                &settings.target_size,
                "File size",
                1,
                1_000_000,
                &mut problems,
                &mut target_size,
            );
            if mb.is_none() && problems.is_empty() {
                problems.push("Choose a file size for the target-size rate control.".to_string());
            } else if let Some(mb) = mb {
                if !facts.has_duration() {
                    problems.push(
                        "A target file size needs the source duration, which this title does not report."
                            .to_string(),
                    );
                } else {
                    let bit_rate = (mb as f64 * 8_000.0 / facts.duration_seconds).round() as u32;
                    notes.push(format!(
                        "A {mb} MB file over {} is about {bit_rate} kb/s of video.",
                        crate::viewmodel::format_duration(facts.duration_seconds)
                    ));
                }
            }
        }
        RateControl::Lossless => {}
    }

    let mut two_pass = ToggleViewModel::new(
        "twoPass",
        "Two-pass encode",
        "Analyses the source first, then encodes against what it found.",
        settings.two_pass,
    );
    if !rate.benefits_from_two_pass() {
        two_pass = two_pass.blocked(format!(
            "A second pass only changes the result of a bit rate or file-size target, not {}.",
            rate.label()
        ));
    }
    if settings.two_pass && settings.lossless {
        problems.push("A lossless encode has nothing to gain from a second pass.".to_string());
    }
    let mut lossless = ToggleViewModel::new(
        "lossless",
        "Lossless",
        "Keeps every frame as it was, which makes the file large.",
        settings.lossless,
    );
    if rate == RateControl::Lossless {
        lossless = lossless.blocked("The lossless rate control is already on.");
    }

    let preset = ChoiceGroupViewModel::build(
        "encoderPreset",
        "Encoder preset",
        "How hard the encoder works per frame.",
        &settings.preset,
        &encoder_presets(&settings.encoder),
        true,
        "",
    );
    let profile = ChoiceGroupViewModel::build(
        "encoderProfile",
        "Profile",
        "Which set of coding tools the encoder may use.",
        &settings.profile,
        &encoder_profiles(&settings.encoder),
        true,
        "",
    );
    let tune = ChoiceGroupViewModel::build(
        "encoderTune",
        "Tune",
        "Which kind of picture the encoder is tuned for.",
        &settings.tune,
        &encoder_tunes(&settings.encoder),
        true,
        "",
    );
    let level = ChoiceGroupViewModel::build(
        "encoderLevel",
        "Level",
        "The decoder complexity the file promises.",
        &settings.level,
        &encoder_levels(&settings.encoder),
        true,
        "",
    );

    // A preset, profile, or level the new encoder does not offer is a mistake
    // the tab has to name rather than quietly drop.
    for (field, name, chosen) in [
        (&preset, "preset", &settings.preset),
        (&profile, "profile", &settings.profile),
        (&level, "level", &settings.level),
    ] {
        if chosen != "default"
            && !field
                .options
                .iter()
                .any(|option| option.id == *chosen && option.available)
        {
            problems.push(format!(
                "'{chosen}' is not a {name} the {} encoder offers.",
                settings.encoder.to_ascii_uppercase()
            ));
        }
    }
    if settings.tune != "default" && !tune.options.iter().any(|option| option.id == settings.tune) {
        problems.push(format!(
            "The {} encoder has no '{}' tune in this build.",
            settings.encoder.to_ascii_uppercase(),
            settings.tune
        ));
    }

    let colour_range = ChoiceGroupViewModel::build(
        "colourRange",
        "Colour range",
        "Whether dark and bright values use the full range.",
        &settings.colour_range,
        &colour_range_options(),
        true,
        "",
    );

    let frame_options = ChoiceGroup::of([
        ("source", "Keep the source rate"),
        ("constant", "Fixed rate"),
        ("variable", "Variable rate"),
    ]);
    let frame_mode = settings.frame_rate_kind();
    let frame_rate_mode = ChoiceGroupViewModel::build(
        "frameRateMode",
        "Frame rate",
        "How frames are timed in the output.",
        frame_mode.as_str(),
        &frame_options.options,
        true,
        "",
    );

    let mut frame_rate = NumberViewModel::new(
        "frameRate",
        "Frames per second",
        settings.frame_rate.text.trim(),
        "fps",
    )
    .ranged(1.0, 240.0);
    if frame_mode != FrameRateMode::Constant {
        let reason = if frame_mode == FrameRateMode::Source {
            "The source frame rate is kept as it is."
        } else {
            "A variable rate needs no number; frames are timed by the source."
        };
        frame_rate = frame_rate.blocked(reason);
    } else {
        match settings.frame_rate.number("Frames per second") {
            Err(e) => {
                frame_rate = frame_rate.invalid(e.clone());
                problems.push(e);
            }
            Ok(None) => problems.push("Choose a frame rate for the fixed-rate mode.".to_string()),
            Ok(Some(value)) => {
                if !(1.0..=240.0).contains(&value) {
                    problems.push("Frames per second must be between 1 and 240.".to_string());
                } else if facts.has_frame_rate() && value > facts.frame_rate * 1.5 {
                    notes.push(format!(
                        "{} fps is well above the source's {} fps, so frames will be repeated.",
                        value,
                        crate::viewmodel::format_frame_rate(facts.frame_rate)
                    ));
                }
            }
        }
    }

    let plan_label = {
        let passes = if settings.two_pass && rate.benefits_from_two_pass() {
            "two passes"
        } else {
            "one pass"
        };
        format!("{} · {passes}", settings.quality_label())
    };

    // HDR metadata is a property of the source and a capability of the encoder,
    // so both are resolved here rather than left to the job builder: a machine
    // whose HEVC encoder was compiled without Dolby Vision RPU coding says so on
    // the row, instead of the request being accepted and quietly ignored.
    let (dv, hdr10) = hdr_rows(settings, facts, &mut notes, &mut problems);

    VideoTabViewModel {
        encoder,
        rate_control,
        quality,
        bitrate,
        target_size,
        two_pass,
        lossless,
        preset,
        profile,
        tune,
        dolby_vision: dv,
        hdr10_plus: hdr10,
        level,
        colour_range,
        frame_rate_mode,
        frame_rate,
        plan_label,
        notes,
        ready: problems.is_empty(),
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            frame_rate: 23.976,
            duration_seconds: 3600.0,
            ..Default::default()
        }
    }

    #[test]
    fn an_hdr_row_is_closed_with_a_reason_when_the_source_has_nothing_to_carry() {
        let plain = tab_view(&VideoSettings::default(), &facts());
        assert!(!plain.dolby_vision.available);
        assert!(
            plain
                .dolby_vision
                .reason
                .contains("no Dolby Vision metadata"),
            "{}",
            plain.dolby_vision.reason
        );
        assert!(!plain.hdr10_plus.available);
        assert!(
            plain.hdr10_plus.reason.contains("no HDR10+ metadata"),
            "{}",
            plain.hdr10_plus.reason
        );
    }

    #[test]
    fn a_source_with_hdr_metadata_opens_both_hdr_rows() {
        let hdr = TitleFacts {
            dolby_vision_profile: 8,
            dolby_vision_rpu: true,
            hdr10_plus: true,
            ..facts()
        };
        let view = tab_view(&VideoSettings::default(), &hdr);
        assert!(view.dolby_vision.available, "{}", view.dolby_vision.reason);
        assert!(view.hdr10_plus.available, "{}", view.hdr10_plus.reason);
        // Both keep the metadata unless the user says otherwise: dropping it is
        // a change to the picture, so it is not the silent default.
        assert_eq!(view.dolby_vision.selected, "pass_through");
        assert_eq!(view.hdr10_plus.selected, "pass_through");
        assert_eq!(view.dolby_vision.options.len(), 3);
        assert_eq!(view.hdr10_plus.options.len(), 2);
    }

    #[test]
    fn an_hdr_choice_reaches_the_job_and_an_unknown_one_is_refused() {
        let hdr = TitleFacts {
            dolby_vision_rpu: true,
            hdr10_plus: true,
            ..facts()
        };
        let tabs = crate::tabs::TabSettings {
            video: VideoSettings {
                dolby_vision: "strip".into(),
                hdr10_plus: "strip".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let spec = tabs
            .to_job_spec("/in.mkv", "/out.mkv", &hdr, &Default::default())
            .expect("a job");
        assert_eq!(
            spec.dolby_vision.action,
            Some(DolbyVisionAction::Strip),
            "the choice has to reach the encoder"
        );
        assert_eq!(spec.hdr10_plus.action, Some(Hdr10PlusAction::Strip));
        assert!(spec.dolby_vision.enabled, "the source carries it");
        assert!(spec.hdr10_plus.enabled);

        let refused = VideoInput {
            dolby_vision: Some("melt-it-down".into()),
            ..Default::default()
        }
        .apply(&mut VideoSettings::default());
        assert!(
            refused.is_err(),
            "an unrecognised action must not be stored"
        );
    }

    #[test]
    fn the_defaults_are_a_constant_quality_h264_encode() {
        let settings = VideoSettings::default();
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.encoder.selected, "h264");
        assert_eq!(view.rate_control.selected, "quality");
        assert_eq!(view.quality.text, "20");
        assert!(view.quality.available);
        assert!(
            !view.bitrate.available,
            "the bit rate field is not read in this mode"
        );
        assert!(
            view.bitrate.reason.contains("constant quality"),
            "{}",
            view.bitrate.reason
        );
        assert!(!view.two_pass.available);
        assert!(
            view.two_pass.reason.contains("constant quality"),
            "{}",
            view.two_pass.reason
        );
        assert!(!view.frame_rate.available);
        assert!(view.plan_label.contains("constant quality 20"));
        assert!(view.plan_label.contains("one pass"));
    }

    #[test]
    fn a_quality_value_outside_the_range_is_refused() {
        let settings = VideoSettings {
            quality: NumField::of("80"),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts());
        assert!(!view.ready);
        assert!(
            view.problems.iter().any(|p| p.contains("between 0 and 51")),
            "{:?}",
            view.problems
        );
        assert!(!view.quality.valid);

        let missing = view_with_quality("");
        assert!(missing
            .problems
            .iter()
            .any(|p| p.contains("Choose a quality value")));
        let not_a_number = view_with_quality("high");
        assert!(not_a_number
            .problems
            .iter()
            .any(|p| p.contains("whole number") || p.contains("Quality")));
    }

    fn view_with_quality(text: &str) -> VideoTabViewModel {
        tab_view(
            &VideoSettings {
                quality: NumField {
                    text: text.to_string(),
                },
                ..Default::default()
            },
            &facts(),
        )
    }

    #[test]
    fn a_bitrate_mode_wants_a_bit_rate_and_offers_two_pass() {
        let settings = VideoSettings {
            rate_control: "bitrate".into(),
            bitrate: NumField::of("2500"),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.bitrate.available);
        assert!(!view.quality.available);
        assert!(view.two_pass.available);

        let missing = tab_view(
            &VideoSettings {
                rate_control: "bitrate".into(),
                ..Default::default()
            },
            &facts(),
        );
        assert!(missing
            .problems
            .iter()
            .any(|p| p.contains("Choose a bit rate")));
    }

    #[test]
    fn a_target_size_needs_a_duration_to_work_out_from() {
        let settings = VideoSettings {
            rate_control: "target_size".into(),
            target_size: NumField::of("700"),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(
            view.notes.iter().any(|n| n.contains("about 1556 kb/s")),
            "{:?}",
            view.notes
        );

        let unknown_duration = tab_view(
            &settings,
            &TitleFacts {
                number: 1,
                ..Default::default()
            },
        );
        assert!(unknown_duration
            .problems
            .iter()
            .any(|p| p.contains("source duration")));
    }

    #[test]
    fn two_pass_is_pointless_against_a_lossless_encode() {
        let settings = VideoSettings {
            rate_control: "bitrate".into(),
            bitrate: NumField::of("4000"),
            two_pass: true,
            lossless: true,
            ..Default::default()
        };
        let view = tab_view(&settings, &facts());
        assert!(view.problems.iter().any(|p| p.contains("lossless encode")));
        assert!(view.plan_label.contains("two passes"));
        assert!(view.lossless.checked, "the switch reflects the choice");
    }

    #[test]
    fn encoder_choices_are_re_read_when_the_encoder_changes() {
        let mut settings = VideoSettings {
            encoder: "h264".into(),
            preset: "ultrafast".into(),
            profile: "high10".into(),
            level: "1.0".into(),
            tune: "film".into(),
            ..Default::default()
        };
        assert!(tab_view(&settings, &facts()).ready);
        settings.encoder = "av1".into();
        let view = tab_view(&settings, &facts());
        assert!(!view.ready);
        assert!(view.problems.iter().any(|p| p.contains("'ultrafast'")));
        assert!(view.problems.iter().any(|p| p.contains("'high10'")));
        assert!(view.problems.iter().any(|p| p.contains("'1.0'")));
        assert!(view.problems.iter().any(|p| p.contains("'film'")));
        assert!(view.tune.options.len() <= 3, "AV1 has few tunes");
    }

    #[test]
    fn a_fixed_frame_rate_needs_a_sane_number() {
        let settings = VideoSettings {
            frame_rate_mode: "constant".into(),
            frame_rate: NumField::of("60"),
            ..Default::default()
        };
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.frame_rate.available);
        assert!(
            view.notes.iter().any(|n| n.contains("repeated")),
            "{:?}",
            view.notes
        );

        let silly = tab_view(
            &VideoSettings {
                frame_rate_mode: "constant".into(),
                frame_rate: NumField::of("600"),
                ..Default::default()
            },
            &facts(),
        );
        assert!(silly
            .problems
            .iter()
            .any(|p| p.contains("between 1 and 240")));

        let missing = tab_view(
            &VideoSettings {
                frame_rate_mode: "constant".into(),
                frame_rate: NumField::blank(),
                ..Default::default()
            },
            &facts(),
        );
        assert!(missing
            .problems
            .iter()
            .any(|p| p.contains("Choose a frame rate")));
    }

    #[test]
    fn unknown_names_are_refused_with_the_name_in_the_message() {
        let mut settings = VideoSettings::default();
        let error = VideoInput {
            encoder: Some("theora".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown encoder");
        assert!(error.contains("theora"), "{error}");

        let mode = VideoInput {
            rate_control: Some("psychic".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown rate control");
        assert!(mode.contains("psychic"), "{mode}");

        let preset = VideoInput {
            preset: Some("turbo".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown preset");
        assert!(preset.contains("turbo"), "{preset}");

        let frame = VideoInput {
            frame_rate_mode: Some("sideways".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown frame rate mode");
        assert!(frame.contains("sideways"), "{frame}");
    }

    #[test]
    fn a_valid_change_is_applied() {
        let mut settings = VideoSettings::default();
        VideoInput {
            encoder: Some("VP9".into()),
            rate_control: Some("Constant Quality".into()),
            quality: Some("18".into()),
            two_pass: Some(true),
            preset: Some("veryslow".into()),
            colour_range: Some("full".into()),
            frame_rate_mode: Some("vfr".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("change");
        assert_eq!(settings.encoder, "vp9");
        assert_eq!(settings.rate_control, "quality");
        assert_eq!(settings.quality, NumField::of("18"));
        assert!(settings.two_pass);
        assert_eq!(settings.preset, "veryslow");
        assert_eq!(settings.colour_range, "full");
        assert_eq!(settings.frame_rate_mode, "variable");
    }
}
