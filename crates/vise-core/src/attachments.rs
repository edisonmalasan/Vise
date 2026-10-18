//! Cover art and attachment handling, with the container rules that apply.
//!
//! Cover art and attachments are not tags: they are extra streams in the file,
//! and which containers can carry them differs. A user who asks for a cover on
//! a WebM is asking for something the muxer has no representation for, so this
//! module resolves the request into what the encoder will be told and, when the
//! container cannot carry it, a reason rather than a silently dropped picture.

use serde::{Deserialize, Serialize};

use crate::ContainerFormat;

/// What the encoder will be told about cover art and attachments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentDecision {
    /// Whether the request asked for cover art.
    pub cover_art: bool,
    /// Whether the request asked for other attachments.
    pub attachments: bool,
    /// Whether the container can carry cover art.
    pub cover_art_available: bool,
    /// Whether the container can carry attachments.
    pub attachments_available: bool,
    /// Why cover art cannot be written, when it cannot.
    pub cover_art_reason: String,
    /// Why attachments cannot be written, when they cannot.
    pub attachments_reason: String,
    /// The mapping arguments the decision produces.
    pub args: Vec<String>,
    /// One line saying what the output carries.
    pub claim: String,
}

impl AttachmentDecision {
    /// Whether the decision can be carried out as asked.
    pub fn supported(&self) -> bool {
        (!self.cover_art || self.cover_art_available)
            && (!self.attachments || self.attachments_available)
    }

    /// Why the decision cannot be met, when it cannot.
    pub fn reason(&self) -> Option<String> {
        if self.supported() {
            return None;
        }
        let mut reasons = Vec::new();
        if self.cover_art && !self.cover_art_available {
            reasons.push(self.cover_art_reason.clone());
        }
        if self.attachments && !self.attachments_available {
            reasons.push(self.attachments_reason.clone());
        }
        Some(reasons.join(" "))
    }
}

/// Resolve a cover-art and attachment request for a container.
pub fn resolve(
    container: ContainerFormat,
    cover_art: bool,
    attachments: bool,
) -> AttachmentDecision {
    // Cover art is a picture stream: Matroska carries it as an attachment and
    // MP4 as a cover atom. WebM's picture track is not exposed by this build's
    // muxer, so it is closed with the reason rather than dropped.
    let (cover_art_available, cover_art_reason) = match container {
        ContainerFormat::Mkv | ContainerFormat::Mp4 => (true, String::new()),
        ContainerFormat::Webm => (
            false,
            "WebM output carries no cover-art track in this build; use Matroska or MP4."
                .to_string(),
        ),
    };

    // Attachments are arbitrary data streams. Matroska is the only container
    // this build offers that carries them as a first-class track.
    let (attachments_available, attachments_reason) = match container {
        ContainerFormat::Mkv => (true, String::new()),
        _ => (
            false,
            "Matroska is the only container this build offers that carries attachments."
                .to_string(),
        ),
    };

    let mut args: Vec<String> = Vec::new();
    if cover_art && cover_art_available {
        args.push("-map".into());
        args.push("0:v?".into());
    }
    if attachments && attachments_available {
        args.push("-map".into());
        args.push("0:t?".into());
    }

    let mut claim = String::new();
    if cover_art && cover_art_available {
        claim.push_str("The cover art is written");
    }
    if attachments && attachments_available {
        if !claim.is_empty() {
            claim.push_str(", and ");
        }
        claim.push_str("the attachments are written");
    }
    if claim.is_empty() {
        claim = "No cover art or attachments are written.".to_string();
    } else {
        claim.push('.');
    }

    AttachmentDecision {
        cover_art,
        attachments,
        cover_art_available,
        attachments_available,
        cover_art_reason,
        attachments_reason,
        args,
        claim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matroska_carries_both_cover_art_and_attachments() {
        let decision = resolve(ContainerFormat::Mkv, true, true);
        assert!(decision.supported());
        assert!(decision.cover_art_available);
        assert!(decision.attachments_available);
        assert_eq!(decision.args, vec!["-map", "0:v?", "-map", "0:t?"]);
        assert!(decision.claim.contains("cover art"));
        assert!(decision.claim.contains("attachments"));
        assert!(decision.reason().is_none());
    }

    #[test]
    fn mp4_carries_cover_art_but_not_attachments() {
        let decision = resolve(ContainerFormat::Mp4, true, true);
        assert!(!decision.supported());
        assert!(decision.cover_art_available);
        assert!(!decision.attachments_available);
        assert_eq!(decision.args, vec!["-map", "0:v?"]);
        let reason = decision.reason().expect("a reason");
        assert!(reason.contains("Matroska"), "{reason}");
    }

    #[test]
    fn webm_carries_neither_and_says_so() {
        let decision = resolve(ContainerFormat::Webm, true, true);
        assert!(!decision.supported());
        assert!(!decision.cover_art_available);
        assert!(!decision.attachments_available);
        assert!(decision.args.is_empty());
        let reason = decision.reason().expect("a reason");
        assert!(reason.contains("WebM"), "{reason}");
        assert!(reason.contains("Matroska"), "{reason}");
    }

    #[test]
    fn a_request_that_asks_for_nothing_is_supported_everywhere() {
        for container in [
            ContainerFormat::Mkv,
            ContainerFormat::Mp4,
            ContainerFormat::Webm,
        ] {
            let decision = resolve(container, false, false);
            assert!(decision.supported(), "{container:?}");
            assert!(decision.args.is_empty());
            assert!(decision.claim.contains("No cover art"));
        }
    }
}
