use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use vise_core::{JobSpec, MessageKey, Transcoder};

use crate::cli::{BatchArgs, Say, TranscodeArgs};
use crate::preset;

static CANCEL_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn setup_cancel_handler(say: &Say) {
    let _ = ::ctrlc::set_handler(|| {
        if let Ok(mut guard) = CANCEL_PATH.lock() {
            if let Some(path) = guard.take() {
                let _ = fs::remove_file(path.clone());
            }
        }
        eprintln!("cancelled");
        std::process::exit(130);
    });
    if let Some(line) = say.fallback_line() {
        // The language asked for is not one this build ships, so one line says
        // which language is being used instead rather than leaving the reader to
        // work it out from the refusals.
        eprintln!("{line}");
    }
}

/// What the user asked the file itself to be, apart from the settings a preset
/// carries.
///
/// Grouped because these three are one question — what shape the finished file
/// takes — and because a refusal about any of them is one sentence with the same
/// three answers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Choices<'a> {
    /// The container to write.
    pub format: Option<&'a str>,
    /// The video codec to write.
    pub video_codec: Option<&'a str>,
    /// The audio codec to write.
    pub audio_codec: Option<&'a str>,
}

pub fn transcode_command(args: TranscodeArgs, say: &mut Say) -> Result<(), (i32, String)> {
    setup_cancel_handler(say);
    transcode_one(
        &args.input,
        &args.output,
        args.title,
        args.preset.as_deref(),
        Choices {
            format: args.format.as_deref(),
            video_codec: args.video_codec.as_deref(),
            audio_codec: args.audio_codec.as_deref(),
        },
        say,
    )
}

pub fn transcode_one(
    input: &std::path::Path,
    output: &std::path::Path,
    title: Option<u32>,
    preset_path: Option<&std::path::Path>,
    choices: Choices<'_>,
    say: &mut Say,
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

    if let Some(f) = choices.format {
        if let Some(c) = preset::parse_container(f) {
            spec.container = c;
        } else {
            let sentence = say.line(MessageKey::CliUnknownFormat, &[("value", f)]);
            return Err((2, sentence));
        }
    }

    if let Some(v) = choices.video_codec {
        if let Some(c) = preset::parse_video(v) {
            spec.video_codec = c;
        } else {
            let sentence = say.line(MessageKey::CliUnknownVideoCodec, &[("value", v)]);
            return Err((2, sentence));
        }
    }
    if let Some(a) = choices.audio_codec {
        if let Some(c) = preset::parse_audio(a) {
            spec.audio_codec = c;
        } else {
            let sentence = say.line(MessageKey::CliUnknownAudioCodec, &[("value", a)]);
            return Err((2, sentence));
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
                let problem = fs::remove_file(final_output.clone()).err();
                if let Some(e) = problem {
                    let sentence = say.line(
                        MessageKey::CliRemovingOutput,
                        &[("problem", &e.to_string())],
                    );
                    return Err((1, sentence));
                }
            }
            if let Err(e) = fs::rename(temp_output.clone(), final_output.clone()) {
                let sentence = say.line(
                    MessageKey::CliRenamingOutput,
                    &[("problem", &e.to_string())],
                );
                return Err((1, sentence));
            }
            println!(
                "transcode: completed output={} progress=100%",
                final_output.display()
            );
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(temp_output.clone());
            let sentence = say.line(
                MessageKey::CliTranscodeFailed,
                &[("problem", &e.to_string())],
            );
            Err((1, sentence))
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

pub fn batch_command(args: BatchArgs, say: &mut Say) -> Result<(), (i32, String)> {
    setup_cancel_handler(say);
    let raw = fs::read_to_string(&args.jobs).map_err(|e| {
        let sentence = say.line(MessageKey::CliReadingJobs, &[("problem", &e.to_string())]);
        (2, sentence)
    })?;
    let jobs: Vec<BatchJob> = if raw.trim_start().starts_with('[') {
        serde_json::from_str(&raw).map_err(|e| {
            let sentence = say.line(MessageKey::CliReadingJobs, &[("problem", &e.to_string())]);
            (2, sentence)
        })?
    } else {
        let batch: BatchFile = serde_json::from_str(&raw).map_err(|e| {
            let sentence = say.line(MessageKey::CliReadingJobs, &[("problem", &e.to_string())]);
            (2, sentence)
        })?;
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
            Choices {
                format: job.format.as_deref(),
                video_codec: job.video_codec.as_deref(),
                audio_codec: job.audio_codec.as_deref(),
            },
            say,
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
        let summary = failures
            .into_iter()
            .map(|(n, m)| format!("job {n}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        let sentence = say.line(MessageKey::CliBatchFailures, &[("summary", &summary)]);
        Err((1, sentence))
    }
}

pub fn inspect_preset(file: PathBuf) -> Result<(), (i32, String)> {
    let preset = preset::load(&file).map_err(|e| (2, e))?;
    let pretty = serde_json::to_string_pretty(&preset).map_err(|e| (1, e.to_string()))?;
    println!("{pretty}");
    Ok(())
}
