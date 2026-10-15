use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::ops;

#[derive(Debug, Parser)]
#[command(
    name = "vise",
    version,
    about = "Vise transcoder command-line interface"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Transcode a single input file.
    Transcode(TranscodeArgs),
    /// Run a batch of transcode jobs from a file.
    Batch(BatchArgs),
    /// Work with preset files.
    Presets {
        #[command(subcommand)]
        command: PresetCommands,
    },
}

#[derive(Debug, clap::Args)]
pub struct TranscodeArgs {
    #[arg(long)]
    pub input: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub title: Option<u32>,
    #[arg(long)]
    pub preset: Option<PathBuf>,
    #[arg(long)]
    pub format: Option<String>,
    #[arg(long)]
    pub video_codec: Option<String>,
    #[arg(long)]
    pub audio_codec: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct BatchArgs {
    #[arg(long)]
    pub jobs: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum PresetCommands {
    Inspect {
        #[arg(long)]
        file: PathBuf,
    },
}

pub fn parse() -> Cli {
    use clap::Parser;
    Cli::parse()
}

pub fn run(cli: Cli) -> Result<(), (i32, String)> {
    match cli.command {
        Commands::Transcode(args) => ops::transcode_command(args),
        Commands::Batch(args) => ops::batch_command(args),
        Commands::Presets {
            command: PresetCommands::Inspect { file },
        } => ops::inspect_preset(file),
    }
}

// Re-export for convenience in main.
impl Cli {}
