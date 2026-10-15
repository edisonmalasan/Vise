use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use vise_core::FfprobeScanner;

fn ffmpeg() -> PathBuf {
    FfprobeScanner::default()
        .ffmpeg_path()
        .expect("ffmpeg must be available")
        .to_path_buf()
}

fn source(dir: &Path) -> PathBuf {
    let out = dir.join("source.mp4");
    let _ = std::fs::remove_file(&out);
    let status = StdCommand::new(ffmpeg())
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
        .expect("run ffmpeg");
    assert!(status.success());
    out
}

#[test]
fn single_transcode() {
    let dir = std::env::temp_dir().join("vise-cli-single");
    let _ = std::fs::create_dir_all(&dir);
    let src = source(&dir);
    let out = dir.join("out.mp4");
    let _ = std::fs::remove_file(&out);

    let mut cmd = Command::cargo_bin("vise").unwrap();
    cmd.args([
        "transcode",
        "--input",
        src.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
    ]);
    cmd.assert().success();
    assert!(out.exists());
}

#[test]
fn preset_transcode() {
    let dir = std::env::temp_dir().join("vise-cli-preset");
    let _ = std::fs::create_dir_all(&dir);
    let src = source(&dir);
    let out = dir.join("out.mkv");
    let _ = std::fs::remove_file(&out);
    let preset = dir.join("preset.json");
    std::fs::write(&preset, r#"{"format":"mkv"}"#).unwrap();

    let mut cmd = Command::cargo_bin("vise").unwrap();
    cmd.args([
        "transcode",
        "--input",
        src.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--preset",
        preset.to_str().unwrap(),
    ]);
    cmd.assert().success();
    assert!(out.exists());
}

#[test]
fn batch_transcode() {
    let dir = std::env::temp_dir().join("vise-cli-batch");
    let _ = std::fs::create_dir_all(&dir);
    let src = source(&dir);
    let out1 = dir.join("b1.mp4");
    let out2 = dir.join("b2.mkv");
    let _ = std::fs::remove_file(&out1);
    let _ = std::fs::remove_file(&out2);
    let jobs = dir.join("jobs.json");
    let content = serde_json::to_string(&serde_json::json!([
        {"input": src, "output": out1},
        {"input": src, "output": out2, "format": "mkv"}
    ]))
    .unwrap();
    std::fs::write(&jobs, content).unwrap();

    let mut cmd = Command::cargo_bin("vise").unwrap();
    cmd.args(["batch", "--jobs", jobs.to_str().unwrap()]);
    cmd.assert().success();
    assert!(out1.exists());
    assert!(out2.exists());
}
