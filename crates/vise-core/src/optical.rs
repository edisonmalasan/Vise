//! What this build's media tools can read from an optical disc.
//!
//! Reading a DVD needs libdvdread and reading a Blu-ray needs libbluray, and
//! both are optional parts of an ffmpeg build: the "essentials" build has
//! neither. A build that has the library names a demuxer the tool knows; a
//! build that does not must say so plainly, because the alternative is an
//! error about an unknown format, which reads like a damaged disc rather than
//! a missing capability.
//!
//! The answer is asked of the tool itself rather than assumed from its name:
//! the demuxer is looked for in the list the tool prints.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::media::OpticalSourceKind;

/// The arguments that say where the input is, for an optical source.
///
/// A DVD is read through the `dvdvideo` demuxer and a Blu-ray through the
/// `bluray` protocol; the same choice is made here as in the scanner, so the
/// two cannot drift apart.
pub fn input_arguments(path: &Path) -> Vec<String> {
    crate::media::input_arguments(path)
}

/// The arguments that say where the input is, for a tool that takes the input
/// after `-i`.
pub fn encoder_input_arguments(path: &Path) -> Vec<String> {
    crate::media::encoder_input_arguments(path)
}

/// What the media tool on this machine can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OpticalCapabilities {
    /// Whether the tool can read a DVD.
    pub dvd: bool,
    /// Whether the tool can read a Blu-ray.
    pub bluray: bool,
}

impl OpticalCapabilities {
    /// Ask the tool what it can read.
    ///
    /// A DVD is a demuxer and a Blu-ray is a protocol, so the two are asked
    /// about in the lists where each of them appears.
    pub fn detect(ffmpeg: Option<&Path>) -> Self {
        let Some(ffmpeg) = ffmpeg else {
            return Self::default();
        };
        let formats = run(ffmpeg, &["-formats"]);
        let protocols = run(ffmpeg, &["-protocols"]);
        Self {
            dvd: formats.to_ascii_lowercase().contains("dvdvideo"),
            bluray: protocols.to_ascii_lowercase().contains("bluray"),
        }
    }

    /// Read the answer out of the list the tool printed.
    ///
    /// Public and pure so a test can pin the reading against a captured list
    /// without running the tool.
    pub fn from_formats(text: &str) -> Self {
        let lower = text.to_ascii_lowercase();
        Self {
            dvd: lower.contains("dvdvideo"),
            bluray: lower.contains("bluray"),
        }
    }

    /// Whether this build can read the given kind of source.
    pub fn can_read(&self, kind: OpticalSourceKind) -> bool {
        match kind {
            OpticalSourceKind::Dvd | OpticalSourceKind::Iso => self.dvd,
            OpticalSourceKind::BluRay => self.bluray,
        }
    }

    /// Why the source cannot be read, when it cannot.
    ///
    /// The message names what is missing rather than what went wrong, because
    /// the user's next step is a different one in each case: install a build
    /// with the library, or check the disc.
    pub fn refusal(&self, kind: OpticalSourceKind) -> Option<String> {
        if self.can_read(kind) {
            return None;
        }
        Some(match kind {
            OpticalSourceKind::Dvd => {
                "this build of ffmpeg cannot read a DVD: it was made without the DVD support"
                    .to_string()
            }
            OpticalSourceKind::BluRay => {
                "this build of ffmpeg cannot read a Blu-ray: it was made without the Blu-ray support"
                    .to_string()
            }
            OpticalSourceKind::Iso => {
                "this build of ffmpeg cannot read an optical image: it was made without the DVD support"
                    .to_string()
            }
        })
    }
}

/// The output of one of the tool's listing commands.
fn run(ffmpeg: &Path, args: &[&str]) -> String {
    let Ok(output) = Command::new(ffmpeg).arg("-hide_banner").args(args).output() else {
        return String::new();
    };
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_with_the_libraries_reports_both() {
        // A DVD is a demuxer and a Blu-ray is a protocol, so the two are listed
        // in different places; this is what the lists look like.
        let formats = " D   dvdvideo        DVD-Video\n DE matroska       Matroska\n";
        let protocols = "  bluray\n  file\n";
        let caps = OpticalCapabilities {
            dvd: OpticalCapabilities::from_formats(formats).dvd,
            bluray: protocols.to_ascii_lowercase().contains("bluray"),
        };
        assert!(caps.dvd);
        assert!(caps.bluray);
        assert!(caps.can_read(OpticalSourceKind::Dvd));
        assert!(caps.can_read(OpticalSourceKind::BluRay));
        assert!(caps.refusal(OpticalSourceKind::Dvd).is_none());
    }

    #[test]
    fn a_build_without_the_libraries_reports_neither() {
        let listing = " DE matroska       Matroska\n DE mp4           MP4\n";
        let caps = OpticalCapabilities::from_formats(listing);
        assert!(!caps.dvd);
        assert!(!caps.bluray);
        assert!(!caps.can_read(OpticalSourceKind::Dvd));
        assert!(!caps.can_read(OpticalSourceKind::BluRay));
    }

    #[test]
    fn a_build_with_only_one_library_reports_only_that_one() {
        let caps = OpticalCapabilities::from_formats(" D   dvdvideo        DVD-Video\n");
        assert!(caps.dvd);
        assert!(!caps.bluray);
    }

    #[test]
    fn the_refusal_names_the_missing_capability() {
        let caps = OpticalCapabilities::default();
        let refusal = caps.refusal(OpticalSourceKind::Dvd).expect("a refusal");
        assert!(refusal.contains("DVD"), "{refusal}");
        assert!(refusal.contains("without"), "{refusal}");

        let refusal = caps.refusal(OpticalSourceKind::BluRay).expect("a refusal");
        assert!(refusal.contains("Blu-ray"), "{refusal}");
    }

    #[test]
    fn an_optical_image_is_a_dvd_as_far_as_the_library_goes() {
        // An image could hold either, and the library that reads it is the one
        // that reads DVDs, so that is what is asked for.
        let caps = OpticalCapabilities::from_formats(" D   dvdvideo        DVD-Video\n");
        assert!(caps.can_read(OpticalSourceKind::Iso));
        assert!(caps.refusal(OpticalSourceKind::Iso).is_none());
    }
}
