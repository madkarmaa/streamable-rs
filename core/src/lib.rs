mod constants;
pub mod models;
pub mod utils;

use crate::{constants::*, models::ApiRequest};
use reqwest::cookie::{CookieStore, Jar};
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    cookie_store: Arc<Jar>,
    auth_base_url: Url,
}

impl Client {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_auth_base_url(Url::parse(AUTH_BASE_URL).expect("valid auth base URL"))
    }

    fn with_auth_base_url(auth_base_url: Url) -> Result<Self, reqwest::Error> {
        let cookie_store = Arc::new(Jar::default());

        // cookie_store must be enabled to allow for session persistence across requests
        let client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_store))
            .build()?;

        Ok(Self {
            client,
            cookie_store,
            auth_base_url,
        })
    }

    async fn execute<R>(&self, req: &R) -> Result<R::Response, reqwest::Error>
    where
        R: ApiRequest,
    {
        let request_url = Url::parse(req.url()).expect("API request URL must be valid");
        let mut endpoint_url = self.auth_base_url.clone();
        endpoint_url.set_path(request_url.path());
        endpoint_url.set_query(request_url.query());

        self.client
            .request(req.method(), endpoint_url)
            .json(req)
            .send()
            .await?
            .error_for_status()?
            .json::<R::Response>()
            .await
    }

    fn has_cookie(&self, url: &Url, cookie_name: &str) -> bool {
        self.cookie_store
            .cookies(url)
            .and_then(|header| header.to_str().ok().map(|s| s.to_string()))
            .map(|cookies_str| {
                // Parse the "key=value; key2=value2" header format
                cookies_str.split(';').any(|pair| {
                    let mut parts = pair.split('=');
                    match (parts.next(), parts.next()) {
                        (Some(key), _) => key.trim() == cookie_name,
                        _ => false,
                    }
                })
            })
            .unwrap_or(false)
    }

    /// Checks if the client has a valid `session` cookie, indicating an authenticated user
    pub fn is_authenticated(&self) -> bool {
        self.has_cookie(&self.auth_base_url, "session")
    }

    pub async fn register(
        &self,
        email: Option<String>,
        password: Option<String>,
        username: Option<String>,
    ) -> Result<models::AuthenticatedUser, reqwest::Error> {
        let email = email.unwrap_or_else(utils::generate_random_username);
        let password = password.unwrap_or_else(utils::generate_random_password);
        let username = username.unwrap_or_else(|| email.clone());

        let request = models::CreateUserRequest {
            email,
            password,
            username,
        };
        self.execute(&request).await
    }

    pub async fn login(
        &self,
        email: String,
        password: String,
    ) -> Result<models::AuthenticatedUser, reqwest::Error> {
        let request = models::LoginRequest::new(email, password);
        self.execute(&request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::*;

    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    use serde_json::json;
    #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, method, path},
    };

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

    #[tokio::test]
    async fn test_api_client_initialization() {
        let client = Client::new();
        assert!(client.is_ok());
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

        let response = client.register(None, None, None).await?;

        assert!(!response.email.is_empty());
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

        let registered_user = registration_client
            .register(Some(email.clone()), Some(password.clone()), None)
            .await?;

        assert_eq!(registered_user.email, email);
        assert!(registration_client.is_authenticated());

        #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
        let login_client = Client::new()?;

        #[cfg(not(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER"))]
        let login_client = Client::with_auth_base_url(Url::parse(&mock_server.uri())?)?;

        let logged_in_user = login_client.login(email.clone(), password).await?;

        assert_eq!(logged_in_user.email, email);
        assert!(login_client.is_authenticated());

        Ok(())
    }
}
