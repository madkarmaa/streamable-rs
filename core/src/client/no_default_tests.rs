use super::*;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingTransport {
    requests: Arc<Mutex<Vec<crate::transport::Request>>>,
}

impl HttpTransport for RecordingTransport {
    type Error = std::io::Error;

    fn execute(
        &self,
        request: crate::transport::Request,
    ) -> impl std::future::Future<
        Output = std::result::Result<crate::transport::Response, Self::Error>,
    > + Send {
        lock_unpoisoned(&self.requests).push(request);
        std::future::ready(Ok(crate::transport::Response {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(b"true"),
        }))
    }
}

#[test]
fn caller_supplied_transport_executes_without_default_features() {
    let transport = RecordingTransport::default();
    let requests = Arc::clone(&transport.requests);
    let client = StreamableClient::with_transport(transport);

    tokio_test::block_on(client.delete_video("custom"))
        .expect("custom transport response should decode");

    let requests = lock_unpoisoned(&requests);
    let request = requests
        .first()
        .expect("custom transport should receive one request");
    assert_eq!(request.method, http::Method::DELETE);
    assert_eq!(request.url.path(), "/api/v1/videos/custom");
    assert!(request.headers.is_empty());
    assert!(matches!(request.body, Body::Empty));
    drop(requests);
}
