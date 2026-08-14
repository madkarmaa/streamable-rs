use bytes::Bytes;
use reqwest::StatusCode;
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
    status_error: Option<reqwest::Error>,
}

impl ApiResponse {
    pub(crate) const fn new(
        status: StatusCode,
        endpoint: Url,
        body: Bytes,
        status_error: Option<reqwest::Error>,
    ) -> Self {
        Self {
            status,
            endpoint,
            body,
            status_error,
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
        if let Some(error) = self.status_error {
            return Err(StreamableError::Request(error));
        }

        Ok(serde_json::from_slice(&self.body)?)
    }

    pub(crate) fn into_empty(self) -> Result<()> {
        if let Some(error) = self.status_error {
            return Err(StreamableError::Request(error));
        }

        Ok(())
    }

    pub(crate) fn api_error(&self) -> Option<crate::models::ErrorResponse> {
        serde_json::from_slice(&self.body).ok()
    }
}
