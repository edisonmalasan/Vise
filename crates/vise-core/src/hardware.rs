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

    pub fn supports_encoder(&self, name: &str) -> bool {
        let needle = name.to_lowercase();
        self.encoders
            .iter()
            .any(|e| e.to_lowercase().contains(&needle))
    }

    pub fn supports_decode(&self, codec: &str) -> bool {
        self.supports_decoder(codec)
    }

    pub fn supports_encode(&self, codec: &str) -> bool {
        self.supports_encoder(codec)
    }

    pub fn software_encoder(&self, codec: VideoCodec) -> &'static str {
        match codec {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::Av1 => "libaom-av1",
            VideoCodec::Vp9 => "libvpx-vp9",
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
