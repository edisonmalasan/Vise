/// Errors produced by the core foundation APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A requested job could not be found in the session.
    JobNotFound(u64),
    /// The supplied value is invalid for the requested operation.
    Invalid(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::JobNotFound(id) => write!(f, "job not found: {id}"),
            CoreError::Invalid(msg) => write!(f, "invalid value: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}
