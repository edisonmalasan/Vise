/// Errors produced by the core foundation APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A requested job could not be found in the session.
    JobNotFound(u64),
    /// The supplied value is invalid for the requested operation.
    Invalid(String),
    /// A required external tool (ffmpeg/ffprobe) is not available.
    MissingTool(String),
    /// A subprocess command failed to run or returned an error.
    CommandFailed(String),
    /// The supplied media is invalid or could not be probed.
    InvalidMedia(String),
    /// A file system operation failed.
    Io(String),
    /// Failed to parse tool output.
    Parse(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::JobNotFound(id) => write!(f, "job not found: {id}"),
            CoreError::Invalid(msg) => write!(f, "invalid value: {msg}"),
            CoreError::MissingTool(msg) => write!(f, "missing tool: {msg}"),
            CoreError::CommandFailed(msg) => write!(f, "command failed: {msg}"),
            CoreError::InvalidMedia(msg) => write!(f, "invalid media: {msg}"),
            CoreError::Io(msg) => write!(f, "io error: {msg}"),
            CoreError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}
