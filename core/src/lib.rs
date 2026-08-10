mod constants;
pub mod models;
pub mod utils;

use crate::{constants::*, models::ApiRequest};
use reqwest::cookie::{CookieStore, Jar};
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct StreamableClient {
    client: reqwest::Client,
    cookie_store: Arc<Jar>,
}

impl StreamableClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        let cookie_store = Arc::new(Jar::default());

        // cookie_store must be enabled to allow for session persistence across requests
        let client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_store))
            .build()?;

        Ok(Self {
            client,
            cookie_store,
        })
    }

    async fn execute<R>(&self, req: &R) -> Result<R::Response, reqwest::Error>
    where
        R: ApiRequest,
    {
        self.client
            .request(req.method(), req.url())
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
        let url = Url::parse(AUTH_BASE_URL).expect("valid URL");
        self.has_cookie(&url, "session")
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
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::utils::*;

    #[tokio::test]
    async fn test_api_client_initialization() {
        let client = StreamableClient::new();
        assert!(client.is_ok());
    }

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    #[tokio::test]
    async fn test_successful_random_registration() -> anyhow::Result<()> {
        let client = StreamableClient::new()?;
        let response = client.register(None, None, None).await?;

        assert!(!response.email.is_empty());
        assert!(client.is_authenticated());

        Ok(())
    }

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    #[tokio::test]
    async fn test_successful_registration_and_login() -> anyhow::Result<()> {
        let email = generate_random_username();
        let password = generate_random_password();
        let registration_client = StreamableClient::new()?;

        let registered_user = registration_client
            .register(Some(email.clone()), Some(password.clone()), None)
            .await?;

        assert_eq!(registered_user.email, email);
        assert!(registration_client.is_authenticated());

        let login_client = StreamableClient::new()?;
        let logged_in_user = login_client.login(email.clone(), password).await?;

        assert_eq!(logged_in_user.email, email);
        assert!(login_client.is_authenticated());

        Ok(())
    }
}
