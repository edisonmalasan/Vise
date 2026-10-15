use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    #[default]
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Progress {
    /// Fraction complete in the range 0.0..=1.0.
    pub fraction: f64,
    /// Estimated milliseconds remaining, if known.
    pub eta_ms: Option<u64>,
    /// Bytes produced so far, if known.
    pub bytes_done: Option<u64>,
}

impl Progress {
    pub fn new(fraction: f64) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            eta_ms: None,
            bytes_done: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStatus {
    pub state: JobState,
    pub progress: Progress,
    pub message: Option<String>,
}

impl Default for JobStatus {
    fn default() -> Self {
        Self {
            state: JobState::Queued,
            progress: Progress::default(),
            message: None,
        }
    }
}
