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
    #[tracing::instrument(level = "debug", err(level = "debug"))]
    pub fn new() -> Result<Self, ReqwestTransportError> {
        let transport = Self {
            client: reqwest::Client::builder().build()?,
        };
        tracing::debug!("created reqwest transport");
        Ok(transport)
    }
}

impl HttpTransport for ReqwestTransport {
    type Error = ReqwestTransportError;

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            http.method = %request.method,
            url = %request.url,
            request.body.kind = request.body.kind(),
            request.body.length = request.body.in_memory_len(),
        ),
        err(level = "debug")
    )]
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
            Body::MultipartFile(file_part) => {
                let file = tokio::fs::File::open(file_part.path).await?;
                let part = reqwest::multipart::Part::stream(reqwest::Body::from(file))
                    .file_name(file_part.file_name)
                    .mime_str(&file_part.media_type)?;
                builder.multipart(reqwest::multipart::Form::new().part(file_part.field_name, part))
            }
        };

        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await?;

        tracing::debug!(
            http.status = status.as_u16(),
            response.body.length = body.len(),
            "received HTTP response"
        );

        Ok(Response {
            status,
            headers,
            body,
        })
    }
}
