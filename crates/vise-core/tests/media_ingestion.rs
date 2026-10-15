use std::path::{Path, PathBuf};
use std::process::Command;
use vise_core::{FfprobeScanner, MediaScanner};

fn tool(name: &str) -> Option<PathBuf> {
    let scanner = vise_core::FfprobeScanner::default();
    match name {
        "ffmpeg" => scanner.ffmpeg_path().map(PathBuf::from),
        "ffprobe" => scanner.ffprobe_path().map(PathBuf::from),
        _ => None,
    }
}

fn generate_chaptered_media(dir: &Path) -> PathBuf {
    let ffmpeg = tool("ffmpeg").expect("ffmpeg must be available");
    let base = dir.join("base.mp4");
    let chapters = dir.join("chapters.txt");
    let out = dir.join("chaptered.mp4");

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
        .arg(&base)
        .status()
        .expect("run ffmpeg base");
    assert!(status.success(), "base generation failed");

    std::fs::write(
        &chapters,
        ";FFMETADATA1\n\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=1000\ntitle=Chapter 1\n\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=1000\nEND=2000\ntitle=Chapter 2\n",
    )
    .expect("write chapters");

    let status = Command::new(&ffmpeg)
        .args(["-y", "-i"])
        .arg(&base)
        .args(["-i"])
        .arg(&chapters)
        .args(["-map_metadata", "1", "-c", "copy"])
        .arg(&out)
        .status()
        .expect("run ffmpeg chapters");
    assert!(status.success(), "chapter remux failed");
    out
}

#[test]
fn parser_fixture_extracts_metadata_streams_and_chapters() {
    let fixture = r#"{
      "format": {
        "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
        "format_long_name": "QuickTime / MOV",
        "duration": "2.000000",
        "size": "12345",
        "bit_rate": "49380",
        "tags": { "title": "Fixture" }
      },
      "streams": [
        {
          "index": 0,
          "codec_type": "video",
          "codec_name": "h264",
          "codec_long_name": "H.264 / AVC",
          "width": 320,
          "height": 240,
          "avg_frame_rate": "10/1",
          "bit_rate": "40000"
        },
        {
          "index": 1,
          "codec_type": "audio",
          "codec_name": "aac",
          "channels": 2,
          "sample_rate": "48000",
          "channel_layout": "stereo",
          "bit_rate": "9380"
        },
        {
          "index": 2,
          "codec_type": "subtitle",
          "codec_name": "mov_text",
          "tags": { "language": "eng", "title": "English" }
        }
      ],
      "chapters": [
        { "start_time": "0.000000", "end_time": "1.000000", "tags": { "title": "Chapter 1" } },
        { "start_time": "1.000000", "end_time": "2.000000", "tags": { "title": "Chapter 2" } }
      ]
    }"#;

    let result = vise_core::parse_ffprobe_output(fixture.as_bytes(), Path::new("fixture.mp4"))
        .expect("parse fixture");

    assert_eq!(result.path, PathBuf::from("fixture.mp4"));
    assert_eq!(
        result.format.format_name.as_deref(),
        Some("mov,mp4,m4a,3gp,3g2,mj2")
    );
    assert_eq!(result.format.size_bytes, Some(12345));
    assert!((result.format.duration_seconds.unwrap() - 2.0).abs() < 1e-9);

    let video: Vec<_> = result
        .streams
        .iter()
        .filter(|s| s.kind == vise_core::TrackKind::Video)
        .collect();
    let audio: Vec<_> = result
        .streams
        .iter()
        .filter(|s| s.kind == vise_core::TrackKind::Audio)
        .collect();
    let subs: Vec<_> = result
        .streams
        .iter()
        .filter(|s| s.kind == vise_core::TrackKind::Subtitle)
        .collect();
    assert_eq!(video.len(), 1);
    assert_eq!(audio.len(), 1);
    assert_eq!(subs.len(), 1);
    assert_eq!(video[0].width, Some(320));
    assert_eq!(audio[0].channels, Some(2));
    assert_eq!(subs[0].language.as_deref(), Some("eng"));

    assert_eq!(result.chapters.len(), 2);
    assert_eq!(result.chapters[0].title.as_deref(), Some("Chapter 1"));
    assert!(result.titles.len() == 1);
    assert_eq!(result.titles[0].chapters.len(), 2);
}

#[test]
fn scan_generated_media_extracts_video_audio_and_chapters() {
    let dir = std::env::temp_dir().join("vise-media-scan");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let media = generate_chaptered_media(&dir);

    let scanner = FfprobeScanner::new();
    let result = scanner.scan(&media).expect("scan generated media");

    assert!(result
        .streams
        .iter()
        .any(|s| s.kind == vise_core::TrackKind::Video));
    assert!(result
        .streams
        .iter()
        .any(|s| s.kind == vise_core::TrackKind::Audio));
    assert!(
        result.chapters.len() >= 2,
        "expected chapters, got {}",
        result.chapters.len()
    );
    assert!(result.chapters[0].title.as_deref() == Some("Chapter 1"));
    assert!(result.format.duration_seconds.unwrap_or(0.0) > 1.0);
}

#[test]
fn preview_extraction_writes_image_file() {
    let dir = std::env::temp_dir().join("vise-media-preview");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let media = generate_basic_media(&dir);
    let out = dir.join("preview.jpg");

    let scanner = FfprobeScanner::new();
    scanner
        .extract_preview(&media, &out, 0.5)
        .expect("extract preview");
    assert!(out.exists(), "preview file should exist");
    assert!(
        std::fs::metadata(&out).unwrap().len() > 0,
        "preview should be non-empty"
    );
}

fn generate_basic_media(dir: &Path) -> PathBuf {
    let ffmpeg = tool("ffmpeg").expect("ffmpeg must be available");
    let out = dir.join("basic.mp4");
    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=160x120:rate=5",
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
        .expect("run ffmpeg basic");
    assert!(status.success(), "basic generation failed");
    out
}

#[test]
fn crop_detection_runs_on_generated_media() {
    let dir = std::env::temp_dir().join("vise-media-crop");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let media = generate_basic_media(&dir);

    let scanner = FfprobeScanner::new();
    let crop = scanner.detect_crop(&media).expect("crop detect");
    assert!(crop.width > 0 && crop.height > 0);
}
