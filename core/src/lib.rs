mod constants;
pub mod errors;
pub mod models;
pub mod response;
pub mod utils;

use crate::{constants::*, models::ApiRequest};
pub use errors::{Result, StreamableError};
pub use response::ApiResponse;
use url::Url;

/// Marker for a client without an authenticated session.
#[derive(Debug)]
pub struct Unauthenticated;

/// Marker for a client with an authenticated session.
#[derive(Debug)]
pub struct Authenticated {
    user: models::AuthenticatedUser,
}

/// Client returned by [`Client::new`] and [`AuthenticatedClient::logout`].
///
/// It cannot call authenticated-only methods:
///
/// ```compile_fail
/// use streamable::UnauthenticatedClient;
///
/// fn requires_authentication(client: UnauthenticatedClient) {
///     client.user();
/// }
/// ```
pub type UnauthenticatedClient = Client<Unauthenticated>;

/// Client returned by [`UnauthenticatedClient::register`] and
/// [`UnauthenticatedClient::login`].
///
/// It must call [`AuthenticatedClient::logout`] before authenticating again:
///
/// ```compile_fail
/// use streamable::AuthenticatedClient;
///
/// async fn login_again(client: AuthenticatedClient) {
///     client.login("user@example.com".into(), "password".into()).await;
/// }
///
/// async fn register_again(client: AuthenticatedClient) {
///     client.register(None, None, None).await;
/// }
/// ```
pub type AuthenticatedClient = Client<Authenticated>;

/// Streamable API client.
pub struct Client<State = Unauthenticated> {
    client: reqwest::Client,
    auth_base_url: Url,
    state: State,
}

impl Client<Unauthenticated> {
    pub fn new() -> Result<Self> {
        Self::with_auth_base_url(Url::parse(AUTH_BASE_URL).expect("valid auth base URL"))
    }

    fn with_auth_base_url(auth_base_url: Url) -> Result<Self> {
        let client = reqwest::Client::builder().cookie_store(true).build()?;

        Ok(Self {
            client,
            auth_base_url,
            state: Unauthenticated,
        })
    }

    /// Checks whether the client has an authenticated session.
    pub fn is_authenticated(&self) -> bool {
        false
    }

    /// Registers a new user.
    ///
    /// If `email`, `password`, or `username` are not provided, they will be randomly generated.
    ///
    /// Only email+password registration **IS** supported.
    ///
    /// Google OAuth registration is **NOT** supported.
    ///
    /// Facebook registration is **NOT** supported.
    pub async fn register(
        self,
        email: Option<String>,
        password: Option<String>,
        username: Option<String>,
    ) -> Result<AuthenticatedClient> {
        let email = email.unwrap_or_else(utils::generate_random_username);
        let password = password.unwrap_or_else(utils::generate_random_password);
        let username = username.unwrap_or_else(|| email.clone());

        let request = models::CreateUserRequest::new(email, password, username);
        let user = self.execute(&request).await?;

        Ok(self.into_authenticated(user))
    }

    /// Logs in an existing user.
    ///
    /// Only email+password login **IS** supported.
    ///
    /// Google OAuth login is **NOT** supported.
    ///
    /// Facebook login is **NOT** supported.
    pub async fn login(self, email: String, password: String) -> Result<AuthenticatedClient> {
        let request = models::LoginRequest::new(email, password);
        let user = self.execute(&request).await?;

        Ok(self.into_authenticated(user))
    }

    fn into_authenticated(self, user: models::AuthenticatedUser) -> AuthenticatedClient {
        Client {
            client: self.client,
            auth_base_url: self.auth_base_url,
            state: Authenticated { user },
        }
    }
}

impl Client<Authenticated> {
    /// The currently authenticated user's data.
    pub fn user(&self) -> &models::AuthenticatedUser {
        &self.state.user
    }

    /// Checks whether the client has an authenticated session.
    pub fn is_authenticated(&self) -> bool {
        true
    }

    /// Logs out the currently authenticated user.
    pub fn logout(self) -> Result<UnauthenticatedClient> {
        Client::with_auth_base_url(self.auth_base_url)
    }
}

impl<State> Client<State> {
    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest,
    {
        let request_url = Url::parse(req.url()).expect("API request URL must be valid");
        let mut endpoint_url = self.auth_base_url.clone();
        endpoint_url.set_path(request_url.path());
        endpoint_url.set_query(request_url.query());

        let response = self
            .client
            .request(req.method(), endpoint_url.clone())
            .json(req)
            .send()
            .await?;

        let status = response.status();
        let status_error = response.error_for_status_ref().err();
        let body = response.bytes().await?;

        req.decode_response(ApiResponse::new(status, endpoint_url, body, status_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::*;

    use serde_json::json;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    use wiremock::matchers::body_json;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    use wiremock::{Match, Request};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

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

    fn mock_client(server: &MockServer) -> Result<UnauthenticatedClient> {
        Client::with_auth_base_url(
            Url::parse(&server.uri()).expect("mock server URI must be valid"),
        )
    }

    fn expect_streamable_error<T>(result: Result<T>, context: &str) -> StreamableError {
        match result {
            Ok(_) => panic!("{context}"),
            Err(error) => error,
        }
    }

    #[tokio::test]
    async fn test_api_client_initialization() {
        let client = Client::new().expect("client should initialize");

        assert!(!client.is_authenticated());
    }

    #[tokio::test]
    async fn test_successful_random_registration() -> anyhow::Result<()> {
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let client = Client::new()?;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let mock_server = MockServer::start().await;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        mock_registration(&mock_server, "generated-user@example.com").await;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let client = Client::with_auth_base_url(Url::parse(&mock_server.uri())?)?;

        let client = client.register(None, None, None).await?;

        assert!(!client.user().email.is_empty());
        assert!(client.is_authenticated());

        Ok(())
    }

    #[tokio::test]
    async fn test_successful_registration_and_login() -> anyhow::Result<()> {
        let email = generate_random_username();
        let password = generate_random_password();

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let mock_server = MockServer::start().await;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        mock_registration_with_credentials(&mock_server, &email, &password).await;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        mock_login(&mock_server, &email, &password).await;

        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let registration_client = Client::new()?;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let registration_client = Client::with_auth_base_url(Url::parse(&mock_server.uri())?)?;

        let registered_client = registration_client
            .register(Some(email.clone()), Some(password.clone()), None)
            .await?;

        assert_eq!(registered_client.user().email, email);
        assert!(registered_client.is_authenticated());

        let login_client = registered_client.logout()?;

        assert!(!login_client.is_authenticated());

        let logged_in_client = login_client.login(email.clone(), password).await?;

        assert_eq!(logged_in_client.user().email, email);
        assert!(logged_in_client.is_authenticated());

        Ok(())
    }

    #[tokio::test]
    async fn registration_reports_email_already_in_use() -> anyhow::Result<()> {
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let email = generate_random_username();
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let password = generate_random_password();
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let _registered_client = Client::new()?
            .register(Some(email.clone()), Some(password.clone()), None)
            .await?;
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let client = Client::new()?;

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
        let client = mock_client(&mock_server)?;

        let error = expect_streamable_error(
            client.register(Some(email), Some(password), None).await,
            "registration should fail",
        );

        assert!(matches!(
            error,
            StreamableError::EmailAlreadyInUse { ref message }
                if message.contains("Email already in use")
        ));

        Ok(())
    }

    #[tokio::test]
    async fn login_reports_invalid_credentials() -> anyhow::Result<()> {
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let client = Client::new()?;
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
        let client = mock_client(&mock_server)?;
        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let email = "user@example.com".to_string();
        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let password = "wrong".to_string();

        let error =
            expect_streamable_error(client.login(email, password).await, "login should fail");

        assert!(matches!(
            error,
            StreamableError::InvalidCredentials { ref message }
                if message == "Invalid username or password"
        ));

        Ok(())
    }

    #[tokio::test]
    async fn authentication_reports_rate_limits() -> anyhow::Result<()> {
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
                mock_client(&mock_server)?
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

        Ok(())
    }

    #[tokio::test]
    async fn registration_reports_password_validation() -> anyhow::Result<()> {
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let client = Client::new()?;
        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let email = generate_random_username();

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let mock_server = MockServer::start().await;
        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        mock_json_error(
            &mock_server,
            "/users",
            400,
            "ValidationError",
            "Password does not meet requirements",
        )
        .await;
        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let client = mock_client(&mock_server)?;
        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let email = "user@example.com".to_string();

        let error = expect_streamable_error(
            client
                .register(Some(email), Some("weak".to_string()), None)
                .await,
            "registration should fail",
        );

        assert!(matches!(&error, StreamableError::PasswordValidation { .. }));

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        assert_eq!(error.to_string(), "Password does not meet requirements");

        Ok(())
    }

    #[tokio::test]
    async fn authentication_reports_invalid_sessions() -> anyhow::Result<()> {
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
                mock_client(&mock_server)?
                    .login("user@example.com".to_string(), "Password1".to_string())
                    .await,
                "login should fail with an invalid session",
            )
        };

        assert!(matches!(error, StreamableError::InvalidSession { .. }));

        Ok(())
    }
}
