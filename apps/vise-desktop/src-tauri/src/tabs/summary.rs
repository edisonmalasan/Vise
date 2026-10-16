//! The summary tab: what the output will be, and the flags that shape it.
//!
//! This is the tab a user reads before queueing anything, so it repeats the
//! decisions made elsewhere rather than repeating their controls. The one rule
//! that belongs here is container compatibility: a flag that only one container
//! can honour has to say so as soon as the container changes.

use serde::{Deserialize, Serialize};

use crate::tabs::common::{ChoiceGroupViewModel, ToggleViewModel};
use crate::tabs::compatibility::{audio_codecs_for, container_options, video_codecs_for};
use crate::tabs::video::VideoSettings;

/// The output choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SummarySettings {
    /// Container the output is written in.
    pub container: String,
    /// Video codec.
    pub video_codec: String,
    /// Audio codec.
    pub audio_codec: String,
    /// Put the MP4 index at the front.
    pub web_optimized: bool,
    /// Restrict an MP4 to what older portable players read.
    pub ipod_compatible: bool,
    /// Nudge audio onto the video timeline.
    pub align_audio_video: bool,
    /// Copy the source tags to the output.
    pub passthrough_metadata: bool,
}

impl Default for SummarySettings {
    fn default() -> Self {
        Self {
            container: "mp4".to_string(),
            video_codec: "h264".to_string(),
            audio_codec: "aac".to_string(),
            web_optimized: true,
            ipod_compatible: false,
            align_audio_video: false,
            passthrough_metadata: true,
        }
    }
}

/// Output choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SummaryInput {
    /// Container the output is written in.
    pub container: Option<String>,
    /// Video codec.
    pub video_codec: Option<String>,
    /// Audio codec.
    pub audio_codec: Option<String>,
    /// Put the MP4 index at the front.
    pub web_optimized: Option<bool>,
    /// Restrict an MP4 to what older portable players read.
    pub ipod_compatible: Option<bool>,
    /// Nudge audio onto the video timeline.
    pub align_audio_video: Option<bool>,
    /// Copy the source tags to the output.
    pub passthrough_metadata: Option<bool>,
}

impl SummaryInput {
    /// Apply the change, refusing names this build does not offer.
    pub fn apply(self, settings: &mut SummarySettings) -> Result<(), String> {
        if let Some(container) = self.container {
            let container = container.trim().to_ascii_lowercase();
            if !container_options()
                .iter()
                .any(|option| option.id == container)
            {
                return Err(format!(
                    "'{container}' is not a container this build offers."
                ));
            }
            settings.container = container;
        }
        if let Some(codec) = self.video_codec {
            let codec = codec.trim().to_ascii_lowercase();
            if !video_codecs_for(&settings.container)
                .iter()
                .any(|option| option.id == codec && option.available)
            {
                return Err(format!(
                    "'{codec}' cannot be written into {} output.",
                    settings.container.to_ascii_uppercase()
                ));
            }
            settings.video_codec = codec;
        }
        if let Some(codec) = self.audio_codec {
            let codec = codec.trim().to_ascii_lowercase();
            if !audio_codecs_for(&settings.container)
                .iter()
                .any(|option| option.id == codec && option.available)
            {
                return Err(format!(
                    "'{codec}' audio cannot be written into {} output.",
                    settings.container.to_ascii_uppercase()
                ));
            }
            settings.audio_codec = codec;
        }
        if let Some(value) = self.web_optimized {
            settings.web_optimized = value;
        }
        if let Some(value) = self.ipod_compatible {
            settings.ipod_compatible = value;
        }
        if let Some(value) = self.align_audio_video {
            settings.align_audio_video = value;
        }
        if let Some(value) = self.passthrough_metadata {
            settings.passthrough_metadata = value;
        }
        Ok(())
    }
}

/// The summary tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTabViewModel {
    /// Container picker.
    pub container: ChoiceGroupViewModel,
    /// Video codec picker.
    pub video_codec: ChoiceGroupViewModel,
    /// Audio codec picker.
    pub audio_codec: ChoiceGroupViewModel,
    /// Front-loaded index switch.
    pub web_optimized: ToggleViewModel,
    /// Portable-player switch.
    pub ipod_compatible: ToggleViewModel,
    /// Audio and video alignment switch.
    pub align_audio_video: ToggleViewModel,
    /// Metadata passthrough switch.
    pub passthrough_metadata: ToggleViewModel,
    /// One line naming the container and the codecs.
    pub format_label: String,
    /// The rate control, read from the video tab.
    pub quality_label: String,
    /// One line describing the flags that are on.
    pub flag_label: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Build the summary tab.
pub fn tab_view(settings: &SummarySettings, video: &VideoSettings) -> SummaryTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let container = settings.container.as_str();
    let is_mp4 = container.eq_ignore_ascii_case("mp4");

    let container_view = ChoiceGroupViewModel::build(
        "summaryContainer",
        "Container",
        "The kind of file the output will be.",
        container,
        &container_options(),
        true,
        "",
    );

    let video_options = video_codecs_for(container);
    let video_codec = ChoiceGroupViewModel::build(
        "summaryVideoCodec",
        "Video",
        "Which video coding the output uses.",
        &settings.video_codec,
        &video_options,
        true,
        "",
    );
    if !video_options
        .iter()
        .any(|option| option.id == settings.video_codec && option.available)
    {
        problems.push(format!(
            "{} output cannot carry {} video; choose a different encoder or container.",
            container.to_ascii_uppercase(),
            settings.video_codec.to_ascii_uppercase()
        ));
    }

    let audio_options = audio_codecs_for(container);
    let audio_codec = ChoiceGroupViewModel::build(
        "summaryAudioCodec",
        "Audio",
        "Which audio coding the output uses.",
        &settings.audio_codec,
        &audio_options,
        true,
        "",
    );
    if !audio_options
        .iter()
        .any(|option| option.id == settings.audio_codec && option.available)
    {
        problems.push(format!(
            "{} output cannot carry {} audio; choose a different encoder or container.",
            container.to_ascii_uppercase(),
            settings.audio_codec.to_ascii_uppercase()
        ));
    }

    // The two flags that only mean something inside an MP4.
    let mp4_reason = "Only an MP4 has a front-loaded index; other containers need no such flag.";
    let mut web_optimized = ToggleViewModel::new(
        "summaryWebOptimized",
        "Web optimised",
        "Puts the index at the front so playback can start before the file has downloaded.",
        settings.web_optimized,
    );
    let mut ipod_compatible = ToggleViewModel::new(
        "summaryIpodCompatible",
        "Older portable players",
        "Limits the MP4 to a baseline profile old players can read.",
        settings.ipod_compatible,
    );
    if !is_mp4 {
        web_optimized = web_optimized.blocked(mp4_reason);
        if settings.web_optimized && !is_mp4 {
            problems.push(
                "The web-optimised index is an MP4 feature; it has no effect in this container."
                    .to_string(),
            );
        }
        ipod_compatible =
            ipod_compatible.blocked("The portable-player profile applies to MP4 output only.");
        if settings.ipod_compatible {
            problems.push(
                "Portable-player compatibility is an MP4 feature; switch the container to MP4 or turn the flag off."
                    .to_string(),
            );
        }
    }

    let align_audio_video = ToggleViewModel::new(
        "summaryAlignAudioVideo",
        "Align audio and video",
        "Nudges audio onto the video timeline when the two start apart.",
        settings.align_audio_video,
    );
    let passthrough_metadata = ToggleViewModel::new(
        "summaryPassthroughMetadata",
        "Keep the source tags",
        "Copies title, artist, and chapter tags to the output.",
        settings.passthrough_metadata,
    );

    let format_label = format!(
        "{} · {} video · {} audio",
        container.to_ascii_uppercase(),
        settings.video_codec.to_ascii_uppercase(),
        settings.audio_codec.to_ascii_uppercase()
    );

    let mut flags: Vec<&str> = Vec::new();
    if settings.web_optimized && is_mp4 {
        flags.push("web-optimised index");
    }
    if settings.ipod_compatible {
        flags.push("portable-player profile");
    }
    if settings.align_audio_video {
        flags.push("audio aligned to video");
    }
    if settings.passthrough_metadata {
        flags.push("source tags kept");
    }
    let flag_label = if flags.is_empty() {
        "No output flags are set.".to_string()
    } else {
        format!("On: {}.", flags.join(", "))
    };

    SummaryTabViewModel {
        container: container_view,
        video_codec,
        audio_codec,
        web_optimized,
        ipod_compatible,
        align_audio_video,
        passthrough_metadata,
        format_label,
        quality_label: video.quality_label(),
        flag_label,
        ready: problems.is_empty(),
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_an_mp4_with_the_index_at_the_front() {
        let settings = SummarySettings::default();
        let view = tab_view(&settings, &VideoSettings::default());
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.format_label, "MP4 · H264 video · AAC audio");
        assert!(view.web_optimized.checked);
        assert!(view.passthrough_metadata.checked);
        assert!(!view.ipod_compatible.checked);
        assert!(view.flag_label.contains("web-optimised index"));
        assert!(view.flag_label.contains("source tags kept"));
        assert!(view.quality_label.contains("constant quality 20"));
    }

    #[test]
    fn switching_to_webm_closes_the_mp4_only_flags() {
        let settings = SummarySettings {
            container: "webm".into(),
            video_codec: "vp9".into(),
            audio_codec: "opus".into(),
            web_optimized: false,
            ..Default::default()
        };
        let view = tab_view(&settings, &VideoSettings::default());
        assert!(view.ready, "{:?}", view.problems);
        assert!(!view.web_optimized.available);
        assert!(
            view.web_optimized.reason.contains("MP4"),
            "{}",
            view.web_optimized.reason
        );
        assert!(!view.ipod_compatible.available);
        assert_eq!(view.format_label, "WEBM · VP9 video · OPUS audio");
        assert!(view.flag_label.contains("source tags kept"));
        assert!(!view.flag_label.contains("web-optimised"));
    }

    #[test]
    fn a_codec_the_container_cannot_carry_is_a_problem_naming_both() {
        let settings = SummarySettings {
            container: "webm".into(),
            video_codec: "h264".into(),
            audio_codec: "opus".into(),
            ..Default::default()
        };
        let view = tab_view(&settings, &VideoSettings::default());
        assert!(!view.ready);
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("WEBM") && p.contains("H264")));
        assert!(
            !view
                .video_codec
                .options
                .iter()
                .find(|o| o.id == "h264")
                .expect("h264")
                .available
        );
    }

    #[test]
    fn a_mp4_only_flag_left_on_after_a_container_change_is_named() {
        let mut settings = SummarySettings {
            ipod_compatible: true,
            web_optimized: true,
            ..Default::default()
        };
        // The user turns the portable-player flag on in MP4, then changes the
        // container behind it.
        settings.container = "mkv".into();
        let view = tab_view(&settings, &VideoSettings::default());
        assert!(!view.ready);
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("Portable-player compatibility is an MP4 feature")));
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("web-optimised index is an MP4 feature")));
    }

    #[test]
    fn turning_the_flags_off_and_on_is_accepted() {
        let mut settings = SummarySettings::default();
        SummaryInput {
            web_optimized: Some(false),
            ipod_compatible: Some(true),
            align_audio_video: Some(true),
            passthrough_metadata: Some(false),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("flags");
        assert!(!settings.web_optimized);
        assert!(settings.ipod_compatible);
        assert!(settings.align_audio_video);
        assert!(!settings.passthrough_metadata);
        let view = tab_view(&settings, &VideoSettings::default());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.flag_label.contains("portable-player profile"));
        assert!(view.flag_label.contains("audio aligned to video"));
    }

    #[test]
    fn names_this_build_does_not_offer_are_refused() {
        let mut settings = SummarySettings::default();
        let container = SummaryInput {
            container: Some("avi".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown container");
        assert!(container.contains("avi"), "{container}");

        let video = SummaryInput {
            video_codec: Some("theora".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown video codec");
        assert!(video.contains("theora"), "{video}");

        let audio = SummaryInput {
            audio_codec: Some("dts".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown audio codec");
        assert!(audio.contains("dts"), "{audio}");

        // A codec the container cannot take is refused as well.
        SummaryInput {
            container: Some("webm".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("webm");
        assert_eq!(settings.container, "webm");
        let wrong = SummaryInput {
            video_codec: Some("h265".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("h265 into webm");
        assert!(wrong.contains("WEBM"), "{wrong}");
    }
}
