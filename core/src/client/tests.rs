use super::*;
use crate::{StreamableError, utils::*};

use serde_json::json;
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::matchers::body_json;
#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
use wiremock::{Match, Request};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
static REMOTE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
struct NoCookieHeader;

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

    StreamableClient::with_base_urls(base_url.clone(), base_url)
}

fn expect_streamable_error<T>(result: Result<T>, context: &str) -> StreamableError {
    match result {
        Ok(_) => panic!("{context}"),
        Err(error) => error,
    }
}

#[tokio::test]
async fn test_api_client_initialization() {
    let client = StreamableClient::new().expect("client should initialize");

    assert!(!client.is_authenticated());
}

#[test]
fn configured_base_urls_are_stored() {
    let auth_base_url =
        Url::parse("http://auth.example.test").expect("mock auth URL should be valid");
    let api_base_url = Url::parse("http://api.example.test").expect("mock API URL should be valid");
    let client = StreamableClient::with_base_urls(auth_base_url.clone(), api_base_url.clone())
        .expect("client should initialize");

    assert_eq!(client.auth_base_url, auth_base_url);
    assert_eq!(client.api_base_url, api_base_url);
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

#[tokio::test]
async fn privacy_settings_update_omits_none_fields_and_refreshes_user() {
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    {
        let _remote_test_guard = REMOTE_TEST_LOCK.lock().await;
        let (mut client, _, _) = StreamableClient::new()
            .expect("client should initialize")
            .register(None, None, None)
            .await
            .expect("registration should succeed");
        let allow_download = !client.user().privacy_settings.allow_download;

        client
            .change_privacy_settings(Some(allow_download), None, None)
            .await
            .expect("remote privacy settings update should succeed");

        assert_eq!(
            client.user().privacy_settings.allow_download,
            allow_download
        );
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
    let client = StreamableClient::new().expect("client should initialize");
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let email = generate_random_username();
    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    let password = generate_random_password();

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
