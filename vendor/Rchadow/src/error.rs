use std::path::PathBuf;

/// Crate-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A required file or directory path was not configured.
    #[error("missing required path: {0}")]
    MissingPath(&'static str),

    /// A configured file path does not exist.
    #[error("path does not exist: {path}")]
    PathNotFound {
        /// Missing path.
        path: PathBuf,
    },

    /// Wrapper for filesystem errors.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path associated with the operation.
        path: PathBuf,
        /// Original I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Wrapper for JSON serialization errors.
    #[error("JSON error at {path}: {source}")]
    Json {
        /// Path associated with the JSON document.
        path: PathBuf,
        /// Original JSON error.
        #[source]
        source: serde_json::Error,
    },

    /// A launch request could not be completed.
    #[error("launch failed: {0}")]
    Launch(String),

    /// A database backend operation failed.
    #[error("{backend} database error: {message}")]
    Database {
        /// Backend name.
        backend: &'static str,
        /// Error details.
        message: String,
    },
}

/// Convenient crate result alias.
pub type Result<T> = std::result::Result<T, Error>;
