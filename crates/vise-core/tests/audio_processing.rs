use std::path::PathBuf;
use std::process::Command;

use vise_core::{AudioMixdown, AudioProcessingOptions, AudioTrackConfig, Transcoder};

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
    let opts = AudioProcessingOptions {
        source_track: Some(1),
        mixdown: Some("stereo".to_string()),
        channels: Some(2),
        sample_rate: Some(48000),
        dither: true,
        dither_mode: Some("triangular".to_string()),
        gain: Some(1.5),
        volume: Some(2.0),
        delay: Some(100.0),
        normalize: true,
        ..Default::default()
    };
    let chain = opts.build_filter_chain();
    assert!(chain.contains("stereo"), "{chain}");
    assert!(chain.contains("aresample=48000"), "{chain}");
    assert!(chain.contains("volume=1.5"), "{chain}");
    assert!(chain.contains("adelay=100|100"), "{chain}");
    assert!(chain.contains("loudnorm"), "{chain}");

    // mixdown enum alias path
    let opts2 = AudioProcessingOptions {
        mixdown_enum: Some(AudioMixdown::Stereo),
        ..Default::default()
    };
    assert!(opts2.build().contains("stereo"));
}

#[test]
fn audio_track_config_basic() {
    let cfg = AudioTrackConfig {
        source_track_id: Some(0),
        passthrough: true,
        ..Default::default()
    };
    assert!(cfg.passthrough);
}

#[test]
fn transcode_with_audio_processing() {
    let dir = std::env::temp_dir().join("vise-audio-processing");
    let _ = std::fs::create_dir_all(&dir);
    let source = generate_source(&dir);
    let out = dir.join("ap.mp4");
    let _ = std::fs::remove_file(&out);

    let mut spec = vise_core::JobSpec {
        input_path: source.to_string_lossy().to_string(),
        output_path: out.to_string_lossy().to_string(),
        container: vise_core::ContainerFormat::Mp4,
        ..Default::default()
    };
    spec.audio_processing.sample_rate = Some(22050);
    spec.audio_processing.gain = Some(0.5);

    let ts = Transcoder::default();
    let result = ts.transcode(&spec).expect("transcode");
    assert!(out.exists(), "output missing");
    let _ = result;
}

#[test]
fn passthrough_ac3_codec() {
    // compile-level sanity
    let _ = vise_core::AudioCodec::Ac3;
    let _ = vise_core::AudioCodec::Eac3;
}
