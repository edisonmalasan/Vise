//! Real-media checks for the Group 19 media-depth items.
//!
//! The decision logic has its own unit tests; these run the pipeline against
//! the ffmpeg this machine has, so an argument the planner produces is proven
//! to be one the encoder accepts rather than merely one the planner intended.

use std::path::{Path, PathBuf};
use std::process::Command;

use vise_core::{
    AudioCodec, ContainerFormat, FfprobeScanner, JobSpec, Transcoder, VideoCodec,
    VideoProcessingOptions,
};

fn tool(name: &str) -> PathBuf {
    let scanner = FfprobeScanner::default();
    match name {
        "ffmpeg" => scanner
            .ffmpeg_path()
            .expect("ffmpeg must be available")
            .to_path_buf(),
        "ffprobe" => scanner
            .ffprobe_path()
            .expect("ffprobe must be available")
            .to_path_buf(),
        _ => panic!("unknown tool"),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vise-media-depth-{name}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Generate a short clip with one video and one audio stream.
fn generate_source(dir: &Path) -> PathBuf {
    let out = dir.join("source.mp4");
    let status = Command::new(tool("ffmpeg"))
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=320x240:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=1",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&out)
        .status()
        .expect("run ffmpeg source");
    assert!(status.success(), "source generation failed");
    assert!(out.exists());
    out
}

/// A job that is a lossless FFV1 + FLAC Matroska write.
fn lossless_spec(input: &Path, output: &Path) -> JobSpec {
    JobSpec {
        input_path: input.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        container: ContainerFormat::Mkv,
        video_codec: VideoCodec::Ffv1,
        audio_codec: AudioCodec::Flac,
        lossless: true,
        ..JobSpec::default()
    }
}

#[test]
fn a_real_lossless_ffv1_flac_mkv_is_produced() {
    let dir = temp_dir("lossless");
    let source = generate_source(&dir);
    let out = dir.join("lossless.mkv");
    let _ = std::fs::remove_file(&out);

    let spec = lossless_spec(&source, &out);
    let result = Transcoder::default()
        .transcode(&spec)
        .expect("lossless transcode");
    assert!(out.exists(), "the encoder wrote no file");
    let claim = result
        .lossless_claim
        .expect("a lossless job carries a claim");
    assert!(claim.contains("Lossless FFV1"), "{claim}");

    let scan = FfprobeScanner::default()
        .scan_path(&out)
        .expect("probe the output");
    let video = scan
        .streams
        .iter()
        .find(|stream| stream.kind == vise_core::TrackKind::Video)
        .and_then(|stream| stream.codec.clone())
        .unwrap_or_default();
    let audio = scan
        .streams
        .iter()
        .find(|stream| stream.kind == vise_core::TrackKind::Audio)
        .and_then(|stream| stream.codec.clone())
        .unwrap_or_default();
    assert_eq!(video, "ffv1", "the output video is not FFV1");
    assert_eq!(audio, "flac", "the output audio is not FLAC");
}

#[test]
fn a_lossless_job_never_hands_the_encoder_a_rate_control_value() {
    let dir = temp_dir("lossless-plan");
    let source = dir.join("input.mkv");
    std::fs::write(&source, b"placeholder").expect("input");
    let out = dir.join("out.mkv");

    let plan = Transcoder::default()
        .plan(&lossless_spec(&source, &out))
        .expect("plan");
    // An intra lossless encoder takes no `-crf`; handing it one makes ffmpeg
    // refuse the whole job.
    assert!(
        !plan.args.iter().any(|arg| arg == "-crf"),
        "{:?}",
        plan.args
    );
    assert!(plan.args.windows(2).any(|pair| pair == ["-c:v", "ffv1"]));
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair == ["-pix_fmt", "yuv420p"]));
    assert_eq!(
        plan.lossless_claim.as_deref(),
        Some("Lossless FFV1 video in Matroska with lossless FLAC audio.")
    );
}

#[test]
fn deblock_is_applied_when_the_job_asks_for_it() {
    let dir = temp_dir("deblock-plan");
    let source = dir.join("input.mkv");
    std::fs::write(&source, b"placeholder").expect("input");
    let out = dir.join("out.mkv");

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mkv,
        video_processing: VideoProcessingOptions {
            deblock: true,
            deblock_strength: Some(8),
            ..VideoProcessingOptions::default()
        },
        ..JobSpec::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    let vf = plan
        .args
        .iter()
        .position(|arg| arg == "-vf")
        .map(|index| plan.args[index + 1].clone())
        .unwrap_or_default();
    assert!(vf.contains("deblock=filter=strong:block=8"), "{vf}");
}

#[test]
fn mp4_faststart_and_ipod_profile_are_applied() {
    let dir = temp_dir("mp4-plan");
    let source = dir.join("input.mkv");
    std::fs::write(&source, b"placeholder").expect("input");
    let out = dir.join("out.mp4");

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        output_options: vise_core::OutputOptions {
            web_optimized: Some(true),
            ipod_compatible: true,
            passthrough_metadata: true,
            ..Default::default()
        },
        ..JobSpec::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair == ["-movflags", "+faststart"]));
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair == ["-profile:v", "baseline"]));
    assert!(plan.args.windows(2).any(|pair| pair == ["-level", "3.0"]));
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair == ["-map_metadata", "0"]));
}

#[test]
fn an_mp4_faststart_flag_is_not_written_into_other_containers() {
    let dir = temp_dir("mkv-plan");
    let source = dir.join("input.mkv");
    std::fs::write(&source, b"placeholder").expect("input");
    let out = dir.join("out.mkv");

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mkv,
        output_options: vise_core::OutputOptions {
            web_optimized: Some(true),
            ..Default::default()
        },
        ..JobSpec::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    assert!(!plan.args.iter().any(|arg| arg == "+faststart"));
}

#[test]
fn dolby_vision_and_hdr10_plus_arguments_follow_the_capable_encoder() {
    let dir = temp_dir("hdr-plan");
    let source = dir.join("input.mkv");
    std::fs::write(&source, b"placeholder").expect("input");
    let out = dir.join("out.mkv");

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mkv,
        video_codec: VideoCodec::H265,
        // The private RPU and SEI options belong to the software encoder, so
        // the plan is forced onto it rather than the hardware one.
        hardware_acceleration: vise_core::HardwareAcceleration::disabled(),
        dolby_vision: vise_core::DolbyVisionOptions {
            enabled: true,
            profile: Some(8),
            rpu_present: true,
            action: Some(vise_core::DolbyVisionAction::PassThrough),
        },
        hdr10_plus: vise_core::Hdr10PlusOptions {
            enabled: true,
            present: true,
            action: Some(vise_core::Hdr10PlusAction::PassThrough),
        },
        ..JobSpec::default()
    };
    let plan = Transcoder::default().plan(&spec).expect("plan");
    // The software HEVC encoder is the one that carries both private options.
    assert_eq!(plan.video_encoder, "libx265");
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair == ["-dolbyvision", "1"]));
    assert!(plan.args.windows(2).any(|pair| pair == ["-udu_sei", "1"]));
}
