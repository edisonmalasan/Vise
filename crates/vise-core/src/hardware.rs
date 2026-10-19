//! Hardware acceleration capability detection and encode/decode option model.
//!
//! This module models ffmpeg hardware accelerators, decoders, and encoders.
//! Detection is based on the installed ffmpeg binary; all paths fall back to
//! software encoding/decoding when hardware support is unavailable.

use crate::VideoCodec;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwareMode {
    #[default]
    Auto,
    Hardware,
    Software,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwarePreference {
    #[default]
    Auto,
    Prefer,
    Force,
    Disable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwareApi {
    #[default]
    Auto,
    Cuda,
    Nvenc,
    Qsv,
    Vaapi,
    Amf,
    Videotoolbox,
    D3d11va,
    Dxva2,
    Opencl,
    Vulkan,
}

impl HardwareApi {
    /// The API a media-tools accelerator name corresponds to, or `Auto` for a
    /// name this build does not know.
    ///
    /// The names the tools publish (`-hwaccels`) and the names of this enum are
    /// not the same list: `nvenc` and `cuda` are one API under two spellings, and
    /// some accelerators have no API here at all. A name that resolves is used
    /// rather than ignored, so a caller that has one does not have to spell it
    /// twice in two vocabularies.
    pub fn of_accelerator(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "cuda" | "cuvid" | "nvenc" | "nvdec" => Self::Cuda,
            "qsv" => Self::Qsv,
            "vaapi" | "vdpau" => Self::Vaapi,
            "amf" => Self::Amf,
            "videotoolbox" => Self::Videotoolbox,
            "d3d11va" => Self::D3d11va,
            "dxva2" | "d3d11va_copy" => Self::Dxva2,
            "opencl" => Self::Opencl,
            "vulkan" => Self::Vulkan,
            _ => Self::Auto,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cuda => "cuda",
            Self::Nvenc => "cuda",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::Amf => "amf",
            Self::Videotoolbox => "videotoolbox",
            Self::D3d11va => "d3d11va",
            Self::Dxva2 => "dxva2",
            Self::Opencl => "opencl",
            Self::Vulkan => "vulkan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HardwareDecodeOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub hwaccel: Option<String>,
    #[serde(default)]
    pub api: HardwareApi,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default = "default_true")]
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HardwareEncodeOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub encoder: Option<String>,
    #[serde(default)]
    pub api: HardwareApi,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default = "default_true")]
    pub fallback: bool,
    #[serde(default)]
    pub quality: Option<u32>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HardwareAcceleration {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: HardwareMode,
    #[serde(default)]
    pub preference: HardwarePreference,
    #[serde(default)]
    pub api: HardwareApi,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default = "default_true")]
    pub fallback: bool,
    #[serde(default)]
    pub decode: HardwareDecodeOptions,
    #[serde(default)]
    pub encode: HardwareEncodeOptions,
}

impl HardwareAcceleration {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            mode: HardwareMode::Software,
            preference: HardwarePreference::Disable,
            fallback: false,
            decode: HardwareDecodeOptions {
                enabled: false,
                fallback: false,
                ..Default::default()
            },
            encode: HardwareEncodeOptions {
                enabled: false,
                fallback: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn auto() -> Self {
        Self {
            enabled: true,
            mode: HardwareMode::Auto,
            preference: HardwarePreference::Auto,
            fallback: true,
            decode: HardwareDecodeOptions {
                enabled: true,
                fallback: true,
                ..Default::default()
            },
            encode: HardwareEncodeOptions {
                enabled: true,
                fallback: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled || self.mode == HardwareMode::Hardware || self.mode == HardwareMode::Auto
    }

    pub fn effective_api(&self) -> HardwareApi {
        if self.api != HardwareApi::Auto {
            self.api
        } else if self.encode.api != HardwareApi::Auto {
            self.encode.api
        } else {
            self.decode.api
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardwareCapabilities {
    pub ffmpeg_path: Option<PathBuf>,
    pub hwaccels: Vec<String>,
    pub decoders: Vec<String>,
    pub encoders: Vec<String>,
    /// The accelerator devices this machine actually has.
    ///
    /// An encoder being compiled into ffmpeg says nothing about whether it can
    /// run: a build made for NVIDIA lists the NVIDIA encoders on a machine with
    /// no NVIDIA card. The device list is what tells the two apart.
    #[serde(default)]
    pub devices: Vec<String>,
}

impl HardwareCapabilities {
    pub fn detect() -> Self {
        Self::detect_with(None)
    }

    pub fn detect_with(ffmpeg: Option<PathBuf>) -> Self {
        let ffmpeg = ffmpeg.or_else(|| {
            crate::FfprobeScanner::default()
                .ffmpeg_path()
                .map(PathBuf::from)
        });
        let Some(ffmpeg) = ffmpeg else {
            return Self::default();
        };
        let hwaccels = run_lines(&ffmpeg, &["-hwaccels"]);
        let decoders = run_lines(&ffmpeg, &["-decoders"]);
        let encoders = run_lines(&ffmpeg, &["-encoders"]);
        Self {
            ffmpeg_path: Some(ffmpeg),
            hwaccels,
            decoders,
            encoders,
            devices: detect_devices(),
        }
    }

    pub fn supports_decoder(&self, name: &str) -> bool {
        let needle = name.to_lowercase();
        self.decoders
            .iter()
            .any(|d| d.to_lowercase().contains(&needle))
            || self
                .hwaccels
                .iter()
                .any(|h| h.to_lowercase().contains(&needle))
    }

    /// Whether an encoder can actually be used on this machine.
    ///
    /// A hardware encoder also needs the device it drives. The list ffmpeg
    /// prints includes the encoders it was built with, whatever hardware is
    /// plugged in, so a machine with no NVIDIA card but a build that mentions
    /// NVIDIA would otherwise be offered a codec that cannot run.
    pub fn supports_encoder(&self, name: &str) -> bool {
        let needle = name.to_lowercase();
        let listed = self
            .encoders
            .iter()
            .any(|e| e.to_lowercase().contains(&needle));
        if !listed {
            return false;
        }
        match device_for_encoder(&needle) {
            Some(device) => self.has_device(device),
            None => true,
        }
    }

    /// Whether this machine has a given accelerator device.
    pub fn has_device(&self, device: &str) -> bool {
        self.devices.iter().any(|d| d.eq_ignore_ascii_case(device))
    }

    pub fn supports_decode(&self, codec: &str) -> bool {
        self.supports_decoder(codec)
    }

    pub fn supports_encode(&self, codec: &str) -> bool {
        self.supports_encoder(codec)
    }

    /// The software encoder this build reaches for when no hardware encoder is
    /// usable for `codec`.
    ///
    /// The professional codecs are named here so a preset can fall back to the
    /// same answer the pipeline would have reached on its own.
    pub fn software_encoder(&self, codec: VideoCodec) -> &'static str {
        match codec {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::Av1 => "libaom-av1",
            VideoCodec::Vp9 => "libvpx-vp9",
            VideoCodec::ProRes => "prores_ks",
            VideoCodec::Dnxhr => "dnxhd",
            VideoCodec::Ffv1 => "ffv1",
        }
    }

    pub fn preferred_encoder(&self, codec: VideoCodec, hw: &HardwareAcceleration) -> String {
        if !hw.is_enabled() || !hw.encode.enabled && hw.mode == HardwareMode::Software {
            // fall through to software if explicitly disabled
        }
        if hw.mode == HardwareMode::Software || hw.preference == HardwarePreference::Disable {
            return self.software_encoder(codec).to_string();
        }
        if let Some(enc) = &hw.encode.encoder {
            if self.supports_encoder(enc) {
                return enc.clone();
            }
            if !hw.fallback && !hw.encode.fallback {
                return enc.clone();
            }
        }
        let candidates: Vec<&str> = match codec {
            VideoCodec::H264 => vec![
                "h264_nvenc",
                "h264_qsv",
                "h264_vaapi",
                "h264_amf",
                "h264_videotoolbox",
            ],
            VideoCodec::H265 => vec![
                "hevc_nvenc",
                "hevc_qsv",
                "hevc_vaapi",
                "hevc_amf",
                "hevc_videotoolbox",
            ],
            VideoCodec::Av1 => vec!["av1_nvenc", "av1_qsv", "av1_vaapi", "av1_amf"],
            VideoCodec::Vp9 => vec!["vp9_vaapi"],
            // The professional codecs are editing formats; no GPU encodes them,
            // so there is no hardware candidate to look for.
            VideoCodec::ProRes | VideoCodec::Dnxhr | VideoCodec::Ffv1 => Vec::new(),
        };
        for c in candidates {
            if self.supports_encoder(c) {
                return c.to_string();
            }
        }
        self.software_encoder(codec).to_string()
    }

    pub fn preferred_decoder_arg(&self, hw: &HardwareAcceleration) -> Option<String> {
        if !hw.is_enabled()
            || hw.mode == HardwareMode::Software
            || hw.preference == HardwarePreference::Disable
        {
            return None;
        }
        if let Some(name) = &hw.decode.hwaccel {
            if name != "auto" && self.supports_decoder(name) {
                return Some(name.clone());
            }
        }
        let api = hw.effective_api();
        if api != HardwareApi::Auto {
            return Some(api.as_str().to_string());
        }
        if hw.decode.enabled {
            // Prefer ffmpeg's auto hardware acceleration when available.
            if self.hwaccels.iter().any(|h| h.trim() == "auto") || !self.hwaccels.is_empty() {
                return Some("auto".to_string());
            }
        }
        None
    }

    pub fn is_hardware_encoder(&self, name: &str) -> bool {
        let n = name.to_lowercase();
        !(n.starts_with("lib") || n == "aac" || n.ends_with("copy"))
    }
}

fn run_lines(ffmpeg: &Path, args: &[&str]) -> Vec<String> {
    let output = Command::new(ffmpeg).args(args).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The accelerator device an encoder drives, when it needs one.
///
/// The software encoders drive nothing, so they are not in this table and are
/// always available.
pub fn device_for_encoder(encoder: &str) -> Option<&'static str> {
    let name = encoder.to_lowercase();
    if name.contains("nvenc") || name.contains("cuvid") {
        Some("cuda")
    } else if name.contains("qsv") {
        Some("qsv")
    } else if name.contains("vaapi") {
        Some("vaapi")
    } else if name.contains("amf") {
        Some("amf")
    } else if name.contains("videotoolbox") {
        Some("videotoolbox")
    } else {
        None
    }
}

/// The accelerator devices this machine actually has.
///
/// The check is deliberately about the device rather than about the driver: a
/// machine with no NVIDIA card has no device node and no driver library, so
/// either answers the question, and the device node is the one that is present
/// on every platform the same way.
pub fn detect_devices() -> Vec<String> {
    let mut devices: Vec<String> = Vec::new();

    // The NVIDIA device node is the one thing that is unambiguously the card.
    if std::path::Path::new("/dev/nvidiactl").exists() {
        devices.push("cuda".to_string());
    }

    // A render node is what the DRM-based encoders drive. Which vendor it is
    // cannot be told from the node alone, so the encoders that use it are
    // offered when one is there, and the vendor-specific ones are not.
    if std::path::Path::new("/dev/dri").is_dir() {
        if let Ok(entries) = std::fs::read_dir("/dev/dri") {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with("renderD") {
                    devices.push("vaapi".to_string());
                    break;
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows the driver's own library is the marker: it is installed
        // with the card and is absent without it.
        let system = std::env::var_os("SystemRoot")
            .map(|root| PathBuf::from(root).join("System32"))
            .unwrap_or_else(|| PathBuf::from("C:/Windows/System32"));
        if system.join("nvcuda.dll").is_file() {
            devices.push("cuda".to_string());
        }
        if system.join("amf.dll").is_file() {
            devices.push("amf".to_string());
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Every Mac has the hardware encoder, and ffmpeg on the platform is
        // built against it.
        devices.push("videotoolbox".to_string());
    }

    devices
}

// Convenience aliases used by callers/tests.
pub type HardwareCapability = HardwareCapabilities;
pub type HardwareCaps = HardwareCapabilities;

pub type HardwareAccelerator = HardwareAcceleration;
pub type HardwareAccel = HardwareAcceleration;
pub type HardwareHwAccel = HardwareAcceleration;
pub type HwAccel = HardwareAcceleration;
pub type HwAcceleration = HardwareAcceleration;

pub type HardwareAccelerationOptions = HardwareAcceleration;
pub type HardwareAccelOptions = HardwareAcceleration;
pub type HardwareAcceleratorOptions = HardwareAcceleration;
pub type HardwareOptions = HardwareAcceleration;
pub type HardwareConfig = HardwareAcceleration;

pub type HardwareDecoder = HardwareApi;
pub type HardwareEncoder = String;

pub type HardwareDecode = HardwareDecodeOptions;
pub type HardwareEncode = HardwareEncodeOptions;
pub type HardwareDecoderOptions = HardwareDecodeOptions;
pub type HardwareEncoderOptions = HardwareEncodeOptions;

pub fn detect_hardware_capabilities() -> HardwareCapabilities {
    HardwareCapabilities::detect()
}

pub fn detect_capabilities() -> HardwareCapabilities {
    HardwareCapabilities::detect()
}

pub fn detect() -> HardwareCapabilities {
    HardwareCapabilities::detect()
}

pub fn hardware_capabilities() -> HardwareCapabilities {
    HardwareCapabilities::detect()
}

impl TranscoderFacade for HardwareCapabilities {}
pub trait TranscoderFacade {}

// free-standing helpers matching common call styles
pub fn detect_hardware() -> HardwareCapabilities {
    HardwareCapabilities::detect()
}

// Re-export nothing extra

// noop module-level helper type for compatibility
#[derive(Default)]
pub struct HardwareManager {
    pub capabilities: HardwareCapabilities,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            capabilities: HardwareCapabilities::detect(),
        }
    }
    pub fn detect(&mut self) {
        self.capabilities = HardwareCapabilities::detect();
    }
    pub fn capabilities(&self) -> &HardwareCapabilities {
        &self.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_encoders(encoders: &[&str], devices: &[&str]) -> HardwareCapabilities {
        HardwareCapabilities {
            ffmpeg_path: None,
            hwaccels: Vec::new(),
            decoders: Vec::new(),
            encoders: encoders.iter().map(|e| e.to_string()).collect(),
            devices: devices.iter().map(|d| d.to_string()).collect(),
        }
    }

    #[test]
    fn a_software_encoder_needs_no_device() {
        let caps = with_encoders(&["libx264", "libx265"], &[]);
        assert!(caps.supports_encoder("libx264"));
        assert!(caps.supports_encoder("libx265"));
    }

    #[test]
    fn a_hardware_encoder_without_its_device_is_not_offered() {
        // ffmpeg lists the NVIDIA encoders whether or not there is a card.
        let caps = with_encoders(&["h264_nvenc", "hevc_nvenc"], &[]);
        assert!(
            !caps.supports_encoder("h264_nvenc"),
            "a machine with no card must not be offered the card encoder"
        );
    }

    #[test]
    fn a_hardware_encoder_with_its_device_is_offered() {
        let caps = with_encoders(&["h264_nvenc", "hevc_nvenc"], &["cuda"]);
        assert!(caps.supports_encoder("h264_nvenc"));
        assert!(caps.supports_encoder("hevc_nvenc"));
    }

    #[test]
    fn a_device_for_one_vendor_does_not_unlock_another() {
        let caps = with_encoders(&["h264_nvenc", "h264_qsv", "h264_vaapi"], &["cuda"]);
        assert!(caps.supports_encoder("h264_nvenc"));
        assert!(!caps.supports_encoder("h264_qsv"));
        assert!(!caps.supports_encoder("h264_vaapi"));
    }

    #[test]
    fn an_encoder_that_is_not_listed_is_not_offered_whatever_the_device() {
        let caps = with_encoders(&["libx264"], &["cuda"]);
        assert!(!caps.supports_encoder("h264_nvenc"));
    }

    #[test]
    fn each_vendor_encoder_names_the_device_it_needs() {
        assert_eq!(device_for_encoder("h264_nvenc"), Some("cuda"));
        assert_eq!(device_for_encoder("hevc_nvenc"), Some("cuda"));
        assert_eq!(device_for_encoder("h264_qsv"), Some("qsv"));
        assert_eq!(device_for_encoder("h264_vaapi"), Some("vaapi"));
        assert_eq!(device_for_encoder("h264_amf"), Some("amf"));
        assert_eq!(
            device_for_encoder("h264_videotoolbox"),
            Some("videotoolbox")
        );
        assert_eq!(device_for_encoder("libx264"), None);
        assert_eq!(device_for_encoder("aac"), None);
    }

    #[test]
    fn a_machine_with_no_card_offers_the_software_encoder() {
        let caps = with_encoders(&["h264_nvenc", "libx264"], &[]);
        assert_eq!(
            caps.preferred_encoder(VideoCodec::H264, &HardwareAcceleration::default()),
            "libx264",
            "the software encoder is what a machine with no card must fall back to"
        );
    }

    #[test]
    fn a_machine_with_a_card_offers_it() {
        let caps = with_encoders(&["h264_nvenc", "libx264"], &["cuda"]);
        assert_eq!(
            caps.preferred_encoder(VideoCodec::H264, &HardwareAcceleration::default()),
            "h264_nvenc"
        );
    }
}
