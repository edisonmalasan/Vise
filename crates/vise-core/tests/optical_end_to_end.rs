//! The optical-disc path, from the source on disk to what the user is told.
//!
//! These are the checks that a disc is read as a disc rather than as a file
//! that happens to be a folder: the source is recognised, the demuxer is
//! named, and the window says what it is reading.

use std::path::PathBuf;

use vise_core::optical::OpticalCapabilities;
use vise_core::OpticalSourceKind;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vise-optical-e2e-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn ffmpeg() -> Option<PathBuf> {
    vise_core::FfprobeScanner::default()
        .ffmpeg_path()
        .map(PathBuf::from)
}

#[test]
fn a_dvd_folder_is_read_as_a_dvd_all_the_way_to_the_window() {
    let dir = scratch("dvd");
    std::fs::create_dir_all(dir.join("VIDEO_TS")).expect("VIDEO_TS");

    // The source is recognised.
    let kind = vise_core::optical_source_kind(&dir).expect("a disc");
    assert_eq!(kind, OpticalSourceKind::Dvd);
    assert_eq!(kind.as_str(), "dvd");

    // The demuxer is named before the input, and the input is still an input.
    let arguments = vise_core::optical::encoder_input_arguments(&dir);
    assert_eq!(arguments[0], "-f");
    assert_eq!(arguments[1], "dvdvideo");
    assert_eq!(arguments[2], "-i");
    assert!(arguments[3].ends_with("vise-optical-e2e-dvd"));

    // And the tool on this machine is asked whether it can read one.
    let Some(ffmpeg) = ffmpeg() else {
        return;
    };
    let capabilities = OpticalCapabilities::detect(Some(&ffmpeg));
    if capabilities.can_read(OpticalSourceKind::Dvd) {
        // The reader is reached: the tool's complaint is about the empty disc,
        // not about a format it has never heard of.
        let output = std::process::Command::new(&ffmpeg)
            .arg("-hide_banner")
            .args(&arguments)
            .arg("-t")
            .arg("1")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output()
            .expect("the tool runs");
        let message = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        assert!(
            !message.contains("not known"),
            "the tool must know the demuxer: {message}"
        );
    } else {
        // The build cannot read one, and it says so by name.
        let refusal = capabilities
            .refusal(OpticalSourceKind::Dvd)
            .expect("a refusal");
        assert!(refusal.contains("DVD"), "{refusal}");
    }
}

#[test]
fn a_blu_ray_folder_is_read_through_the_bluray_protocol() {
    let dir = scratch("bluray");
    std::fs::create_dir_all(dir.join("BDMV")).expect("BDMV");

    let kind = vise_core::optical_source_kind(&dir).expect("a disc");
    assert_eq!(kind, OpticalSourceKind::BluRay);

    let arguments = vise_core::optical::encoder_input_arguments(&dir);
    assert_eq!(arguments[0], "-i", "{arguments:?}");
    assert!(
        arguments[1].starts_with("bluray:"),
        "the protocol is used: {arguments:?}"
    );
}

#[test]
fn an_ordinary_folder_is_not_a_disc() {
    let dir = scratch("ordinary");
    assert_eq!(vise_core::optical_source_kind(&dir), None);
    let arguments = vise_core::optical::encoder_input_arguments(&dir);
    assert_eq!(
        arguments,
        vec!["-i".to_string(), dir.to_string_lossy().to_string()]
    );
}
