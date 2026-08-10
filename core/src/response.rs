use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use url::Url;

use crate::errors::{Result, StreamableError};

/// HTTP response data passed to an [`ApiRequest`](crate::models::ApiRequest) decoder.
pub struct ApiResponse {
    status: StatusCode,
    endpoint: Url,
    body: Bytes,
    status_error: Option<reqwest::Error>,
}

impl ApiResponse {
    pub(crate) fn new(
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

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    pub fn json<T>(self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if let Some(error) = self.status_error {
            return Err(StreamableError::Request(error));
        }

        Ok(serde_json::from_slice(&self.body)?)
    }

    pub(crate) fn api_error(&self) -> Option<crate::models::ErrorResponse> {
        serde_json::from_slice(&self.body).ok()
    }
}
