//! The hardware tab: what the encoder may use, and what decoding runs on.
//!
//! Two halves, and the split matters:
//!
//! * **Encoder availability is an observation, not a control.** Detection runs
//!   the installed encoder's own `-encoders` list and reports what was found,
//!   one read-only row per hardware encoder with a reason when it is missing.
//!   Offering a switch beside each one would be offering a choice nothing reads.
//! * **Decode device and acceleration are controls**, because they are choices
//!   the user makes and the pipeline reads them: `tabs` puts them into the job
//!   specification's hardware options, and `vise_core`'s pipeline turns them
//!   into the decoder argument and the encoder it reaches for. What the
//!   preferences show and what the encoder is given therefore come from the same
//!   two values.
//!
//! A device is only offered if detection found it. A device this machine does
//! not have is not a choice with a bad outcome, it is not a choice, so it is
//! listed as missing with the reason rather than offered and then refused.

use serde::{Deserialize, Serialize};
use vise_core::{HardwareApi, HardwareCapabilities};

use crate::preferences::{
    want_one_of, ActionViewModel, ChoiceGroupViewModel, FactViewModel, PreferenceTabViewModel,
};
use crate::tabs::common::ChoiceOption;

/// Field identifier for the acceleration choice.
pub const ACCELERATION: &str = "hardware.acceleration";
/// Field identifier for the decode device.
pub const DECODE_DEVICE: &str = "hardware.decode_device";

/// The decode device that is not a device: decoding in software.
pub const SOFTWARE_DECODE: &str = "software";
/// The decode device that lets the media tools choose.
pub const AUTOMATIC_DECODE: &str = "auto";

/// Hardware encoders this build knows how to ask about.
///
/// The list is what a reader would look for, not what this machine has: each one
/// is reported present or missing, and the ones a machine does not have are what
/// makes the report worth reading.
const KNOWN_ENCODERS: [(&str, &str); 8] = [
    ("h264_nvenc", "H.264 on an NVIDIA card"),
    ("hevc_nvenc", "H.265 on an NVIDIA card"),
    ("h264_qsv", "H.264 on Intel Quick Sync"),
    ("hevc_qsv", "H.265 on Intel Quick Sync"),
    ("h264_vaapi", "H.264 on a VAAPI device"),
    ("hevc_vaapi", "H.265 on a VAAPI device"),
    ("h264_videotoolbox", "H.264 through VideoToolbox"),
    ("hevc_videotoolbox", "H.265 through VideoToolbox"),
];

/// Whether the encoder should reach for the graphics card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccelerationPreference {
    /// Let the media tools decide, falling back to software.
    #[default]
    Automatic,
    /// Use the graphics card, and fall back to software when it cannot.
    WhenAvailable,
    /// Refuse the graphics card.
    Never,
}

impl AccelerationPreference {
    /// Every choice, in the order the picker lists them.
    pub const ALL: [AccelerationPreference; 3] = [
        AccelerationPreference::Automatic,
        AccelerationPreference::WhenAvailable,
        AccelerationPreference::Never,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            AccelerationPreference::Automatic => "automatic",
            AccelerationPreference::WhenAvailable => "when_available",
            AccelerationPreference::Never => "never",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            AccelerationPreference::Automatic => "Let the encoder decide",
            AccelerationPreference::WhenAvailable => "Use the graphics card when it can",
            AccelerationPreference::Never => "Never use the graphics card",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            AccelerationPreference::Automatic => {
                "The encoder is given the hardware options and decides what it can use."
            }
            AccelerationPreference::WhenAvailable => {
                "A hardware encoder is asked for first, and software encoding takes over when it cannot."
            }
            AccelerationPreference::Never => {
                "Every encode runs on the processor. Slower, and identical output on every machine."
            }
        }
    }

    /// Whether this answer wants the graphics card at all.
    pub fn wants_hardware(self) -> bool {
        !matches!(self, AccelerationPreference::Never)
    }

    /// Read a choice the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            ACCELERATION,
            value,
            &[
                (
                    AccelerationPreference::Automatic.id(),
                    AccelerationPreference::Automatic.label(),
                ),
                (
                    AccelerationPreference::WhenAvailable.id(),
                    AccelerationPreference::WhenAvailable.label(),
                ),
                (
                    AccelerationPreference::Never.id(),
                    AccelerationPreference::Never.label(),
                ),
            ],
        )?;
        AccelerationPreference::ALL
            .into_iter()
            .find(|choice| choice.id() == typed)
            .ok_or_else(|| {
                format!("'{typed}' is not something this build does with the graphics card.")
            })
    }
}

/// The hardware preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareSettings {
    /// Whether the encoder may use the graphics card.
    pub acceleration: AccelerationPreference,
    /// The device decoding runs on, or one of the two answers that is not one.
    pub decode_device: String,
}

impl Default for HardwareSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// Decoding is automatic and acceleration is left to the encoder: both are the
    /// answers that are right on a machine with a card and on one without, so a
    /// fresh install does not need a decision before it can encode anything.
    pub fn new() -> Self {
        Self {
            acceleration: AccelerationPreference::Automatic,
            decode_device: AUTOMATIC_DECODE.to_string(),
        }
    }

    /// Whether the pipeline should be told the hardware options at all.
    pub fn uses_hardware(&self) -> bool {
        self.acceleration.wants_hardware()
    }

    /// The decode preference rank the worker contract carries.
    ///
    /// Zero is software decoding, which is what the contract says it means; one
    /// is "the tools decide". A named device is sent with its own name rather
    /// than squeezed into this rank, so the two answers stay separable.
    pub fn decode_preference(&self) -> i32 {
        if self
            .decode_device
            .trim()
            .eq_ignore_ascii_case(SOFTWARE_DECODE)
        {
            0
        } else {
            1
        }
    }
}

/// Read a decode device the interface layer sent.
///
/// Only the two answers that are not devices are accepted here. Whether a named
/// device is on *this* machine is not known here — that is what detection is for
/// — so the name is normalised and `PreferencesState::set` checks it against what
/// detection reported. A device that is not on the machine would be stored and
/// then ignored by the media tools, so it is refused there with the reason the
/// tab already shows.
pub fn read_device(value: &serde_json::Value) -> Result<String, String> {
    let typed = crate::preferences::want_text(DECODE_DEVICE, value)?;
    let device = typed.trim().to_ascii_lowercase();
    if device.is_empty() {
        return Err(format!(
            "Decode on needs an answer: {AUTOMATIC_DECODE} or {SOFTWARE_DECODE}, or a device this machine has."
        ));
    }
    Ok(device)
}

/// What hardware detection found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardwareAvailability {
    /// Whether the media tools were found at all.
    ///
    /// Detection shells out to the encoder, so a machine with no encoder has no
    /// answer rather than an answer of "none".
    pub probed: bool,
    /// Path the detection ran against.
    pub tool_path: String,
    /// Devices the media tools reported for hardware decoding.
    pub decode_devices: Vec<String>,
    /// Hardware encoders that are present.
    pub encoders: Vec<String>,
    /// Hardware encoders that are not.
    pub missing_encoders: Vec<String>,
}

impl HardwareAvailability {
    /// What nothing found, which is what a window opens with before detection
    /// has run.
    pub fn unchecked() -> Self {
        Self::default()
    }

    /// Record what detection found.
    ///
    /// A run that found no tool at all is recorded as unchecked rather than as a
    /// machine with no hardware: the two look the same in a list and mean
    /// opposite things, and the difference is what decides whether the decode
    /// choices can be offered at all.
    pub fn of(capabilities: &HardwareCapabilities) -> Self {
        let tool_path = capabilities
            .ffmpeg_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let probed = capabilities.ffmpeg_path.is_some();
        let mut devices: Vec<String> = capabilities
            .hwaccels
            .iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty() && name != "auto")
            .collect();
        devices.sort();
        devices.dedup();
        let mut present = Vec::new();
        let mut missing = Vec::new();
        for (name, _) in KNOWN_ENCODERS {
            if capabilities.supports_encoder(name) {
                present.push(name.to_string());
            } else {
                missing.push(name.to_string());
            }
        }
        Self {
            probed,
            tool_path,
            decode_devices: devices,
            encoders: present,
            missing_encoders: missing,
        }
    }

    /// Whether `device` is one this machine reported.
    pub fn has_decode_device(&self, device: &str) -> bool {
        let device = device.trim().to_ascii_lowercase();
        device == AUTOMATIC_DECODE
            || device == SOFTWARE_DECODE
            || self.decode_devices.contains(&device)
    }

    /// Why `device` cannot be chosen, when it cannot.
    pub fn decode_device_reason(&self, device: &str) -> String {
        if self.has_decode_device(device) {
            return String::new();
        }
        if !self.probed {
            return "The media tools have not been looked for yet, so no device can be named. \
                    Look again on this tab first."
                .to_string();
        }
        format!(
            "This machine did not report '{device}' as a hardware decoding device; choose {AUTOMATIC_DECODE} or {SOFTWARE_DECODE}."
        )
    }
}

/// The hardware options the pipeline is given.
///
/// One function, used by both the view model and `tabs`, so what the preference
/// says and what a queued job is given cannot come from two translations of the
/// same two strings.
///
/// Turning the graphics card off covers decoding too: a card the user has asked
/// the window to leave alone should not be handed half the job on the strength
/// of a separate question about the decoder.
///
/// A named device is an accelerator name — the names detection reports are what
/// the media tools publish as `-hwaccels` — so it goes into `hwaccel` and into the
/// API, and deliberately not into `device`: the pipeline turns `device` into
/// `-hwaccel_device`, which takes a device path or index, and handing it the name
/// of an accelerator would ask the tools for a device called `d3d11va`.
pub fn to_hardware_options(settings: &HardwareSettings) -> vise_core::HardwareAcceleration {
    let device = settings.decode_device.trim();
    let encode_on = settings.uses_hardware();
    let named = encode_on
        && !device.is_empty()
        && !device.eq_ignore_ascii_case(AUTOMATIC_DECODE)
        && !device.eq_ignore_ascii_case(SOFTWARE_DECODE);
    // Decoding in software is an answer about the decoder only. Asking for the
    // graphics card to encode while decoding in software is a combination that
    // works, so it is honoured rather than overridden.
    let decode_on = encode_on && !device.eq_ignore_ascii_case(SOFTWARE_DECODE);
    let api = if named {
        HardwareApi::of_accelerator(device)
    } else {
        HardwareApi::Auto
    };
    vise_core::HardwareAcceleration {
        enabled: encode_on,
        mode: match settings.acceleration {
            AccelerationPreference::Automatic => vise_core::HardwareMode::Auto,
            AccelerationPreference::WhenAvailable => vise_core::HardwareMode::Hardware,
            AccelerationPreference::Never => vise_core::HardwareMode::Software,
        },
        preference: match settings.acceleration {
            AccelerationPreference::Automatic => vise_core::HardwarePreference::Auto,
            AccelerationPreference::WhenAvailable => vise_core::HardwarePreference::Prefer,
            AccelerationPreference::Never => vise_core::HardwarePreference::Disable,
        },
        api,
        // Left unset on purpose; see the note above.
        device: None,
        // Software encoding has to be a hard refusal rather than a preference,
        // or the pipeline would still reach for a card the user said not to use.
        fallback: encode_on,
        decode: vise_core::HardwareDecodeOptions {
            enabled: decode_on,
            // The automatic answer is the tools choosing for themselves, which
            // ffmpeg spells `auto`; a named accelerator is passed through as it
            // stands, and software decoding asks for nothing at all.
            hwaccel: match (decode_on, named) {
                (true, true) => Some(device.to_string()),
                (true, false) => Some(AUTOMATIC_DECODE.to_string()),
                (false, _) => None,
            },
            api,
            device: None,
            fallback: decode_on,
        },
        encode: vise_core::HardwareEncodeOptions {
            enabled: encode_on,
            encoder: None,
            api,
            device: None,
            fallback: encode_on,
            quality: None,
        },
    }
}

/// The problems these preferences have.
///
/// A device this machine does not have is not a problem reported here: it is a
/// value that cannot be chosen at all, and the reason for that comes from
/// detection, which only the tab has. What is left is the one answer that cannot
/// be stored without being wrong, and `read_device` already refuses it.
pub fn validate(settings: &HardwareSettings) -> Vec<String> {
    let mut problems = Vec::new();
    if settings.decode_device.trim().is_empty() {
        problems.push("Decode on has no answer.".to_string());
    }
    problems
}

/// Build the hardware tab.
pub fn tab_view(
    settings: &HardwareSettings,
    availability: &HardwareAvailability,
) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "hardware",
        "Hardware",
        "Whether the encoder uses the graphics card, and what decoding runs on.",
    );
    tab.group_titles
        .push("Encoders this machine has".to_string());
    if !availability.probed {
        tab.facts.push(FactViewModel::unknown(
            "hardware.probe",
            "Media tools",
            "The media tools were not found, so nothing can be said about what this machine can do.",
        ));
    } else {
        tab.facts.push(FactViewModel::present(
            "hardware.probe",
            "Media tools",
            availability.tool_path.clone(),
        ));
    }
    tab.actions.push(ActionViewModel::new(
        "preferencesRecheckHardware",
        "Look for the media tools again",
        "Asks the media tools what this machine can decode and encode with.",
    ));
    for name in &availability.encoders {
        tab.facts.push(FactViewModel::present(
            &format!("hardware.encoder.{name}"),
            name,
            "Available",
        ));
    }
    for name in &availability.missing_encoders {
        tab.facts.push(FactViewModel::missing(
            &format!("hardware.encoder.{name}"),
            name,
            "The media tools on this machine do not offer this encoder.",
        ));
    }
    if availability.probed && availability.encoders.is_empty() {
        tab.facts.push(FactViewModel::unknown(
            "hardware.encoder.none",
            "Hardware encoders",
            "No hardware encoder was reported, so every encode will run on the processor.",
        ));
    }

    tab.choices.push(
        ChoiceGroupViewModel::build(
            ACCELERATION,
            "Hardware acceleration",
            "Put into the hardware options of every job this window queues.",
            settings.acceleration.id(),
            &AccelerationPreference::ALL
                .iter()
                .map(|choice| ChoiceOption::new(choice.id(), choice.label()))
                .collect::<Vec<_>>(),
            true,
            "",
        )
        .with_hint("The choices above say what this machine can do; this says what is asked for."),
    );

    let mut devices: Vec<ChoiceOption> = vec![
        ChoiceOption::new(AUTOMATIC_DECODE, "Let the media tools choose"),
        ChoiceOption::new(SOFTWARE_DECODE, "Decode in software"),
    ];
    for device in &availability.decode_devices {
        devices.push(ChoiceOption::new(device, device));
    }
    let selected = settings.decode_device.trim().to_ascii_lowercase();
    let available = availability.has_decode_device(&selected);
    let reason = availability.decode_device_reason(&selected);
    tab.choices.push(
        ChoiceGroupViewModel::build(
            DECODE_DEVICE,
            "Decode on",
            "Sent with every job and used to build the decoder the media tools are given.",
            &selected,
            &devices,
            available,
            &reason,
        )
        .with_hint("Decoding in software is slower and produces the same picture."),
    );
    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    fn probed(devices: &[&str], encoders: &[&str]) -> HardwareAvailability {
        let mut availability = HardwareAvailability {
            probed: true,
            tool_path: "C:\\tools\\ffmpeg.exe".to_string(),
            decode_devices: devices.iter().map(|d| d.to_string()).collect(),
            ..HardwareAvailability::default()
        };
        for (name, _) in KNOWN_ENCODERS {
            if encoders.contains(&name) {
                availability.encoders.push(name.to_string());
            } else {
                availability.missing_encoders.push(name.to_string());
            }
        }
        availability
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = HardwareSettings::new();
        assert_eq!(settings.acceleration, AccelerationPreference::Automatic);
        assert_eq!(settings.decode_device, AUTOMATIC_DECODE);
        assert!(settings.uses_hardware());
        assert_eq!(settings.decode_preference(), 1);
        assert_eq!(settings, HardwareSettings::default());
    }

    #[test]
    fn a_device_this_machine_does_not_have_is_not_stored_rather_than_saved_and_ignored() {
        // The name is normalised here; whether it is on this machine is what
        // detection decides, and the setter is where the two meet.
        let error = read_device(&value("\"\"")).expect_err("no device");
        assert!(error.contains("Decode on"), "{error}");
        assert_eq!(
            read_device(&value("\" auto \"")).expect("automatic"),
            AUTOMATIC_DECODE
        );
        assert_eq!(
            read_device(&value("\"SOFTWARE\"")).expect("software"),
            SOFTWARE_DECODE
        );
        assert_eq!(
            read_device(&value("\" D3D11VA \"")).expect("named"),
            "d3d11va",
            "the name is normalised so the same device has one spelling"
        );
        assert!(
            !availability_has(&probed(&["d3d11va"], &[]), "vdpau"),
            "a device this machine did not report is not on offer"
        );
        assert!(availability_has(&probed(&["d3d11va"], &[]), "d3d11va"));
    }

    fn availability_has(availability: &HardwareAvailability, device: &str) -> bool {
        availability.has_decode_device(device)
    }

    #[test]
    fn a_device_detection_did_not_find_is_offered_as_missing_with_a_reason() {
        let availability = probed(&["d3d11va"], &["h264_qsv"]);
        assert!(availability_has(&availability, "d3d11va"));
        let reason = availability.decode_device_reason("vaapi");
        assert!(reason.contains("vaapi"), "{reason}");
        assert!(reason.contains("did not report"), "{reason}");
        assert!(availability.decode_device_reason("d3d11va").is_empty());
        assert!(
            availability
                .decode_device_reason(AUTOMATIC_DECODE)
                .is_empty(),
            "the automatic answer needs no device"
        );
    }

    #[test]
    fn a_machine_with_no_media_tools_says_so_rather_than_saying_no_hardware() {
        // The two look identical in a list and mean opposite things, so the
        // unprobed case is worded differently and offers no devices.
        let unchecked = HardwareAvailability::unchecked();
        assert!(!unchecked.probed);
        let reason = unchecked.decode_device_reason("d3d11va");
        assert!(reason.contains("not been looked for"), "{reason}");
        let view = tab_view(&HardwareSettings::new(), &unchecked);
        assert_eq!(view.choices[1].options.len(), 2, "only the two answers");
        // The control that would answer the question is on offer either way:
        // looking is what makes the list mean something.
        assert!(view.actions[0].available);
        assert_eq!(view.actions[0].id, "preferencesRecheckHardware");
    }

    #[test]
    fn the_acceleration_choice_becomes_the_hardware_options_a_job_is_given() {
        let never = HardwareSettings {
            acceleration: AccelerationPreference::Never,
            decode_device: AUTOMATIC_DECODE.to_string(),
        };
        let options = to_hardware_options(&never);
        assert!(!options.is_enabled());
        assert_eq!(options.mode, vise_core::HardwareMode::Software);
        assert_eq!(options.preference, vise_core::HardwarePreference::Disable);
        // Saying never use the graphics card covers decoding as well as encoding:
        // a card the user has asked the window to leave alone should not be handed
        // half the job just because decoding is a separate question on the tab.
        assert!(!options.decode.enabled);
        assert!(options.decode.hwaccel.is_none());
        assert!(!options.encode.enabled);
        assert!(!options.fallback);

        let preferred = HardwareSettings {
            acceleration: AccelerationPreference::WhenAvailable,
            decode_device: "d3d11va".to_string(),
        };
        let options = to_hardware_options(&preferred);
        assert!(options.is_enabled());
        assert_eq!(options.mode, vise_core::HardwareMode::Hardware);
        assert_eq!(
            options.decode.hwaccel.as_deref(),
            Some("d3d11va"),
            "the accelerator is named for -hwaccel"
        );
        assert_eq!(options.api, HardwareApi::D3d11va);
        assert!(
            options.device.is_none(),
            "-hwaccel_device takes a device, not an accelerator name"
        );
        assert!(options.encode.enabled);
        assert!(options.fallback, "software takes over when the card cannot");
    }

    #[test]
    fn an_automatic_decode_device_lets_the_tools_choose_rather_than_naming_one() {
        let settings = HardwareSettings::new();
        let options = to_hardware_options(&settings);
        assert!(options.is_enabled());
        assert_eq!(options.decode.hwaccel.as_deref(), Some("auto"));
        assert_eq!(options.api, HardwareApi::Auto);
        assert!(options.device.is_none());
        assert!(options.encode.enabled);
    }

    #[test]
    fn software_decoding_turns_the_decoder_off_and_leaves_the_encoder_alone() {
        let settings = HardwareSettings {
            acceleration: AccelerationPreference::Automatic,
            decode_device: SOFTWARE_DECODE.to_string(),
        };
        let options = to_hardware_options(&settings);
        assert!(!options.decode.enabled);
        assert!(options.decode.hwaccel.is_none());
        assert_eq!(settings.decode_preference(), 0, "zero is software decoding");
        assert!(
            options.encode.enabled,
            "decoding in software is an answer about the decoder, not the encoder"
        );
    }

    #[test]
    fn the_tab_reports_the_encoders_it_found_and_the_ones_it_did_not() {
        let availability = probed(&["d3d11va", "dxva2"], &["h264_nvenc"]);
        let view = tab_view(&HardwareSettings::new(), &availability);
        assert_eq!(view.id, "hardware");
        assert_eq!(view.group_titles.len(), 1);
        let found: Vec<&str> = view
            .facts
            .iter()
            .filter(|fact| fact.available)
            .map(|fact| fact.label.as_str())
            .collect();
        assert!(found.contains(&"h264_nvenc"), "{found:?}");
        assert!(found.contains(&"Media tools"), "{found:?}");
        let missing: Vec<&str> = view
            .facts
            .iter()
            .filter(|fact| !fact.available)
            .map(|fact| fact.label.as_str())
            .collect();
        assert!(missing.contains(&"hevc_nvenc"), "{missing:?}");
        assert!(missing.iter().all(|label| !label.is_empty()), "{missing:?}");
        // The two detected devices are offered alongside the two answers.
        let decode = &view.choices[1];
        assert_eq!(decode.options.len(), 4);
        assert!(decode.available);
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_stored_device_this_machine_does_not_have_closes_the_control_with_the_reason() {
        let availability = probed(&[], &[]);
        let settings = HardwareSettings {
            acceleration: AccelerationPreference::Automatic,
            decode_device: "vaapi".to_string(),
        };
        let view = tab_view(&settings, &availability);
        let decode = &view.choices[1];
        assert!(!decode.available);
        assert!(decode.reason.contains("vaapi"), "{}", decode.reason);
        assert!(decode.selected == "vaapi", "what was stored is still shown");
    }

    #[test]
    fn an_unknown_acceleration_is_refused_with_the_names_that_would_work() {
        let error = AccelerationPreference::read(&value("\"always\"")).expect_err("unknown");
        assert!(error.contains("always"), "{error}");
        assert!(error.contains("when_available"), "{error}");
        assert_eq!(
            AccelerationPreference::read(&value("\"never\"")).expect("never"),
            AccelerationPreference::Never
        );
    }
}
