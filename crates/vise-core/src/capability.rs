//! A report of what this machine's media tools can actually do.
//!
//! The hardware preferences tab answers "can this machine use an accelerator".
//! That is a yes or no for one machine, in one window, and it is gone the
//! moment the window closes. A report is the other thing: the whole picture,
//! written down, in a form a person can read and a program can read, so it can
//! be attached to a question about why an encode looked the way it did.
//!
//! # Detection is injected, not performed
//!
//! Detection shells out to ffmpeg twice — once for the encoder and decoder
//! lists, once for the HEVC encoder's help text — and that is a fact about the
//! machine, not about the report. So [`CapabilityReport::from_detected`] takes
//! the detection as an argument and the report is a pure function of it, which
//! is what lets a test pin the report against a ffmpeg build nobody in this
//! repository owns. [`CapabilityReport::detect`] is the thin wrapper that
//! actually asks the machine.
//!
//! # The lists are sorted and de-duplicated
//!
//! ffmpeg's encoder list is not a set: it carries capability flags, and two
//! lines can name the same encoder under different flags. A report whose list
//! order changed between two runs on the same machine would be useless for
//! diffing, so the report sorts and de-duplicates everything it carries.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::hardware::HardwareCapabilities;
use crate::hdr::HdrCapabilities;
use crate::VISE_VERSION;

/// Version of the report document this build writes.
pub const REPORT_VERSION: u32 = 1;

/// Fragments that mark an encoder or decoder as a hardware one.
///
/// Kept as a list rather than as a rule so that a report can say *why* it
/// classified something, and so a build with an accelerator nobody has heard of
/// can be added to the list by one entry rather than by a new code path.
const HARDWARE_MARKERS: &[&str] = &[
    "_nvenc",
    "_qsv",
    "_vaapi",
    "_amf",
    "_videotoolbox",
    "_v4l2m2m",
    "_mediacodec",
    "_omx",
    "_rkmpp",
    "cuda",
    "cuvid",
    "d3d11va",
    "dxva2",
    "nvdec",
    "opencl",
    "qsv",
    "vaapi",
    "vdpau",
    "videotoolbox",
    "vulkan",
];

/// Whether a name names something a GPU rather than the CPU does.
pub fn is_hardware_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    HARDWARE_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// What this machine can do, as a document.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CapabilityReport {
    /// Version of the report document.
    pub version: u32,
    /// The application that produced it.
    pub application: String,
    /// The application version.
    pub application_version: String,
    /// Whether ffmpeg was found at all.
    ///
    /// A report built from a machine with no ffmpeg is still a report: it is the
    /// most useful thing anybody can be handed when the question is "why does
    /// nothing work", so the empty case is stated rather than left out.
    pub detected: bool,
    /// Hardware acceleration names the tools publish.
    pub accelerators: Vec<String>,
    /// Hardware encoders the tools publish.
    pub hardware_encoders: Vec<String>,
    /// How many encoders were published in all.
    pub encoder_count: usize,
    /// How many decoders were published in all.
    pub decoder_count: usize,
    /// Decoders this build's presets ask for, and whether they were found.
    pub wanted_decoders: Vec<WantedCapability>,
    /// Encoders this build's presets ask for, and whether they were found.
    pub wanted_encoders: Vec<WantedCapability>,
    /// Whether the HEVC encoder can carry a Dolby Vision RPU.
    pub hdr_dolby_vision_rpu: bool,
    /// Whether the HEVC encoder can forward user unregistered SEI.
    pub hdr_hdr10_plus_sei: bool,
    /// One line per thing worth knowing, for the human-readable form.
    pub notes: Vec<String>,
}

/// One thing this build asks for, and whether the machine has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedCapability {
    /// The name the tools would publish.
    pub name: String,
    /// Whether it was found.
    pub found: bool,
    /// Whether it is a hardware name rather than a software one.
    pub hardware: bool,
}

/// The encoders this build reaches for when a preset asks for a codec.
///
/// Named here rather than read out of [`HardwareCapabilities::preferred_encoder`]
/// because that method answers a question *about a configuration*, and a report
/// must not depend on the answer of a question nobody asked.
const SOFTWARE_ENCODERS: &[&str] = &[
    "libx264",
    "libx265",
    "libaom-av1",
    "libvpx-vp9",
    "prores_ks",
    "dnxhd",
    "ffv1",
];

/// The decoders this build's presets ask for.
const SOFTWARE_DECODERS: &[&str] = &["h264", "hevc", "av1", "vp9", "mjpeg"];

impl CapabilityReport {
    /// Detect on this machine and report what was found.
    pub fn detect() -> Self {
        let capabilities = HardwareCapabilities::detect();
        let hdr = HdrCapabilities::detect(capabilities.ffmpeg_path.as_deref());
        Self::from_detected(&capabilities, &hdr)
    }

    /// Build a report from an injected detection.
    ///
    /// The whole report is decided here, so a test can pin it against a
    /// detection that no machine in this repository can produce.
    pub fn from_detected(capabilities: &HardwareCapabilities, hdr: &HdrCapabilities) -> Self {
        let encoders = sorted(&capabilities.encoders);
        let hardware_encoders = encoders
            .iter()
            .filter(|name| is_hardware_name(name))
            .cloned()
            .collect::<Vec<String>>();
        let wanted_decoders = SOFTWARE_DECODERS
            .iter()
            .map(|name| WantedCapability {
                name: (*name).to_string(),
                found: capabilities.supports_decoder(name),
                hardware: false,
            })
            .collect::<Vec<_>>();
        let mut wanted_encoders: Vec<WantedCapability> = SOFTWARE_ENCODERS
            .iter()
            .map(|name| WantedCapability {
                name: (*name).to_string(),
                found: capabilities.supports_encoder(name),
                hardware: false,
            })
            .collect();
        // The hardware encoders this build would reach for on this codec set,
        // listed rather than counted, because "there are two" is not a support
        // question and "hevc_nvenc, hevc_qsv" is.
        for name in ["h264_nvenc", "hevc_nvenc", "av1_nvenc", "hevc_amf"] {
            wanted_encoders.push(WantedCapability {
                name: name.to_string(),
                found: capabilities.supports_encoder(name),
                hardware: true,
            });
        }

        let mut notes = Vec::new();
        if capabilities.ffmpeg_path.is_none() {
            notes.push(
                "ffmpeg was not found, so nothing below was detected and every encode will fail."
                    .to_string(),
            );
        } else if capabilities.encoders.is_empty() && capabilities.decoders.is_empty() {
            notes.push(
                "ffmpeg was found but published no encoders or decoders; the tools could not be run."
                    .to_string(),
            );
        }
        if capabilities.hwaccels.is_empty() && !capabilities.encoders.is_empty() {
            notes.push(
                "No hardware acceleration was published, so encoding runs on the processor."
                    .to_string(),
            );
        }
        if hardware_encoders.is_empty() {
            notes.push("No hardware encoder is available on this build of the tools.".to_string());
        }
        if !hdr.dolby_vision_rpu {
            notes.push(
                "The HEVC encoder was built without Dolby Vision RPU coding, so Dolby Vision metadata cannot be carried."
                    .to_string(),
            );
        }
        if !hdr.hdr10_plus_sei {
            notes.push(
                "The HEVC encoder was built without the user SEI path, so HDR10+ dynamic metadata cannot be forwarded."
                    .to_string(),
            );
        }

        CapabilityReport {
            version: REPORT_VERSION,
            application: "Vise Transcoder".to_string(),
            application_version: VISE_VERSION.to_string(),
            detected: capabilities.ffmpeg_path.is_some(),
            accelerators: sorted(&capabilities.hwaccels),
            hardware_encoders,
            encoder_count: capabilities.encoders.len(),
            decoder_count: capabilities.decoders.len(),
            wanted_decoders,
            wanted_encoders,
            hdr_dolby_vision_rpu: hdr.dolby_vision_rpu,
            hdr_hdr10_plus_sei: hdr.hdr10_plus_sei,
            notes,
        }
    }

    /// The report as JSON, for a program.
    ///
    /// Pretty-printed on purpose: this is a document somebody is going to read
    /// in a text editor when a program will not open it, and the machine-readable
    /// form is still valid JSON with the whitespace.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// The report as text, for a person.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {}\n",
            self.application, self.application_version
        ));
        out.push_str(&format!("report version {}\n", self.version));
        out.push_str(if self.detected {
            "The media tools were found and asked.\n"
        } else {
            "The media tools were NOT found; everything below is empty.\n"
        });
        out.push('\n');
        section(&mut out, "Acceleration", &self.accelerators);
        section(&mut out, "Hardware encoders", &self.hardware_encoders);
        out.push_str(&format!(
            "Published by the tools: {} encoder(s), {} decoder(s)\n",
            self.encoder_count, self.decoder_count
        ));
        out.push('\n');
        wanted(
            &mut out,
            "Encoders this build asks for",
            &self.wanted_encoders,
        );
        wanted(
            &mut out,
            "Decoders this build asks for",
            &self.wanted_decoders,
        );
        out.push_str("\nHDR\n");
        out.push_str(&format!(
            "  Dolby Vision RPU coding: {}\n",
            yes_no(self.hdr_dolby_vision_rpu)
        ));
        out.push_str(&format!(
            "  HDR10+ dynamic metadata: {}\n",
            yes_no(self.hdr_hdr10_plus_sei)
        ));
        if !self.notes.is_empty() {
            out.push_str("\nNotes\n");
            for note in &self.notes {
                out.push_str(&format!("  - {note}\n"));
            }
        }
        out
    }

    /// One line describing the report, for a surface header.
    pub fn summary(&self) -> String {
        if !self.detected {
            return "The media tools were not found on this machine.".to_string();
        }
        let mut line = format!(
            "{} encoder(s) and {} decoder(s); {} hardware encoder(s)",
            self.encoder_count,
            self.decoder_count,
            self.hardware_encoders.len()
        );
        if !self.accelerators.is_empty() {
            line.push_str(&format!("; acceleration: {}", self.accelerators.join(", ")));
        }
        line
    }
}

fn sorted(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "available"
    } else {
        "not available"
    }
}

fn section(out: &mut String, heading: &str, names: &[String]) {
    out.push_str(&format!("{heading}\n"));
    if names.is_empty() {
        out.push_str("  (none)\n");
        return;
    }
    for name in names {
        out.push_str(&format!("  {name}\n"));
    }
}

fn wanted(out: &mut String, heading: &str, entries: &[WantedCapability]) {
    out.push_str(&format!("{heading}\n"));
    if entries.is_empty() {
        out.push_str("  (none)\n");
        return;
    }
    for entry in entries {
        out.push_str(&format!(
            "  {:<16} {:<12} {}\n",
            entry.name,
            yes_no(entry.found),
            if entry.hardware {
                "hardware"
            } else {
                "software"
            }
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn detected() -> HardwareCapabilities {
        HardwareCapabilities {
            ffmpeg_path: Some(PathBuf::from("/opt/ffmpeg")),
            hwaccels: vec!["cuda".into(), "vaapi".into(), "cuda".into()],
            encoders: vec![
                "hevc_nvenc".into(),
                "libx265".into(),
                "libx264".into(),
                "hevc_nvenc".into(),
                "h264_vaapi".into(),
                "hevc_amf".into(),
            ],
            decoders: vec!["h264".into(), "hevc".into()],
        }
    }

    fn hdr(rpu: bool, sei: bool) -> HdrCapabilities {
        HdrCapabilities {
            dolby_vision_rpu: rpu,
            hdr10_plus_sei: sei,
        }
    }

    #[test]
    fn a_report_is_built_from_the_injected_detection() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(true, true));
        assert!(report.detected);
        assert_eq!(report.version, REPORT_VERSION);
        assert_eq!(report.application_version, VISE_VERSION);
        assert_eq!(report.accelerators, vec!["cuda", "vaapi"]);
        assert_eq!(
            report.hardware_encoders,
            vec!["h264_vaapi", "hevc_amf", "hevc_nvenc"]
        );
        assert_eq!(report.encoder_count, 6);
        assert_eq!(report.decoder_count, 2);
        assert!(report.hdr_dolby_vision_rpu);
        assert!(report.hdr_hdr10_plus_sei);
        assert!(report.notes.is_empty(), "{:?}", report.notes);
    }

    #[test]
    fn the_lists_are_sorted_and_de_duplicated_so_two_reports_can_be_diffed() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(true, true));
        assert_eq!(
            report.hardware_encoders,
            vec!["h264_vaapi", "hevc_amf", "hevc_nvenc"],
            "the duplicate hevc_nvenc line is gone and the list is ordered"
        );
        assert_eq!(report.accelerators, vec!["cuda", "vaapi"]);
    }

    #[test]
    fn a_machine_with_no_tools_gets_a_report_that_says_so() {
        let report = CapabilityReport::from_detected(
            &HardwareCapabilities::default(),
            &HdrCapabilities::default(),
        );
        assert!(!report.detected);
        assert_eq!(report.encoder_count, 0);
        assert!(report.accelerators.is_empty());
        assert!(report.hardware_encoders.is_empty());
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("ffmpeg was not found")),
            "{:?}",
            report.notes
        );
        assert_eq!(
            report.summary(),
            "The media tools were not found on this machine."
        );
        assert!(report.to_text().contains("NOT found"));
    }

    #[test]
    fn a_machine_with_no_accelerator_says_encoding_runs_on_the_processor() {
        let mut caps = detected();
        caps.hwaccels.clear();
        caps.encoders = vec!["libx264".into(), "libx265".into()];
        let report = CapabilityReport::from_detected(&caps, &hdr(false, false));
        assert!(report.notes.iter().any(|note| note.contains("processor")));
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("No hardware encoder")));
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("Dolby Vision RPU coding")),
            "the missing HDR paths are named rather than left blank: {:?}",
            report.notes
        );
    }

    #[test]
    fn the_encoders_this_build_asks_for_are_answered_one_by_one() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(true, true));
        let x265 = report
            .wanted_encoders
            .iter()
            .find(|entry| entry.name == "libx265")
            .expect("x265");
        assert!(x265.found);
        assert!(!x265.hardware);
        let amf = report
            .wanted_encoders
            .iter()
            .find(|entry| entry.name == "av1_nvenc")
            .expect("av1");
        assert!(!amf.found, "not published by this build");
        assert!(amf.hardware);
        let h265 = report
            .wanted_decoders
            .iter()
            .find(|entry| entry.name == "hevc")
            .expect("hevc");
        assert!(h265.found);
        let av1 = report
            .wanted_decoders
            .iter()
            .find(|entry| entry.name == "av1")
            .expect("av1");
        assert!(!av1.found);
    }

    #[test]
    fn the_json_form_is_valid_json_and_carries_the_same_facts() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(true, false));
        let json = report.to_json();
        let back: CapabilityReport = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, report);
        assert!(json.contains("\"hdrDolbyVisionRpu\": true"));
        assert!(json.contains("\"hdrHdr10PlusSei\": false"));
    }

    #[test]
    fn the_text_form_is_readable_and_names_what_is_missing() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(false, true));
        let text = report.to_text();
        assert!(text.starts_with("Vise Transcoder "));
        assert!(text.contains("Hardware encoders"));
        assert!(text.contains("hevc_nvenc"));
        assert!(text.contains("Dolby Vision RPU coding: not available"));
        assert!(text.contains("HDR10+ dynamic metadata: available"));
        assert!(text.contains("Notes"));
    }

    #[test]
    fn an_empty_list_is_shown_as_none_rather_than_as_a_blank_section() {
        let mut caps = detected();
        caps.hwaccels.clear();
        let report = CapabilityReport::from_detected(&caps, &hdr(true, true));
        assert!(report.to_text().contains("Acceleration\n  (none)\n"));
    }

    #[test]
    fn the_summary_is_one_line_and_names_what_was_found() {
        let report = CapabilityReport::from_detected(&detected(), &hdr(true, true));
        let summary = report.summary();
        assert!(!summary.contains('\n'));
        assert!(summary.contains("6 encoder(s)"));
        assert!(summary.contains("3 hardware encoder(s)"));
        assert!(summary.contains("cuda"));
    }

    #[test]
    fn a_hardware_name_is_recognised_by_the_list_and_nothing_else() {
        assert!(is_hardware_name("hevc_nvenc"));
        assert!(is_hardware_name("h264_qsv"));
        assert!(is_hardware_name("AV1_VAAPI"));
        assert!(is_hardware_name("cuda"));
        assert!(!is_hardware_name("libx265"));
        assert!(!is_hardware_name("aac"));
        assert!(!is_hardware_name("prores_ks"));
    }
}
