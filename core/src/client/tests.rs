use super::*;
use crate::{StreamableError, utils::*};

use serde_json::json;
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::matchers::{body_bytes, body_json, header, query_param};
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::{Match, Request, Respond};
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
fn remote_credentials() -> (String, String) {
    let path = dotenvy::dotenv().expect("remote tests require a readable .env file");
    let mut values = dotenvy::from_path_iter(path)
        .expect("remote tests require a valid .env file")
        .collect::<std::result::Result<std::collections::HashMap<_, _>, _>>()
        .expect("remote tests require valid EMAIL and PASSWORD entries");
    let email = values
        .remove("EMAIL")
        .expect("remote tests require EMAIL in .env");
    let password = values
        .remove("PASSWORD")
        .expect("remote tests require PASSWORD in .env");
    assert!(!email.is_empty(), "remote test EMAIL must not be empty");
    assert!(
        !password.is_empty(),
        "remote test PASSWORD must not be empty"
    );
    (email, password)
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
async fn remote_authenticated_client()
-> tokio::sync::MutexGuard<'static, AuthenticatedStreamableClient> {
    REMOTE_CLIENT
        .get_or_init(|| async {
            let (email, password) = remote_credentials();
            let client = StreamableClient::new()
                .expect("client should initialize")
                .login(email, password)
                .await
                .expect("shared remote account should authenticate");
            tokio::sync::Mutex::new(client)
        })
        .await
        .lock()
        .await
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct NoCookieHeader;

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct CancelUploadOnRequest {
    cancellation: UploadCancellationToken,
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
impl Respond for CancelUploadOnRequest {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        self.cancellation.cancel();
        ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(5))
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
impl Match for NoCookieHeader {
    fn matches(&self, request: &Request) -> bool {
        !request.headers.contains_key("cookie")
    }
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn authenticated_user(email: &str) -> serde_json::Value {
    json!({
        "socket": "mock-socket",
        "total_plays": 0,
        "total_uploads": 0,
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
fn unauthenticated_user(socket: &str, total_plays: u32, total_uploads: u32) -> serde_json::Value {
    json!({
        "socket": socket,
        "total_plays": total_plays,
        "total_uploads": total_uploads
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
    let base_url = Url::parse(&server.uri()).expect("mock server URI must be valid");

    StreamableClient::with_base_url(base_url)
}

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
fn mock_upload_client(server: &MockServer) -> Result<UnauthenticatedStreamableClient> {
    let base_url = Url::parse(&server.uri()).expect("mock server URI must be valid");

    StreamableClient::with_base_url(base_url)
}

fn media_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../media")
        .join(name)
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

    async fn execute(
        &self,
        request: crate::transport::Request,
    ) -> std::result::Result<crate::transport::Response, Self::Error> {
        let requests = Arc::clone(&self.requests);
        lock_unpoisoned(&requests).push(request);
        Ok(crate::transport::Response {
            status: http::StatusCode::OK,
            headers: http::HeaderMap::new(),
            body: bytes::Bytes::from_static(b"true"),
        })
    }
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
async fn video_upload_cancellation_aborts_s3_and_notifies_streamable() {
    let mock_server = MockServer::start().await;
    let video_path = media_path("webm.webm");
    let cancellation = UploadCancellationToken::new();
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
        .respond_with(CancelUploadOnRequest {
            cancellation: cancellation.clone(),
        })
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
        .upload_video_with_cancellation(video_path, None, cancellation.clone())
        .await
        .expect_err("cancelled upload should stop before transcoding");

    assert!(cancellation.is_cancelled());
    assert!(matches!(
        error,
        StreamableError::UploadCancelled {
            shortcode: Some(ref shortcode)
        } if shortcode == "mock"
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
async fn pre_cancelled_video_upload_makes_no_request() {
    let mock_server = MockServer::start().await;
    let client = mock_upload_client(&mock_server).expect("mock client should initialize");
    let cancellation = UploadCancellationToken::new();
    cancellation.cancel();
    let error = client
        .upload_video_with_cancellation(media_path("webm.webm"), None, cancellation)
        .await
        .expect_err("pre-cancelled upload should not start");

    assert!(matches!(
        error,
        StreamableError::UploadCancelled { shortcode: None }
    ));
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
async fn remote_video_upload_reaches_transcoding() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client().await;
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
    let client = remote_authenticated_client().await;
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

#[test]
fn configured_base_url_is_stored() {
    let base_url = Url::parse("http://api.example.test").expect("mock URL should be valid");
    let client =
        StreamableClient::with_base_url(base_url.clone()).expect("client should initialize");

    assert!(matches!(
        client.endpoint_routing,
        EndpointRouting::Override(url) if url == base_url
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
            ResponseTemplate::new(200).set_body_json(unauthenticated_user("anonymous", 12, 3)),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let mut client = mock_client(&mock_server).expect("mock client should initialize");
    let expected_user = models::UnauthenticatedUser {
        socket: "anonymous".to_string(),
        total_plays: 12,
        total_uploads: 3,
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
    refreshed_user["total_uploads"] = json!(7);
    refreshed_user["bio"] = json!("refreshed");
    Mock::given(method("GET"))
        .and(path("/api/v1/me"))
        .and(header("cookie", "session=mock-session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed_user))
        .expect(1)
        .mount(&mock_server)
        .await;

    let (mut client, _, _) = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");
    let user = client
        .refresh_user()
        .await
        .expect("authenticated user refresh should succeed");

    assert_eq!(user.unauthenticated.total_plays, 42);
    assert_eq!(user.unauthenticated.total_uploads, 7);
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

    let (email, _) = remote_credentials();
    let mut client = remote_authenticated_client().await;
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

    let (client, email, password) = client
        .register(None, None, None)
        .await
        .expect("registration should succeed");

    assert!(!email.is_empty());
    assert!(!password.is_empty());
    assert!(client.is_authenticated());

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

    let (registered_client, returned_email, returned_password) = registration_client
        .register(Some(email.clone()), Some(password.clone()), None)
        .await
        .expect("registration should succeed");

    assert_eq!(returned_email, email);
    assert_eq!(returned_password, password);
    assert_eq!(registered_client.user().email, email);
    assert!(registered_client.is_authenticated());

    let login_client = registered_client.logout().expect("logout should succeed");

    assert!(!login_client.is_authenticated());

    let logged_in_client = login_client
        .login(returned_email, returned_password)
        .await
        .expect("login should succeed");

    assert_eq!(logged_in_client.user().email, email);
    assert!(logged_in_client.is_authenticated());
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
async fn remote_change_password_flow() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let (email, current_password) = remote_credentials();
    let mut new_password = generate_random_password();
    while new_password == current_password {
        new_password = generate_random_password();
    }
    let mut wrong_password = generate_random_password();
    while wrong_password == current_password {
        wrong_password = generate_random_password();
    }

    let client = remote_authenticated_client().await;

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
        .await
        .expect("remote password change should succeed");

    client
        .change_password(&new_password, &current_password)
        .await
        .expect("shared remote account password should be restored");
    assert_eq!(client.user().email, email);
    drop(client);
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

    let (client, _, _) = mock_client(&mock_server)
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
    remote_change_password_flow().await;

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

    let (client, _, _) = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let label = client
        .create_label("  important  ")
        .await
        .expect("label creation should succeed");

    assert_eq!(
        label,
        models::Label {
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
        .expect("mock client should initialize")
        .register(Some(email.to_string()), Some(password.to_string()), None)
        .await
        .expect("registration should succeed");

    let label = client
        .rename_label(174_172, "  renamed  ")
        .await
        .expect("label rename should succeed");

    assert_eq!(
        label,
        models::Label {
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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
async fn remote_video_labels_can_be_assigned() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client().await;
    let video = client
        .upload_video(media_path("webm.webm"), None)
        .await
        .expect("remote video upload should reach transcoding");
    let label_name = format!("label-{}", generate_random_password());
    let label = client
        .create_label(&label_name)
        .await
        .expect("remote label creation should succeed");

    client
        .set_video_labels(&video.shortcode, &[label.id])
        .await
        .expect("remote video label assignment should succeed");
    drop(client);
}

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
#[tokio::test]
async fn remote_label_lifecycle() {
    let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
    let client = remote_authenticated_client().await;
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
    let client = remote_authenticated_client().await;
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

    let (client, _, _) = mock_client(&mock_server)
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

    let (client, _, _) = mock_client(&mock_server)
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
    let client = remote_authenticated_client().await;
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
        let mut client = remote_authenticated_client().await;
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
    let (_registered_client, _, _) = StreamableClient::new()
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
    let (email, actual_password) = remote_credentials();
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
