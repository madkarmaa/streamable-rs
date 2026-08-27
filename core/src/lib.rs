#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod client;
mod constants;

#[cfg(test)]
mod test_tracing {
    #[ctor::ctor(unsafe)]
    fn install_subscriber() {
        if std::env::var_os("STREAMABLE_TEST_TRACING").is_some() {
            initialize_subscriber();
        }
    }

    #[tracing_test::traced_test]
    fn initialize_subscriber() {}
}

/// Errors returned by the client.
///
/// ```
/// use streamable::errors::{Result, StreamableError};
/// let result: Result<()> = Err(StreamableError::InvalidSession { message: "expired".into() });
/// assert!(result.is_err());
/// ```
pub mod errors;

/// Data sent to and returned by Streamable.
///
/// ```
/// use streamable::models::{VideoPrivacySettingsUpdate, Visibility};
///
/// let update = VideoPrivacySettingsUpdate {
///     visibility: Some(Visibility::Private),
///     ..Default::default()
/// };
/// ```
pub mod models;

/// Raw API response access for custom decoders.
///
/// ```no_run
/// use streamable::ApiResponse;
/// fn status(response: &ApiResponse) { println!("{}", response.status()); }
/// ```
pub mod response;

/// Runtime-neutral HTTP transport contract and default reqwest adapter.
///
/// ```
/// use streamable::transport::Body;
///
/// let body = Body::Empty;
/// assert!(matches!(body, Body::Empty));
/// ```
pub mod transport;

/// Small helpers used by the client.
///
/// ```
/// let password = streamable::utils::generate_random_password();
/// assert!((8..=20).contains(&password.len()));
/// ```
pub mod utils;

pub use client::{
    Authenticated, AuthenticatedStreamableClient, Collection, CollectionDetails, CollectionPage,
    CollectionSummary, Label, Registration, StreamableClient, Unauthenticated,
    UnauthenticatedStreamableClient, Video, VideoUpload, VideoUploadHandle,
};
pub use errors::{Result, StreamableError};
pub use response::ApiResponse;
