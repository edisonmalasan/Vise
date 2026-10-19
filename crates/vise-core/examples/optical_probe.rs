//! Read what a real media tool can read, and print it.
//!
//! The answer is asked of the tool on this machine rather than assumed, so the
//! check can be run against the tool a release would actually ship.

use std::path::PathBuf;

fn main() {
    let ffmpeg = std::env::args().nth(1).map(PathBuf::from).or_else(|| {
        vise_core::FfprobeScanner::default()
            .ffmpeg_path()
            .map(PathBuf::from)
    });

    let Some(ffmpeg) = ffmpeg else {
        println!("no ffmpeg was found");
        return;
    };
    println!("tool: {}", ffmpeg.display());

    let caps = vise_core::optical::OpticalCapabilities::detect(Some(&ffmpeg));
    println!("can read a DVD:     {}", caps.dvd);
    println!("can read a Blu-ray: {}", caps.bluray);

    // A path that is not a disc is still worth asking about: the point is to
    // see which demuxer the tool reaches for, and what it says when there is
    // nothing there.
    let missing = std::env::temp_dir().join("vise-optical-probe");
    let _ = std::fs::remove_dir_all(&missing);
    let _ = std::fs::create_dir_all(missing.join("VIDEO_TS"));
    let arguments = vise_core::optical::encoder_input_arguments(&missing);
    println!("a DVD folder is read as: {arguments:?}");

    let output = std::process::Command::new(&ffmpeg)
        .arg("-hide_banner")
        .args(&arguments)
        .arg("-t")
        .arg("1")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output();
    match output {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stderr).to_string();
            let first = text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("(no message)");
            println!("the tool says: {first}");
        }
        Err(error) => println!("the tool could not be run: {error}"),
    }
}
