use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by the client.
///
/// ```
/// use streamable::StreamableError;
/// let error = StreamableError::InvalidSession { message: "expired".into() };
/// assert_eq!(error.to_string(), "expired");
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

    /// A resource's originating client session was invalidated by logout.
    #[error("the resource was invalidated when its client logged out")]
    ResourceInvalidated,

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

    /// Streamable rejected a video analytics request.
    #[error(
        "getting analytics for video '{shortcode}' failed with HTTP status {status}: {message}"
    )]
    VideoAnalyticsFailed {
        /// Video shortcode.
        shortcode: String,
        /// HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a live view count request.
    #[error(
        "getting live views for video '{shortcode}' failed with HTTP status {status}: {message}"
    )]
    VideoLiveViewsFailed {
        /// Video shortcode.
        shortcode: String,
        /// HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// A collection does not exist.
    #[error("Collection '{shortcode}' not found.")]
    CollectionNotFound {
        /// Missing collection shortcode.
        shortcode: String,
    },

    /// Streamable rejected collection creation.
    #[error("creating a collection failed with HTTP status {status}: {message}")]
    CollectionCreationFailed {
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a collection count request.
    #[error("counting collections failed with HTTP status {status}: {message}")]
    CollectionCountFailed {
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a collection list request.
    #[error("listing collections failed with HTTP status {status}: {message}")]
    CollectionListFailed {
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a collection detail request.
    #[error("getting collection '{shortcode}' failed with HTTP status {status}: {message}")]
    CollectionFetchFailed {
        /// Collection shortcode.
        shortcode: String,
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected a collection update.
    #[error("updating collection '{shortcode}' failed with HTTP status {status}: {message}")]
    CollectionUpdateFailed {
        /// Collection shortcode.
        shortcode: String,
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// Streamable rejected collection deletion.
    #[error("deleting collection '{shortcode}' failed with HTTP status {status}: {message}")]
    CollectionDeletionFailed {
        /// Collection shortcode.
        shortcode: String,
        /// Numeric HTTP status.
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

    /// The thumbnail upload path is not an image file.
    #[error("Path '{}' is not a valid image file", path.display())]
    InvalidImageFile {
        /// Rejected path.
        path: PathBuf,
    },

    /// A requested video-frame thumbnail offset is invalid.
    #[error("Thumbnail offset must be finite and non-negative, got {seconds}")]
    InvalidThumbnailOffset {
        /// Rejected offset in seconds.
        seconds: f64,
    },

    /// Streamable rejected a video thumbnail change.
    #[error(
        "updating thumbnail for video '{shortcode}' failed with HTTP status {status}: {message}"
    )]
    VideoThumbnailUpdateFailed {
        /// Video whose thumbnail was not changed.
        shortcode: String,
        /// Numeric HTTP status.
        status: u16,
        /// Server message.
        message: String,
    },

    /// The S3 upload request could not be signed.
    #[error("the S3 upload request could not be signed")]
    UploadSigning,

    /// An allocated upload failed and Streamable also rejected its cleanup request.
    #[error("video upload '{shortcode}' failed ({source}) and rollback failed ({rollback})")]
    UploadRollback {
        /// Allocated video shortcode.
        shortcode: String,
        /// Original upload failure.
        source: Box<Self>,
        /// Cleanup request failure.
        rollback: Box<Self>,
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

    /// The HTTP transport could not complete a request.
    #[error("HTTP transport failed: {source}")]
    Transport {
        /// Transport-specific source error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// An HTTP response had a non-success status not mapped to a domain error.
    #[error("HTTP status {status} from {endpoint}")]
    HttpStatus {
        /// Numeric HTTP status.
        status: u16,
        /// Request endpoint.
        endpoint: String,
    },

    /// A request body could not be encoded.
    #[error("request body could not be encoded: {0}")]
    RequestEncode(serde_json::Error),

    /// A cookie could not be represented as an HTTP header.
    #[error("invalid HTTP header value: {0}")]
    InvalidHeader(http::header::InvalidHeaderValue),

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

impl StreamableError {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::EmailAlreadyInUse { .. } => "email_already_in_use",
            Self::InvalidCredentials { .. } => "invalid_credentials",
            Self::InvalidSession { .. } => "invalid_session",
            Self::ResourceInvalidated => "resource_invalidated",
            Self::PasswordValidation { .. } => "password_validation",
            Self::LabelAlreadyExists { .. } => "label_already_exists",
            Self::LabelNotFound { .. } => "label_not_found",
            Self::VideoLabelAssignmentFailed { .. } => "video_label_assignment_failed",
            Self::VideoPrivacyUpdateFailed { .. } => "video_privacy_update_failed",
            Self::VideoPrivacyResetFailed { .. } => "video_privacy_reset_failed",
            Self::VideoAnalyticsFailed { .. } => "video_analytics_failed",
            Self::VideoLiveViewsFailed { .. } => "video_live_views_failed",
            Self::CollectionNotFound { .. } => "collection_not_found",
            Self::CollectionCreationFailed { .. } => "collection_creation_failed",
            Self::CollectionCountFailed { .. } => "collection_count_failed",
            Self::CollectionListFailed { .. } => "collection_list_failed",
            Self::CollectionFetchFailed { .. } => "collection_fetch_failed",
            Self::CollectionUpdateFailed { .. } => "collection_update_failed",
            Self::CollectionDeletionFailed { .. } => "collection_deletion_failed",
            Self::RateLimitExceeded { .. } => "rate_limit_exceeded",
            Self::InvalidVideoFile { .. } => "invalid_video_file",
            Self::InvalidImageFile { .. } => "invalid_image_file",
            Self::InvalidThumbnailOffset { .. } => "invalid_thumbnail_offset",
            Self::VideoThumbnailUpdateFailed { .. } => "video_thumbnail_update_failed",
            Self::UploadSigning => "upload_signing",
            Self::UploadRollback { .. } => "upload_rollback",
            Self::UnexpectedVideoDeletionResponse { .. } => "unexpected_video_deletion_response",
            Self::Io(_) => "io",
            Self::Transport { .. } => "transport",
            Self::HttpStatus { .. } => "http_status",
            Self::RequestEncode(_) => "request_encode",
            Self::InvalidHeader(_) => "invalid_header",
            Self::ResponseDecode(_) => "response_decode",
            Self::UrlParse(_) => "url_parse",
        }
    }

    pub(crate) fn transport(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Transport {
            source: Box::new(error),
        }
    }
}
