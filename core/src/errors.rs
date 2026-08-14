use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by the client.
///
/// ```
/// use streamable::StreamableError;
/// let error = StreamableError::UploadCancelled { shortcode: None };
/// assert_eq!(error.to_string(), "video upload was cancelled");
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StreamableError {
    /// The email is already registered.
    #[error("{message}")]
    EmailAlreadyInUse {
        /// Server message.
        message: String,
    },

    /// The email or password is wrong.
    #[error("{message}")]
    InvalidCredentials {
        /// Server message.
        message: String,
    },

    /// The login session is missing or expired.
    #[error("{message}")]
    InvalidSession {
        /// Server message.
        message: String,
    },

    /// The password does not meet the rules.
    #[error("{message}")]
    PasswordValidation {
        /// Password rule message.
        message: String,
    },

    /// A label already has this name.
    #[error("Label '{name}' already exists.")]
    LabelAlreadyExists {
        /// Existing name.
        name: String,
    },

    /// The label does not exist.
    #[error("Label ID {id} not found.")]
    LabelNotFound {
        /// Missing label ID.
        id: u64,
    },

    /// Streamable rejected the new video labels.
    #[error("setting labels for video '{shortcode}' failed with HTTP status {status}")]
    VideoLabelAssignmentFailed {
        /// Video shortcode.
        shortcode: String,
        /// HTTP status.
        status: u16,
    },

    /// Streamable rejected a video privacy change.
    #[error("updating privacy for video '{shortcode}' failed with HTTP status {status}: {message}")]
    VideoPrivacyUpdateFailed {
        /// Video shortcode.
        shortcode: String,
        /// HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a video privacy reset.
    #[error(
        "resetting privacy for video '{shortcode}' failed with HTTP status {status}: {message}"
    )]
    VideoPrivacyResetFailed {
        /// Video shortcode.
        shortcode: String,
        /// HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Too many requests reached an endpoint.
    #[error("Rate limit exceeded for {endpoint}. Try again later.")]
    RateLimitExceeded {
        /// Limited endpoint.
        endpoint: String,
    },

    /// The upload path is not a video file.
    #[error("Path '{}' is not a valid video file", path.display())]
    InvalidVideoFile {
        /// Rejected path.
        path: PathBuf,
    },

    /// The S3 upload request could not be signed.
    #[error("the S3 upload request could not be signed: {message}")]
    UploadSigning {
        /// Error message.
        message: String,
    },

    /// The upload was cancelled.
    #[error("video upload was cancelled")]
    UploadCancelled {
        /// Video shortcode, if one was assigned.
        shortcode: Option<String>,
    },

    /// Video deletion returned an unexpected body.
    #[error("video deletion for '{shortcode}' returned unexpected response body: {response:?}")]
    UnexpectedVideoDeletionResponse {
        /// Video shortcode.
        shortcode: String,
        /// Response body.
        response: String,
    },

    /// A local file operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The HTTP request failed.
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    /// The response body had an unexpected shape.
    #[error(transparent)]
    ResponseDecode(#[from] serde_json::Error),

    /// A URL was invalid.
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}

/// Result type returned by Streamable operations.
///
/// ```
/// fn upload() -> streamable::Result<()> { Ok(()) }
/// assert!(upload().is_ok());
/// ```
pub type Result<T> = std::result::Result<T, StreamableError>;
