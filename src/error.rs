use thiserror::Error;

pub type Result<T> = std::result::Result<T, PeekabooError>;

#[derive(Debug, Error)]
pub enum PeekabooError {
    #[error("`{0}` is not supported on this platform")]
    UnsupportedPlatform(&'static str),
    #[error("missing argument: {0}")]
    MissingArgument(&'static str),
    #[error("invalid coordinates: {0}")]
    InvalidCoordinates(String),
    #[error("target not found: {0}")]
    TargetNotFound(String),
    #[error("command failed: {program} exited {status}: {stderr}")]
    CommandFailed {
        program: String,
        status: i32,
        stderr: String,
    },
    #[error("{0}")]
    System(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}
