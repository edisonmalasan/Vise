//! What each container can actually carry.
//!
//! The tabs ask the same questions in different places: can this codec go in
//! this container, can this subtitle track be passed through, which preset
//! names does this encoder take. The answers live here so the summary, video,
//! audio, and subtitle tabs cannot disagree with each other, and so a rule
//! change is one edit rather than four.

use vise_core::{AudioCodec, ContainerFormat, VideoCodec};

use crate::tabs::common::ChoiceOption;
use crate::tabs::source::TitleFacts;

/// Containers the job model offers, with what each one is good at.
pub fn container_options() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("mp4", "MP4"),
        ChoiceOption::new("mkv", "Matroska (MKV)"),
        ChoiceOption::new("webm", "WebM"),
    ]
}

/// The container named by `text`, if this build offers it.
pub fn container_of(text: &str) -> Option<ContainerFormat> {
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "mp4" | "mpeg4" => Some(ContainerFormat::Mp4),
        "mkv" | "matroska" => Some(ContainerFormat::Mkv),
        "webm" => Some(ContainerFormat::Webm),
        _ => None,
    }
}

/// The video codec named by `text`, if this build offers it.
pub fn video_codec_of(text: &str) -> Option<VideoCodec> {
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "h264" | "avc" | "x264" => Some(VideoCodec::H264),
        "h265" | "hevc" | "x265" => Some(VideoCodec::H265),
        "av1" => Some(VideoCodec::Av1),
        "vp9" => Some(VideoCodec::Vp9),
        _ => None,
    }
}

/// The audio codec named by `text`, if this build offers it.
pub fn audio_codec_of(text: &str) -> Option<AudioCodec> {
    // Separators are dropped so "e-ac3", "E AC3", and "eac3" all name one codec.
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '_'], "")
        .as_str()
    {
        "aac" => Some(AudioCodec::Aac),
        "opus" | "libopus" => Some(AudioCodec::Opus),
        "vorbis" | "libvorbis" => Some(AudioCodec::Vorbis),
        "flac" => Some(AudioCodec::Flac),
        "mp3" | "libmp3lame" => Some(AudioCodec::Mp3),
        "ac3" => Some(AudioCodec::Ac3),
        "eac3" | "ddp" => Some(AudioCodec::Eac3),
        _ => None,
    }
}

/// Every video codec this build offers.
pub fn video_codec_options() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("h264", "H.264 (AVC)"),
        ChoiceOption::new("h265", "H.265 (HEVC)"),
        ChoiceOption::new("av1", "AV1"),
        ChoiceOption::new("vp9", "VP9"),
    ]
}

/// Every audio codec this build offers.
pub fn audio_codec_options() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("aac", "AAC"),
        ChoiceOption::new("opus", "Opus"),
        ChoiceOption::new("vorbis", "Vorbis"),
        ChoiceOption::new("flac", "FLAC (lossless)"),
        ChoiceOption::new("mp3", "MP3"),
        ChoiceOption::new("ac3", "AC3"),
        ChoiceOption::new("eac3", "E-AC3"),
    ]
}

/// Video codecs the chosen container accepts.
///
/// WebM is a VP9 container, so the choice is made for the user rather than
/// quietly ignored later by the encoder.
pub fn video_codecs_for(container: &str) -> Vec<ChoiceOption> {
    let webm = container.trim().eq_ignore_ascii_case("webm");
    video_codec_options()
        .into_iter()
        .map(|option| {
            if webm && option.id != "vp9" {
                option.blocked("WebM carries VP9 video only.")
            } else {
                option
            }
        })
        .collect()
}

/// Audio codecs the chosen container accepts.
pub fn audio_codecs_for(container: &str) -> Vec<ChoiceOption> {
    let container = container.trim().to_ascii_lowercase();
    let reason_for = |id: &str| -> Option<&'static str> {
        match (container.as_str(), id) {
            ("webm", "aac")
            | ("webm", "ac3")
            | ("webm", "eac3")
            | ("webm", "flac")
            | ("webm", "mp3") => Some("WebM carries Opus or Vorbis audio only."),
            ("mp4", "vorbis") => {
                Some("MP4 players rarely read Vorbis audio; AAC or Opus is safer.")
            }
            _ => None,
        }
    };
    audio_codec_options()
        .into_iter()
        .map(|option| match reason_for(&option.id) {
            Some(reason) => option.blocked(reason),
            None => option,
        })
        .collect()
}

/// The rate-control mode named by `text`, in the terms the job model uses.
///
/// The tabs name a mode in their own words; a preset has to carry the mode the
/// job reads, so the two spellings are settled here rather than in the window.
pub fn encoding_mode_of(text: &str) -> Option<vise_core::EncodingMode> {
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "quality" | "crf" | "cq" | "constant_quality" => {
            Some(vise_core::EncodingMode::ConstantQuality)
        }
        "bitrate" | "abr" | "average_bitrate" => Some(vise_core::EncodingMode::AverageBitrate),
        "target_size" | "target file size" => Some(vise_core::EncodingMode::TargetSize),
        "two_pass" => Some(vise_core::EncodingMode::TwoPass),
        "lossless" => Some(vise_core::EncodingMode::Lossless),
        _ => None,
    }
}

/// The encoder a preset built from these tabs is written for.
///
/// The tabs choose a codec, and the codec names the encoder that writes it in
/// software. Reading it back through the core's own table keeps one answer:
/// a preset saved here names the encoder the pipeline would reach for on its own,
/// and the catalog's availability rules can be asked about it.
pub fn encoder_of(text: &str) -> Option<vise_core::EncoderId> {
    if let Some(named) = vise_core::EncoderId::parse(text) {
        return Some(named);
    }
    let codec = video_codec_of(text)?;
    vise_core::EncoderId::parse(vise_core::HardwareCapabilities::default().software_encoder(codec))
}

/// The target file size a rate control names, in bytes.
///
/// The tabs ask for megabytes and the job asks for bytes, and a size-targeted
/// preset that carried the wrong unit would aim at a different file from the one
/// the user asked for. The conversion is the one the queue already uses, so a
/// preset saved from the tabs and a job typed on them agree to the byte.
pub fn target_size_bytes_of(text: &str, megabytes: Option<u32>) -> Option<u64> {
    if !encoding_mode_of(text)?.is_target_size() {
        return None;
    }
    megabytes
        .map(u64::from)
        .map(|mb| mb.saturating_mul(1_000_000))
}

/// Whether the encoder writes bitmap subtitle pictures rather than text.
pub fn is_bitmap_subtitle(codec: &str) -> bool {
    matches!(
        codec
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str(),
        "pgs"
            | "pgssub"
            | "hdmv_pgs_subtitle"
            | "dvdsub"
            | "dvd_subtitle"
            | "dvbsub"
            | "dvb_subtitle"
            | "xsub"
            | "dvb_teletext"
            | "hardsub"
    )
}

/// Whether the encoder writes text subtitles.
pub fn is_text_subtitle(codec: &str) -> bool {
    matches!(
        codec
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str(),
        "subrip"
            | "srt"
            | "ass"
            | "ssa"
            | "webvtt"
            | "vtt"
            | "mov_text"
            | "tx3g"
            | "text"
            | "subviewer"
            | "microdvd"
            | "stl"
            | "eia_608"
            | "cc"
    )
}

/// Whether a subtitle track can be passed into `container` unchanged.
///
/// `Ok` carries a note for the interface when the track survives but has to be
/// rewritten, such as text moving into an MP4 as a timed-text track.
pub fn subtitle_passthrough(container: &str, codec: &str) -> Result<(), String> {
    let codec = codec.trim().to_ascii_lowercase();
    if codec.is_empty() || codec == "unknown" {
        return Err("This subtitle track has no codec, so it cannot be planned.".to_string());
    }
    match container.trim().to_ascii_lowercase().as_str() {
        "webm" => {
            if codec == "webvtt" || codec == "vtt" {
                Ok(())
            } else {
                Err("WebM output carries WebVTT subtitle text only.".to_string())
            }
        }
        "mp4" => {
            if is_bitmap_subtitle(&codec) {
                Err("MP4 cannot carry bitmap subtitle tracks such as PGS or DVD subtitles; convert them to text or burn them in.".to_string())
            } else if is_text_subtitle(&codec) {
                Ok(())
            } else {
                Err(format!(
                    "'{codec}' subtitle tracks are not planned for MP4 output yet."
                ))
            }
        }
        _ => Ok(()),
    }
}

/// Whether subtitles can be burned into the picture.
///
/// Burn-in draws onto video frames, so a title without a picture stream cannot
/// offer it whatever the container can carry.
pub fn burn_in_available(facts: &TitleFacts) -> Result<(), String> {
    if !facts.is_chosen() {
        return Err("Choose a title before choosing how subtitles are handled.".to_string());
    }
    if !facts.has_frame_size() {
        return Err("The scan reported no picture for this title, so there is nothing to draw subtitles on.".to_string());
    }
    Ok(())
}

/// How many channels a mixdown target carries.
pub fn mixdown_channels(mixdown: &str) -> Option<u32> {
    match mixdown
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "")
        .as_str()
    {
        "auto" | "keep" | "source" => None,
        "mono" | "1.0" => Some(1),
        "stereo" | "2.0" => Some(2),
        "5.1" => Some(6),
        "7.1" => Some(8),
        _ => None,
    }
}

/// Whether a mixdown can be made from a source of `source_channels` channels.
///
/// Mixing down is fine — surround becomes stereo, stereo becomes mono. Mixing
/// *up* would have to invent the missing speakers, so it is refused rather than
/// offered and quietly faked.
pub fn mixdown_is_possible(mixdown: &str, source_channels: u32) -> Result<Option<u32>, String> {
    let Some(target) = mixdown_channels(mixdown) else {
        return Ok(None);
    };
    if source_channels == 0 {
        return Err("The scan did not report a channel count for this track.".to_string());
    }
    if target > source_channels {
        let target_label = match target {
            1 => "mono",
            2 => "stereo",
            6 => "5.1",
            8 => "7.1",
            _ => "that layout",
        };
        return Err(format!(
            "This track has {source_channels} channel(s), so {target_label} cannot be mixed from it."
        ));
    }
    Ok(Some(target))
}

/// Encoder speed presets, per video codec family.
pub fn encoder_presets(codec: &str) -> Vec<ChoiceOption> {
    let options: &[(&str, &str)] = match codec.trim() {
        "av1" => &[
            ("fastest", "Fastest"),
            ("faster", "Faster"),
            ("fast", "Fast"),
            ("medium", "Medium"),
            ("slow", "Slow"),
            ("slower", "Slower"),
            ("slowest", "Slowest"),
        ],
        "vp9" => &[
            ("realtime", "Realtime"),
            ("veryfast", "Very fast"),
            ("faster", "Faster"),
            ("fast", "Fast"),
            ("medium", "Medium"),
            ("slow", "Slow"),
            ("slower", "Slower"),
            ("veryslow", "Very slow"),
        ],
        _ => &[
            ("ultrafast", "Ultrafast"),
            ("superfast", "Superfast"),
            ("veryfast", "Very fast"),
            ("faster", "Faster"),
            ("fast", "Fast"),
            ("medium", "Medium"),
            ("slow", "Slow"),
            ("slower", "Slower"),
            ("veryslow", "Very slow"),
        ],
    };
    let mut choices = vec![ChoiceOption::new("default", "Encoder default")];
    choices.extend(
        options
            .iter()
            .map(|(id, label)| ChoiceOption::new(id, label)),
    );
    choices
}

/// Coding profiles, per video codec family.
pub fn encoder_profiles(codec: &str) -> Vec<ChoiceOption> {
    let options: &[(&str, &str)] = match codec.trim() {
        "av1" => &[("main", "Main")],
        "vp9" => &[("main", "Main"), ("high", "High")],
        _ => &[
            ("baseline", "Baseline"),
            ("main", "Main"),
            ("high", "High"),
            ("high10", "High 10-bit"),
        ],
    };
    let mut choices = vec![ChoiceOption::new("default", "Encoder default")];
    choices.extend(
        options
            .iter()
            .map(|(id, label)| ChoiceOption::new(id, label)),
    );
    choices
}

/// Content tunes, per video codec family.
pub fn encoder_tunes(codec: &str) -> Vec<ChoiceOption> {
    let options: &[(&str, &str)] = match codec.trim() {
        "h264" | "h265" => &[
            ("film", "Film"),
            ("animation", "Animation"),
            ("grain", "Grain-heavy"),
            ("stillimage", "Still images"),
            ("fastdecode", "Fast decode"),
            ("zerolatency", "Low latency"),
        ],
        "av1" => &[("ssim", "Structural similarity"), ("psnr", "Peak signal")],
        "vp9" => &[],
        _ => &[],
    };
    let mut choices = vec![ChoiceOption::new("default", "Encoder default")];
    if options.is_empty() {
        // An encoder family with no tunes still gets a row, and the row says
        // why it has nothing in it.
        choices = vec![ChoiceOption::taken(
            "none",
            "No tunes for this encoder",
            "This encoder family has no content tunes in this build.",
        )];
    }
    choices.extend(
        options
            .iter()
            .map(|(id, label)| ChoiceOption::new(id, label)),
    );
    choices
}

/// Coding levels, per video codec family.
pub fn encoder_levels(codec: &str) -> Vec<ChoiceOption> {
    let levels: &[&str] = match codec.trim() {
        "av1" => &[
            "2.0", "3.0", "4.0", "4.1", "5.0", "5.1", "5.2", "5.3", "6.0", "7.0", "7.1",
        ],
        "vp9" => &[
            "1.0", "2.0", "2.1", "3.0", "3.1", "4.0", "4.1", "5.0", "5.1",
        ],
        _ => &[
            "1.0", "1.1", "1.2", "1.3", "2.0", "2.1", "2.2", "3.0", "3.1", "3.2", "4.0", "4.1",
            "4.2", "5.0", "5.1", "5.2",
        ],
    };
    let mut choices = vec![ChoiceOption::new("default", "Encoder default")];
    choices.extend(
        levels
            .iter()
            .map(|level| ChoiceOption::new(level, &format!("Level {level}"))),
    );
    choices
}

/// Sample rates the audio tab offers.
pub fn sample_rates() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("keep", "Keep the source rate"),
        ChoiceOption::new("48000", "48 kHz"),
        ChoiceOption::new("44100", "44.1 kHz"),
        ChoiceOption::new("32000", "32 kHz"),
        ChoiceOption::new("24000", "24 kHz"),
        ChoiceOption::new("22050", "22.05 kHz"),
        ChoiceOption::new("16000", "16 kHz"),
    ]
}

/// Mixdown targets the audio tab offers.
pub fn mixdowns() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("auto", "Keep the source layout"),
        ChoiceOption::new("mono", "Mono"),
        ChoiceOption::new("stereo", "Stereo"),
        ChoiceOption::new("5.1", "5.1 surround"),
        ChoiceOption::new("7.1", "7.1 surround"),
    ]
}

/// Dither modes the audio tab offers.
pub fn dither_modes() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("none", "No dither"),
        ChoiceOption::new("triangular", "Triangular"),
        ChoiceOption::new("rectangular", "Rectangular"),
    ]
}

/// Audio encoders a per-track row may choose, including "follow the job".
pub fn audio_encoder_options(container: &str) -> Vec<ChoiceOption> {
    let mut choices = vec![ChoiceOption::new("job", "Follow the job")];
    choices.extend(
        audio_codecs_for(container)
            .into_iter()
            .filter(|option| option.available)
            .filter(|option| option.id != "flac" || container.trim().eq_ignore_ascii_case("mkv")),
    );
    choices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_and_codec_names_are_read_leniently_but_not_widely() {
        assert_eq!(container_of("MP4"), Some(ContainerFormat::Mp4));
        assert_eq!(container_of(" matroska "), Some(ContainerFormat::Mkv));
        assert_eq!(container_of("Web-M"), None);
        assert_eq!(container_of("avi"), None);
        assert_eq!(video_codec_of("hevc"), Some(VideoCodec::H265));
        assert_eq!(video_codec_of("x264"), Some(VideoCodec::H264));
        assert_eq!(video_codec_of("theora"), None);
        assert_eq!(audio_codec_of("e-ac3"), Some(AudioCodec::Eac3));
        assert_eq!(audio_codec_of("dts"), None);
    }

    #[test]
    fn webm_pins_video_to_vp9_with_a_reason() {
        let options = video_codecs_for("webm");
        let vp9 = options.iter().find(|o| o.id == "vp9").expect("vp9");
        assert!(vp9.available);
        let h264 = options.iter().find(|o| o.id == "h264").expect("h264");
        assert!(!h264.available);
        assert!(h264.reason.contains("VP9"), "{}", h264.reason);

        let mp4 = video_codecs_for("mp4");
        assert!(mp4.iter().all(|option| option.available));
    }

    #[test]
    fn audio_codec_availability_follows_the_container() {
        let webm = audio_codecs_for("webm");
        for id in ["aac", "mp3", "flac", "ac3"] {
            let option = webm.iter().find(|o| o.id == id).expect(id);
            assert!(!option.available, "{id} should not be offered for WebM");
            assert!(option.reason.contains("Opus"), "{}", option.reason);
        }
        let opus = webm.iter().find(|o| o.id == "opus").expect("opus");
        assert!(opus.available);

        let mp4 = audio_codecs_for("mp4");
        assert!(
            !mp4.iter()
                .find(|o| o.id == "vorbis")
                .expect("vorbis")
                .available
        );
        assert!(audio_codecs_for("mkv").iter().all(|o| o.available));
    }

    #[test]
    fn subtitle_passthrough_is_limited_by_the_container() {
        assert!(subtitle_passthrough("mkv", "pgs").is_ok());
        assert!(subtitle_passthrough("mkv", "subrip").is_ok());
        assert!(subtitle_passthrough("mp4", "subrip").is_ok());
        let bitmap = subtitle_passthrough("mp4", "pgs").expect_err("bitmap into mp4");
        assert!(bitmap.contains("bitmap"), "{bitmap}");
        let webm = subtitle_passthrough("webm", "subrip").expect_err("text into webm");
        assert!(webm.contains("WebVTT"), "{webm}");
        assert!(subtitle_passthrough("webm", "webvtt").is_ok());
        let unknown = subtitle_passthrough("mkv", "  ").expect_err("no codec");
        assert!(unknown.contains("no codec"), "{unknown}");
    }

    #[test]
    fn burn_in_needs_a_picture_stream() {
        let chosen = TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            ..Default::default()
        };
        assert!(burn_in_available(&chosen).is_ok());
        let silent = TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            audio: vec![Default::default()],
            ..Default::default()
        };
        assert!(
            burn_in_available(&silent).is_ok(),
            "audio-only titles are out of scope here"
        );
        let bare = TitleFacts::default();
        assert!(burn_in_available(&bare)
            .expect_err("nothing chosen")
            .contains("Choose a title"));
    }

    #[test]
    fn mixing_down_is_allowed_and_mixing_up_is_not() {
        assert_eq!(mixdown_channels("auto"), None);
        assert_eq!(mixdown_channels("5.1"), Some(6));
        assert_eq!(mixdown_channels("Stereo"), Some(2));
        assert_eq!(mixdown_is_possible("stereo", 6).expect("down"), Some(2));
        assert_eq!(mixdown_is_possible("mono", 2).expect("down"), Some(1));
        assert_eq!(mixdown_is_possible("auto", 6).expect("keep"), None);
        let up = mixdown_is_possible("7.1", 6).expect_err("up");
        assert!(up.contains("6 channel"), "{up}");
        assert!(up.contains("7.1"), "{up}");
        assert!(mixdown_is_possible("stereo", 0)
            .expect_err("unknown")
            .contains("channel count"));
    }

    #[test]
    fn encoder_options_differ_between_codec_families() {
        assert!(encoder_presets("av1").iter().any(|o| o.id == "slowest"));
        assert!(encoder_presets("h264").iter().any(|o| o.id == "ultrafast"));
        assert!(!encoder_presets("h264").iter().any(|o| o.id == "slowest"));
        assert!(encoder_profiles("h264").iter().any(|o| o.id == "high10"));
        assert!(!encoder_profiles("av1").iter().any(|o| o.id == "high10"));
        let vp9_tune = encoder_tunes("vp9");
        assert_eq!(vp9_tune.len(), 1);
        assert!(!vp9_tune[0].available, "the row says why it is empty");
        assert!(encoder_tunes("h264").iter().any(|o| o.id == "film"));
        assert!(encoder_levels("av1").iter().any(|o| o.id == "7.1"));
        assert!(!encoder_levels("av1").iter().any(|o| o.id == "1.0"));
        assert!(encoder_presets("h265").iter().all(|o| o.available));
    }

    #[test]
    fn sample_rates_mixdowns_and_dither_are_offered_in_a_usable_order() {
        assert_eq!(sample_rates()[0].id, "keep");
        assert_eq!(mixdowns()[0].id, "auto");
        assert_eq!(dither_modes()[0].id, "none");
        let encoders = audio_encoder_options("mp4");
        assert_eq!(encoders[0].id, "job");
        assert!(!encoders.iter().any(|o| o.id == "flac"), "MP4 is left out");
        assert!(audio_encoder_options("mkv").iter().any(|o| o.id == "flac"));
    }
}
