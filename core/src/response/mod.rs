use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use url::Url;

use crate::errors::{Result, StreamableError};

#[cfg(test)]
mod tests;

/// HTTP response data passed to an [`ApiRequest`](crate::models::ApiRequest) decoder.
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
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    /// Returns the effective request URL.
    pub const fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    #[must_use]
    /// Returns the response body as lossily decoded UTF-8.
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    /// Deserializes the response body after checking the HTTP status.
    ///
    /// # Errors
    ///
    /// Returns the stored transport error or a JSON decoding error.
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
