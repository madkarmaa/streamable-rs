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
///
/// ```
/// use streamable::transport::NoDefaultTransport;
///
/// let transport: Option<NoDefaultTransport> = None;
/// assert!(transport.is_none());
/// ```
#[cfg(not(feature = "reqwest"))]
#[derive(Debug)]
pub enum NoDefaultTransport {}

/// Transport used by [`crate::StreamableClient`] when no transport type is specified.
///
/// ```
/// use streamable::transport::{DefaultTransport, ReqwestTransport};
///
/// let transport: DefaultTransport = ReqwestTransport::new()?;
/// # Ok::<(), streamable::transport::ReqwestTransportError>(())
/// ```
#[cfg(feature = "reqwest")]
pub type DefaultTransport = ReqwestTransport;

/// Placeholder default when the `reqwest` feature is disabled.
///
/// ```
/// use streamable::transport::DefaultTransport;
///
/// let transport: Option<DefaultTransport> = None;
/// assert!(transport.is_none());
/// ```
#[cfg(not(feature = "reqwest"))]
pub type DefaultTransport = NoDefaultTransport;

/// Request body understood by every transport.
///
/// ```
/// use bytes::Bytes;
/// use streamable::transport::Body;
///
/// let body = Body::Bytes(Bytes::from_static(b"{}"));
/// assert!(matches!(body, Body::Bytes(_)));
/// ```
#[derive(Debug)]
pub enum Body {
    /// No request body.
    Empty,
    /// Complete in-memory request body.
    Bytes(Bytes),
    /// File whose bytes must be streamed by the transport.
    File(PathBuf),
    /// One file encoded as a multipart form by the transport.
    MultipartFile(MultipartFile),
}

impl Body {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Bytes(_) => "bytes",
            Self::File(_) => "file",
            Self::MultipartFile(_) => "multipart_file",
        }
    }

    pub(crate) const fn in_memory_len(&self) -> Option<usize> {
        match self {
            Self::Bytes(bytes) => Some(bytes.len()),
            Self::Empty | Self::File(_) | Self::MultipartFile(_) => None,
        }
    }
}

/// File part for a single-file multipart form request.
///
/// ```
/// use std::path::PathBuf;
/// use streamable::transport::MultipartFile;
///
/// let file = MultipartFile {
///     field_name: "file".into(),
///     file_name: "thumbnail.png".into(),
///     media_type: "image/png".into(),
///     path: PathBuf::from("thumbnail.png"),
/// };
/// assert_eq!(file.field_name, "file");
/// ```
#[derive(Debug)]
pub struct MultipartFile {
    /// Form field name.
    pub field_name: String,
    /// Original file name sent in the part metadata.
    pub file_name: String,
    /// Media type sent in the part metadata.
    pub media_type: String,
    /// Local file whose bytes must be streamed.
    pub path: PathBuf,
}

/// Complete runtime-neutral HTTP request.
///
/// ```
/// use http::{HeaderMap, Method};
/// use streamable::transport::{Body, Request};
/// use url::Url;
///
/// let request = Request {
///     method: Method::GET,
///     url: Url::parse("https://example.com/video")?,
///     headers: HeaderMap::new(),
///     body: Body::Empty,
/// };
/// assert_eq!(request.method, Method::GET);
/// # Ok::<(), url::ParseError>(())
/// ```
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
///
/// ```
/// use bytes::Bytes;
/// use http::{HeaderMap, StatusCode};
/// use streamable::transport::Response;
///
/// let response = Response {
///     status: StatusCode::OK,
///     headers: HeaderMap::new(),
///     body: Bytes::from_static(b"true"),
/// };
/// assert_eq!(response.status, StatusCode::OK);
/// ```
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
///
/// ```no_run
/// use http::StatusCode;
/// use streamable::transport::{HttpTransport, Request};
///
/// async fn status<T: HttpTransport>(
///     transport: &T,
///     request: Request,
/// ) -> Result<StatusCode, T::Error> {
///     Ok(transport.execute(request).await?.status)
/// }
/// ```
pub trait HttpTransport: Send + Sync {
    /// Error returned when a request cannot be executed.
    type Error: Error + Send + Sync + 'static;

    /// Executes one HTTP request.
    ///
    /// ```no_run
    /// use streamable::transport::{HttpTransport, Request, Response};
    ///
    /// async fn send<T: HttpTransport>(
    ///     transport: &T,
    ///     request: Request,
    /// ) -> Result<Response, T::Error> {
    ///     transport.execute(request).await
    /// }
    /// ```
    fn execute(
        &self,
        request: Request,
    ) -> impl Future<Output = std::result::Result<Response, Self::Error>> + Send;
}
