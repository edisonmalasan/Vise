//! The optical-disc support, checked against the tools themselves.
//!
//! A DVD is read through the `dvdvideo` demuxer and a Blu-ray through the
//! `bluray` protocol, and both come from optional parts of an ffmpeg build. The
//! tests here pin the decision and, where a tool is available, check that the
//! tool really is reached.

use std::path::{Path, PathBuf};

use vise_core::optical::OpticalCapabilities;
use vise_core::OpticalSourceKind;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vise-optical-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn ffmpeg() -> Option<PathBuf> {
    vise_core::FfprobeScanner::default()
        .ffmpeg_path()
        .map(Path::to_path_buf)
}

#[test]
fn a_dvd_folder_is_recognised() {
    let dir = scratch("dvd-folder");
    std::fs::create_dir_all(dir.join("VIDEO_TS")).expect("VIDEO_TS");
    assert_eq!(
        vise_core::optical_source_kind(&dir),
        Some(OpticalSourceKind::Dvd)
    );
}

#[test]
fn a_blu_ray_folder_is_recognised() {
    let dir = scratch("bluray-folder");
    std::fs::create_dir_all(dir.join("BDMV")).expect("BDMV");
    assert_eq!(
        vise_core::optical_source_kind(&dir),
        Some(OpticalSourceKind::BluRay)
    );
}

#[test]
fn an_image_is_recognised_as_an_optical_source() {
    let dir = scratch("image");
    let image = dir.join("movie.iso");
    std::fs::write(&image, b"not really an image").expect("write");
    assert_eq!(
        vise_core::optical_source_kind(&image),
        Some(OpticalSourceKind::Iso)
    );
}

#[test]
fn an_ordinary_file_is_not_an_optical_source() {
    let dir = scratch("ordinary");
    let file = dir.join("movie.mkv");
    std::fs::write(&file, b"a file").expect("write");
    assert_eq!(vise_core::optical_source_kind(&file), None);
}

#[test]
fn a_dvd_is_read_through_its_demuxer() {
    let dir = scratch("dvd-args");
    std::fs::create_dir_all(dir.join("VIDEO_TS")).expect("VIDEO_TS");
    let arguments = vise_core::optical::encoder_input_arguments(&dir);
    assert_eq!(
        arguments[0..3],
        ["-f".to_string(), "dvdvideo".to_string(), "-i".to_string()],
        "the demuxer is named before the input: {arguments:?}"
    );
    assert!(arguments[3].ends_with("vise-optical-dvd-args"));
}

#[test]
fn a_blu_ray_is_read_through_its_protocol() {
    let dir = scratch("bluray-args");
    std::fs::create_dir_all(dir.join("BDMV")).expect("BDMV");
    let arguments = vise_core::optical::encoder_input_arguments(&dir);
    assert_eq!(
        arguments[0], "-i",
        "the input is still an input: {arguments:?}"
    );
    assert!(
        arguments[1].starts_with("bluray:"),
        "the protocol is used: {arguments:?}"
    );
}

#[test]
fn an_ordinary_file_is_read_as_itself() {
    let dir = scratch("file-args");
    let file = dir.join("movie.mkv");
    std::fs::write(&file, b"a file").expect("write");
    let arguments = vise_core::optical::encoder_input_arguments(&file);
    assert_eq!(
        arguments,
        vec!["-i".to_string(), file.to_string_lossy().to_string()]
    );
}

#[test]
fn the_tool_is_asked_which_discs_it_can_read() {
    let Some(ffmpeg) = ffmpeg() else {
        // Without a tool there is nothing to ask, and saying so is the answer.
        assert!(!OpticalCapabilities::detect(None).dvd);
        return;
    };
    let capabilities = OpticalCapabilities::detect(Some(&ffmpeg));
    // The answer is whatever the tool says. A tool with the libraries can read
    // both, and one without them can read neither; either way the reading must
    // agree with what the tool printed.
    let formats = std::process::Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-formats")
        .output()
        .expect("the tool runs");
    let listed = String::from_utf8_lossy(&formats.stdout).to_ascii_lowercase();
    assert_eq!(
        capabilities.dvd,
        listed.contains("dvdvideo"),
        "the answer must match what the tool printed"
    );
}

#[test]
fn a_tool_with_the_libraries_reaches_the_dvd_reader() {
    let Some(ffmpeg) = ffmpeg() else {
        return;
    };
    let capabilities = OpticalCapabilities::detect(Some(&ffmpeg));
    if !capabilities.dvd {
        // This build cannot read a DVD, which is a fact and not a failure.
        assert!(capabilities.refusal(OpticalSourceKind::Dvd).is_some());
        return;
    }
    // A folder that is not a disc: the point is that the tool is reached
    // through the DVD reader rather than refusing the format outright.
    let dir = scratch("reached");
    std::fs::create_dir_all(dir.join("VIDEO_TS")).expect("VIDEO_TS");
    let output = std::process::Command::new(&ffmpeg)
        .arg("-hide_banner")
        .args(vise_core::optical::encoder_input_arguments(&dir))
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
        "the tool must know the demuxer it was given: {message}"
    );
}

#[test]
fn a_build_without_the_libraries_refuses_by_name() {
    let capabilities = OpticalCapabilities::default();
    let refusal = capabilities
        .refusal(OpticalSourceKind::Dvd)
        .expect("a refusal");
    assert!(refusal.contains("DVD"), "{refusal}");
    assert!(refusal.contains("without"), "{refusal}");
    let _ = Path::new("");
}
