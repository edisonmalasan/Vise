//! Output framing flags, encoder options, and chapter marker handling.
//!
//! These tests read the resolved plan rather than running an encode, because
//! the argument list is where a flag either reaches the encoder or is lost.

use std::path::PathBuf;
use std::process::Command;

use vise_core::{
    Chapter, ChapterMode, ChapterProcessingOptions, ColourRange, ContainerFormat, JobSpec,
    OutputOptions, TranscodePlan, Transcoder, VideoEncodingOptions,
};

fn tool(name: &str) -> PathBuf {
    vise_core::FfprobeScanner::default()
        .ffmpeg_path()
        .unwrap_or_else(|| panic!("{name}"))
        .to_path_buf()
}

fn generate_source(dir: &std::path::Path) -> PathBuf {
    let ffmpeg = tool("ffmpeg");
    let out = dir.join("source.mp4");
    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=320x240:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=2",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&out)
        .status()
        .expect("run ffmpeg source");
    assert!(status.success());
    out
}

fn flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|at| args.get(at + 1))
        .cloned()
}

/// The value of the last occurrence of a repeated flag, which is the one that
/// wins when an encoder reads the argument list from left to right.
fn last_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .rposition(|arg| arg == flag)
        .and_then(|at| args.get(at + 1))
        .cloned()
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn plan_for(dir: &std::path::Path, container: ContainerFormat) -> (TranscodePlan, PathBuf) {
    let source = generate_source(dir);
    let out = dir.join(format!(
        "out.{}",
        if container == ContainerFormat::Mkv {
            "mkv"
        } else {
            "mp4"
        }
    ));
    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container,
        ..Default::default()
    };
    (Transcoder::default().plan(&spec).expect("plan"), out)
}

#[test]
fn the_portable_player_flag_pins_a_baseline_mp4_profile() {
    let spec = JobSpec {
        output_options: OutputOptions {
            ipod_compatible: true,
            ..Default::default()
        },
        video_encoding: VideoEncodingOptions {
            profile: Some("high".into()),
            level: Some("4.2".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let options = spec.video_encoding_options();
    assert_eq!(options.profile.as_deref(), Some("baseline"));
    assert_eq!(
        options.level.as_deref(),
        Some("3.0"),
        "the compatibility requirement wins over the picked profile"
    );

    let plain = JobSpec {
        video_encoding: VideoEncodingOptions {
            profile: Some("high".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        plain.video_encoding_options().profile.as_deref(),
        Some("high")
    );
}

#[test]
fn output_flags_report_when_nothing_is_asked_for() {
    assert!(OutputOptions {
        web_optimized: Some(false),
        ..Default::default()
    }
    .is_empty());
    assert!(
        !OutputOptions::default().is_empty(),
        "a front-loaded index is what an MP4 gets by default"
    );
    assert!(!OutputOptions {
        align_audio_video: true,
        ..Default::default()
    }
    .is_empty());
    assert_eq!(OutputOptions::default().mp4_compatibility(), None);
    assert!(!OutputOptions {
        web_optimized: Some(false),
        ..Default::default()
    }
    .wants_web_optimized());
    assert!(OutputOptions {
        ipod_compatible: true,
        web_optimized: Some(false),
        ..Default::default()
    }
    .wants_web_optimized());
}

#[test]
fn the_front_loaded_index_can_be_turned_off() {
    let dir = std::env::temp_dir().join("vise-output-flags");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("no-faststart.mp4");
    let _ = std::fs::remove_file(&out);
    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        output_options: OutputOptions {
            web_optimized: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    assert!(
        !has(&plan.args, "-movflags"),
        "an explicitly turned-off flag is not passed"
    );
}

#[test]
fn only_mp4_gets_the_front_loaded_index() {
    let dir = std::env::temp_dir().join("vise-output-flags");
    let _ = std::fs::create_dir_all(&dir);

    let (plan, out) = plan_for(&dir, ContainerFormat::Mp4);
    let _ = std::fs::remove_file(&out);
    assert_eq!(
        flag(&plan.args, "-movflags").as_deref(),
        Some("+faststart"),
        "an MP4 keeps its playable-on-a-web index by default"
    );
    let _ = std::fs::remove_file(plan.chapter_metadata_path.as_deref().unwrap_or(&out));

    let (plan, out) = plan_for(&dir, ContainerFormat::Mkv);
    let _ = std::fs::remove_file(&out);
    assert!(
        !has(&plan.args, "-movflags"),
        "Matroska has no equivalent flag, so none is invented"
    );
}

#[test]
fn web_optimised_and_align_audio_video_reach_the_plan() {
    let dir = std::env::temp_dir().join("vise-output-flags");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("aligned.mp4");
    let _ = std::fs::remove_file(&out);

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        output_options: OutputOptions {
            web_optimized: Some(true),
            align_audio_video: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    assert_eq!(flag(&plan.args, "-movflags").as_deref(), Some("+faststart"));
    let audio_filter = flag(&plan.args, "-af").unwrap_or_default();
    assert!(
        audio_filter.contains("aresample=async=1:first_pts=0"),
        "{audio_filter}"
    );
}

#[test]
fn encoder_options_reach_the_plan() {
    let dir = std::env::temp_dir().join("vise-output-flags");
    let _ = std::fs::create_dir_all(&dir);
    let (mut plan, out) = plan_for(&dir, ContainerFormat::Mp4);
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(plan.chapter_metadata_path.clone().unwrap_or(out.clone()));

    let source = generate_source(&dir);
    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        video_encoding: VideoEncodingOptions {
            preset: Some("veryfast".into()),
            profile: Some("main".into()),
            tune: Some("film".into()),
            level: Some("3.1".into()),
            colour_range: Some(ColourRange::Limited),
        },
        ..Default::default()
    };
    plan = Transcoder::default().plan(&spec).expect("plan");
    assert_eq!(flag(&plan.args, "-preset").as_deref(), Some("veryfast"));
    assert_eq!(flag(&plan.args, "-profile:v").as_deref(), Some("main"));
    assert_eq!(flag(&plan.args, "-tune").as_deref(), Some("film"));
    assert_eq!(flag(&plan.args, "-level").as_deref(), Some("3.1"));
    assert_eq!(flag(&plan.args, "-color_range").as_deref(), Some("tv"));
}

#[test]
fn an_edited_marker_list_becomes_a_metadata_document() {
    let options = ChapterProcessingOptions {
        mode: Some(ChapterMode::Edit),
        edit: true,
        chapters: vec![
            Chapter {
                start_seconds: Some(0.0),
                end_seconds: Some(600.0),
                title: Some("Opening".into()),
                ..Default::default()
            },
            Chapter {
                start_seconds: Some(600.5),
                title: Some("Middle".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert!(options.uses_edited_chapters());
    let document = options.render_ffmetadata();
    assert!(document.starts_with(";FFMETADATA1\n"), "{document}");
    assert!(document.contains("TIMEBASE=1/1000"), "{document}");
    assert!(
        document.contains("START=0\nEND=600000\ntitle=Opening"),
        "{document}"
    );
    assert!(document.contains("START=600500"), "{document}");
    assert!(
        !options.build().contains("-map_chapters"),
        "the source markers are not copied on top of the edited ones: {}",
        options.build()
    );
}

#[test]
fn a_marker_without_a_start_time_is_left_out() {
    let options = ChapterProcessingOptions {
        chapters: vec![
            Chapter {
                start_seconds: None,
                title: Some("Nowhere".into()),
                ..Default::default()
            },
            Chapter {
                start_seconds: Some(12.0),
                end_seconds: Some(3.0),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let document = options.render_ffmetadata();
    assert!(!document.contains("Nowhere"), "{document}");
    assert!(
        !document.contains("END="),
        "an end before the start is dropped"
    );
    assert!(document.contains("START=12000"), "{document}");
}

#[test]
fn dropping_chapters_leaves_nothing_to_write() {
    let none = ChapterProcessingOptions {
        mode: Some(ChapterMode::None),
        chapters: vec![Chapter {
            start_seconds: Some(0.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(!none.uses_edited_chapters());
    assert!(!none.build().contains("-map_chapters"));
}

#[test]
fn edited_markers_become_a_second_input_that_is_cleaned_up() {
    let dir = std::env::temp_dir().join("vise-chapter-sidecar");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("chapters.mp4");
    let _ = std::fs::remove_file(&out);

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        // The encoder is pinned to the software one. This test is about the
        // chapter sidecar, and a machine with a graphics card would otherwise
        // run it through a codec the machine running the test may not have.
        hardware_acceleration: vise_core::HardwareAcceleration::disabled(),
        chapter_processing: ChapterProcessingOptions {
            mode: Some(ChapterMode::Edit),
            edit: true,
            chapters: vec![Chapter {
                start_seconds: Some(0.0),
                end_seconds: Some(1.0),
                title: Some("Only chapter".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
        ..Default::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    let sidecar = plan
        .chapter_metadata_path
        .clone()
        .expect("a sidecar is written for the edited markers");
    assert!(sidecar.is_file(), "the sidecar should exist after planning");
    assert!(std::fs::read_to_string(&sidecar)
        .expect("sidecar")
        .contains("title=Only chapter"));
    assert_eq!(
        last_flag(&plan.args, "-map_metadata").as_deref(),
        Some("1"),
        "the edited markers win over the source metadata"
    );
    assert!(
        !plan
            .args
            .windows(2)
            .any(|pair| pair == ["-map_chapters 0", ""]),
        "the source chapters are not mapped as well"
    );

    let result = Transcoder::default().execute(&plan);
    assert!(out.exists(), "output missing");
    assert!(result.is_ok(), "{result:?}");
    assert!(!sidecar.exists(), "the sidecar is removed after the encode");
}
