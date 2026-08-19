//! Runtime-neutral HTTP request and response types.

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use std::{error::Error, future::Future, path::PathBuf};
use url::Url;

#[cfg(feature = "reqwest")]
mod reqwest;

#[cfg(feature = "reqwest")]
pub use reqwest::{ReqwestTransport, ReqwestTransportError};

/// Default transport placeholder when the `reqwest` feature is disabled.
///
/// Construct clients with [`crate::StreamableClient::with_transport`] in that configuration.
#[cfg(not(feature = "reqwest"))]
#[derive(Debug)]
pub enum NoDefaultTransport {}

/// Transport used by [`crate::StreamableClient`] when no transport type is specified.
#[cfg(feature = "reqwest")]
pub type DefaultTransport = ReqwestTransport;

/// Placeholder default when the `reqwest` feature is disabled.
#[cfg(not(feature = "reqwest"))]
pub type DefaultTransport = NoDefaultTransport;

/// Request body understood by every transport.
#[derive(Debug)]
pub enum Body {
    /// No request body.
    Empty,
    /// Complete in-memory request body.
    Bytes(Bytes),
    /// File whose bytes must be streamed by the transport.
    File(PathBuf),
}

/// Complete runtime-neutral HTTP request.
#[derive(Debug)]
pub struct Request {
    /// HTTP method.
    pub method: Method,
    /// Absolute request URL.
    pub url: Url,
    /// Request headers.
    pub headers: HeaderMap,
    /// Request body.
    pub body: Body,
}

/// Complete buffered HTTP response.
#[derive(Debug)]
pub struct Response {
    /// HTTP status.
    pub status: StatusCode,
    /// Response headers.
    pub headers: HeaderMap,
    /// Complete response body.
    pub body: Bytes,
}

/// Runtime-independent HTTP executor.
pub trait HttpTransport: Send + Sync {
    /// Error returned when a request cannot be executed.
    type Error: Error + Send + Sync + 'static;

    /// Executes one HTTP request.
    fn execute(
        &self,
        request: Request,
    ) -> impl Future<Output = std::result::Result<Response, Self::Error>> + Send;
}
