mod constants;
pub mod models;
pub mod utils;

use crate::{constants::*, models::ApiRequest};
use reqwest::cookie::{CookieStore, Jar};
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    cookie_store: Arc<Jar>,
}

impl ApiClient {
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

    /// Executes any ApiRequest and automatically deserializes the matching Response type
    pub async fn execute<R>(&self, req: &R) -> Result<R::Response, reqwest::Error>
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
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::utils::*;

    #[tokio::test]
    async fn test_api_client_initialization() {
        let client = ApiClient::new();
        assert!(client.is_ok());
    }

    #[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
    #[tokio::test]
    async fn test_successful_random_registration() -> anyhow::Result<()> {
        let client = ApiClient::new()?;
        let username = generate_random_username();
        let request = CreateUserRequest {
            email: username.clone(),
            password: generate_random_password(),
            username: username.clone(),
        };

        let response = client.execute(&request).await?;

        assert_eq!(response.email, username);
        assert!(client.is_authenticated());

        Ok(())
    }
}
