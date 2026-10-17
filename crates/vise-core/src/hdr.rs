//! Dolby Vision and HDR10+ metadata handling.
//!
//! High dynamic range is carried in side data rather than in the coded picture,
//! so a job can do three honest things with it: carry it through, take it out,
//! or be told the machine cannot do the one it asked for. This module is where
//! those decisions are made, and it separates the *decision* — pure, unit
//! testable — from the *mechanism*, which is one probe of the encoder the build
//! was compiled with.
//!
//! Nothing here pretends more than the machine supports. A build whose HEVC
//! encoder has no Dolby Vision RPU coding and no user-SEI forwarding still
//! reports the request, still describes what would happen, and says plainly
//! that it cannot, rather than dropping the metadata and calling the result
//! HDR.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::VideoCodec;

/// What a job asks the encoder to do with Dolby Vision metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DolbyVisionAction {
    /// Carry the source's RPU through the HEVC stream.
    #[default]
    PassThrough,
    /// Take the RPU out of the stream into a sidecar file.
    ExtractRpu,
    /// Remove the RPU, leaving the HDR10 base layer.
    Strip,
}

impl DolbyVisionAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PassThrough => "pass_through",
            Self::ExtractRpu => "extract_rpu",
            Self::Strip => "strip",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "pass_through" | "passthrough" | "copy" | "keep" | "carry" => Some(Self::PassThrough),
            "extract_rpu" | "extract" | "export" | "sidecar" => Some(Self::ExtractRpu),
            "strip" | "drop" | "remove" | "none" => Some(Self::Strip),
            _ => None,
        }
    }

    /// The name shown in a picker or a log line.
    pub fn label(self) -> &'static str {
        match self {
            Self::PassThrough => "Carry the Dolby Vision RPU",
            Self::ExtractRpu => "Extract the RPU to a sidecar",
            Self::Strip => "Strip the RPU (HDR10 only)",
        }
    }
}

/// Dolby Vision handling for one job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DolbyVisionOptions {
    /// Whether the job asks for Dolby Vision metadata at all.
    pub enabled: bool,
    /// Source Dolby Vision profile, when the scan reported one.
    pub profile: Option<u8>,
    /// Whether the source carries an RPU to handle.
    pub rpu_present: bool,
    /// What to do with the RPU.
    pub action: Option<DolbyVisionAction>,
}

/// What a job asks the encoder to do with HDR10+ dynamic metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Hdr10PlusAction {
    /// Carry the dynamic metadata through the HEVC stream.
    #[default]
    PassThrough,
    /// Remove the dynamic metadata, leaving static HDR10.
    Strip,
}

impl Hdr10PlusAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PassThrough => "pass_through",
            Self::Strip => "strip",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "pass_through" | "passthrough" | "copy" | "keep" | "carry" => Some(Self::PassThrough),
            "strip" | "drop" | "remove" | "none" => Some(Self::Strip),
            _ => None,
        }
    }
}

/// HDR10+ dynamic metadata handling for one job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Hdr10PlusOptions {
    /// Whether the job asks for HDR10+ metadata at all.
    pub enabled: bool,
    /// Whether the source carries dynamic metadata to handle.
    pub present: bool,
    /// What to do with the dynamic metadata.
    pub action: Option<Hdr10PlusAction>,
}

/// What the machine's ffmpeg build can do with HDR side data.
///
/// Discovered by asking the HEVC encoder what options it has rather than by
/// assuming, because a build that was compiled without libx265 has no Dolby
/// Vision RPU coding at all and a build without the user-SEI path has no way
/// to forward HDR10+.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HdrCapabilities {
    /// The HEVC encoder can code a Dolby Vision RPU (`-dolbyvision`).
    pub dolby_vision_rpu: bool,
    /// The HEVC encoder can forward user unregistered SEI (`-udu_sei`), which
    /// is the path HDR10+ dynamic metadata travels.
    pub hdr10_plus_sei: bool,
}

impl HdrCapabilities {
    /// Ask the encoder what it can do.
    pub fn detect(ffmpeg: Option<&Path>) -> Self {
        let ffmpeg = match ffmpeg {
            Some(path) => path.to_path_buf(),
            None => return Self::default(),
        };
        let output = Command::new(ffmpeg)
            .arg("-hide_banner")
            .arg("-h")
            .arg("encoder=libx265")
            .output();
        let Ok(output) = output else {
            return Self::default();
        };
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Self::from_probe(&text)
    }

    /// Read capabilities out of an encoder help text.
    ///
    /// Public and pure so a test can pin the detection against a probe string
    /// without spawning ffmpeg, and so a build that cannot run the tool at all
    /// can still be reasoned about.
    pub fn from_probe(text: &str) -> Self {
        Self {
            dolby_vision_rpu: text.contains("-dolbyvision"),
            hdr10_plus_sei: text.contains("-udu_sei"),
        }
    }
}

/// The outcome of an HDR decision: what the encoder will be told, and what the
/// finished file can honestly be called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HdrOutcome {
    /// Whether the request can be carried out on this machine.
    pub supported: bool,
    /// Encoder arguments the decision produces.
    pub args: Vec<String>,
    /// One line saying what will happen.
    pub claim: String,
    /// Why the request cannot be met, when it cannot.
    pub reason: Option<String>,
}

impl DolbyVisionOptions {
    /// Resolve the decision against the machine's capabilities.
    pub fn resolve(&self, caps: &HdrCapabilities, video_codec: VideoCodec) -> HdrOutcome {
        if !self.enabled {
            return HdrOutcome {
                supported: true,
                args: Vec::new(),
                claim: "No Dolby Vision metadata was requested.".to_string(),
                reason: None,
            };
        }
        if video_codec != VideoCodec::H265 {
            return HdrOutcome {
                supported: false,
                args: Vec::new(),
                claim: "Dolby Vision metadata is not carried.".to_string(),
                reason: Some(format!(
                    "Dolby Vision travels in HEVC, and this job writes {}.",
                    video_codec.label()
                )),
            };
        }
        let action = self.action.unwrap_or_default();
        match action {
            DolbyVisionAction::Strip => HdrOutcome {
                supported: true,
                args: Vec::new(),
                claim: "The Dolby Vision RPU is stripped; the HDR10 base layer remains.".to_string(),
                reason: None,
            },
            DolbyVisionAction::ExtractRpu => HdrOutcome {
                supported: false,
                args: Vec::new(),
                claim: "The Dolby Vision RPU is not extracted.".to_string(),
                reason: Some(
                    "this build's encoder has no command-line way to extract the RPU to a sidecar; \
                     carry it or strip it instead"
                        .to_string(),
                ),
            },
            DolbyVisionAction::PassThrough => {
                if caps.dolby_vision_rpu {
                    HdrOutcome {
                        supported: true,
                        args: vec!["-dolbyvision".into(), "1".into()],
                        claim: "The Dolby Vision RPU is carried in the HEVC stream.".to_string(),
                        reason: None,
                    }
                } else {
                    HdrOutcome {
                        supported: false,
                        args: Vec::new(),
                        claim: "The Dolby Vision RPU is not carried.".to_string(),
                        reason: Some(
                            "the HEVC encoder this build uses cannot code a Dolby Vision RPU"
                                .to_string(),
                        ),
                    }
                }
            }
        }
    }
}

impl Hdr10PlusOptions {
    /// Resolve the decision against the machine's capabilities.
    pub fn resolve(&self, caps: &HdrCapabilities, video_codec: VideoCodec) -> HdrOutcome {
        if !self.enabled {
            return HdrOutcome {
                supported: true,
                args: Vec::new(),
                claim: "No HDR10+ metadata was requested.".to_string(),
                reason: None,
            };
        }
        if video_codec != VideoCodec::H265 {
            return HdrOutcome {
                supported: false,
                args: Vec::new(),
                claim: "HDR10+ metadata is not carried.".to_string(),
                reason: Some(format!(
                    "HDR10+ dynamic metadata travels in HEVC, and this job writes {}.",
                    video_codec.label()
                )),
            };
        }
        let action = self.action.unwrap_or_default();
        match action {
            Hdr10PlusAction::Strip => HdrOutcome {
                supported: true,
                args: Vec::new(),
                claim: "HDR10+ dynamic metadata is stripped; static HDR10 remains.".to_string(),
                reason: None,
            },
            Hdr10PlusAction::PassThrough => {
                if caps.hdr10_plus_sei {
                    HdrOutcome {
                        supported: true,
                        args: vec!["-udu_sei".into(), "1".into()],
                        claim: "HDR10+ dynamic metadata is forwarded as user SEI.".to_string(),
                        reason: None,
                    }
                } else {
                    HdrOutcome {
                        supported: false,
                        args: Vec::new(),
                        claim: "HDR10+ dynamic metadata is not carried.".to_string(),
                        reason: Some(
                            "the HEVC encoder this build uses cannot forward user SEI, \
                             which is the path HDR10+ metadata travels"
                                .to_string(),
                        ),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe text a libx265 build with both paths produces.
    const FULL_PROBE: &str = "libx265 AVOptions:\n  -dolbyvision <boolean> Enable Dolby Vision RPU coding\n  -udu_sei <boolean> Use user data unregistered SEI\n";
    /// A build compiled without libx265: nothing is available.
    const EMPTY_PROBE: &str = "Encoder libx265 [libx265 H.265 / HEVC]:\n";

    fn full() -> HdrCapabilities {
        HdrCapabilities::from_probe(FULL_PROBE)
    }

    #[test]
    fn capabilities_are_read_from_the_encoder_probe() {
        let caps = full();
        assert!(caps.dolby_vision_rpu);
        assert!(caps.hdr10_plus_sei);
        let bare = HdrCapabilities::from_probe(EMPTY_PROBE);
        assert!(!bare.dolby_vision_rpu);
        assert!(!bare.hdr10_plus_sei);
        // A build that cannot be probed at all reports nothing rather than guessing.
        assert_eq!(HdrCapabilities::default(), HdrCapabilities {
            dolby_vision_rpu: false,
            hdr10_plus_sei: false
        });
    }

    #[test]
    fn dolby_vision_pass_through_needs_hevc_and_a_capable_encoder() {
        let options = DolbyVisionOptions {
            enabled: true,
            profile: Some(8),
            rpu_present: true,
            action: Some(DolbyVisionAction::PassThrough),
        };
        let outcome = options.resolve(&full(), VideoCodec::H265);
        assert!(outcome.supported, "{}", outcome.claim);
        assert_eq!(outcome.args, vec!["-dolbyvision", "1"]);
        assert!(outcome.claim.contains("RPU"));

        let on_a_plain_codec = options.resolve(&full(), VideoCodec::H264);
        assert!(!on_a_plain_codec.supported);
        assert!(on_a_plain_codec.reason.as_deref().unwrap().contains("HEVC"));

        let on_a_limited_machine = options.resolve(&HdrCapabilities::default(), VideoCodec::H265);
        assert!(!on_a_limited_machine.supported);
        assert!(on_a_limited_machine
            .reason
            .as_deref()
            .unwrap()
            .contains("cannot code"));
        // A request that cannot be met is not quietly dropped: the claim says so.
        assert!(on_a_limited_machine.claim.contains("not carried"));
    }

    #[test]
    fn dolby_vision_can_be_stripped_or_refused() {
        let strip = DolbyVisionOptions {
            enabled: true,
            action: Some(DolbyVisionAction::Strip),
            ..Default::default()
        };
        let outcome = strip.resolve(&HdrCapabilities::default(), VideoCodec::H265);
        assert!(outcome.supported);
        assert!(outcome.args.is_empty());
        assert!(outcome.claim.contains("HDR10 base layer"));

        let extract = DolbyVisionOptions {
            enabled: true,
            action: Some(DolbyVisionAction::ExtractRpu),
            ..Default::default()
        };
        let outcome = extract.resolve(&full(), VideoCodec::H265);
        assert!(!outcome.supported);
        assert!(outcome.reason.as_deref().unwrap().contains("sidecar"));

        let off = DolbyVisionOptions::default();
        assert!(off.resolve(&full(), VideoCodec::H265).args.is_empty());
    }

    #[test]
    fn hdr10_plus_forwarding_needs_the_user_sei_path() {
        let options = Hdr10PlusOptions {
            enabled: true,
            present: true,
            action: Some(Hdr10PlusAction::PassThrough),
        };
        let outcome = options.resolve(&full(), VideoCodec::H265);
        assert!(outcome.supported);
        assert_eq!(outcome.args, vec!["-udu_sei", "1"]);

        let on_a_limited_machine = options.resolve(&HdrCapabilities::default(), VideoCodec::H265);
        assert!(!on_a_limited_machine.supported);
        assert!(on_a_limited_machine
            .reason
            .as_deref()
            .unwrap()
            .contains("user SEI"));

        let strip = Hdr10PlusOptions {
            enabled: true,
            action: Some(Hdr10PlusAction::Strip),
            ..Default::default()
        };
        assert!(strip.resolve(&HdrCapabilities::default(), VideoCodec::H265).supported);
    }

    #[test]
    fn the_actions_read_back_every_way_this_build_spells_them() {
        assert_eq!(DolbyVisionAction::parse("copy"), Some(DolbyVisionAction::PassThrough));
        assert_eq!(
            DolbyVisionAction::parse("extract-rpu"),
            Some(DolbyVisionAction::ExtractRpu)
        );
        assert_eq!(DolbyVisionAction::parse("strip"), Some(DolbyVisionAction::Strip));
        assert_eq!(DolbyVisionAction::parse("shred"), None);
        assert_eq!(Hdr10PlusAction::parse("passthrough"), Some(Hdr10PlusAction::PassThrough));
        assert_eq!(Hdr10PlusAction::parse("drop"), Some(Hdr10PlusAction::Strip));
        assert_eq!(Hdr10PlusAction::parse("maybe"), None);
        assert_eq!(DolbyVisionAction::default(), DolbyVisionAction::PassThrough);
    }
}
