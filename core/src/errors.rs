use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by the Streamable API client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StreamableError {
    /// Signup failed because the email is already registered.
    #[error("{message}")]
    EmailAlreadyInUse {
        /// Message returned by Streamable.
        message: String,
    },

    /// Login failed because the email or password is incorrect.
    #[error("{message}")]
    InvalidCredentials {
        /// Authentication failure message.
        message: String,
    },

    /// An authenticated operation failed because the session is missing or expired.
    #[error("{message}")]
    InvalidSession {
        /// Session failure message.
        message: String,
    },

    /// Password validation failed.
    #[error("{message}")]
    PasswordValidation {
        /// Password requirement message.
        message: String,
    },

    /// Label creation failed because the authenticated user already has a label with this name.
    #[error("Label '{name}' already exists.")]
    LabelAlreadyExists {
        /// Conflicting label name.
        name: String,
    },

    /// Label operation failed because the authenticated user does not have this label.
    #[error("Label ID {id} not found.")]
    LabelNotFound {
        /// Missing label identifier.
        id: u64,
    },

    /// Streamable rejected the request because the endpoint rate limit was exceeded.
    #[error("Rate limit exceeded for {endpoint}. Try again later.")]
    RateLimitExceeded {
        /// Endpoint whose rate limit was exceeded.
        endpoint: String,
    },

    /// The requested upload path is not a recognized video file.
    #[error("Path '{}' is not a valid video file", path.display())]
    InvalidVideoFile {
        /// Rejected local path.
        path: PathBuf,
    },

    /// Streamable's temporary S3 configuration could not be signed.
    #[error("the S3 upload request could not be signed: {message}")]
    UploadSigning {
        /// Signing failure detail.
        message: String,
    },

    /// The caller cancelled a video upload.
    #[error("video upload was cancelled")]
    UploadCancelled {
        /// Assigned shortcode, or `None` when cancellation preceded assignment.
        shortcode: Option<String>,
    },

    /// A local file operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The HTTP request failed.
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    /// The API response body did not match the expected model.
    #[error(transparent)]
    ResponseDecode(#[from] serde_json::Error),

    /// A configured or request URL was invalid.
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}

/// Result type returned by Streamable operations.
pub type Result<T> = std::result::Result<T, StreamableError>;
