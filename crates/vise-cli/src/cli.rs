use clap::{Parser, Subcommand};
use std::path::PathBuf;

use vise_core::resolve_catalog;

use crate::ops;

#[derive(Debug, Parser)]
#[command(
    name = "vise",
    version,
    about = "Vise transcoder command-line interface"
)]
pub struct Cli {
    /// The language this build writes its own messages in.
    ///
    /// Read the same way the desktop reads it — from the environment when this is
    /// not given — so a machine that has never been told anything gets the same
    /// words in both frontends. A language this build does not ship is not an
    /// error: the fallback chain answers with the base language and says so.
    #[arg(long, global = true)]
    pub language: Option<String>,
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
    // The words this run is written in are resolved before anything else, because
    // every refusal below is one of them and a refusal in the wrong language is
    // one the user has to guess at.
    let requested = cli
        .language
        .clone()
        .unwrap_or_else(|| vise_core::locale_from_environment().tag().to_string());
    let resolved = resolve_catalog(&requested);
    let catalog = resolved.catalog;
    let mut say = Say {
        catalog,
        report_fallback: !resolved.is_exact(),
        requested,
        locale: resolved.locale.endonym().to_string(),
    };
    match cli.command {
        Commands::Transcode(args) => ops::transcode_command(args, &mut say),
        Commands::Batch(args) => ops::batch_command(args, &mut say),
        Commands::Presets {
            command: PresetCommands::Inspect { file },
        } => ops::inspect_preset(file),
    }
}

/// The messages this run writes its own refusals in.
///
/// One place, so the language a refusal is written in is the same for every
/// refusal and is decided once rather than by whichever function happened to be
/// reached first.
pub struct Say<'a> {
    /// The catalog in force.
    pub catalog: &'a vise_core::Catalog,
    /// Whether the language asked for is not the one in force.
    pub report_fallback: bool,
    /// The tag that was asked for.
    pub requested: String,
    /// The language in force, in its own name.
    pub locale: String,
}

impl Say<'_> {
    /// One sentence, with `{name}` filled in.
    ///
    /// A name the sentence does not carry is left exactly as written rather than
    /// replaced with nothing, for the same reason the pipeline leaves `%name%`
    /// alone in a command: a hole in a message is a fact somebody has to be able
    /// to see.
    pub fn line(&self, key: vise_core::MessageKey, args: &[(&str, &str)]) -> String {
        self.catalog.format(key, args)
    }

    /// The one line saying which language is being used instead of the one asked
    /// for, when there is one to say.
    pub fn fallback_line(&self) -> Option<String> {
        if !self.report_fallback {
            return None;
        }
        Some(self.line(
            vise_core::MessageKey::LanguageNotShipped,
            &[("requested", &self.requested), ("locale", &self.locale)],
        ))
    }
}

// Re-export for convenience in main.
impl Cli {}
