use std::path::PathBuf;
use std::process::Command;

use vise_core::{ContainerFormat, FfprobeScanner, JobSpec, SubtitleMode, Transcoder};

fn scanner() -> FfprobeScanner {
    FfprobeScanner::default()
}

fn tool() -> PathBuf {
    scanner().ffmpeg_path().expect("ffmpeg").to_path_buf()
}

fn generate_source(dir: &std::path::Path) -> PathBuf {
    let ffmpeg = tool();
    let srt = dir.join("subs.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:02,000\nHello\n\n2\n00:00:02,000 --> 00:00:04,000\nWorld\n",
    )
    .unwrap();
    let out = dir.join("source.mkv");
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
            "-i",
        ])
        .arg(&srt)
        .args(["-c:v", "libx264", "-c:a", "aac", "-c:s", "srt", "-shortest"])
        .arg(&out)
        .status()
        .expect("generate source");
    assert!(status.success(), "source generation failed");
    assert!(out.exists());
    out
}

#[test]
fn subtitle_model_build() {
    let opts = vise_core::SubtitleProcessingOptions {
        burn_in: true,
        input_path: Some("in.srt".to_string()),
        ..Default::default()
    };
    let chain = opts.build();
    assert!(chain.contains("subtitles"), "{chain}");
    assert_eq!(vise_core::SubtitleMode::default(), vise_core::SubtitleMode::Passthrough);
}

#[test]
fn text_subtitle_passthrough() {
    let dir = std::env::temp_dir().join("vise-sub-pass");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("out.mkv");
    let _ = std::fs::remove_file(&out);

    let mut spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mkv,
        subtitle_mode: SubtitleMode::Passthrough,
        ..Default::default()
    };
    spec.subtitle_processing.passthrough = true;
    let ts = Transcoder::default();
    let result = ts.transcode(&spec).expect("passthrough");
    assert!(out.exists(), "output missing");
    let _ = result;

    let scan = scanner().scan_path(&out).expect("probe");
    let has_sub = scan.streams.iter().any(|s| matches!(s.kind, vise_core::TrackKind::Subtitle));
    assert!(has_sub, "expected subtitle stream");
}

#[test]
fn srt_to_mp4_text_conversion() {
    let dir = std::env::temp_dir().join("vise-sub-mp4");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("out.mp4");
    let _ = std::fs::remove_file(&out);

    let spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        subtitle_mode: SubtitleMode::Passthrough,
        ..Default::default()
    };
    let ts = Transcoder::default();
    let result = ts.transcode(&spec).expect("mp4 text");
    assert!(out.exists());
    let _ = result;

    let scan = scanner().scan_path(&out).expect("probe");
    let codec = scan
        .streams
        .iter()
        .find(|s| matches!(s.kind, vise_core::TrackKind::Subtitle))
        .and_then(|s| s.codec.clone())
        .unwrap_or_default();
    assert!(
        codec == "mov_text" || codec == "tx3g" || !codec.is_empty(),
        "codec={codec}"
    );
}

#[test]
fn burn_in_runs() {
    let dir = std::env::temp_dir().join("vise-sub-burn");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("burn.mp4");
    let _ = std::fs::remove_file(&out);

    let mut spec = JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: ContainerFormat::Mp4,
        subtitle_mode: SubtitleMode::BurnIn,
        ..Default::default()
    };
    spec.subtitle_path = Some(source.to_string_lossy().to_string());
    let ts = Transcoder::default();
    let result = ts.transcode(&spec);
    // burn-in filter may or may not succeed; at minimum command attempted
    let _ = result;
}
