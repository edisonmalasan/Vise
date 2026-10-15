use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use vise_core::{JobSpec, Transcoder};

use crate::cli::{BatchArgs, TranscodeArgs};
use crate::preset;

static CANCEL_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn setup_cancel_handler() {
    let _ = ::ctrlc::set_handler(|| {
        if let Ok(mut guard) = CANCEL_PATH.lock() {
            if let Some(path) = guard.take() {
                let _ = fs::remove_file(path.clone());
            }
        }
        eprintln!("cancelled");
        std::process::exit(130);
    });
}

pub fn transcode_command(args: TranscodeArgs) -> Result<(), (i32, String)> {
    setup_cancel_handler();
    transcode_one(
        &args.input,
        &args.output,
        args.title,
        args.preset.as_deref(),
        args.format.as_deref(),
        args.video_codec.as_deref(),
        args.audio_codec.as_deref(),
    )
}

pub fn transcode_one(
    input: &std::path::Path,
    output: &std::path::Path,
    title: Option<u32>,
    preset_path: Option<&std::path::Path>,
    format: Option<&str>,
    video_codec: Option<&str>,
    audio_codec: Option<&str>,
) -> Result<(), (i32, String)> {
    let transcoder = Transcoder::default();

    let mut spec = JobSpec {
        input_path: input.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        ..JobSpec::default()
    };

    if let Some(p) = preset_path {
        let preset = preset::load(p).map_err(|e| (2, e))?;
        preset::apply(&mut spec, &preset);
    }

    if let Some(f) = format {
        if let Some(c) = preset::parse_container(f) {
            spec.container = c;
        } else {
            return Err((2, format!("unknown format: {f}")));
        }
    }

    if let Some(v) = video_codec {
        if let Some(c) = preset::parse_video(v) {
            spec.video_codec = c;
        } else {
            return Err((2, format!("unknown video codec: {v}")));
        }
    }
    if let Some(a) = audio_codec {
        if let Some(c) = preset::parse_audio(a) {
            spec.audio_codec = c;
        } else {
            return Err((2, format!("unknown audio codec: {a}")));
        }
    }
    if let Some(t) = title {
        spec.title_index = Some(t);
    }

    // Atomic output path.
    let final_output = output.to_path_buf();
    let parent = final_output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let ext = match spec.container {
        vise_core::ContainerFormat::Mp4 => "mp4",
        vise_core::ContainerFormat::Mkv => "mkv",
        vise_core::ContainerFormat::Webm => "webm",
    };
    let base = final_output
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("vise-output");
    let temp_output = parent.join(format!("{base}.tmp-{}.{}", std::process::id(), ext));

    spec.output_path = temp_output.to_string_lossy().to_string();

    println!(
        "transcode: running input={} output={} progress=0%",
        input.display(),
        final_output.display()
    );

    if let Ok(mut guard) = CANCEL_PATH.lock() {
        *guard = Some(temp_output.clone());
    }

    let result = transcoder.transcode(&spec);

    if let Ok(mut guard) = CANCEL_PATH.lock() {
        *guard = None;
    }

    match result {
        Ok(_) => {
            if final_output.exists() {
                fs::remove_file(final_output.clone())
                    .map_err(|e| (1, format!("removing old output: {e}")))?;
            }
            fs::rename(temp_output.clone(), final_output.clone())
                .map_err(|e| (1, format!("renaming output: {e}")))?;
            println!(
                "transcode: completed output={} progress=100%",
                final_output.display()
            );
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(temp_output.clone());
            Err((1, format!("transcode failed: {e}")))
        }
    }
}

#[derive(serde::Deserialize)]
struct BatchFile {
    #[serde(default)]
    jobs: Vec<BatchJob>,
}

#[derive(serde::Deserialize, Clone)]
struct BatchJob {
    input: String,
    output: String,
    #[serde(default)]
    title: Option<u32>,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    video_codec: Option<String>,
    #[serde(default)]
    audio_codec: Option<String>,
}

pub fn batch_command(args: BatchArgs) -> Result<(), (i32, String)> {
    setup_cancel_handler();
    let raw = fs::read_to_string(&args.jobs).map_err(|e| (2, format!("reading jobs file: {e}")))?;
    let jobs: Vec<BatchJob> = if raw.trim_start().starts_with('[') {
        serde_json::from_str(&raw).map_err(|e| (2, format!("parsing jobs file: {e}")))?
    } else {
        let batch: BatchFile =
            serde_json::from_str(&raw).map_err(|e| (2, format!("parsing jobs file: {e}")))?;
        batch.jobs
    };

    let total = jobs.len();
    let mut failures = Vec::new();

    for (idx, job) in jobs.iter().enumerate() {
        let input = PathBuf::from(&job.input);
        let output = PathBuf::from(&job.output);
        let preset_path = job.preset.as_ref().map(PathBuf::from);
        println!("batch: job {}/{} running", idx + 1, total);
        match transcode_one(
            &input,
            &output,
            job.title,
            preset_path.as_deref(),
            job.format.as_deref(),
            job.video_codec.as_deref(),
            job.audio_codec.as_deref(),
        ) {
            Ok(()) => println!("batch: job {}/{} completed", idx + 1, total),
            Err(e) => {
                failures.push((idx + 1, e.1));
                eprintln!("batch: job {}/{} failed", idx + 1, total);
            }
        }
    }

    if failures.is_empty() {
        println!("batch: all {} jobs completed", total);
        Ok(())
    } else {
        let msg = failures
            .into_iter()
            .map(|(n, m)| format!("job {n}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        Err((1, format!("batch finished with failures: {msg}")))
    }
}

pub fn inspect_preset(file: PathBuf) -> Result<(), (i32, String)> {
    let preset = preset::load(&file).map_err(|e| (2, e))?;
    let pretty = serde_json::to_string_pretty(&preset).map_err(|e| (1, e.to_string()))?;
    println!("{pretty}");
    Ok(())
}
