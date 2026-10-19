//! The built-in preset catalog: the data, its version, and what it means here.
//!
//! The presets that ship with Vise are data rather than code. They live in
//! `data/builtin-presets.json`, are compiled into the binary with
//! [`include_str!`], and are read once. That is the whole point of the module:
//! a preset can be described, reviewed, diffed, and changed without touching the
//! code that applies it, and the shape of the catalog is a contract between the
//! build and the file rather than a convention spread over call sites.
//!
//! Three questions are answered here and nowhere else.
//!
//! * **Which version is this?** Every catalog and every exchange file carries an
//!   explicit major/minor/micro triple. A file from an older build is migrated
//!   forward by the migrations that exist; a file from a newer build is refused
//!   with the number it declared rather than half-read.
//! * **Can this machine run it?** A preset names the encoder it was written for.
//!   [`resolve_availability`] asks the detected capabilities and answers with the
//!   encoder that will actually be used, or with the reason it cannot be used. It
//!   never answers "something else" quietly: a missing hardware encoder either
//!   becomes its documented software equivalent or the preset is unavailable.
//! * **What does it mean?** [`PresetSpec::cleaned`] and [`PresetSpec::normalized`]
//!   in the [`crate::preset`] module put a preset into its canonical form, so two
//!   files that mean the same thing compare equal and a round trip through import
//!   and export changes nothing.

use crate::{
    BuiltInPreset, CoreError, HardwareAcceleration, HardwareApi, HardwareCapabilities, PresetSpec,
    UserPreset, VideoCodec,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::sync::OnceLock;

/// The encoders a preset can be written for.
///
/// The name is the one the media tools publish, because availability is decided
/// by asking them rather than by asking a table of ours. The grouping is by what
/// the encoder needs, not by who makes it: the four software encoders run
/// anywhere, the hardware ones need a chip, and the professional three are
/// editing formats that no GPU in this list encodes.
///
/// It is written and read as the encoder's own identifier rather than as a
/// variant name, so a preset file says `hevc_nvenc` â€” the same word the media
/// tools, the logs, and an operator's command line all use. An identifier this
/// build does not know is refused rather than kept as text nobody will read
/// again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderId {
    /// Software H.264.
    X264,
    /// Software H.265.
    X265,
    /// Software AV1.
    Av1,
    /// Software VP9.
    Vp9,
    /// ProRes, for editing interchange.
    ProRes,
    /// DNxHR, for editing interchange.
    Dnxhr,
    /// FFV1, for lossless preservation.
    Ffv1,
    /// NVENC H.264.
    NvencH264,
    /// NVENC H.265.
    NvencH265,
    /// NVENC AV1.
    NvencAv1,
    /// Quick Sync H.264.
    QsvH264,
    /// Quick Sync H.265.
    QsvH265,
    /// Quick Sync AV1.
    QsvAv1,
    /// AMF H.264.
    AmfH264,
    /// AMF H.265.
    AmfH265,
    /// AMF AV1.
    AmfAv1,
    /// VideoToolbox H.264.
    VtH264,
    /// VideoToolbox H.265.
    VtH265,
}

/// Every encoder, in the order they are listed in the documentation.
///
/// A list rather than a match because the point of parsing is to be
/// exhaustive: an identifier that is not in here is not an encoder this build
/// knows how to check for. It is public so a caller that has to know what a
/// machine *could* offer â€” a test building a machine that has everything, or a
/// report listing the encoders this build knows â€” reads one list rather than
/// keeping its own.
pub const EVERY_ENCODER: [EncoderId; 18] = [
    EncoderId::X264,
    EncoderId::X265,
    EncoderId::Av1,
    EncoderId::Vp9,
    EncoderId::ProRes,
    EncoderId::Dnxhr,
    EncoderId::Ffv1,
    EncoderId::NvencH264,
    EncoderId::NvencH265,
    EncoderId::NvencAv1,
    EncoderId::QsvH264,
    EncoderId::QsvH265,
    EncoderId::QsvAv1,
    EncoderId::AmfH264,
    EncoderId::AmfH265,
    EncoderId::AmfAv1,
    EncoderId::VtH264,
    EncoderId::VtH265,
];

impl Serialize for EncoderId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.identifier())
    }
}

impl<'de> Deserialize<'de> for EncoderId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        EncoderId::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "'{text}' is not an encoder this build knows; run 'vise' after an update, or edit \
                 the file to name an encoder it does know."
            ))
        })
    }
}

impl EncoderId {
    /// The name the media tools publish this encoder under.
    pub fn identifier(self) -> &'static str {
        match self {
            Self::X264 => "libx264",
            Self::X265 => "libx265",
            Self::Av1 => "libaom-av1",
            Self::Vp9 => "libvpx-vp9",
            Self::ProRes => "prores_ks",
            Self::Dnxhr => "dnxhd",
            Self::Ffv1 => "ffv1",
            Self::NvencH264 => "h264_nvenc",
            Self::NvencH265 => "hevc_nvenc",
            Self::NvencAv1 => "av1_nvenc",
            Self::QsvH264 => "h264_qsv",
            Self::QsvH265 => "hevc_qsv",
            Self::QsvAv1 => "av1_qsv",
            Self::AmfH264 => "h264_amf",
            Self::AmfH265 => "hevc_amf",
            Self::AmfAv1 => "av1_amf",
            Self::VtH264 => "h264_videotoolbox",
            Self::VtH265 => "hevc_videotoolbox",
        }
    }

    /// The encoder named by the media tools' own identifier.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_ascii_lowercase();
        EVERY_ENCODER
            .into_iter()
            .find(|encoder| encoder.identifier() == wanted)
    }

    /// The name a person sees.
    pub fn label(self) -> &'static str {
        match self {
            Self::X264 => "software H.264",
            Self::X265 => "software H.265",
            Self::Av1 => "software AV1",
            Self::Vp9 => "software VP9",
            Self::ProRes => "ProRes",
            Self::Dnxhr => "DNxHR",
            Self::Ffv1 => "FFV1",
            Self::NvencH264 => "NVIDIA H.264",
            Self::NvencH265 => "NVIDIA H.265",
            Self::NvencAv1 => "NVIDIA AV1",
            Self::QsvH264 => "Quick Sync H.264",
            Self::QsvH265 => "Quick Sync H.265",
            Self::QsvAv1 => "Quick Sync AV1",
            Self::AmfH264 => "AMD H.264",
            Self::AmfH265 => "AMD H.265",
            Self::AmfAv1 => "AMD AV1",
            Self::VtH264 => "VideoToolbox H.264",
            Self::VtH265 => "VideoToolbox H.265",
        }
    }

    /// Whether this encoder needs a chip to run.
    pub fn is_hardware(self) -> bool {
        self.hardware_family().is_some()
    }

    /// The family this encoder belongs to, or nothing for a software encoder.
    pub fn hardware_family(self) -> Option<HardwareFamily> {
        match self {
            Self::NvencH264 | Self::NvencH265 | Self::NvencAv1 => Some(HardwareFamily::Nvenc),
            Self::QsvH264 | Self::QsvH265 | Self::QsvAv1 => Some(HardwareFamily::Qsv),
            Self::AmfH264 | Self::AmfH265 | Self::AmfAv1 => Some(HardwareFamily::Vcn),
            Self::VtH264 | Self::VtH265 => Some(HardwareFamily::VideoToolbox),
            _ => None,
        }
    }

    /// The accelerator the media tools spell this encoder's family with.
    pub fn hardware_api(self) -> HardwareApi {
        match self.hardware_family() {
            Some(HardwareFamily::Qsv) => HardwareApi::Qsv,
            Some(HardwareFamily::Nvenc) => HardwareApi::Nvenc,
            Some(HardwareFamily::Vcn) => HardwareApi::Amf,
            Some(HardwareFamily::VideoToolbox) => HardwareApi::Videotoolbox,
            Some(HardwareFamily::MediaFoundation) => HardwareApi::D3d11va,
            _ => HardwareApi::Auto,
        }
    }

    /// The codec this encoder writes.
    pub fn codec(self) -> VideoCodec {
        match self {
            Self::X264 | Self::NvencH264 | Self::QsvH264 | Self::AmfH264 | Self::VtH264 => {
                VideoCodec::H264
            }
            Self::X265 | Self::NvencH265 | Self::QsvH265 | Self::AmfH265 | Self::VtH265 => {
                VideoCodec::H265
            }
            Self::Av1 | Self::NvencAv1 | Self::QsvAv1 | Self::AmfAv1 => VideoCodec::Av1,
            Self::Vp9 => VideoCodec::Vp9,
            Self::ProRes => VideoCodec::ProRes,
            Self::Dnxhr => VideoCodec::Dnxhr,
            Self::Ffv1 => VideoCodec::Ffv1,
        }
    }

    /// The software encoder that writes the same codec, when there is one.
    ///
    /// This is what a hardware preset becomes when the chip is not there. There
    /// is deliberately nothing for the professional codecs: a proxy is not an
    /// interchange master, so a preset that asks for one is unavailable rather
    /// than quietly given something else.
    pub fn software_stand_in(self) -> Option<Self> {
        match self.codec() {
            VideoCodec::H264 => Some(Self::X264),
            VideoCodec::H265 => Some(Self::X265),
            VideoCodec::Av1 => Some(Self::Av1),
            VideoCodec::Vp9 => Some(Self::Vp9),
            _ => None,
        }
    }
}

/// The catalog this build carries.
///
/// Nothing else in the workspace reads the asset file: a test checks it against
/// the schema, and everything else asks for the parsed catalog.
pub const BUILTIN_CATALOG_JSON: &str = include_str!("../data/builtin-presets.json");

/// The version a file is read as when it declares none.
///
/// A preset file written before the version existed still has to be readable, so
/// it is read as the oldest version rather than refused, and the migrations take
/// it forward.
pub const PRESET_UNVERSIONED: PresetVersion = PresetVersion {
    major: 0,
    minor: 0,
    micro: 0,
};

/// The version this build writes.
///
/// A *major* step means an older build cannot read the file and a newer one
/// refuses it rather than guessing: something the reader would have skipped has
/// become load-bearing. A *minor* step means members were added that a reader
/// from before can safely not know about, which is why unknown members are kept
/// rather than dropped. A *micro* step is wording and packing, and changes
/// nothing about how the file is read.
pub const PRESET_SCHEMA_VERSION: PresetVersion = PresetVersion {
    major: 1,
    minor: 0,
    micro: 0,
};

/// The three numbers a preset file carries.
///
/// The numbers are separate rather than one counter because they answer
/// different questions: the major says whether the file can be read at all, the
/// minor says whether the reader knows everything in it, and the micro says
/// nothing about meaning at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PresetVersion {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
}

impl PresetVersion {
    pub fn new(major: u32, minor: u32, micro: u32) -> Self {
        Self {
            major,
            minor,
            micro,
        }
    }

    /// Whether a file declaring this version can be read by this build.
    ///
    /// A newer *major* cannot: the reader would have to skip a member that the
    /// writer made load-bearing, and writing it back out would throw that member
    /// away. A newer *minor* can, because those members are additive by
    /// definition, and they are carried through untouched so the build that wrote
    /// them still finds them.
    pub fn is_readable(&self) -> bool {
        self.major <= PRESET_SCHEMA_VERSION.major
    }

    /// Whether a file declaring this version needs a migration to be read.
    pub fn needs_migration(&self) -> bool {
        *self != PRESET_SCHEMA_VERSION && self.is_readable()
    }

    /// The version as it is written on a status line.
    pub fn label(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.micro)
    }
}

impl Default for PresetVersion {
    fn default() -> Self {
        PRESET_UNVERSIONED
    }
}

/// The categories the catalog is filed under.
///
/// The order is the order the browser offers them in, which is the order a
/// person works through: what is this, where does it go, what can play it,
/// which container, what runs on this machine, and the editing formats last
/// because they are for a different job entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetCategory {
    General,
    Web,
    Devices,
    Matroska,
    Hardware,
    Professional,
}

/// Every category, in catalog order.
pub const CATEGORIES: [PresetCategory; 6] = [
    PresetCategory::General,
    PresetCategory::Web,
    PresetCategory::Devices,
    PresetCategory::Matroska,
    PresetCategory::Hardware,
    PresetCategory::Professional,
];

impl PresetCategory {
    /// The stable identifier used in files and in the browser.
    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Web => "web",
            Self::Devices => "devices",
            Self::Matroska => "matroska",
            Self::Hardware => "hardware",
            Self::Professional => "professional",
        }
    }

    /// The name a person sees.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Web => "Web",
            Self::Devices => "Devices",
            Self::Matroska => "Matroska",
            Self::Hardware => "Hardware",
            Self::Professional => "Professional",
        }
    }

    /// One line describing what belongs in this category.
    pub fn description(self) -> &'static str {
        match self {
            Self::General => "Everyday encodes across the usual resolution ladder.",
            Self::Web => "Delivery for a website or a social platform.",
            Self::Devices => "Files chosen so a particular player will read them.",
            Self::Matroska => "Matroska files in each codec the format carries well.",
            Self::Hardware => "Encoders that need a particular chip to run at all.",
            Self::Professional => "Editing interchange and preservation masters.",
        }
    }

    /// The category named by `text`, refusing anything this build does not file.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_ascii_lowercase();
        CATEGORIES
            .into_iter()
            .find(|category| category.id() == wanted)
    }
}

/// The chip a hardware preset is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareFamily {
    /// No chip: the software encoder runs anywhere the media tools do.
    Software,
    /// Intel Quick Sync.
    Qsv,
    /// NVIDIA NVENC.
    Nvenc,
    /// AMD AMF, which the tools also spell VCE.
    Vcn,
    /// Windows Media Foundation.
    ///
    /// The media tools expose Media Foundation as a decoder and not as a video
    /// encoder, so a preset in this family is here for decode acceleration and
    /// encodes in software. That is stated in the entry's own compatibility note
    /// rather than left for the user to discover from a slower encode.
    MediaFoundation,
    /// Apple VideoToolbox.
    VideoToolbox,
}

impl HardwareFamily {
    /// The name a person sees.
    pub fn label(self) -> &'static str {
        match self {
            Self::Software => "Software",
            Self::Qsv => "Intel Quick Sync",
            Self::Nvenc => "NVIDIA NVENC",
            Self::Vcn => "AMD AMF",
            Self::MediaFoundation => "Windows Media Foundation",
            Self::VideoToolbox => "Apple VideoToolbox",
        }
    }

    /// The accelerator the media tools spell this family with.
    pub fn accelerator(self) -> Option<&'static str> {
        match self {
            Self::Software => None,
            Self::Qsv => Some("qsv"),
            Self::Nvenc => Some("cuda"),
            Self::Vcn => Some("amf"),
            Self::MediaFoundation => Some("d3d11va"),
            Self::VideoToolbox => Some("videotoolbox"),
        }
    }

    /// Every family, in catalog order.
    pub const ALL: [HardwareFamily; 6] = [
        Self::Software,
        Self::Qsv,
        Self::Nvenc,
        Self::Vcn,
        Self::MediaFoundation,
        Self::VideoToolbox,
    ];
}

/// One preset in the catalog.
///
/// Everything a person needs to decide whether they want this, and everything
/// the machine has to agree with before it runs, is on the entry rather than
/// spread through the code: the name, what it is for, which players it is meant
/// for, the encoder it was written for, and the settings themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetEntry {
    /// Stable identifier, used by files, settings, and the browser.
    pub id: String,
    /// The name shown in the list.
    pub name: String,
    /// What the preset is for, in words. Required: a preset nobody can read the
    /// purpose of is a list of numbers.
    pub description: String,
    /// Where it is filed.
    pub category: PresetCategory,
    /// The chip it is written for, when it needs one.
    #[serde(default)]
    pub family: Option<HardwareFamily>,
    /// The encoder it is written for.
    ///
    /// This is the requirement, not a preference: it is what
    /// [`resolve_availability`] asks the machine about.
    #[serde(default)]
    pub encoder: Option<EncoderId>,
    /// Which players, platforms, or workflows this preset is meant for.
    #[serde(default)]
    pub compatibility: Option<String>,
    /// The settings themselves.
    pub spec: PresetSpec,
}

impl PresetEntry {
    /// The encoder that will actually be used, asked of the machine.
    pub fn resolve(
        &self,
        caps: &HardwareCapabilities,
        hw: &HardwareAcceleration,
    ) -> PresetResolution {
        resolve_availability(self, caps, hw)
    }

    /// This entry in its canonical form.
    ///
    /// Cleaning an entry is cleaning its text and its settings together, and it
    /// settles the two spellings of "no chip needed": a preset that writes the
    /// software family is the same preset as one that leaves the family out, so
    /// the entry that says so is folded to the entry that does not. Without that
    /// a file written by a build that named the family would not compare equal to
    /// one written by a build that did not, for no difference anyone can act on.
    ///
    /// Nothing here changes what the preset means, and running it twice changes
    /// nothing the second time.
    pub fn cleaned(&self) -> Self {
        let mut out = self.clone();
        // The identifier is trimmed but not repaired: an identifier this build
        // could not use is reported by [`PresetEntry::validate`] rather than
        // quietly rewritten into one it could.
        out.id = out.id.trim().to_string();
        out.name = out.name.trim().to_string();
        out.description = out.description.trim().to_string();
        out.compatibility = out.compatibility.take().and_then(|text| {
            let text = text.trim().to_string();
            (!text.is_empty()).then_some(text)
        });
        if out.family == Some(HardwareFamily::Software) {
            out.family = None;
        }
        out.spec = out.spec.cleaned();
        out
    }

    /// The preset this entry applies, as the rest of the workspace reads one.
    ///
    /// The settings are normalized against the catalog template so a caller
    /// never has to know which members the template already answers, and the
    /// encoder is copied onto the settings because that is where applying a
    /// preset looks for it.
    pub fn to_builtin(&self) -> BuiltInPreset {
        let mut spec = self.spec.normalized(&preset_template());
        spec.encoder = spec.encoder.or(self.encoder);
        BuiltInPreset {
            id: self.id.clone(),
            name: self.name.clone(),
            description: Some(self.description.clone()),
            preset: spec,
        }
    }

    /// This entry as a stored preset the user can change.
    pub fn to_user_preset(&self) -> UserPreset {
        crate::preset::entry_to_user_preset(self)
    }

    /// Everything that has to be true of an entry before the build will ship it.
    ///
    /// This is checked against the embedded catalog by the test suite, so a
    /// malformed entry is a failed test rather than a row in the browser that
    /// does the wrong thing when it is applied.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems: Vec<String> = Vec::new();
        if self.id.trim().is_empty() {
            problems.push("an entry has no id".to_string());
        } else if !self
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!(
                "the id '{}' uses characters a file name or a settings entry cannot carry",
                self.id
            ));
        }
        if self.name.trim().is_empty() {
            problems.push(format!("'{}' has no name", self.id));
        }
        if self.description.trim().is_empty() {
            problems.push(format!("'{}' has no description", self.id));
        }
        if self
            .compatibility
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
            && self.compatibility.is_some()
        {
            problems.push(format!("'{}' has an empty compatibility note", self.id));
        }
        if self.category == PresetCategory::Devices && self.compatibility.is_none() {
            problems.push(format!(
                "'{}' is a device preset and has to say which devices it is for",
                self.id
            ));
        }
        if self.category == PresetCategory::Hardware && self.family.is_none() {
            problems.push(format!(
                "'{}' is a hardware preset and has to name the family it is for",
                self.id
            ));
        }
        if self.encoder.is_none() {
            problems.push(format!("'{}' names no encoder", self.id));
        }
        if let Some(encoder) = self.encoder {
            let family = self.family.unwrap_or(HardwareFamily::Software);
            // The Media Foundation family is here for decoding: the media tools
            // expose it as a decoder and not as a video encoder, so its presets
            // name the software encoder and say why.
            let decodes_only = family == HardwareFamily::MediaFoundation;
            if family != HardwareFamily::Software && !decodes_only && !encoder.is_hardware() {
                problems.push(format!(
                    "'{}' is filed under {} but its encoder runs in software",
                    self.id,
                    family.label()
                ));
            }
            if family != HardwareFamily::Software
                && !decodes_only
                && encoder.hardware_family() != Some(family)
            {
                problems.push(format!(
                    "'{}' is filed under {} but names the {} encoder",
                    self.id,
                    family.label(),
                    encoder
                        .hardware_family()
                        .map(HardwareFamily::label)
                        .unwrap_or("software")
                ));
            }
            if family == HardwareFamily::Software && encoder.is_hardware() {
                problems.push(format!(
                    "'{}' is filed under the software family but names a hardware encoder",
                    self.id
                ));
            }
        }
        if self.spec.container.is_some() && self.spec.format.is_some() {
            problems.push(format!(
                "'{}' names its container twice; a preset says it once",
                self.id
            ));
        }
        let video = self.spec.video_codec_of();
        let audio = self.spec.audio_codec_of();
        if video.is_none() {
            problems.push(format!("'{}' names no video codec", self.id));
        }
        if audio.is_none() {
            problems.push(format!("'{}' names no audio codec", self.id));
        }
        if let Some((width, height)) = self.spec.scale_to {
            if width == 0 || height == 0 {
                problems.push(format!("'{}' scales to nothing", self.id));
            }
            if width % 2 != 0 || height % 2 != 0 {
                problems.push(format!(
                    "'{}' scales to an odd size, which chroma-subsampled encoders cannot code",
                    self.id
                ));
            }
        }
        if let Some(quality) = self.spec.quality {
            if quality > 63 {
                problems.push(format!(
                    "'{}' asks for quality {quality}, which is outside the 0 to 63 range",
                    self.id
                ));
            }
        }
        if self.spec.target_size.is_some()
            && !self
                .spec
                .encoding_mode
                .map(|mode| mode.is_target_size())
                .unwrap_or(false)
        {
            problems.push(format!(
                "'{}' names a target size without asking for a size-targeted rate control",
                self.id
            ));
        }
        if let (Some(mode), Some(cap)) = (self.spec.encoding_mode, self.spec.duration_cap_seconds) {
            if mode.is_target_size() && cap == 0 {
                problems.push(format!("'{}' has a duration cap of nothing", self.id));
            }
        }
        if let (Some(encoder), Some(codec)) = (self.encoder, video) {
            if encoder.codec() != codec {
                problems.push(format!(
                    "'{}' names the {} encoder but the {} codec",
                    self.id,
                    encoder.label(),
                    codec.label()
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }
}

/// The catalog, as it is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetCatalog {
    /// The version of the schema this file is written in.
    pub version: PresetVersion,
    /// The preset every other preset is read against.
    #[serde(default)]
    pub template: PresetSpec,
    /// The presets, in the order they are offered.
    #[serde(default)]
    pub presets: Vec<PresetEntry>,
}

static CATALOG: OnceLock<PresetCatalog> = OnceLock::new();

/// The catalog this build carries.
///
/// Parsed once and then borrowed for as long as the process runs, so a list of
/// a hundred presets costs nothing to show and the surface cannot offer two
/// different answers about the same entry.
pub fn builtin_catalog() -> &'static PresetCatalog {
    CATALOG.get_or_init(|| {
        serde_json::from_str(BUILTIN_CATALOG_JSON)
            .expect("the embedded preset catalog is not valid JSON; the build is broken")
    })
}

/// The preset every other preset is read against.
///
/// A preset that leaves something out inherits it from here, and a preset that
/// tunes nothing is stored without saying so. The template is deliberately plain:
/// it is the file a new user gets, not a house style.
pub fn preset_template() -> PresetSpec {
    builtin_catalog().template.clone()
}

/// Every entry, in catalog order.
pub fn builtin_entries() -> &'static [PresetEntry] {
    &builtin_catalog().presets
}

/// Identifiers that earlier builds stored, and the entry each one now names.
///
/// A settings file, a stored default, or a queued job written by an earlier
/// build names a preset by its identifier, and that word is in other people's
/// files. The entries themselves are named on the catalog's own scheme, so the
/// older words are mapped here rather than left in the data as three
/// irregularly-named rows: the browser shows one regular list, and the old
/// identifier still resolves to the preset it always meant.
pub const LEGACY_PRESET_ALIASES: [(&str, &str); 3] = [
    ("fast-1080p", "general-fast-1080-30-h264"),
    ("high-quality", "general-super-hq-1080-30-mkv"),
    ("web-optimized", "web-vp9-1080-30"),
];

/// The entry identifier `id` names today.
///
/// An identifier this build ships answers itself. An identifier an earlier build
/// stored is mapped to the entry that replaced it. Anything else has no answer,
/// which is different from an entry that happens to be missing: an alias is
/// retired on purpose, an unknown word is somebody's mistake.
pub fn resolve_builtin_id(id: &str) -> Option<&'static str> {
    let wanted = id.trim();
    LEGACY_PRESET_ALIASES
        .iter()
        .find_map(|(old, current)| (*old == wanted).then_some(*current))
        .or_else(|| {
            builtin_catalog()
                .presets
                .iter()
                .find(|entry| entry.id == wanted)
                .map(|entry| entry.id.as_str())
        })
}

/// The entry with this identifier, resolving the identifiers earlier builds
/// stored.
pub fn builtin_entry(id: &str) -> Option<&'static PresetEntry> {
    let wanted = resolve_builtin_id(id)?;
    builtin_catalog()
        .presets
        .iter()
        .find(|entry| entry.id == wanted)
}

/// The entries in one category, in catalog order.
pub fn builtin_entries_in(category: PresetCategory) -> Vec<&'static PresetEntry> {
    builtin_catalog()
        .presets
        .iter()
        .filter(|entry| entry.category == category)
        .collect()
}

/// How many entries each category holds.
pub fn category_counts() -> Vec<(PresetCategory, usize)> {
    CATEGORIES
        .into_iter()
        .map(|category| {
            let count = builtin_catalog()
                .presets
                .iter()
                .filter(|entry| entry.category == category)
                .count();
            (category, count)
        })
        .collect()
}

/// A preset's description in one language, with the fallback recorded.
///
/// The catalog carries the base text for every preset it ships, so that text is
/// the last rung rather than a special case: a locale that has not translated a
/// preset shows the catalog's own words for it, and `translated` says so
/// instead of the row quietly claiming to be localized. A preset this build
/// does not ship at all has no description in any language, and says that too.
pub fn localized_description(id: &str, locale_tag: &str) -> Option<LocalizedDescription> {
    let entry = builtin_entry(id)?;
    let translated = crate::i18n::preset_description(locale_tag, &entry.id);
    Some(match translated {
        Some(found) => LocalizedDescription {
            text: found.text,
            source: found.source,
            translated: true,
        },
        None => LocalizedDescription {
            text: entry.description.clone(),
            source: crate::i18n::MessageSource::Base,
            translated: false,
        },
    })
}

/// One preset description, as the caller reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedDescription {
    /// The sentence to show.
    pub text: String,
    /// Which file answered.
    pub source: crate::i18n::MessageSource,
    /// Whether this is a translation rather than the catalog's base text.
    pub translated: bool,
}

/// What the machine can do with one preset.
///
/// The answer is an encoder rather than a yes or a no, because "available" on its
/// own is not enough to act on: a caller needs to know which encoder will run,
/// whether that is the one the preset was written for or the software stand-in,
/// and why anything was substituted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetResolution {
    /// The preset this is the answer for.
    pub id: String,
    /// Whether the preset can be used on this machine at all.
    pub available: bool,
    /// The encoder that will be used.
    pub encoder: EncoderId,
    /// Whether that encoder runs on a chip rather than on the processor.
    pub hardware: bool,
    /// Whether the encoder is the software stand-in for what was asked for.
    pub fallback: bool,
    /// Why it cannot be used, when it cannot.
    pub reason: String,
    /// What will happen instead, in words, when something was substituted.
    pub note: String,
}

/// Work out what a preset will actually do on this machine.
///
/// The order of the questions is the whole policy:
///
/// 1. **Was anything detected?** When the media tools could not be probed this
///    build knows nothing about them, and closing every preset on the strength of
///    a probe that never ran would be a guess. Nothing is closed here; the
///    question is answered again the next time the window asks.
/// 2. **Is the machine asking for software?** A preset that names a hardware
///    encoder resolves to its software stand-in when the user has switched
///    hardware off, which is a choice rather than a failure.
/// 3. **Does the machine have the encoder?** If it does, that is the answer.
/// 4. **Is there a software stand-in?** A preset whose encoder has one resolves to
///    it and says so, because a slower encode in the same codec is usually what
///    the user wanted and silence about the substitution is not.
/// 5. **Otherwise the preset is unavailable**, with the missing encoder named.
pub fn resolve_availability(
    entry: &PresetEntry,
    caps: &HardwareCapabilities,
    hw: &HardwareAcceleration,
) -> PresetResolution {
    let wanted = entry
        .encoder
        .or(entry.spec.encoder)
        .unwrap_or(EncoderId::X264);
    let switched_off = !hw.is_enabled()
        || hw.mode == crate::HardwareMode::Software
        || hw.preference == crate::HardwarePreference::Disable;
    let mut resolution = PresetResolution {
        id: entry.id.clone(),
        available: true,
        encoder: wanted,
        hardware: wanted.is_hardware(),
        fallback: false,
        reason: String::new(),
        note: String::new(),
    };
    if caps.ffmpeg_path.is_none() {
        resolution.note = format!(
            "The media tools could not be probed, so this preset has not been checked against \
             this machine; {} will be asked for when one is found.",
            wanted.identifier()
        );
        return resolution;
    }
    if wanted.is_hardware() && switched_off {
        return substitute(
            resolution,
            wanted,
            "Hardware encoding is switched off in the preferences.",
        );
    }
    if caps.supports_encoder(wanted.identifier()) {
        resolution.note = note_for(wanted, wanted.is_hardware());
        return resolution;
    }
    substitute(
        resolution,
        wanted,
        &format!(
            "The media tools on this machine do not offer {}.",
            wanted.identifier()
        ),
    )
}

/// Turn a missing encoder into either its software stand-in or a refusal.
fn substitute(mut resolution: PresetResolution, wanted: EncoderId, why: &str) -> PresetResolution {
    match wanted
        .software_stand_in()
        .filter(|stand_in| *stand_in != wanted)
    {
        Some(software) => {
            resolution.encoder = software;
            resolution.hardware = false;
            resolution.fallback = true;
            resolution.reason = String::new();
            resolution.note = format!(
                "{why} This preset will use {} ({}) in software instead, which is slower and \
                 makes a larger file for the same quality.",
                software.identifier(),
                software.label()
            );
            resolution
        }
        None => {
            resolution.available = false;
            resolution.hardware = false;
            resolution.fallback = false;
            resolution.reason = if wanted.is_hardware() {
                format!(
                    "{why} There is no software stand-in for {}, so this preset cannot run here.",
                    wanted.identifier()
                )
            } else {
                format!(
                    "{why} No other encoder this build knows writes {}, so this preset cannot run \
                     here.",
                    wanted.codec().label()
                )
            };
            resolution.note = String::new();
            resolution
        }
    }
}

/// What the user should be told about an encoder that is available.
fn note_for(encoder: EncoderId, hardware: bool) -> String {
    if hardware {
        format!("Uses {} on the chip.", encoder.identifier())
    } else {
        format!(
            "Uses {} in software, so it runs on any machine.",
            encoder.identifier()
        )
    }
}

// ---------------------------------------------------------------------------
// Exchange files
// ---------------------------------------------------------------------------

/// One preset in a file, with the version beside it.
///
/// The version is the document's member rather than the entry's, so a file that
/// carries several entries says once which schema it is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetDocument {
    /// The version the file was written in.
    #[serde(default)]
    pub version: PresetVersion,
    /// The preset.
    pub entry: PresetEntry,
    /// Members this build does not know, kept exactly as they were found.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl PresetDocument {
    /// The document this build writes for `entry`.
    pub fn new(entry: PresetEntry) -> Self {
        Self {
            version: PRESET_SCHEMA_VERSION,
            entry,
            extra: BTreeMap::new(),
        }
    }

    /// The document, with the entry brought in line with the catalog template.
    pub fn normalized(&self) -> Self {
        let mut out = self.clone();
        out.entry.spec = out.entry.spec.normalized(&preset_template());
        out
    }
}

/// What one migration did, so the log can say it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetMigration {
    /// The version the file declared.
    pub from: PresetVersion,
    /// The version it is readable as after this build's migrations.
    pub to: PresetVersion,
    /// What was changed to get there, in words.
    pub applied: Vec<String>,
}

impl PresetMigration {
    /// A migration that changed nothing, because nothing needed changing.
    fn none(from: PresetVersion) -> Self {
        Self {
            from,
            to: PRESET_SCHEMA_VERSION,
            applied: Vec::new(),
        }
    }
}

/// Read one preset document from raw JSON, migrating it forward if it needs it.
///
/// A document that is not an object is refused: there is nothing in it to read,
/// and guessing would mean writing a file over somebody else's. A document whose
/// version this build cannot read is refused by name, with the number it
/// declared, rather than half-read â€” a partial read of a newer document is how a
/// member somebody depended on gets dropped on the next save.
pub fn read_preset_document(value: &serde_json::Value) -> Result<PresetDocument, CoreError> {
    let Some(members) = value.as_object() else {
        return Err(CoreError::Parse(
            "a preset file has to be a JSON object".to_string(),
        ));
    };
    let declared = declared_version(members);
    if !declared.is_readable() {
        return Err(CoreError::Parse(format!(
            "This preset was written by a newer build of Vise (version {}); this build reads \
             version {} and below, so it has been left alone rather than half-read.",
            declared.label(),
            PRESET_SCHEMA_VERSION.label()
        )));
    }
    let mut migrated = value.clone();
    let migration = migrate_preset_document(&mut migrated, declared)?;
    let mut document: PresetDocument = serde_json::from_value(migrated)
        .map_err(|e| CoreError::Parse(format!("that file could not be read as a preset: {e}")))?;
    // Never write a version below the one that was found. A file from a later
    // minor keeps its number, so the build that wrote it can still read what
    // this one saves.
    document.version = if declared > migration.to {
        declared
    } else {
        migration.to
    };
    Ok(document)
}

/// The version a document declared, or the unversioned number when it declared
/// none or declared something that is not a number.
fn declared_version(members: &serde_json::Map<String, serde_json::Value>) -> PresetVersion {
    let mut version = PRESET_UNVERSIONED;
    if let Some(number) = members.get("version").and_then(serde_json::Value::as_u64) {
        version.major = number.min(u64::from(u32::MAX)) as u32;
        return version;
    }
    if let Some(numbers) = members
        .get("version")
        .and_then(serde_json::Value::as_object)
    {
        let read = |name: &str| {
            numbers
                .get(name)
                .and_then(serde_json::Value::as_u64)
                .map(|value| value.min(u64::from(u32::MAX)) as u32)
        };
        version.major = read("major").unwrap_or(0);
        version.minor = read("minor").unwrap_or(0);
        version.micro = read("micro").unwrap_or(0);
    }
    version
}

/// Bring a preset document up to the version this build reads.
///
/// There is one migration today, and it exists because the first version of a
/// preset file said the same thing twice: every setting had a second member that
/// older files wrote and newer files dropped (`format` beside `container`,
/// `video` beside `video_codec`, and so on). Folding them is the whole of it. A
/// document that declares no version is read as the oldest one and taken through
/// the same path, which is why an unversioned file is never refused.
pub fn migrate_preset_document(
    value: &mut serde_json::Value,
    declared: PresetVersion,
) -> Result<PresetMigration, CoreError> {
    let Some(members) = value.as_object_mut() else {
        return Err(CoreError::Parse(
            "a preset file has to be a JSON object".to_string(),
        ));
    };
    let mut migration = PresetMigration::none(declared);
    // A file written before the document had an entry member was the entry
    // itself, so the whole document becomes one entry.
    if !members.contains_key("entry") {
        let flat = std::mem::take(members);
        let mut entry = flat;
        entry.remove("version");
        let spec = match entry.remove("preset") {
            Some(legacy) => {
                migration
                    .applied
                    .push("read a flat preset file as a document".to_string());
                legacy
            }
            None => match entry.remove("spec") {
                Some(legacy) => legacy,
                None => serde_json::Value::Object(entry.clone()),
            },
        };
        entry.insert("spec".to_string(), spec);
        let mut next = serde_json::Map::new();
        next.insert(
            "version".to_string(),
            serde_json::to_value(PRESET_SCHEMA_VERSION)
                .map_err(|e| CoreError::Parse(e.to_string()))?,
        );
        next.insert("entry".to_string(), serde_json::Value::Object(entry));
        *members = next;
    }
    let Some(entry) = members
        .get_mut("entry")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Err(CoreError::Parse(
            "a preset file has to name the preset it carries".to_string(),
        ));
    };
    // Older files wrote the settings as `preset` rather than `spec`.
    if !entry.contains_key("spec") {
        if let Some(legacy) = entry.remove("preset") {
            entry.insert("spec".to_string(), legacy);
            migration
                .applied
                .push("read the settings from 'preset' as 'spec'".to_string());
        }
    }
    if let Some(spec) = entry
        .get_mut("spec")
        .and_then(serde_json::Value::as_object_mut)
    {
        // The duplicate spellings, folded into the members this build reads. A
        // file that carries both spellings keeps the one this build reads: the
        // second is a leftover, not a newer answer.
        for (old, new) in [
            ("format", "container"),
            ("video", "video_codec"),
            ("audio", "audio_codec"),
            ("subtitles", "subtitle_mode"),
            ("title", "title_index"),
        ] {
            if let Some(legacy) = spec.remove(old) {
                if spec.contains_key(new) {
                    migration.applied.push(format!(
                        "ignored the second copy of '{new}' written as '{old}'"
                    ));
                } else {
                    spec.insert(new.to_string(), legacy);
                    migration.applied.push(format!("read '{old}' as '{new}'"));
                }
            }
        }
    }
    if !members.contains_key("version") {
        members.insert(
            "version".to_string(),
            serde_json::to_value(PRESET_SCHEMA_VERSION)
                .map_err(|e| CoreError::Parse(e.to_string()))?,
        );
        migration
            .applied
            .push("gave the file the version it was written before versions existed".to_string());
    }
    migration.from = declared;
    migration.to = PRESET_SCHEMA_VERSION;
    Ok(migration)
}

/// Write one preset to a file as a versioned document.
pub fn export_preset_entry(path: &std::path::Path, entry: &PresetEntry) -> Result<(), CoreError> {
    write_document(path, &PresetDocument::new(entry.clone()))
}

/// Read one preset out of a versioned document.
///
/// A file this build cannot read is refused before anything is written anywhere,
/// so an import that fails leaves the library exactly as it was.
pub fn import_preset_entry(path: &std::path::Path) -> Result<PresetDocument, CoreError> {
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| CoreError::Parse(format!("that file is not valid JSON: {e}")))?;
    read_preset_document(&value)
}

/// Write one preset document to a file.
pub fn export_preset_document(
    path: &std::path::Path,
    document: &PresetDocument,
) -> Result<(), CoreError> {
    write_document(path, document)
}

/// Read one preset document from a file.
pub fn import_preset_document(path: &std::path::Path) -> Result<PresetDocument, CoreError> {
    import_preset_entry(path)
}

fn write_document(path: &std::path::Path, document: &PresetDocument) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| CoreError::Io(e.to_string()))?;
        }
    }
    let raw =
        serde_json::to_string_pretty(document).map_err(|e| CoreError::Parse(e.to_string()))?;
    fs::write(path, raw).map_err(|e| CoreError::Io(e.to_string()))
}

// ---------------------------------------------------------------------------
// Order and copies
// ---------------------------------------------------------------------------

/// A copy of one entry under a new identifier and name.
///
/// A clone is deep: nothing is shared with the original, so changing the copy
/// cannot reach back into the preset it was copied from. That matters because a
/// copy is the only way to change a preset that ships with the build, and a
/// shared spec would let an edit of the copy rewrite the shipped preset.
pub fn clone_entry(entry: &PresetEntry, id: &str, name: &str) -> PresetEntry {
    let mut copy = entry.clone();
    copy.id = id.trim().to_string();
    let wanted = name.trim();
    copy.name = if wanted.is_empty() {
        copy_suffix(&entry.name)
    } else {
        wanted.to_string()
    };
    copy.description = format!(
        "{} Copied from the preset that ships with this build.",
        copy.description
    );
    copy
}

/// Move an entry `delta` places through a list.
///
/// The list is left exactly as it was when the move would take the entry off
/// either end, so a page that asks for "one more up" at the top gets nothing
/// rather than a wrap it did not ask for.
pub fn move_entry_by<T, F: Fn(&T) -> &str>(list: &mut [T], id: &str, delta: i32, key: F) -> bool {
    let Some(index) = list.iter().position(|item| key(item) == id) else {
        return false;
    };
    let target = index as i32 + delta;
    if target < 0 || target as usize >= list.len() {
        return false;
    }
    list.swap(index, target as usize);
    true
}

/// Move an entry to an absolute place in a list.
pub fn move_entry_to<T, F: Fn(&T) -> &str>(
    list: &mut Vec<T>,
    id: &str,
    target: usize,
    key: F,
) -> bool {
    let Some(index) = list.iter().position(|item| key(item) == id) else {
        return false;
    };
    if target >= list.len() || index == target {
        return false;
    }
    let moved = list.remove(index);
    list.insert(target, moved);
    true
}

/// What was said when a copy was made.
pub fn copy_suffix(name: &str) -> String {
    format!("{name} (copy)")
}

/// Whether two entries would write the same file.
///
/// Two presets that differ only in how they are named would still produce one
/// file over the other, which is why the window asks before storing the second.
pub fn writes_the_same_file(first: &PresetEntry, second: &PresetEntry) -> bool {
    first.id != second.id && first.spec == second.spec
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine with no graphics card.
    ///
    /// Its ffmpeg still lists the NVIDIA encoders, because a build made for
    /// NVIDIA lists them whatever is plugged in, so the list alone says nothing
    /// about the hardware.
    fn software() -> HardwareCapabilities {
        HardwareCapabilities {
            devices: Vec::new(),
            ffmpeg_path: Some(std::path::PathBuf::from("ffmpeg")),
            hwaccels: vec!["d3d11va".to_string()],
            decoders: vec!["h264".to_string()],
            encoders: vec![
                "libx264  H.264".to_string(),
                "libx265  HEVC".to_string(),
                "h264_nvenc NVIDIA".to_string(),
            ],
        }
    }

    /// The same machine, with a card in it.
    fn with_card() -> HardwareCapabilities {
        HardwareCapabilities {
            devices: vec!["cuda".to_string()],
            ..software()
        }
    }

    fn entry(id: &str, encoder: Option<EncoderId>, family: Option<HardwareFamily>) -> PresetEntry {
        PresetEntry {
            id: id.to_string(),
            name: "A preset".to_string(),
            description: "For tests.".to_string(),
            category: match family {
                Some(_) => PresetCategory::Hardware,
                None => PresetCategory::General,
            },
            family,
            encoder,
            compatibility: None,
            spec: PresetSpec {
                container: Some(crate::ContainerFormat::Mp4),
                video_codec: Some(encoder.map(EncoderId::codec).unwrap_or(VideoCodec::H264)),
                audio_codec: Some(crate::AudioCodec::Aac),
                quality: Some(22),
                ..Default::default()
            },
        }
    }

    #[test]
    fn a_software_preset_needs_no_hardware_to_answer_for() {
        let resolution = resolve_availability(
            &entry("a", Some(EncoderId::X264), None),
            &software(),
            &HardwareAcceleration::disabled(),
        );
        assert!(resolution.available);
        assert!(!resolution.hardware);
        assert!(!resolution.fallback);
        assert!(resolution.note.contains("libx264"), "{}", resolution.note);
    }

    #[test]
    fn a_hardware_encoder_this_machine_has_is_used_as_written() {
        let resolution = resolve_availability(
            &entry("a", Some(EncoderId::NvencH264), Some(HardwareFamily::Nvenc)),
            &with_card(),
            &HardwareAcceleration::auto(),
        );
        assert!(resolution.available);
        assert!(resolution.hardware);
        assert!(!resolution.fallback);
        assert_eq!(resolution.encoder, EncoderId::NvencH264);
    }

    #[test]
    fn a_hardware_encoder_this_machine_lacks_becomes_its_software_stand_in() {
        let resolution = resolve_availability(
            &entry("a", Some(EncoderId::QsvH265), Some(HardwareFamily::Qsv)),
            &software(),
            &HardwareAcceleration::auto(),
        );
        assert!(resolution.available);
        assert!(resolution.fallback);
        assert!(!resolution.hardware);
        assert_eq!(resolution.encoder, EncoderId::X265);
        assert!(resolution.note.contains("hevc_qsv"), "{}", resolution.note);
    }

    #[test]
    fn switching_hardware_off_is_a_substitution_and_not_a_failure() {
        let resolution = resolve_availability(
            &entry("a", Some(EncoderId::QsvH265), Some(HardwareFamily::Qsv)),
            &software(),
            &HardwareAcceleration::disabled(),
        );
        assert!(resolution.available);
        assert!(resolution.fallback);
        assert!(
            resolution.note.contains("switched off"),
            "{}",
            resolution.note
        );
    }

    #[test]
    fn nothing_is_closed_on_the_strength_of_a_probe_that_never_ran() {
        let resolution = resolve_availability(
            &entry(
                "a",
                Some(EncoderId::VtH265),
                Some(HardwareFamily::VideoToolbox),
            ),
            &HardwareCapabilities::default(),
            &HardwareAcceleration::auto(),
        );
        assert!(resolution.available);
        assert!(
            resolution.note.contains("could not be probed"),
            "{}",
            resolution.note
        );
    }

    #[test]
    fn a_codec_entry_is_migrated_and_an_unknown_one_is_refused() {
        let legacy = serde_json::json!({
            "id": "mine",
            "name": "Mine",
            "description": "A preset.",
            "category": "general",
            "preset": { "format": "webm", "video": "vp9", "audio": "opus" }
        });
        let mut value = legacy.clone();
        let migration = migrate_preset_document(&mut value, PRESET_UNVERSIONED).expect("migration");
        assert_eq!(migration.to, PRESET_SCHEMA_VERSION);
        assert!(migration.applied.len() >= 2, "{:?}", migration.applied);
        let document = read_preset_document(&legacy).expect("read");
        assert_eq!(
            document.entry.spec.container_of(),
            Some(crate::ContainerFormat::Webm)
        );
        assert_eq!(document.entry.spec.video_codec_of(), Some(VideoCodec::Vp9));

        let future = serde_json::json!({
            "version": { "major": 9, "minor": 0, "micro": 0 },
            "entry": {
                "id": "theirs",
                "name": "Theirs",
                "description": "From a later build.",
                "category": "general",
                "spec": {}
            }
        });
        let error = read_preset_document(&future).expect_err("refused");
        assert!(error.to_string().contains("9.0.0"), "{error}");
    }

    #[test]
    fn members_this_build_does_not_know_are_carried_through() {
        let document = serde_json::json!({
            "version": { "major": 1, "minor": 4, "micro": 2 },
            "entry": {
                "id": "mine",
                "name": "Mine",
                "description": "A preset.",
                "category": "general",
                "spec": {}
            },
            "somethingLater": { "kept": true }
        });
        let read = read_preset_document(&document).expect("read");
        assert_eq!(read.version, PresetVersion::new(1, 4, 2));
        assert_eq!(
            read.extra.get("somethingLater"),
            Some(&serde_json::json!({"kept": true}))
        );
        let written = serde_json::to_value(PresetDocument::new(read.entry)).expect("write");
        assert!(written.get("entry").is_some());
    }

    #[test]
    fn a_copy_is_independent_of_the_preset_it_was_copied_from() {
        let original = entry("a", Some(EncoderId::X264), None);
        let mut copy = clone_entry(&original, "b", "");
        assert_eq!(copy.name, "A preset (copy)");
        copy.spec.quality = Some(10);
        copy.name = "Changed".to_string();
        assert_eq!(original.spec.quality, Some(22));
        assert_eq!(original.name, "A preset");
        assert!(!writes_the_same_file(&original, &copy));
    }

    #[test]
    fn moving_an_entry_leaves_the_list_alone_past_either_end() {
        let mut list = vec!["a", "b", "c"];
        assert!(move_entry_by(&mut list, "b", 1, |id| id));
        assert_eq!(list, vec!["a", "c", "b"]);
        assert!(!move_entry_by(&mut list, "a", -1, |id| id));
        assert!(!move_entry_by(&mut list, "b", 1, |id| id));
        assert_eq!(list, vec!["a", "c", "b"]);
        assert!(!move_entry_by(&mut list, "missing", 1, |id| id));
        assert!(move_entry_to(&mut list, "b", 0, |id| id));
        assert_eq!(list, vec!["b", "a", "c"]);
        assert!(!move_entry_to(&mut list, "b", 9, |id| id));
    }

    #[test]
    fn the_catalog_reads_and_holds_the_version_it_says() {
        let catalog = builtin_catalog();
        assert_eq!(catalog.version, PRESET_SCHEMA_VERSION);
        assert!(!catalog.presets.is_empty());
        assert!(!catalog.template.container.is_none());
        for category in CATEGORIES {
            assert!(
                !builtin_entries_in(category).is_empty(),
                "{} has no presets",
                category.label()
            );
        }
    }
}
