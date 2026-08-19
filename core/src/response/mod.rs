use bytes::Bytes;
use http::StatusCode;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use url::Url;

use crate::errors::{Result, StreamableError};

#[cfg(test)]
mod tests;

/// Status, URL, and body from an API response.
///
/// ```no_run
/// fn show(response: &streamable::ApiResponse) {
///     println!("{} {}", response.status(), response.endpoint());
/// }
/// ```
pub struct ApiResponse {
    status: StatusCode,
    endpoint: Url,
    body: Bytes,
}

impl ApiResponse {
    pub(crate) const fn new(status: StatusCode, endpoint: Url, body: Bytes) -> Self {
        Self {
            status,
            endpoint,
            body,
        }
    }

    #[must_use]
    /// Returns the HTTP response status.
    ///
    /// ```no_run
    /// fn ok(response: &streamable::ApiResponse) -> bool {
    ///     response.status().is_success()
    /// }
    /// ```
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    /// Returns the request URL.
    ///
    /// ```no_run
    /// fn host(response: &streamable::ApiResponse) -> Option<&str> {
    ///     response.endpoint().host_str()
    /// }
    /// ```
    pub const fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    #[must_use]
    /// Returns the body as text, replacing invalid UTF-8.
    ///
    /// ```no_run
    /// fn print(response: &streamable::ApiResponse) {
    ///     println!("{}", response.text());
    /// }
    /// ```
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    /// Reads the body as JSON after checking the HTTP status.
    ///
    /// ```no_run
    /// fn decode(response: streamable::ApiResponse) -> streamable::Result<serde_json::Value> {
    ///     response.json()
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an HTTP or JSON error.
    pub fn json<T>(self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.ensure_success()?;

        Ok(serde_json::from_slice(&self.body)?)
    }

    pub(crate) fn into_empty(self) -> Result<()> {
        self.ensure_success()
    }

    pub(crate) fn api_error(&self) -> Option<crate::models::ErrorResponse> {
        serde_json::from_slice(&self.body).ok()
    }

    pub(crate) fn ensure_success(&self) -> Result<()> {
        if self.status.is_success() {
            return Ok(());
        }

        Err(StreamableError::HttpStatus {
            status: self.status.as_u16(),
            endpoint: self.endpoint.to_string(),
        })
    }
}
