#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod client;
mod constants;
/// Error and result types returned by this crate.
pub mod errors;
/// Public request and response data models.
pub mod models;
/// HTTP response wrapper used by custom request decoders.
pub mod response;
/// Standalone helpers for credentials and video-file detection.
pub mod utils;

pub use client::{
    Authenticated, AuthenticatedStreamableClient, StreamableClient, Unauthenticated,
    UnauthenticatedStreamableClient, UploadCancellationToken,
};
pub use errors::{Result, StreamableError};
pub use response::ApiResponse;
