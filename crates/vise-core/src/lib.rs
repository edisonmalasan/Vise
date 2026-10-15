//! Clean-room core foundation for Vise Transcoder.
//!
//! This crate provides the shared data models, session/job API, media buffer and
//! packet types, progress/state model, and geometry helpers used by all
//! frontends. It has no UI dependency.

mod error;
mod geometry;
mod job;
mod media;
mod pipeline;
mod progress;
mod session;

pub use error::CoreError;
pub use geometry::*;
pub use job::{AudioCodec, ContainerFormat, JobSpec, SubtitleMode, VideoCodec};
pub use media::{
    parse_ffprobe_output, ChapterInfo, CropDetection, FfprobeScanner, FormatInfo, MediaBuffer,
    MediaScanner, Packet, ScanResult, StreamInfo, TitleInfo, TrackKind,
};
pub use pipeline::{transcode, transcode_file, TranscodeResult, Transcoder};
pub use progress::{JobState, JobStatus, Progress};
pub use session::{JobId, Session, SessionSnapshot};
