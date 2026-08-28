use super::*;
use crate::{StreamableError, utils::*};

use serde_json::json;
use std::sync::Arc;
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::matchers::{body_bytes, body_json, header, query_param};
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::{Match, Request};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
static REMOTE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
static REMOTE_CLIENT: tokio::sync::OnceCell<tokio::sync::Mutex<AuthenticatedStreamableClient>> =
    tokio::sync::OnceCell::const_new();

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
fn remote_credentials() -> anyhow::Result<(String, String)> {
    let path = dotenvy::dotenv()
        .map_err(|_| anyhow::anyhow!("remote tests require a readable .env file"))?;
    let mut values = dotenvy::from_path_iter(path)
        .map_err(|_| anyhow::anyhow!("remote tests require a valid .env file"))?
        .collect::<std::result::Result<std::collections::HashMap<_, _>, _>>()
        .map_err(|_| anyhow::anyhow!("remote tests require valid EMAIL and PASSWORD entries"))?;
    let email = values
        .remove("EMAIL")
        .ok_or_else(|| anyhow::anyhow!("remote tests require EMAIL in .env"))?;
    let password = values
        .remove("PASSWORD")
        .ok_or_else(|| anyhow::anyhow!("remote tests require PASSWORD in .env"))?;
    anyhow::ensure!(!email.is_empty(), "remote test EMAIL must not be empty");
    anyhow::ensure!(
        !password.is_empty(),
        "remote test PASSWORD must not be empty"
    );
    Ok((email, password))
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
async fn remote_authenticated_client()
-> anyhow::Result<tokio::sync::MutexGuard<'static, AuthenticatedStreamableClient>> {
    let client = REMOTE_CLIENT
        .get_or_try_init(|| async {
            let (email, password) = remote_credentials()?;
            let client = StreamableClient::new()?.login(email, password).await?;
            Ok::<_, anyhow::Error>(tokio::sync::Mutex::new(client))
        })
        .await?;
    Ok(client.lock().await)
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct NoCookieHeader;

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
impl Match for NoCookieHeader {
    fn matches(&self, request: &Request) -> bool {
        !request.headers.contains_key("cookie")
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct NoQuery;

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
impl Match for NoQuery {
    fn matches(&self, request: &Request) -> bool {
        request.url.query().is_none()
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct MultipartScreenshot {
    file_name: String,
    media_type: &'static str,
    contents: Vec<u8>,
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
impl Match for MultipartScreenshot {
    fn matches(&self, request: &Request) -> bool {
        let Some(content_type) = request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };
        let Some(boundary) = content_type.strip_prefix("multipart/form-data; boundary=") else {
            return false;
        };
        let body_text = String::from_utf8_lossy(&request.body);
        let disposition = format!(
            "Content-Disposition: form-data; name=\"screenshot\"; filename=\"{}\"",
            self.file_name
        );
        let closing_boundary = format!("\r\n--{boundary}--\r\n");

        body_text.matches("Content-Disposition: form-data;").count() == 1
            && body_text.contains(&disposition)
            && body_text.contains(&format!("Content-Type: {}", self.media_type))
            && request
                .body
                .windows(self.contents.len())
                .filter(|window| *window == self.contents.as_slice())
                .count()
                == 1
            && request.body.ends_with(closing_boundary.as_bytes())
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn authenticated_user(email: &str) -> serde_json::Value {
    json!({
        "socket": "mock-socket",
        "total_plays": 0,
        "total_uploads": 0,
        "total_videos": 0,
        "id": 1,
        "user_name": email,
        "email": email,
        "date_added": 0.0,
        "color": "#000000",
        "bio": "",
        "restricted": false,
        "plan_name": "free",
        "plan_max_length": 600,
        "plan_max_size": 250.0,
        "privacy_settings": {
            "allow_download": true,
            "allow_sharing": true,
            "hide_view_count": false,
            "visibility": "public"
        }
    })
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn unauthenticated_user(
    socket: &str,
    total_plays: u32,
    total_uploads: u32,
    total_videos: u32,
) -> serde_json::Value {
    json!({
        "socket": socket,
        "total_plays": total_plays,
        "total_uploads": total_uploads,
        "total_videos": total_videos
    })
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
async fn mock_registration(server: &MockServer, email: &str) {
    Mock::given(method("POST"))
        .and(path("/users"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=mock-session; Path=/; HttpOnly")
                .set_body_json(authenticated_user(email)),
        )
        .expect(1)
        .mount(server)
        .await;
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
async fn mock_registration_with_credentials(server: &MockServer, email: &str, password: &str) {
    Mock::given(method("POST"))
        .and(path("/users"))
        .and(body_json(json!({
            "email": email,
            "password": password,
            "username": email,
            "verification_redirect": "https://streamable.com?alert=verified"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=mock-session; Path=/; HttpOnly")
                .set_body_json(authenticated_user(email)),
        )
        .expect(1)
        .mount(server)
        .await;
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
async fn mock_login(server: &MockServer, email: &str, password: &str) {
    Mock::given(method("POST"))
        .and(path("/check"))
        .and(NoCookieHeader)
        .and(body_json(json!({
            "username": email,
            "password": password
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=mock-session; Path=/; HttpOnly")
                .set_body_json(authenticated_user(email)),
        )
        .expect(1)
        .mount(server)
        .await;
}

async fn mock_json_error(
    server: &MockServer,
    request_path: &str,
    status: u16,
    error: &str,
    message: &str,
) {
    Mock::given(method("POST"))
        .and(path(request_path.to_string()))
        .respond_with(ResponseTemplate::new(status).set_body_json(json!({
            "error": error,
            "message": message
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn mock_client(server: &MockServer) -> Result<UnauthenticatedStreamableClient> {
    let base_url = Url::parse(&server.uri())?;

    StreamableClient::with_base_url(base_url)
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn mock_upload_client(server: &MockServer) -> Result<UnauthenticatedStreamableClient> {
    let base_url = Url::parse(&server.uri()).expect("mock server URI must be valid");

    StreamableClient::with_base_url(base_url)
}

fn media_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../media/videos")
        .join(name)
}

fn image_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../media/images")
        .join(name)
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn mock_thumbnail_video(
    shortcode: &str,
    offset: &str,
    dynamic_thumbnail_url: &str,
) -> serde_json::Value {
    let mut video = mock_video(shortcode, false);
    video["thumbnail_url"] = json!("https://cdn.example/image.jpg?Expires=123&Signature=opaque");
    video["dynamic_thumbnail_url"] = json!(dynamic_thumbnail_url);
    video["thumbnail_offset"] = json!(offset);
    video
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn mock_upload_info(video_size: u64) -> serde_json::Value {
    json!({
        "accelerated": false,
        "bucket": "streamables-upload",
        "credentials": {
            "accessKeyId": "AKIDEXAMPLE",
            "secretAccessKey": "secret",
            "sessionToken": "session-token"
        },
        "fields": {
            "key": "upload/mock",
            "bucket": "streamables-upload",
            "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
            "X-Amz-Credential": "AKIDEXAMPLE/20260812/us-east-1/s3/aws4_request",
            "X-Amz-Date": "20260812T100000Z",
            "X-Amz-Security-Token": "session-token",
            "Policy": "policy",
            "X-Amz-Signature": "server-signature"
        },
        "url": "https://s3.amazonaws.com/streamables-upload",
        "video": {
            "shortcode": "mock",
            "status": 0,
            "percent": 0,
            "date_added": 1,
            "url": "https://streamable.com/mock"
        },
        "options": { "preset": "mp4", "shortcode": "mock", "screenshot": true },
        "shortcode": "mock",
        "key": "upload/mock",
        "time": 1,
        "transcoder": null,
        "transcoder_options": {
            "key": "upload/mock",
            "token": "transcoder-token",
            "shortcode": "mock",
            "size": video_size
        }
    })
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn mock_video(shortcode: &str, is_custom: bool) -> serde_json::Value {
    json!({
        "shortcode": shortcode,
        "status": 2,
        "percent": 100,
        "date_added": 1,
        "url": format!("https://streamable.com/{shortcode}"),
        "original_name": "video.webm",
        "duration": 1.0,
        "width": 640,
        "height": 360,
        "privacy_settings": {
            "visibility": "hidden_on_streamable",
            "allow_download": false,
            "allow_sharing": true,
            "domain_restrictions": "off",
            "allowed_domain": "",
            "password_protected": false,
            "hide_view_count": false,
            "is_custom": is_custom
        }
    })
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn bound_mock_video<State, T>(
    client: &StreamableClient<State, T>,
    shortcode: &str,
) -> Video<State, T> {
    let data = serde_json::from_value(mock_video(shortcode, false))
        .expect("mock video should match the wire model");
    Video::new(Arc::clone(&client.core), data)
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn bound_mock_labeled_video<T>(
    client: &AuthenticatedStreamableClient<T>,
    shortcode: &str,
    label_ids: &[u64],
) -> Video<Authenticated, T> {
    let mut value = mock_video(shortcode, false);
    value["labels"] = label_ids
        .iter()
        .map(|id| json!({ "id": id }))
        .collect::<Vec<_>>()
        .into();
    let data = serde_json::from_value(value).expect("mock video should match the wire model");
    Video::new(Arc::clone(&client.core), data)
}

#[allow(clippy::panic)]
fn expect_streamable_error<T>(result: Result<T>, context: &str) -> StreamableError {
    match result {
        Ok(_) => panic!("{context}"),
        Err(error) => error,
    }
}

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
        let requests = Arc::clone(&self.requests);
        lock_unpoisoned(&requests).push(request);
        std::future::ready(Ok(crate::transport::Response {
            status: http::StatusCode::OK,
            headers: http::HeaderMap::new(),
            body: bytes::Bytes::from_static(b"true"),
        }))
    }
}

#[tracing_test::traced_test]
#[test]
fn debug_tracing_reports_request_lifecycle_without_sensitive_payloads() {
    let email = "private-email@example.test";
    let password = "private-password";

    tokio_test::block_on(async {
        let client = StreamableClient::with_transport(RecordingTransport::default());
        let result = client.login(email.to_string(), password.to_string()).await;
        assert!(
            result.is_err(),
            "fixture response should not decode as a user"
        );
    });

    assert!(logs_contain("streamable::models::LoginRequest"));
    assert!(logs_contain("http.method=POST"));
    assert!(logs_contain("request.body.kind=\"bytes\""));
    assert!(logs_contain("prepared API request"));
    assert!(logs_contain("received API response"));
    assert!(logs_contain("error.kind=\"response_decode\""));
    assert!(!logs_contain(email));
    assert!(!logs_contain(password));
    assert!(!logs_contain("username"));
}

#[tokio::test]
async fn test_api_client_initialization() {
    let client = StreamableClient::new().expect("client should initialize");

    assert!(!client.is_authenticated());
    assert_eq!(client.user(), None);
}

#[tokio::test]
async fn caller_supplied_transport_receives_runtime_neutral_request() {
    let transport = RecordingTransport::default();
    let requests = Arc::clone(&transport.requests);
    let client = StreamableClient::with_transport(transport);

    client
        .delete_video("custom")
        .await
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

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_upload_follows_live_web_request_order_and_wire_shapes() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let video_bytes = std::fs::read(&video_path).expect("video fixture should be readable");
    let video_size = u64::try_from(video_bytes.len()).expect("fixture length should fit u64");

    Mock::given(method("GET"))
        .and(path("/api/v1/uploads/shortcode"))
        .and(query_param("size", video_size.to_string()))
        .and(query_param("version", "unknown"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload_info(video_size)))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/initialize"))
        .and(body_json(json!({
            "original_size": video_size,
            "original_name": "webm.webm",
            "upload_source": "web",
            "title": "A custom title"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/mock"))
        .and(header("content-type", "application/octet-stream"))
        .and(header("content-length", video_size.to_string()))
        .and(header("x-amz-content-sha256", "UNSIGNED-PAYLOAD"))
        .and(header("x-amz-security-token", "session-token"))
        .and(header("x-amz-user-agent", "aws-sdk-js/2.1530.0 callback"))
        .and(body_bytes(video_bytes))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/transcode/mock"))
        .and(body_json(json!({
            "upload_source": "web",
            "key": "upload/mock",
            "token": "transcoder-token",
            "shortcode": "mock",
            "size": video_size
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "mock",
            "status": 1,
            "percent": 0,
            "date_added": 1,
            "url": "https://streamable.com/mock",
            "original_name": "webm.webm",
            "duration": null,
            "width": null,
            "height": null
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let video = client
        .upload_video(&video_path, Some("A custom title".to_string()))
        .await
        .expect("video upload should complete through transcoding");

    assert_eq!(video.shortcode, "mock");
    assert_eq!(video.status, 1);
    assert_eq!(video.percent, 0);

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let request_order = requests
        .iter()
        .map(|request| (request.method.as_str(), request.url.path()))
        .collect::<Vec<_>>();
    assert_eq!(
        request_order,
        [
            ("GET", "/api/v1/uploads/shortcode"),
            ("POST", "/api/v1/videos/mock/initialize"),
            ("PUT", "/upload/mock"),
            ("POST", "/api/v1/transcode/mock"),
        ]
    );
    assert!(requests[0].body.is_empty());
    let s3_request = requests.get(2).expect("S3 PUT should be third request");
    let authorization = s3_request
        .headers
        .get("authorization")
        .expect("S3 authorization header should be present")
        .to_str()
        .expect("S3 authorization header should be ASCII");
    assert!(authorization.contains(
        "SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent"
    ));
    assert!(!s3_request.headers.contains_key("x-amz-acl"));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_upload_defaults_title_to_file_stem() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let video_size = std::fs::metadata(&video_path)
        .expect("video fixture should exist")
        .len();

    Mock::given(method("GET"))
        .and(path("/api/v1/uploads/shortcode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload_info(video_size)))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/initialize"))
        .and(body_json(json!({
            "original_size": video_size,
            "original_name": "webm.webm",
            "upload_source": "web",
            "title": "webm"
        })))
        .respond_with(ResponseTemplate::new(400))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/cancel"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let error = client
        .upload_video(video_path, None)
        .await
        .expect_err("mock initialization failure should stop the upload");
    assert!(matches!(
        error,
        StreamableError::HttpStatus { status: 400, .. }
    ));
    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    assert_eq!(
        requests
            .iter()
            .map(|request| request.url.path())
            .collect::<Vec<_>>(),
        [
            "/api/v1/uploads/shortcode",
            "/api/v1/videos/mock/initialize",
            "/api/v1/videos/mock/cancel",
        ]
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_upload_s3_failure_cancels_initialized_upload() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let video_size = std::fs::metadata(&video_path)
        .expect("video fixture should exist")
        .len();
    Mock::given(method("GET"))
        .and(path("/api/v1/uploads/shortcode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload_info(video_size)))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/initialize"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/mock"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/cancel"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let upload = client
        .begin_video_upload(video_path, None)
        .await
        .expect("upload should allocate a shortcode");
    assert_eq!(upload.shortcode(), "mock");
    let error = upload
        .complete()
        .await
        .expect_err("S3 failure should stop before transcoding");

    assert!(matches!(
        error,
        StreamableError::HttpStatus { status: 500, .. }
    ));
    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    assert_eq!(
        requests
            .iter()
            .map(|request| (request.method.as_str(), request.url.path()))
            .collect::<Vec<_>>(),
        [
            ("GET", "/api/v1/uploads/shortcode"),
            ("POST", "/api/v1/videos/mock/initialize"),
            ("PUT", "/upload/mock"),
            ("POST", "/api/v1/videos/mock/cancel"),
        ]
    );
    let cancel_request = requests
        .last()
        .expect("cancellation request should be recorded");
    assert!(cancel_request.body.is_empty());
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn allocated_upload_handle_cancels_before_initialization() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let video_size = std::fs::metadata(&video_path)
        .expect("video fixture should exist")
        .len();
    Mock::given(method("GET"))
        .and(path("/api/v1/uploads/shortcode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload_info(video_size)))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/mock/cancel"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let upload = client
        .begin_video_upload(video_path, None)
        .await
        .expect("upload should allocate a shortcode");
    let handle = upload.handle();
    assert_eq!(handle.shortcode(), "mock");
    let cancel = handle.clone();
    drop(handle);
    drop(upload);
    cancel
        .cancel()
        .await
        .expect("explicit cancellation should succeed");

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    assert_eq!(
        requests
            .iter()
            .map(|request| (request.method.as_str(), request.url.path()))
            .collect::<Vec<_>>(),
        [
            ("GET", "/api/v1/uploads/shortcode"),
            ("POST", "/api/v1/videos/mock/cancel"),
        ]
    );
    assert!(
        requests
            .last()
            .expect("cancel request should be recorded")
            .body
            .is_empty()
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_upload_rejects_non_video_before_network_access() {
    let mock_server = MockServer::start().await;
    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let error = client
        .upload_video(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
            None,
        )
        .await
        .expect_err("non-video file should be rejected");

    assert!(matches!(error, StreamableError::InvalidVideoFile { .. }));
    assert!(
        mock_server
            .received_requests()
            .await
            .expect("mock server should record requests")
            .is_empty()
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_upload_maps_shortcode_rate_limit_before_mutation() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let video_size = std::fs::metadata(&video_path)
        .expect("video fixture should exist")
        .len();
    Mock::given(method("GET"))
        .and(path("/api/v1/uploads/shortcode"))
        .and(query_param("size", video_size.to_string()))
        .and(query_param("version", "unknown"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let error = client
        .upload_video(video_path, None)
        .await
        .expect_err("rate-limited upload should fail");

    assert!(matches!(error, StreamableError::RateLimitExceeded { .. }));
    assert_eq!(
        mock_server
            .received_requests()
            .await
            .expect("mock server should record requests")
            .len(),
        1
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn unauthenticated_delete_video_sends_bodyless_request_and_accepts_only_literal_true() {
    let mock_server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/abc123"))
        .and(NoCookieHeader)
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");

    client
        .delete_video("abc123")
        .await
        .expect("literal true should confirm video deletion");

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let delete_request = requests
        .iter()
        .find(|request| request.method.as_str() == "DELETE")
        .expect("delete request should be recorded");
    assert!(delete_request.body.is_empty());
    assert!(!delete_request.headers.contains_key("content-type"));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn delete_video_rejects_non_literal_success_bodies() {
    let mock_server = MockServer::start().await;
    let responses = [
        ("false-body", 200, "false"),
        ("json-string", 200, "\"true\""),
        ("empty-body", 204, ""),
    ];

    for (shortcode, status, body) in responses {
        Mock::given(method("DELETE"))
            .and(path(format!("/api/v1/videos/{shortcode}")))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .expect(1)
            .mount(&mock_server)
            .await;
    }

    let client = mock_client(&mock_server).expect("mock client should initialize");

    for (shortcode, _, expected_body) in responses {
        let error = client
            .delete_video(shortcode)
            .await
            .expect_err("non-literal success response should fail");
        assert!(matches!(
            error,
            StreamableError::UnexpectedVideoDeletionResponse {
                shortcode: ref actual_shortcode,
                response: ref actual_body,
            } if actual_shortcode == shortcode && actual_body == expected_body
        ));
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn delete_video_preserves_common_and_http_errors() {
    let mock_server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/expired"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "InvalidSessionError",
            "message": "Session expired"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/rate-limited"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");

    let session_error = client
        .delete_video("expired")
        .await
        .expect_err("expired session should fail");
    assert!(matches!(
        session_error,
        StreamableError::InvalidSession { ref message } if message == "Session expired"
    ));

    let status_error = client
        .delete_video("missing")
        .await
        .expect_err("ordinary HTTP error should fail");
    assert!(matches!(
        status_error,
        StreamableError::HttpStatus { status: 404, .. }
    ));

    let rate_limit_error = client
        .delete_video("rate-limited")
        .await
        .expect_err("rate-limited deletion should fail");
    assert!(matches!(
        rate_limit_error,
        StreamableError::RateLimitExceeded { ref endpoint }
            if endpoint.ends_with("/api/v1/videos/rate-limited")
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn delete_video_propagates_transport_errors() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("an unused local port should be available");
    let address = listener
        .local_addr()
        .expect("unused local port should have an address");
    drop(listener);
    let base_url =
        Url::parse(&format!("http://{address}")).expect("unused local address should form a URL");
    let client = StreamableClient::with_base_url(base_url).expect("mock client should initialize");

    let error = client
        .delete_video("transport")
        .await
        .expect_err("transport failure should propagate");

    assert!(matches!(error, StreamableError::Transport { .. }));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_unauthenticated_video_deletion_is_observable() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = StreamableClient::new().expect("anonymous remote client should initialize");
    let video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");

    client
        .delete_video(&video.shortcode)
        .await
        .expect("remote video deletion should succeed");
    let deleted = client
        .get_video(&video.shortcode)
        .await
        .expect_err("deleted remote video should not remain readable");

    assert!(matches!(
        deleted,
        StreamableError::HttpStatus { status: 404, .. }
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn set_video_thumbnail_frame_patches_exact_offset_and_decodes_video() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/screenshots/abc123"))
        .and(NoQuery)
        .and(header("cookie", "session=mock-session"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({ "thumbOffset": 12.5 })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_thumbnail_video(
                "abc123",
                "12.5",
                "//cdn.example/abc123-screenshot.jpg",
            )),
        )
        .expect(1)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_video(registration.client(), "abc123");

    video
        .set_thumbnail_frame(12.5)
        .await
        .expect("frame thumbnail update should succeed");

    assert_eq!(video.thumbnail_offset.as_deref(), Some("12.5"));
    assert_eq!(
        video.dynamic_thumbnail_url.as_deref(),
        Some("//cdn.example/abc123-screenshot.jpg")
    );
    assert_eq!(
        video.thumbnail_url.as_deref(),
        Some("https://cdn.example/image.jpg?Expires=123&Signature=opaque")
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn upload_video_thumbnail_posts_one_streamed_screenshot_file() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    let image_file = image_path("png.png");
    let image_contents = std::fs::read(&image_file).expect("PNG fixture should be readable");
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/screenshots/abc123/upload"))
        .and(NoQuery)
        .and(header("cookie", "session=mock-session"))
        .and(MultipartScreenshot {
            file_name: "png.png".to_string(),
            media_type: "image/png",
            contents: image_contents,
        })
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_thumbnail_video(
                "abc123",
                "-1",
                "//cdn.example/upload-abc123.jpg",
            )),
        )
        .expect(1)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_video(registration.client(), "abc123");

    video
        .upload_thumbnail(&image_file)
        .await
        .expect("custom thumbnail upload should succeed");

    assert_eq!(video.thumbnail_offset.as_deref(), Some("-1"));
    assert_eq!(
        video.dynamic_thumbnail_url.as_deref(),
        Some("//cdn.example/upload-abc123.jpg")
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_thumbnail_inputs_are_validated_before_requests() {
    let mock_server = MockServer::start().await;
    let client = mock_client(&mock_server).expect("mock client should initialize");
    let mut video = bound_mock_video(&client, "abc123");

    for seconds in [-1.0, f64::NAN, f64::INFINITY] {
        let error = video
            .set_thumbnail_frame(seconds)
            .await
            .expect_err("invalid thumbnail offset should be rejected");
        assert!(matches!(
            error,
            StreamableError::InvalidThumbnailOffset { .. }
        ));
    }

    let error = video
        .upload_thumbnail(media_path("webm.webm"))
        .await
        .expect_err("video content should be rejected as a thumbnail image");
    assert!(matches!(error, StreamableError::InvalidImageFile { .. }));
    assert!(
        mock_server
            .received_requests()
            .await
            .expect("mock server should record requests")
            .is_empty(),
        "invalid inputs must not reach Streamable"
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_thumbnail_operations_map_endpoint_and_common_errors() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    let image_file = image_path("png.png");
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/screenshots/rejected"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "message": "Offset exceeds video duration"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/screenshots/rejected/upload"))
        .respond_with(ResponseTemplate::new(415).set_body_json(json!({
            "message": "Unsupported image"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/screenshots/expired"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "message": "Session expired"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/screenshots/rate-limited/upload"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut rejected = bound_mock_video(registration.client(), "rejected");
    let mut expired = bound_mock_video(registration.client(), "expired");
    let mut rate_limited = bound_mock_video(registration.client(), "rate-limited");

    let frame_error = expect_streamable_error(
        rejected.set_thumbnail_frame(100.0).await,
        "rejected frame thumbnail should fail",
    );
    let upload_error = expect_streamable_error(
        rejected.upload_thumbnail(&image_file).await,
        "rejected custom thumbnail should fail",
    );
    let session_error = expect_streamable_error(
        expired.set_thumbnail_frame(1.0).await,
        "expired thumbnail session should fail",
    );
    let rate_limit_error = expect_streamable_error(
        rate_limited.upload_thumbnail(&image_file).await,
        "rate-limited thumbnail upload should fail",
    );

    assert!(matches!(
        frame_error,
        StreamableError::VideoThumbnailUpdateFailed {
            ref shortcode,
            status: 422,
            ref message
        } if shortcode == "rejected" && message == "Offset exceeds video duration"
    ));
    assert!(matches!(
        upload_error,
        StreamableError::VideoThumbnailUpdateFailed {
            ref shortcode,
            status: 415,
            ref message
        } if shortcode == "rejected" && message == "Unsupported image"
    ));
    assert!(matches!(
        session_error,
        StreamableError::InvalidSession { ref message } if message == "Session expired"
    ));
    assert!(matches!(
        rate_limit_error,
        StreamableError::RateLimitExceeded { ref endpoint }
            if endpoint.ends_with("/api/v1/screenshots/rate-limited/upload")
    ));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_upload_reaches_transcoding() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");
    drop(client);

    assert_eq!(
        video.url,
        format!("https://streamable.com/{}", video.shortcode)
    );
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_upload_cancellation_is_accepted() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let video_path = media_path("webm.webm");
    let size = std::fs::metadata(&video_path)
        .expect("video fixture should exist")
        .len();
    let upload_info = client
        .execute(&models::ShortcodeRequest::new(size))
        .await
        .expect("remote shortcode request should succeed");
    client
        .execute(&models::InitializeVideoUploadRequest::new(
            &upload_info.shortcode,
            size,
            "webm.webm".to_string(),
            "webm".to_string(),
        ))
        .await
        .expect("remote initialization should succeed");

    client
        .cancel_video_upload(&upload_info.shortcode)
        .await
        .expect("remote cancellation should succeed");
    drop(client);
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_thumbnail_branches_are_reversible() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let mut video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");
    let image_file = image_path("png.png");

    video
        .upload_thumbnail(&image_file)
        .await
        .expect("remote custom thumbnail upload should succeed");
    assert_eq!(video.thumbnail_offset.as_deref(), Some("-1"));
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    video
        .set_thumbnail_frame(0.5)
        .await
        .expect("remote frame thumbnail restore should succeed");
    drop(client);

    assert_ne!(video.thumbnail_offset.as_deref(), Some("-1"));
}

#[test]
fn configured_base_url_is_stored() {
    let base_url = Url::parse("http://api.example.test").expect("mock URL should be valid");
    let client =
        StreamableClient::with_base_url(base_url.clone()).expect("client should initialize");

    assert!(matches!(
        client.core.endpoint_routing,
        EndpointRouting::Override(ref url) if url == &base_url
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn unauthenticated_client_refreshes_basic_user_data() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/me"))
        .and(NoCookieHeader)
        .respond_with(
            ResponseTemplate::new(200).set_body_json(unauthenticated_user("anonymous", 12, 4, 3)),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let mut client = mock_client(&mock_server).expect("mock client should initialize");
    let expected_user = models::UnauthenticatedUser {
        socket: "anonymous".to_string(),
        total_plays: 12,
        total_uploads: 4,
        total_videos: 3,
    };
    {
        let user = client
            .refresh_user()
            .await
            .expect("unauthenticated user refresh should succeed");
        assert_eq!(user, &expected_user);
    }
    assert_eq!(client.user(), Some(&expected_user));

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let me_request = requests
        .iter()
        .find(|request| request.method.as_str() == "GET")
        .expect("me request should be recorded");
    assert!(me_request.body.is_empty());
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn authenticated_client_refreshes_full_user_data() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;

    let mut refreshed_user = authenticated_user(email);
    refreshed_user["total_plays"] = json!(42);
    refreshed_user["total_uploads"] = json!(8);
    refreshed_user["total_videos"] = json!(7);
    refreshed_user["bio"] = json!("refreshed");
    Mock::given(method("GET"))
        .and(path("/api/v1/me"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed_user))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mut client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let user = client
        .refresh_user()
        .await
        .expect("authenticated user refresh should succeed");

    assert_eq!(user.total_plays, 42);
    assert_eq!(user.total_uploads, 8);
    assert_eq!(user.total_videos, 7);
    assert_eq!(user.unauthenticated.total_videos, 7);
    assert_eq!(user.bio, "refreshed");
    assert_eq!(client.user().bio, "refreshed");
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_me_refreshes_both_user_states() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let mut client = StreamableClient::new().expect("client should initialize");

    assert!(client.user().is_none());
    let unauthenticated_user = client
        .refresh_user()
        .await
        .expect("remote unauthenticated user refresh should succeed");
    assert!(!unauthenticated_user.socket.is_empty());
    assert!(client.user().is_some());

    let (email, _) = remote_credentials().expect("remote credentials should load");
    let mut client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let authenticated_user = client
        .refresh_user()
        .await
        .expect("remote authenticated user refresh should succeed");

    assert_eq!(authenticated_user.email, email);
    drop(client);
}

#[tokio::test]
async fn test_successful_random_registration() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let client = StreamableClient::new().expect("client should initialize");

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let mock_server = MockServer::start().await;

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    mock_registration(&mock_server, "generated-user@example.com").await;

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let registration = client
        .register(None, None, None)
        .await
        .expect("registration should succeed");
    let email = registration.email().to_owned();
    let password = registration.password().to_owned();

    assert!(!email.is_empty());
    assert!(!password.is_empty());
    assert!(registration.is_authenticated());

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    {
        let requests = mock_server
            .received_requests()
            .await
            .expect("mock server should record requests");
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("request body should be JSON");

        assert_eq!(body["email"], email);
        assert_eq!(body["password"], password);
        assert_eq!(body["username"], email);
    }
}

#[tokio::test]
async fn test_successful_registration_and_login() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;

    let email = generate_random_username();
    let password = generate_random_password();

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let mock_server = MockServer::start().await;

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    mock_registration_with_credentials(&mock_server, &email, &password).await;

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    mock_login(&mock_server, &email, &password).await;

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let registration_client = StreamableClient::new().expect("client should initialize");

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let registration_client = mock_client(&mock_server).expect("mock client should initialize");

    let registration = registration_client
        .register(Some(email.clone()), Some(password.clone()), None)
        .await
        .expect("registration should succeed");
    let returned_email = registration.email().to_owned();
    let returned_password = registration.password().to_owned();

    assert_eq!(returned_email, email);
    assert_eq!(returned_password, password);
    assert_eq!(registration.user().email, email);
    assert!(registration.is_authenticated());

    let login_client = registration.logout();

    assert!(!login_client.is_authenticated());

    let logged_in_client = login_client
        .login(returned_email, returned_password)
        .await
        .expect("login should succeed");

    assert_eq!(logged_in_client.user().email, email);
    assert!(logged_in_client.is_authenticated());
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
async fn remote_change_password_flow() -> anyhow::Result<()> {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let (email, current_password) = remote_credentials()?;
    let mut new_password = generate_random_password();
    while new_password == current_password {
        new_password = generate_random_password();
    }
    let mut wrong_password = generate_random_password();
    while wrong_password == current_password {
        wrong_password = generate_random_password();
    }

    let client = remote_authenticated_client().await?;

    let error = expect_streamable_error(
        client.change_password(&wrong_password, &new_password).await,
        "wrong current password should fail",
    );
    assert!(matches!(
        error,
        StreamableError::InvalidCredentials { ref message }
            if message == "Current password is incorrect."
    ));

    let error = expect_streamable_error(
        client.change_password(&current_password, "weak").await,
        "weak new password should fail",
    );
    assert!(matches!(
        error,
        StreamableError::PasswordValidation { ref message }
            if message.starts_with("Password must ")
    ));

    client
        .change_password(&current_password, &new_password)
        .await?;

    client
        .change_password(&new_password, &current_password)
        .await?;
    assert_eq!(client.user().email, email);
    drop(client);
    Ok(())
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
async fn mock_change_password_responses(
    server: &MockServer,
    current_password: &str,
    new_password: &str,
    wrong_password: &str,
    validation_message: &str,
) {
    let responses = [
        (
            wrong_password,
            new_password,
            200,
            json!({
                "error": "AuthError",
                "message": "An error occurred while changing your password. Please try again."
            }),
        ),
        (
            current_password,
            "weak",
            400,
            json!({
                "error": "ValidationError",
                "message": validation_message
            }),
        ),
        (
            current_password,
            new_password,
            200,
            json!({ "message": "Password changed" }),
        ),
    ];

    for (provided_password, requested_password, status, response) in responses {
        Mock::given(method("POST"))
            .and(path("/me/change_password"))
            .and(header("cookie", "session=mock-session"))
            .and(body_json(json!({
                "session": "mock-session",
                "current_password": provided_password,
                "new_password": requested_password
            })))
            .respond_with(ResponseTemplate::new(status).set_body_json(response))
            .expect(1)
            .mount(server)
            .await;
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
async fn mocked_change_password_flow() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let current_password = "Password1";
    let new_password = "NewPassword2";
    let wrong_password = "WrongPassword1";
    let validation_message = "Password must be at least 8 characters, and contain at least one uppercase letter (A-Z), one lowercase letter (a-z), and one number (0-9).";

    mock_registration_with_credentials(&mock_server, email, current_password).await;
    mock_change_password_responses(
        &mock_server,
        current_password,
        new_password,
        wrong_password,
        validation_message,
    )
    .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(
            Some(email.to_string()),
            Some(current_password.to_string()),
            None,
        )
        .await
        .expect("registration should succeed");

    let error = expect_streamable_error(
        client.change_password(wrong_password, new_password).await,
        "wrong current password should fail",
    );
    assert!(matches!(
        error,
        StreamableError::InvalidCredentials { ref message }
            if message == "Current password is incorrect."
    ));

    let error = expect_streamable_error(
        client.change_password(current_password, "weak").await,
        "weak new password should fail",
    );
    assert!(matches!(
        error,
        StreamableError::PasswordValidation { ref message }
            if message == validation_message
    ));

    client
        .change_password(current_password, new_password)
        .await
        .expect("password change should succeed");
}

#[tokio::test]
async fn change_password_uses_session_cookie_and_changes_credentials() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    remote_change_password_flow()
        .await
        .expect("remote password flow should restore shared credentials");

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    mocked_change_password_flow().await;
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn create_label_posts_trimmed_name_and_returns_label() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "name": "important" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "name": "important",
            "id": 174_172
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let label = client
        .create_label("  important  ")
        .await
        .expect("label creation should succeed");

    assert_eq!(
        label.data(),
        &models::Label {
            name: "important".to_string(),
            id: 174_172
        }
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn create_label_reports_duplicate_name() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/labels"))
        .and(body_json(json!({ "name": "important" })))
        .respond_with(ResponseTemplate::new(409))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let error = expect_streamable_error(
        client.create_label("important").await,
        "duplicate label creation should fail",
    );

    assert!(matches!(
        error,
        StreamableError::LabelAlreadyExists { ref name } if name == "important"
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn delete_label_sends_bodyless_request() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/labels/174172"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    client
        .delete_label(174_172)
        .await
        .expect("label deletion should succeed");

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let delete_request = requests
        .iter()
        .find(|request| request.method.as_str() == "DELETE")
        .expect("delete request should be recorded");
    assert!(delete_request.body.is_empty());
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn delete_label_reports_missing_id() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/labels/696969"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let error = expect_streamable_error(
        client.delete_label(696_969).await,
        "missing label deletion should fail",
    );

    assert!(matches!(
        error,
        StreamableError::LabelNotFound { id: 696_969 }
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn rename_label_patches_trimmed_name_and_returns_label() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/labels/174172"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "name": "renamed" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 174_172,
            "name": "renamed"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let label = client
        .rename_label(174_172, "  renamed  ")
        .await
        .expect("label rename should succeed");

    assert_eq!(
        label.data(),
        &models::Label {
            id: 174_172,
            name: "renamed".to_string()
        }
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn rename_label_reports_missing_id() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/labels/696969"))
        .and(body_json(json!({ "name": "renamed" })))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let error = expect_streamable_error(
        client.rename_label(696_969, "renamed").await,
        "missing label rename should fail",
    );

    assert!(matches!(
        error,
        StreamableError::LabelNotFound { id: 696_969 }
    ));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn set_video_labels_posts_ordered_absolute_replacement() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "labels": [42, 7, 18] })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    client
        .set_video_labels("abc123", &[42, 7, 18])
        .await
        .expect("video label replacement should succeed");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn set_video_labels_posts_empty_replacement() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "labels": [] })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    client
        .set_video_labels("abc123", &[])
        .await
        .expect("empty video label replacement should succeed");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_clear_labels_posts_empty_replacement_and_updates_snapshot() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "labels": [] })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_labeled_video(registration.client(), "abc123", &[42, 7]);

    video
        .clear_labels()
        .await
        .expect("video label clearing should succeed");

    assert!(video.labels.is_empty());
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_remove_labels_refreshes_filters_and_updates_snapshot() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    let mut refreshed = mock_video("abc123", false);
    refreshed["labels"] = json!([{ "id": 42 }, { "id": 7 }, { "id": 18 }]);
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "labels": [42, 18] })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_labeled_video(registration.client(), "abc123", &[1]);

    video
        .remove_labels(&[7, 999])
        .await
        .expect("selected video labels should be removed");

    let remaining_ids = video
        .labels
        .iter()
        .map(|label| label.id)
        .collect::<Vec<_>>();
    assert_eq!(remaining_ids, [42, 18]);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_remove_labels_skips_replacement_when_ids_are_absent() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    let mut refreshed = mock_video("abc123", false);
    refreshed["labels"] = json!([{ "id": 42 }, { "id": 7 }]);
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_labeled_video(registration.client(), "abc123", &[1]);

    video
        .remove_labels(&[999])
        .await
        .expect("absent video label ids should be ignored");

    let remaining_ids = video
        .labels
        .iter()
        .map(|label| label.id)
        .collect::<Vec<_>>();
    assert_eq!(remaining_ids, [42, 7]);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_remove_labels_with_empty_ids_makes_no_requests() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_video("abc123", false)))
        .expect(0)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/abc123/labels"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;
    let registration = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let mut video = bound_mock_labeled_video(registration.client(), "abc123", &[42]);

    video
        .remove_labels(&[])
        .await
        .expect("empty video label removal should be a no-op");

    assert_eq!(video.labels[0].id, 42);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn set_video_labels_maps_assignment_and_common_errors() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/rejected/labels"))
        .respond_with(ResponseTemplate::new(500).set_body_string("not json"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/expired/labels"))
        .respond_with(ResponseTemplate::new(401).set_body_string("not json"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/videos/rate-limited/labels"))
        .respond_with(ResponseTemplate::new(429).set_body_string("not json"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let rejected = expect_streamable_error(
        client.set_video_labels("rejected", &[7]).await,
        "rejected video label replacement should fail",
    );
    let expired = expect_streamable_error(
        client.set_video_labels("expired", &[7]).await,
        "expired video label replacement should fail",
    );
    let rate_limited = expect_streamable_error(
        client.set_video_labels("rate-limited", &[7]).await,
        "rate-limited video label replacement should fail",
    );

    assert!(matches!(
        rejected,
        StreamableError::VideoLabelAssignmentFailed {
            ref shortcode,
            status: 500
        } if shortcode == "rejected"
    ));
    assert!(matches!(expired, StreamableError::InvalidSession { .. }));
    assert!(matches!(
        rate_limited,
        StreamableError::RateLimitExceeded { .. }
    ));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_label_resources_can_be_assigned_and_cleared() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let mut video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");
    let label_name = format!("label-{}", generate_random_password());
    let label = client
        .create_label(&label_name)
        .await
        .expect("remote label creation should succeed");

    video
        .set_labels(&[label.id])
        .await
        .expect("remote video label assignment should succeed");
    video
        .remove_labels(&[label.id, u64::MAX])
        .await
        .expect("remote selected video label removal should succeed");
    assert!(video.labels.is_empty());
    video
        .clear_labels()
        .await
        .expect("remote video label clearing should succeed");
    assert!(video.labels.is_empty());
    drop(client);
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_label_lifecycle() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let name = format!("label-{}", generate_random_password());
    let renamed_name = format!("{name}-renamed");

    let created = client
        .create_label(&name)
        .await
        .expect("remote label creation should succeed");
    let renamed = client
        .rename_label(created.id, &renamed_name)
        .await
        .expect("remote label rename should succeed");
    drop(client);
    assert_eq!(created.name, name);
    assert_eq!(renamed.id, created.id);
    assert_eq!(renamed.name, renamed_name);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn collection_creation_works_without_a_session() {
    let mock_server = MockServer::start().await;
    let shortcodes = vec!["first".to_string(), "second".to_string()];
    Mock::given(method("POST"))
        .and(path("/api/v1/collections"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_json(json!({
            "shortcodes": ["first", "second"],
            "title": "Highlights"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "shortcode": "shared1",
            "title": "Highlights",
            "videos": [
                { "shortcode": "first", "title": "First", "plays": 0 },
                { "shortcode": "second", "title": "Second", "plays": 0 }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let created = client
        .create_collection(&shortcodes, Some("Highlights"))
        .await
        .expect("anonymous collection creation should succeed");

    assert_eq!(created.shortcode, "shared1");
    assert_eq!(created.title.as_deref(), Some("Highlights"));
    assert_eq!(created.videos[0].shortcode, "first");
    assert_eq!(created.videos[1].shortcode, "second");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn anonymous_collection_lifecycle_uses_exact_wire_contracts() {
    let mock_server = MockServer::start().await;
    let shortcodes = vec!["first".to_string(), "second".to_string()];
    let reversed = vec!["second".to_string(), "first".to_string()];

    Mock::given(method("POST"))
        .and(path("/api/v1/collections"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_json(json!({ "shortcodes": ["first", "second"] })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "shortcode": "shared1",
            "title": null,
            "videos": [
                { "shortcode": "first", "title": "First", "plays": 0 },
                { "shortcode": "second", "title": "Second", "plays": 0 }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/count"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 1 })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections"))
        .and(NoQuery)
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "collections": [{
                "shortcode": "shared1",
                "title": null,
                "created_at": "2026-08-13T10:00:00Z",
                "updated_at": "2026-08-13T10:00:00Z",
                "thumbnail_url": "https://cdn.example/thumbnail.jpg"
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "shared1",
            "title": null,
            "is_owner": true,
            "white_label": false,
            "show_streamable_brand": true,
            "videos": [
                {
                    "shortcode": "first",
                    "title": "First",
                    "plays": 0,
                    "date_added": "2026-08-13T10:00:00Z"
                },
                {
                    "shortcode": "second",
                    "title": "Second",
                    "plays": 0,
                    "date_added": "2026-08-13T10:01:00Z"
                }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_json(json!({ "title": "Highlights" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "shared1",
            "title": "Highlights",
            "videos": [
                { "shortcode": "first", "title": "First", "plays": 0 },
                { "shortcode": "second", "title": "Second", "plays": 0 }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_json(json!({ "shortcodes": ["second", "first"] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "shared1",
            "title": "Highlights",
            "videos": [
                { "shortcode": "second", "title": "Second", "plays": 0 },
                { "shortcode": "first", "title": "First", "plays": 0 }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");

    let created = client
        .create_collection(&shortcodes, None)
        .await
        .expect("collection creation should succeed");
    let count = client
        .count_collections()
        .await
        .expect("collection count should succeed");
    let page = client
        .list_collections(None, None)
        .await
        .expect("collection list should succeed");
    let details = client
        .get_collection("shared1")
        .await
        .expect("collection details should succeed");
    let titled = client
        .update_collection_title("shared1", "Highlights")
        .await
        .expect("collection title update should succeed");
    let reordered = client
        .replace_collection_videos("shared1", &reversed)
        .await
        .expect("collection membership replacement should succeed");
    client
        .delete_collection("shared1")
        .await
        .expect("collection deletion should accept an empty HTTP 200 body");

    assert_eq!(created.videos[0].shortcode, "first");
    assert_eq!(created.videos[1].shortcode, "second");
    assert_eq!(count, 1);
    assert_eq!(page.collections[0].shortcode, "shared1");
    assert!(details.is_owner);
    assert_eq!(details.videos[1].date_added, "2026-08-13T10:01:00Z");
    assert_eq!(titled.title.as_deref(), Some("Highlights"));
    assert_eq!(reordered.videos[0].shortcode, "second");
    assert_eq!(reordered.videos[1].shortcode, "first");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn public_collection_details_work_without_a_session() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/public1"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "public1",
            "title": "Public collection",
            "is_owner": false,
            "white_label": false,
            "show_streamable_brand": true,
            "videos": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let collection = client
        .get_collection("public1")
        .await
        .expect("public collection should be readable");

    assert!(!collection.is_owner);
    assert_eq!(collection.title.as_deref(), Some("Public collection"));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn collection_operations_map_endpoint_and_common_errors() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/collections"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "message": "At least two videos are required"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/count"))
        .respond_with(ResponseTemplate::new(500).set_body_string("count unavailable"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections"))
        .and(query_param("page", "9"))
        .and(query_param("count", "20"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "message": "list unavailable"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections"))
        .and(query_param("page", "10"))
        .and(query_param("count", "20"))
        .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/collections/rejected"))
        .respond_with(ResponseTemplate::new(502).set_body_string("fetch unavailable"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/collections/missing"))
        .and(body_json(json!({ "title": "Title" })))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/collections/rejected"))
        .and(body_json(json!({ "shortcodes": [] })))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "message": "membership rejected"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/collections/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/collections/rejected"))
        .respond_with(ResponseTemplate::new(500).set_body_string("delete unavailable"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let one_shortcode = vec!["only".to_string()];

    let creation = expect_streamable_error(
        client.create_collection(&one_shortcode, None).await,
        "rejected collection creation should fail",
    );
    let count = expect_streamable_error(
        client.count_collections().await,
        "rejected collection count should fail",
    );
    let list = expect_streamable_error(
        client.list_collections(Some(9), Some(20)).await,
        "rejected collection list should fail",
    );
    let expired = expect_streamable_error(
        client.list_collections(Some(10), Some(20)).await,
        "expired collection list should fail",
    );
    let missing_fetch = expect_streamable_error(
        client.get_collection("missing").await,
        "missing collection fetch should fail",
    );
    let rejected_fetch = expect_streamable_error(
        client.get_collection("rejected").await,
        "rejected collection fetch should fail",
    );
    let missing_update = expect_streamable_error(
        client.update_collection_title("missing", "Title").await,
        "missing collection update should fail",
    );
    let rejected_update = expect_streamable_error(
        client.replace_collection_videos("rejected", &[]).await,
        "rejected collection update should fail",
    );
    let missing_delete = expect_streamable_error(
        client.delete_collection("missing").await,
        "missing collection deletion should fail",
    );
    let rejected_delete = expect_streamable_error(
        client.delete_collection("rejected").await,
        "rejected collection deletion should fail",
    );

    assert!(matches!(
        creation,
        StreamableError::CollectionCreationFailed {
            status: 400,
            ref message
        } if message == "At least two videos are required"
    ));
    assert!(matches!(
        count,
        StreamableError::CollectionCountFailed {
            status: 500,
            ref message
        } if message == "count unavailable"
    ));
    assert!(matches!(
        list,
        StreamableError::CollectionListFailed {
            status: 503,
            ref message
        } if message == "list unavailable"
    ));
    assert!(matches!(expired, StreamableError::InvalidSession { .. }));
    assert!(matches!(
        missing_fetch,
        StreamableError::CollectionNotFound { ref shortcode } if shortcode == "missing"
    ));
    assert!(matches!(
        rejected_fetch,
        StreamableError::CollectionFetchFailed {
            ref shortcode,
            status: 502,
            ref message
        } if shortcode == "rejected" && message == "fetch unavailable"
    ));
    assert!(matches!(
        missing_update,
        StreamableError::CollectionNotFound { ref shortcode } if shortcode == "missing"
    ));
    assert!(matches!(
        rejected_update,
        StreamableError::CollectionUpdateFailed {
            ref shortcode,
            status: 422,
            ref message
        } if shortcode == "rejected" && message == "membership rejected"
    ));
    assert!(matches!(
        missing_delete,
        StreamableError::CollectionNotFound { ref shortcode } if shortcode == "missing"
    ));
    assert!(matches!(
        rejected_delete,
        StreamableError::CollectionDeletionFailed {
            ref shortcode,
            status: 500,
            ref message
        } if shortcode == "rejected" && message == "delete unavailable"
    ));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn remote_anonymous_collection_lifecycle() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = StreamableClient::new().expect("anonymous remote client should initialize");
    assert!(!client.is_authenticated());
    assert!(client.user().is_none());
    let mut video_shortcodes = Vec::new();
    let title = format!("anonymous-collection-{}", generate_random_password());

    let exercise_result: std::result::Result<(), String> = async {
        let initial_count = client
            .count_collections()
            .await
            .map_err(|error| format!("initial anonymous collection count failed: {error}"))?;
        client
            .list_collections(None, None)
            .await
            .map_err(|error| format!("initial anonymous collection list failed: {error}"))?;

        let first = client
            .upload_video(media_path("webm.webm"), Some(format!("{title}-first")))
            .await
            .map_err(|error| format!("first anonymous video upload failed: {error}"))?;
        video_shortcodes.push(first.shortcode.clone());
        let second = client
            .upload_video(media_path("webm.webm"), Some(format!("{title}-second")))
            .await
            .map_err(|error| format!("second anonymous video upload failed: {error}"))?;
        video_shortcodes.push(second.shortcode.clone());

        let created = client
            .create_collection(&video_shortcodes, None)
            .await
            .map_err(|error| format!("anonymous collection creation failed: {error}"))?;

        let count_after_create = client
            .count_collections()
            .await
            .map_err(|error| format!("post-create anonymous collection count failed: {error}"))?;
        if count_after_create != initial_count.saturating_add(1) {
            return Err(format!(
                "anonymous collection count did not increase from {initial_count} to {count_after_create}"
            ));
        }
        let page = client
            .list_collections(None, None)
            .await
            .map_err(|error| format!("post-create anonymous collection list failed: {error}"))?;
        if !page
            .collections
            .iter()
            .any(|collection| collection.shortcode == created.shortcode)
        {
            return Err("anonymous collection list omitted the created collection".to_string());
        }

        let titled = client
            .update_collection_title(&created.shortcode, &title)
            .await
            .map_err(|error| format!("anonymous collection title update failed: {error}"))?;
        if titled.title.as_deref() != Some(title.as_str()) {
            return Err("anonymous collection title response changed the title".to_string());
        }

        let reversed = video_shortcodes.iter().rev().cloned().collect::<Vec<_>>();
        let reordered = client
            .replace_collection_videos(&created.shortcode, &reversed)
            .await
            .map_err(|error| format!("anonymous collection reorder failed: {error}"))?;
        if reordered
            .videos
            .iter()
            .map(|video| &video.shortcode)
            .ne(reversed.iter())
        {
            return Err("anonymous collection update changed requested video order".to_string());
        }

        client
            .delete_collection(&created.shortcode)
            .await
            .map_err(|error| format!("anonymous collection deletion failed: {error}"))?;

        let count_after_delete = client
            .count_collections()
            .await
            .map_err(|error| format!("post-delete anonymous collection count failed: {error}"))?;
        if count_after_delete != initial_count {
            return Err(format!(
                "anonymous collection count did not return from {count_after_create} to {initial_count}"
            ));
        }

        Ok(())
    }
    .await;

    assert!(!client.is_authenticated());
    assert!(client.user().is_none());

    assert!(
        exercise_result.is_ok(),
        "{}",
        exercise_result
            .err()
            .as_deref()
            .unwrap_or("remote anonymous collection lifecycle failed")
    );
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn remote_collection_lifecycle_preserves_order_and_member_videos() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let mut video_shortcodes = Vec::new();
    let title = format!("collection-{}", generate_random_password());

    let exercise_result: std::result::Result<(), String> = async {
        let initial_count = client
            .count_collections()
            .await
            .map_err(|error| format!("initial collection count failed: {error}"))?;
        let first = client
            .upload_video(media_path("webm.webm"), Some(format!("{title}-first")))
            .await
            .map_err(|error| format!("first remote video upload failed: {error}"))?;
        video_shortcodes.push(first.shortcode.clone());
        let second = client
            .upload_video(media_path("webm.webm"), Some(format!("{title}-second")))
            .await
            .map_err(|error| format!("second remote video upload failed: {error}"))?;
        video_shortcodes.push(second.shortcode.clone());

        let created = client
            .create_collection(&video_shortcodes, None)
            .await
            .map_err(|error| format!("remote collection creation failed: {error}"))?;
        if created
            .videos
            .iter()
            .map(|video| &video.shortcode)
            .ne(video_shortcodes.iter())
        {
            return Err("remote collection creation changed video order".to_string());
        }

        let count_after_create = client
            .count_collections()
            .await
            .map_err(|error| format!("post-create collection count failed: {error}"))?;
        if count_after_create != initial_count.saturating_add(1) {
            return Err(format!(
                "collection count did not increase from {initial_count} to {count_after_create}"
            ));
        }
        client
            .list_collections(None, None)
            .await
            .map_err(|error| format!("remote collection list failed: {error}"))?;
        let details = client
            .get_collection(&created.shortcode)
            .await
            .map_err(|error| format!("remote collection detail failed: {error}"))?;
        if !details.is_owner
            || details
                .videos
                .iter()
                .map(|video| &video.shortcode)
                .ne(video_shortcodes.iter())
        {
            return Err("remote collection details changed ownership or video order".to_string());
        }

        let titled = client
            .update_collection_title(&created.shortcode, &title)
            .await
            .map_err(|error| format!("remote collection title update failed: {error}"))?;
        if titled.title.as_deref() != Some(title.as_str()) {
            return Err("remote collection title response did not preserve the title".to_string());
        }

        let reversed = video_shortcodes.iter().rev().cloned().collect::<Vec<_>>();
        let reordered = client
            .replace_collection_videos(&created.shortcode, &reversed)
            .await
            .map_err(|error| format!("remote collection reorder failed: {error}"))?;
        if reordered
            .videos
            .iter()
            .map(|video| &video.shortcode)
            .ne(reversed.iter())
        {
            return Err("remote collection update changed requested video order".to_string());
        }

        client
            .delete_collection(&created.shortcode)
            .await
            .map_err(|error| format!("remote collection deletion failed: {error}"))?;
        let count_after_delete = client
            .count_collections()
            .await
            .map_err(|error| format!("post-delete collection count failed: {error}"))?;
        if count_after_delete != initial_count {
            return Err(format!(
                "collection count did not return from {count_after_create} to {initial_count}"
            ));
        }

        match client.get_collection(&created.shortcode).await {
            Err(StreamableError::CollectionNotFound { .. }) => {}
            Err(error) => {
                return Err(format!(
                    "deleted remote collection returned the wrong error: {error}"
                ));
            }
            Ok(_) => return Err("deleted remote collection remained readable".to_string()),
        }
        for shortcode in &video_shortcodes {
            client
                .get_video(shortcode)
                .await
                .map_err(|error| format!("collection deletion removed member video: {error}"))?;
        }

        Ok(())
    }
    .await;

    drop(client);

    let exercise_error = exercise_result.err();
    assert!(
        exercise_error.is_none(),
        "{}",
        exercise_error
            .as_deref()
            .unwrap_or("remote collection lifecycle failed")
    );
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn unauthenticated_video_analytics_requests_are_bodyless() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123/analytics"))
        .and(NoCookieHeader)
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "countries": [{ "source": "US", "count": 3 }],
            "platforms": [{ "source": "desktop", "count": 2 }],
            "referrers": [{ "source": "direct", "count": 1 }],
            "group": "day",
            "plays": [
                { "date": "2026-08-13", "count": 0 },
                { "date": "2026-08-14", "count": 3 }
            ],
            "from_date": "2026-08-13",
            "to_date": "2026-08-14"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123/analytics/live"))
        .and(NoCookieHeader)
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 0 })))
        .expect(1)
        .mount(&mock_server)
        .await;
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let summary = client
        .get_video_analytics("abc123")
        .await
        .expect("video analytics should succeed");
    let live = client
        .get_video_live_views("abc123")
        .await
        .expect("live views should succeed");

    assert_eq!(summary.group, "day");
    assert_eq!(summary.plays[0].count, 0);
    assert_eq!(summary.countries[0].source, "US");
    assert_eq!(live.count, 0);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_analytics_requests_map_endpoint_and_common_errors() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/rejected/analytics"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": "InternalServerError",
            "message": "Analytics unavailable"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/rejected/analytics/live"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Live count unavailable"))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/expired/analytics"))
        .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
        .expect(1)
        .mount(&mock_server)
        .await;
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let summary_error = expect_streamable_error(
        client.get_video_analytics("rejected").await,
        "rejected video analytics should fail",
    );
    let live_error = expect_streamable_error(
        client.get_video_live_views("rejected").await,
        "rejected live views should fail",
    );
    let session_error = expect_streamable_error(
        client.get_video_analytics("expired").await,
        "expired video analytics should fail",
    );

    assert!(matches!(
        summary_error,
        StreamableError::VideoAnalyticsFailed {
            ref shortcode,
            status: 500,
            ref message
        } if shortcode == "rejected" && message == "Analytics unavailable"
    ));
    assert!(matches!(
        live_error,
        StreamableError::VideoLiveViewsFailed {
            ref shortcode,
            status: 503,
            ref message
        } if shortcode == "rejected" && message == "Live count unavailable"
    ));
    assert!(matches!(
        session_error,
        StreamableError::InvalidSession { .. }
    ));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_live_views_can_be_fetched() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");

    client
        .get_video_live_views(&video.shortcode)
        .await
        .expect("remote live views should be available");
    drop(client);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn unauthenticated_video_privacy_update_and_explicit_refresh_succeed() {
    let mock_server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/videos/abc123/settings"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .and(body_json(json!({
            "visibility": "hidden_on_streamable"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123"))
        .and(NoCookieHeader)
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_video("abc123", true)))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");
    let update = models::VideoPrivacySettingsUpdate {
        visibility: Some(models::Visibility::HiddenOnStreamable),
        ..models::VideoPrivacySettingsUpdate::default()
    };

    client
        .update_video_privacy("abc123", &update)
        .await
        .expect("video privacy update should succeed");
    let video = client
        .get_video("abc123")
        .await
        .expect("video refresh should succeed");
    let settings = video
        .privacy_settings
        .as_ref()
        .expect("refreshed video should include privacy settings");

    assert_eq!(settings.visibility, models::Visibility::HiddenOnStreamable);
    assert!(settings.is_custom);
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn update_video_privacy_serializes_password_removal_as_null() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/videos/abc123/settings"))
        .and(body_json(json!({ "password": null })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let update = models::VideoPrivacySettingsUpdate {
        password: Some(models::VideoPasswordUpdate::Remove),
        ..models::VideoPrivacySettingsUpdate::default()
    };

    client
        .update_video_privacy("abc123", &update)
        .await
        .expect("password removal should succeed");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn reset_video_privacy_sends_bodyless_delete() {
    let mock_server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/abc123/settings"))
        .and(NoCookieHeader)
        .and(header("content-type", "application/json"))
        .and(header("pragma", "no-cache"))
        .and(header("cache-control", "no-cache"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");

    client
        .reset_video_privacy("abc123")
        .await
        .expect("video privacy reset should succeed");

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let reset_request = requests
        .iter()
        .find(|request| request.method.as_str() == "DELETE")
        .expect("privacy reset request should be recorded");
    assert!(reset_request.body.is_empty());
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_privacy_operations_map_endpoint_and_common_errors() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/videos/rejected/settings"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "message": "Invalid visibility"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/rejected/settings"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": "InternalServerError",
            "message": "Reset failed"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/videos/expired/settings"))
        .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let update = models::VideoPrivacySettingsUpdate {
        visibility: Some(models::Visibility::Private),
        ..models::VideoPrivacySettingsUpdate::default()
    };

    let update_error = expect_streamable_error(
        client.update_video_privacy("rejected", &update).await,
        "rejected privacy update should fail",
    );
    let reset_error = expect_streamable_error(
        client.reset_video_privacy("rejected").await,
        "rejected privacy reset should fail",
    );
    let session_error = expect_streamable_error(
        client.update_video_privacy("expired", &update).await,
        "expired privacy update should fail",
    );

    assert!(matches!(
        update_error,
        StreamableError::VideoPrivacyUpdateFailed {
            ref shortcode,
            status: 400,
            ref message
        } if shortcode == "rejected" && message == "Invalid visibility"
    ));
    assert!(matches!(
        reset_error,
        StreamableError::VideoPrivacyResetFailed {
            ref shortcode,
            status: 500,
            ref message
        } if shortcode == "rejected" && message == "Reset failed"
    ));
    assert!(matches!(
        session_error,
        StreamableError::InvalidSession { .. }
    ));
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_video_privacy_can_be_updated_and_refreshed() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client()
        .await
        .expect("shared remote account should authenticate");
    let video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");
    let update = models::VideoPrivacySettingsUpdate {
        visibility: Some(models::Visibility::Private),
        ..models::VideoPrivacySettingsUpdate::default()
    };
    let updated_result = client.update_video_privacy(&video.shortcode, &update).await;
    let refreshed_result = client.get_video(&video.shortcode).await;
    drop(client);
    updated_result.expect("remote video privacy update should succeed");
    let refreshed = refreshed_result.expect("remote updated video refresh should succeed");
    assert_eq!(
        refreshed
            .privacy_settings
            .as_ref()
            .expect("remote video should include updated privacy settings")
            .visibility,
        models::Visibility::Private
    );
}

#[tokio::test]
async fn privacy_settings_update_omits_none_fields_and_refreshes_user() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    {
        let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
        let mut client = remote_authenticated_client()
            .await
            .expect("shared remote account should authenticate");
        let allow_download = !client.user().privacy_settings.allow_download;

        client
            .change_privacy_settings(Some(allow_download), None, None)
            .await
            .expect("remote privacy settings update should succeed");

        assert_eq!(
            client.user().privacy_settings.allow_download,
            allow_download
        );
        drop(client);
    }

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    {
        let mock_server = MockServer::start().await;
        let email = "user@example.com";
        let password = "Password1";

        mock_login(&mock_server, email, password).await;

        let mut updated_user = authenticated_user(email);
        updated_user["privacy_settings"]["allow_download"] = json!(false);
        updated_user["privacy_settings"]["visibility"] = json!("private");

        Mock::given(method("PATCH"))
            .and(path("/api/v1/me/settings"))
            .and(body_json(json!({
                "allow_download": false,
                "visibility": "private"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_user))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = mock_client(&mock_server)
            .expect("mock client should initialize")
            .login(email.to_string(), password.to_string())
            .await
            .expect("login should succeed");

        {
            let settings = client
                .change_privacy_settings(Some(false), None, Some(models::Visibility::Private))
                .await
                .expect("privacy settings update should succeed");

            assert!(!settings.allow_download);
            assert!(settings.allow_sharing);
            assert!(matches!(settings.visibility, models::Visibility::Private));
        }

        assert!(!client.user().privacy_settings.allow_download);
        assert!(matches!(
            client.user().privacy_settings.visibility,
            models::Visibility::Private
        ));
    }
}

#[tokio::test]
async fn registration_reports_email_already_in_use() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let email = generate_random_username();
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let password = generate_random_password();
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _registration = StreamableClient::new()
        .expect("client should initialize")
        .register(Some(email.clone()), Some(password.clone()), None)
        .await
        .expect("first registration should succeed");
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let client = StreamableClient::new().expect("client should initialize");

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let mock_server = MockServer::start().await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    Mock::given(method("POST"))
        .and(path("/users"))
        .respond_with(ResponseTemplate::new(400).set_body_string("Email already in use"))
        .expect(1)
        .mount(&mock_server)
        .await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let email = "used@example.com".to_string();
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let password = "Password1".to_string();
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let client = mock_client(&mock_server).expect("mock client should initialize");

    let error = expect_streamable_error(
        client.register(Some(email), Some(password), None).await,
        "registration should fail",
    );

    assert!(matches!(
        error,
        StreamableError::EmailAlreadyInUse { ref message }
            if message.contains("Email already in use")
    ));
}

#[tokio::test]
async fn login_reports_invalid_credentials() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let (email, actual_password) = remote_credentials().expect("remote credentials should load");
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let client = StreamableClient::new().expect("client should initialize");
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let password = loop {
        let candidate = generate_random_password();
        if candidate != actual_password {
            break candidate;
        }
    };

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let mock_server = MockServer::start().await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    mock_json_error(
        &mock_server,
        "/check",
        200,
        "AuthError",
        "Invalid username or password",
    )
    .await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let client = mock_client(&mock_server).expect("mock client should initialize");
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let email = "user@example.com".to_string();
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let password = "wrong".to_string();

    let error = expect_streamable_error(client.login(email, password).await, "login should fail");

    assert!(matches!(
        error,
        StreamableError::InvalidCredentials { ref message }
            if message == "Invalid username or password"
    ));
}

#[tokio::test]
async fn authentication_reports_rate_limits() {
    let error = {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/check"))
            .respond_with(ResponseTemplate::new(429))
            .expect(1)
            .mount(&mock_server)
            .await;

        let endpoint = format!("{}/check", mock_server.uri());
        let error = expect_streamable_error(
            mock_client(&mock_server)
                .expect("mock client should initialize")
                .login("user@example.com".to_string(), "Password1".to_string())
                .await,
            "login should be rate limited",
        );

        assert!(matches!(
            error,
            StreamableError::RateLimitExceeded { endpoint: ref actual }
                if actual == &endpoint
        ));

        error
    };

    assert!(matches!(error, StreamableError::RateLimitExceeded { .. }));
}

#[tokio::test]
async fn registration_reports_password_validation() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let client = StreamableClient::new().expect("client should initialize");
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let email = generate_random_username();

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let mock_server = MockServer::start().await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    Mock::given(method("POST"))
        .and(path("/users"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            "Password must be at least 8 characters, and contain at least one uppercase letter (A-Z), one lowercase letter (a-z), and one number (0-9).",
        ))
        .expect(1)
        .mount(&mock_server)
        .await;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let client = mock_client(&mock_server).expect("mock client should initialize");
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    let email = "user@example.com".to_string();

    let error = expect_streamable_error(
        client
            .register(Some(email), Some("weak".to_string()), None)
            .await,
        "registration should fail",
    );

    assert!(matches!(
        &error,
        StreamableError::PasswordValidation { message }
            if message.starts_with("Password must ")
    ));
}

#[tokio::test]
async fn authentication_reports_invalid_sessions() {
    let error = {
        let mock_server = MockServer::start().await;
        mock_json_error(
            &mock_server,
            "/check",
            401,
            "InvalidSessionError",
            "Session has expired",
        )
        .await;

        expect_streamable_error(
            mock_client(&mock_server)
                .expect("mock client should initialize")
                .login("user@example.com".to_string(), "Password1".to_string())
                .await,
            "login should fail with an invalid session",
        )
    };

    assert!(matches!(error, StreamableError::InvalidSession { .. }));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn video_resource_deletes_after_originating_client_is_dropped() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/videos/abc123"))
        .and(NoCookieHeader)
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_video("abc123", false)))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/videos/abc123"))
        .and(NoCookieHeader)
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");
    let video = client
        .get_video("abc123")
        .await
        .expect("video lookup should succeed");
    drop(client);

    video
        .delete()
        .await
        .expect("client-bound video deletion should succeed");

    let requests = mock_server
        .received_requests()
        .await
        .expect("mock server should record requests");
    let delete_request = requests
        .iter()
        .find(|request| request.method.as_str() == "DELETE")
        .expect("delete request should be recorded");
    assert!(delete_request.body.is_empty());
    assert!(!delete_request.headers.contains_key("content-type"));
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn registration_chains_into_label_resource_with_retained_session() {
    let mock_server = MockServer::start().await;
    let email = "user@example.com";
    let password = "Password1";
    mock_registration_with_credentials(&mock_server, email, password).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/labels"))
        .and(header("cookie", "session=mock-session"))
        .and(body_json(json!({ "name": "temporary" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "name": "temporary",
            "id": 174172
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/labels/174172"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let label = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed")
        .create_label("temporary")
        .await
        .expect("label creation should succeed");

    label
        .delete()
        .await
        .expect("client-bound label deletion should succeed");
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
#[tokio::test]
async fn collection_resource_updates_and_deletes_after_client_is_dropped() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/collections"))
        .and(NoCookieHeader)
        .and(body_json(json!({ "shortcodes": ["first"] })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "shortcode": "shared1",
            "title": null,
            "videos": [{ "shortcode": "first", "title": "First", "plays": 0 }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .and(body_json(json!({ "title": "Highlights" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "shortcode": "shared1",
            "title": "Highlights",
            "videos": [{ "shortcode": "first", "title": "First", "plays": 0 }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/collections/shared1"))
        .and(NoCookieHeader)
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = mock_client(&mock_server).expect("mock client should initialize");
    let shortcodes = vec!["first".to_string()];
    let collection = client
        .create_collection(&shortcodes, None)
        .await
        .expect("collection creation should succeed");
    drop(client);

    let collection = collection
        .set_title("Highlights")
        .await
        .expect("client-bound title update should succeed");
    assert_eq!(collection.title.as_deref(), Some("Highlights"));
    collection
        .delete()
        .await
        .expect("client-bound collection deletion should succeed");
}
