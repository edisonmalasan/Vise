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

/// A refusal is written in the language the run was asked for.
///
/// The pipeline is shared with the desktop, so this is the same resolution the
/// window uses: an exact tag gets that language's sentence, and a tag this build
/// does not ship falls back to the base language and says so.
#[test]
fn a_refusal_is_written_in_the_language_that_was_asked_for() {
    let dir = std::env::temp_dir().join("vise-cli-language");
    let _ = std::fs::create_dir_all(&dir);
    let input = dir.join("in.mkv");
    let out = dir.join("out.mkv");
    let _ = std::fs::remove_file(&input);

    let mut english = Command::cargo_bin("vise").unwrap();
    english.args([
        "--language",
        "en",
        "transcode",
        "--input",
        input.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--format",
        "not-a-container",
    ]);
    let refusal = english.output().expect("runs");
    let said = String::from_utf8_lossy(&refusal.stderr);
    assert!(said.contains("not a container this build writes"), "{said}");

    let mut german = Command::cargo_bin("vise").unwrap();
    german.args([
        "--language",
        "de",
        "transcode",
        "--input",
        input.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--format",
        "not-a-container",
    ]);
    let refusal = german.output().expect("runs");
    let said = String::from_utf8_lossy(&refusal.stderr);
    assert!(said.contains("kein Container"), "{said}");

    let mut arabic = Command::cargo_bin("vise").unwrap();
    arabic.args([
        "--language",
        "ar",
        "transcode",
        "--input",
        input.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--audio-codec",
        "not-a-codec",
    ]);
    let refusal = arabic.output().expect("runs");
    let said = String::from_utf8_lossy(&refusal.stderr);
    assert!(said.contains("ليست مرمّز صوت"), "{said}");
}

/// A language this build does not ship is not an error: the run goes ahead in the
/// base language and says which one it used.
#[test]
fn a_language_this_build_does_not_ship_falls_back_and_says_so() {
    let dir = std::env::temp_dir().join("vise-cli-language-fallback");
    let _ = std::fs::create_dir_all(&dir);
    let input = dir.join("in.mkv");
    let out = dir.join("out.mkv");
    let _ = std::fs::remove_file(&input);

    let mut cmd = Command::cargo_bin("vise").unwrap();
    cmd.args([
        "--language",
        "kl",
        "transcode",
        "--input",
        input.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--format",
        "not-a-container",
    ]);
    let refusal = cmd.output().expect("runs");
    let said = String::from_utf8_lossy(&refusal.stderr);
    assert!(said.contains("kl"), "{said}");
    assert!(said.contains("English"), "{said}");
    assert!(said.contains("not a container this build writes"), "{said}");
}
