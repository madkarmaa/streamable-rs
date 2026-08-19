use super::{Body, HttpTransport, Request, Response};
use thiserror::Error;

/// Errors returned by [`ReqwestTransport`].
#[derive(Debug, Error)]
pub enum ReqwestTransportError {
    /// Opening a file-backed request body failed.
    #[error(transparent)]
    File(#[from] std::io::Error),
    /// Sending a request or reading its response failed.
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

/// Default HTTP transport backed by reqwest and Tokio.
#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Builds a transport with reqwest's default client configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when reqwest cannot build its client.
    pub fn new() -> Result<Self, ReqwestTransportError> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
        })
    }
}

impl HttpTransport for ReqwestTransport {
    type Error = ReqwestTransportError;

    async fn execute(&self, request: Request) -> Result<Response, Self::Error> {
        let mut builder = self
            .client
            .request(request.method, request.url)
            .headers(request.headers);

        builder = match request.body {
            Body::Empty => builder,
            Body::Bytes(body) => builder.body(body),
            Body::File(path) => {
                let file = tokio::fs::File::open(path).await?;
                builder.body(reqwest::Body::from(file))
            }
        };

        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await?;

        Ok(Response {
            status,
            headers,
            body,
        })
    }
}
