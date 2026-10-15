use std::path::PathBuf;
use std::process::Command;

use vise_core::{AudioCodec, ContainerFormat, FfprobeScanner, Transcoder, VideoCodec};

fn scanner() -> FfprobeScanner {
    FfprobeScanner::default()
}

fn tool(name: &str) -> PathBuf {
    match name {
        "ffmpeg" => scanner()
            .ffmpeg_path()
            .expect("ffmpeg must be available")
            .to_path_buf(),
        "ffprobe" => scanner()
            .ffprobe_path()
            .expect("ffprobe must be available")
            .to_path_buf(),
        _ => panic!("unknown tool"),
    }
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
    assert!(status.success(), "source generation failed");
    assert!(out.exists());
    out
}

#[test]
fn transcode_to_mp4() {
    let dir = std::env::temp_dir().join("vise-transcode-mp4");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("out.mp4");
    let _ = std::fs::remove_file(&out);

    let ts = Transcoder::default();
    let result = ts
        .transcode_path(&source, &out, Some(ContainerFormat::Mp4))
        .expect("mp4 transcode");
    assert!(out.exists());
    assert_eq!(result.video_codec, VideoCodec::H264);
    assert_eq!(result.audio_codec, AudioCodec::Aac);

    let scanner = scanner();
    let scan = scanner.scan_path(&out).expect("probe");
    let video = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Video))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    let audio = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Audio))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    assert_eq!(video, "h264");
    assert_eq!(audio, "aac");
}

#[test]
fn transcode_to_mkv() {
    let dir = std::env::temp_dir().join("vise-transcode-mkv");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("out.mkv");
    let _ = std::fs::remove_file(&out);

    let ts = Transcoder::default();
    let result = ts
        .transcode_path(&source, &out, Some(ContainerFormat::Mkv))
        .expect("mkv transcode");
    assert!(out.exists());
    assert_eq!(result.container, ContainerFormat::Mkv);

    let scanner = scanner();
    let scan = scanner.scan_path(&out).expect("probe");
    let video = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Video))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    let audio = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Audio))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    assert_eq!(video, "h264");
    assert_eq!(audio, "aac");
}

#[test]
fn transcode_to_webm() {
    let dir = std::env::temp_dir().join("vise-transcode-webm");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("out.webm");
    let _ = std::fs::remove_file(&out);

    let ts = Transcoder::default();
    let result = ts
        .transcode_path(&source, &out, Some(ContainerFormat::Webm))
        .expect("webm transcode");
    assert!(out.exists());
    assert_eq!(result.video_codec, VideoCodec::Vp9);
    assert_eq!(result.audio_codec, AudioCodec::Opus);

    let scanner = scanner();
    let scan = scanner.scan_path(&out).expect("probe");
    let video = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Video))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    let audio = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Audio))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    assert_eq!(video, "vp9");
    assert_eq!(audio, "opus");
}

#[test]
fn end_to_end_job_spec() {
    let dir = std::env::temp_dir().join("vise-transcode-e2e");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("e2e.mp4");
    let _ = std::fs::remove_file(&out);

    let spec = vise_core::JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        ..Default::default()
    };
    let ts = Transcoder::default();
    let result = ts.transcode(&spec).expect("e2e");
    assert!(out.exists());
    let duration = result.duration_seconds.unwrap_or(0.0);
    assert!((duration - 2.0).abs() < 0.6, "duration {duration}");
}
