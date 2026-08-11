use thiserror::Error;

/// Errors returned by the Streamable API client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StreamableError {
    /// Signup failed because the email is already registered.
    #[error("{message}")]
    EmailAlreadyInUse { message: String },

    /// Login failed because the email or password is incorrect.
    #[error("{message}")]
    InvalidCredentials { message: String },

    /// An authenticated operation failed because the session is missing or expired.
    #[error("{message}")]
    InvalidSession { message: String },

    /// Password validation failed.
    #[error("{message}")]
    PasswordValidation { message: String },

    /// Streamable rejected the request because the endpoint rate limit was exceeded.
    #[error("Rate limit exceeded for {endpoint}. Try again later.")]
    RateLimitExceeded { endpoint: String },

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

pub type Result<T> = std::result::Result<T, StreamableError>;
