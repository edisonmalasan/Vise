//! Lossless preservation planning.
//!
//! A lossless claim is easy to make and easy to break: the container has to
//! carry the codec, the codec has to be an intra lossless one, and the audio has
//! to be reproduced without loss too. This module resolves a job into the
//! concrete encoders a lossless Matroska write needs and states, in one line,
//! whether what comes out really is lossless — or why it is not.
//!
//! The rules are deliberately narrower than "any intra codec counts". FFV1 is
//! the one video codec this build can honestly call lossless; ProRes and DNxHR
//! are editing interchange formats whose profiles are visually lossy, so a job
//! built around them is described as what it is rather than sold as lossless.

use serde::{Deserialize, Serialize};

use crate::{AudioCodec, ContainerFormat, JobSpec, VideoCodec};

/// The encoder a lossless video codec resolves to.
pub fn lossless_video_encoder(codec: VideoCodec) -> Option<&'static str> {
    match codec {
        VideoCodec::Ffv1 => Some("ffv1"),
        VideoCodec::ProRes => Some("prores_ks"),
        VideoCodec::Dnxhr => Some("dnxhd"),
        _ => None,
    }
}

/// Whether a video codec reproduces the source exactly.
///
/// Only FFV1 is mathematically lossless here. ProRes and DNxHR are preserved
/// separately as editing formats; calling them lossless would be a claim this
/// build cannot stand behind.
pub fn video_is_lossless(codec: VideoCodec) -> bool {
    matches!(codec, VideoCodec::Ffv1)
}

/// Whether an audio codec reproduces the source exactly.
pub fn audio_is_lossless(codec: AudioCodec) -> bool {
    matches!(codec, AudioCodec::Flac | AudioCodec::Pcm)
}

/// The encoder a lossless audio codec resolves to.
pub fn lossless_audio_encoder(codec: AudioCodec) -> Option<&'static str> {
    match codec {
        AudioCodec::Flac => Some("flac"),
        AudioCodec::Pcm => Some("pcm_s16le"),
        _ => None,
    }
}

/// A resolved lossless write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LosslessPlan {
    /// The container the plan writes.
    pub container: ContainerFormat,
    /// The video codec the plan writes.
    pub video_codec: VideoCodec,
    /// The audio codec the plan writes.
    pub audio_codec: AudioCodec,
    /// Concrete video encoder name, when the codec resolves to one.
    pub video_encoder: Option<String>,
    /// Concrete audio encoder name, when the codec resolves to one.
    pub audio_encoder: Option<String>,
    /// Pixel format the plan pins.
    pub pix_fmt: Option<String>,
    /// Whether the plan can honestly claim no loss.
    pub lossless: bool,
    /// One line saying what the finished file is.
    pub claim: String,
    /// Why the plan is not lossless, when it is not.
    pub reason: Option<String>,
}

impl LosslessPlan {
    /// Resolve the lossless write a job describes.
    pub fn resolve(spec: &JobSpec) -> LosslessPlan {
        let container = spec.container;
        let video_codec = spec.video_codec;
        let audio_codec = spec.audio_codec;
        let audio_opts = spec.audio_processing_options();
        let passthrough = audio_opts.passthrough || audio_opts.copy;

        let video_encoder = lossless_video_encoder(video_codec).map(str::to_string);
        // A passthrough job keeps the source audio exactly as it is, so there
        // is no audio encoder to name.
        let audio_encoder = if passthrough {
            None
        } else {
            lossless_audio_encoder(audio_codec).map(str::to_string)
        };

        let pix_fmt = if video_codec == VideoCodec::Ffv1 {
            Some(if spec.video_processing_options().ten_bit {
                "yuv420p10le".to_string()
            } else {
                "yuv420p".to_string()
            })
        } else {
            None
        };

        let mut reasons: Vec<String> = Vec::new();
        if container != ContainerFormat::Mkv {
            reasons.push("a lossless Matroska write needs the Matroska container".to_string());
        }
        if !video_is_lossless(video_codec) {
            reasons.push(format!(
                "{} is an editing interchange format, not a mathematically lossless codec",
                video_codec.label()
            ));
        }
        if !passthrough && !audio_is_lossless(audio_codec) {
            reasons.push(format!(
                "{} audio is not lossless",
                audio_codec.label()
            ));
        }

        let lossless = reasons.is_empty();
        let claim = if lossless {
            let audio = if passthrough {
                "the source audio passed through unchanged".to_string()
            } else {
                format!("lossless {} audio", audio_codec.label())
            };
            format!(
                "Lossless {} video in Matroska with {}.",
                video_codec.label(),
                audio
            )
        } else {
            format!(
                "Not a lossless write: {}.",
                reasons.join("; ")
            )
        };
        let reason = if lossless {
            None
        } else {
            Some(reasons.join("; "))
        };

        LosslessPlan {
            container,
            video_codec,
            audio_codec,
            video_encoder,
            audio_encoder,
            pix_fmt,
            lossless,
            claim,
            reason,
        }
    }

    /// The encoder arguments a lossless write needs.
    ///
    /// Note what is absent: a lossless intra codec takes no `-crf`, so a
    /// constant-quality value must not leak in and be refused by the encoder.
    pub fn encoder_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if let Some(encoder) = &self.video_encoder {
            args.push("-c:v".into());
            args.push(encoder.clone());
        }
        if let Some(encoder) = &self.audio_encoder {
            args.push("-c:a".into());
            args.push(encoder.clone());
        }
        if let Some(pix_fmt) = &self.pix_fmt {
            args.push("-pix_fmt".into());
            args.push(pix_fmt.clone());
        }
        args
    }
}

/// Whether a job asks for a lossless write.
pub fn is_lossless_request(spec: &JobSpec) -> bool {
    spec.lossless_enabled() || spec.resolved_encoding_mode().is_lossless()
}

/// Validate a lossless request, returning every problem at once.
///
/// The container and codec rules live here rather than in the worker's generic
/// validator because they are specific to what "lossless" means: a job that
/// asks for no loss and is handed a lossy codec has been misunderstood, not
/// merely configured differently.
pub fn validate_lossless(spec: &JobSpec) -> Result<LosslessPlan, Vec<String>> {
    let plan = LosslessPlan::resolve(spec);
    if plan.lossless {
        return Ok(plan);
    }
    Err(vec![plan.claim])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioProcessingOptions, VideoProcessingOptions};

    fn ffv1_mkv() -> JobSpec {
        JobSpec {
            container: ContainerFormat::Mkv,
            video_codec: VideoCodec::Ffv1,
            audio_codec: AudioCodec::Flac,
            lossless: true,
            video_processing: VideoProcessingOptions::default(),
            audio_processing: AudioProcessingOptions::default(),
            ..JobSpec::default()
        }
    }

    #[test]
    fn an_ffv1_flac_mkv_is_a_lossless_write() {
        let plan = LosslessPlan::resolve(&ffv1_mkv());
        assert!(plan.lossless, "{}", plan.claim);
        assert_eq!(plan.video_encoder.as_deref(), Some("ffv1"));
        assert_eq!(plan.audio_encoder.as_deref(), Some("flac"));
        assert_eq!(plan.pix_fmt.as_deref(), Some("yuv420p"));
        assert!(plan.claim.contains("Lossless FFV1"));
        assert!(plan.claim.contains("FLAC"));
        assert!(validate_lossless(&ffv1_mkv()).is_ok());
    }

    #[test]
    fn pcm_audio_and_ten_bit_video_resolve_to_their_encoders() {
        let mut spec = ffv1_mkv();
        spec.audio_codec = AudioCodec::Pcm;
        spec.video_processing.ten_bit = true;
        let plan = LosslessPlan::resolve(&spec);
        assert!(plan.lossless);
        assert_eq!(plan.audio_encoder.as_deref(), Some("pcm_s16le"));
        assert_eq!(plan.pix_fmt.as_deref(), Some("yuv420p10le"));
        let args = plan.encoder_args();
        assert!(args.windows(2).any(|w| w == ["-c:a", "pcm_s16le"]));
        assert!(args.windows(2).any(|w| w == ["-pix_fmt", "yuv420p10le"]));
        assert!(!args.iter().any(|a| a == "-crf"));
    }

    #[test]
    fn passthrough_audio_is_lossless_but_named_for_what_it_is() {
        let mut spec = ffv1_mkv();
        spec.audio_processing.passthrough = true;
        let plan = LosslessPlan::resolve(&spec);
        assert!(plan.lossless);
        assert!(plan.audio_encoder.is_none());
        assert!(plan.claim.contains("passed through"), "{}", plan.claim);
    }

    #[test]
    fn a_lossy_codec_or_container_breaks_the_claim_and_says_which() {
        let mut mp4 = ffv1_mkv();
        mp4.container = ContainerFormat::Mp4;
        let plan = LosslessPlan::resolve(&mp4);
        assert!(!plan.lossless);
        assert!(plan.claim.contains("Matroska container"), "{}", plan.claim);
        assert!(validate_lossless(&mp4).is_err());

        let mut h264 = ffv1_mkv();
        h264.video_codec = VideoCodec::H264;
        let plan = LosslessPlan::resolve(&h264);
        assert!(!plan.lossless);
        assert!(plan.claim.contains("H.264"), "{}", plan.claim);

        let mut aac = ffv1_mkv();
        aac.audio_codec = AudioCodec::Aac;
        let plan = LosslessPlan::resolve(&aac);
        assert!(!plan.lossless);
        assert!(plan.claim.contains("AAC audio is not lossless"), "{}", plan.claim);
    }

    #[test]
    fn prores_is_an_editing_format_not_a_lossless_claim() {
        let mut spec = ffv1_mkv();
        spec.video_codec = VideoCodec::ProRes;
        let plan = LosslessPlan::resolve(&spec);
        assert!(!plan.lossless);
        assert!(plan.claim.contains("editing interchange"), "{}", plan.claim);
        assert_eq!(plan.video_encoder.as_deref(), Some("prores_ks"));
    }

    #[test]
    fn a_lossless_request_is_recognised_from_the_mode_too() {
        let mut spec = ffv1_mkv();
        spec.lossless = false;
        spec.encoding_mode = Some(crate::EncodingMode::Lossless);
        assert!(is_lossless_request(&spec));
        assert!(LosslessPlan::resolve(&spec).lossless);

        let mut plain = ffv1_mkv();
        plain.lossless = false;
        assert!(!is_lossless_request(&plain));
    }
}
