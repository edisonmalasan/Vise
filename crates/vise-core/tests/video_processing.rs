use std::path::PathBuf;
use std::process::Command;

use vise_core::{FrameRateMode, Transcoder, VideoProcessingOptions};

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

#[test]
fn filter_chain_contains_features() {
    let opts = VideoProcessingOptions {
        crop: Some((10, 20, 100, 80)),
        scale: Some((1280, 720)),
        deinterlace: true,
        detelecine: true,
        denoise: true,
        sharpen: true,
        rotate: Some(90),
        pad: Some((1920, 1080)),
        grayscale: true,
        chroma_smooth: true,
        ten_bit: true,
        hdr: true,
        frame_rate: Some(FrameRateMode::Constant(30.0)),
        ..Default::default()
    };
    let chain = opts.build_filter_chain();
    assert!(chain.contains("crop=100:80:10:20"), "{chain}");
    assert!(chain.contains("yadif"), "{chain}");
    assert!(chain.contains("detelecine"), "{chain}");
    assert!(chain.contains("hqdn3d"), "{chain}");
    assert!(chain.contains("unsharp"), "{chain}");
    assert!(chain.contains("transpose=1"), "{chain}");
    assert!(chain.contains("scale=1280:720"), "{chain}");
    assert!(chain.contains("pad=1920:1080"), "{chain}");
    assert!(chain.contains("format=gray"), "{chain}");
    assert!(chain.contains("chromanr"), "{chain}");
    assert!(chain.contains("format=yuv420p10le"), "{chain}");
    assert!(chain.contains("fps=30"), "{chain}");
}

#[test]
fn transcode_with_video_processing() {
    let dir = std::env::temp_dir().join("vise-video-processing");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("vp.mp4");
    let _ = std::fs::remove_file(&out);

    let mut spec = vise_core::JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: vise_core::ContainerFormat::Mp4,
        ..Default::default()
    };
    spec.video_processing.scale = Some((160, 120));
    spec.video_processing.deinterlace = true;

    let ts = Transcoder::default();
    let result = ts.transcode(&spec).expect("transcode");
    assert!(out.exists(), "output missing");
    let _ = result;
}
