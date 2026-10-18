//! Clean-room core foundation for Vise Transcoder.
//!
//! This crate provides the shared data models, session/job API, media buffer and
//! packet types, progress/state model, and geometry helpers used by all
//! frontends. It has no UI dependency.

pub mod attachments;
mod audio;
mod capability;
mod catalog;
mod chapter;
mod error;
mod geometry;
mod hardware;
pub mod hdr;
pub mod i18n;
mod job;
mod lossless;
mod media;
mod metadata;
mod pipeline;
mod preset;
mod progress;
mod release;
mod session;
mod subtitle;
pub mod sweep;
pub use sweep::{
    SweepExecutor, SweepOutcome, SweepPlan, SweepProgress, SweepRange, SweepRequest,
    SweepResultRow, SweepResults, SweepRun, SweepState, CONFIRMED_SPAN, MAX_SWEEP_RUNS,
    SWEEP_FOLDER,
};
mod video;
pub use attachments::*;
pub use audio::*;
pub use capability::*;
pub use catalog::*;
pub use chapter::*;
pub use error::CoreError;
pub use geometry::*;
pub use hardware::*;
pub use hdr::*;
pub use i18n::*;
pub use job::{
    AudioCodec, ContainerFormat, EncodingMode, JobSpec, OutputOptions, SubtitleMode, VideoCodec,
};
pub use lossless::*;
pub use media::{
    optical_source_kind, parse_ffprobe_output, ChapterInfo, CropDetection, FfprobeScanner,
    FormatInfo, MediaBuffer, MediaScanner, OpticalSourceKind, Packet, ScanResult, StreamInfo,
    TitleInfo, TrackKind,
};
pub use metadata::*;
pub use pipeline::{transcode, transcode_file, TranscodePlan, TranscodeResult, Transcoder};
pub use preset::*;
pub use progress::{JobState, JobStatus, Progress};
pub use release::{ReleaseInfo, UpdateManifest, VISE_VERSION};
pub use session::{Job, JobId, Session, SessionSnapshot};
pub use subtitle::*;
pub use video::*;
