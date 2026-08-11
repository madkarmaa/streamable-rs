use crate::{
    constants::{API_BASE_URL, AUTH_BASE_URL},
    errors::{Result, StreamableError},
    models::{self, ApiRequest},
    response::ApiResponse,
    utils,
};
use reqwest::cookie::{CookieStore, Jar};
use std::sync::Arc;
use url::Url;

/// Marker for a client without an authenticated session.
#[derive(Debug)]
pub struct Unauthenticated;

/// Marker for a client with an authenticated session.
#[derive(Debug)]
pub struct Authenticated {
    user: models::AuthenticatedUser,
}

/// Client returned by [`StreamableClient::new`] and
/// [`AuthenticatedStreamableClient::logout`].
///
/// It cannot call authenticated-only methods:
///
/// ```compile_fail
/// use streamable::UnauthenticatedStreamableClient;
///
/// fn requires_authentication(client: UnauthenticatedStreamableClient) {
///     client.user();
/// }
/// ```
pub type UnauthenticatedStreamableClient = StreamableClient<Unauthenticated>;

/// Client returned by [`UnauthenticatedStreamableClient::register`] and
/// [`UnauthenticatedStreamableClient::login`].
///
/// It must call [`AuthenticatedStreamableClient::logout`] before authenticating again:
///
/// ```compile_fail
/// use streamable::AuthenticatedStreamableClient;
///
/// async fn login_again(client: AuthenticatedStreamableClient) {
///     client.login("user@example.com".into(), "password".into()).await;
/// }
///
/// async fn register_again(client: AuthenticatedStreamableClient) {
///     client.register(None, None, None).await;
/// }
/// ```
pub type AuthenticatedStreamableClient = StreamableClient<Authenticated>;

/// Streamable API client.
pub struct StreamableClient<State = Unauthenticated> {
    client: reqwest::Client,
    cookie_jar: Arc<Jar>,
    auth_base_url: Url,
    api_base_url: Url,
    state: State,
}

impl StreamableClient<Unauthenticated> {
    /// Creates a client using the production authentication and API base URLs.
    ///
    /// # Errors
    ///
    /// Returns an error when a base URL is invalid or the HTTP client cannot be built.
    pub fn new() -> Result<Self> {
        Self::with_base_urls(Url::parse(AUTH_BASE_URL)?, Url::parse(API_BASE_URL)?)
    }

    fn with_base_urls(auth_base_url: Url, api_base_url: Url) -> Result<Self> {
        let cookie_jar = Arc::new(Jar::default());
        let client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .build()?;

        Ok(Self {
            client,
            cookie_jar,
            auth_base_url,
            api_base_url,
            state: Unauthenticated,
        })
    }

    /// Checks whether the client has an authenticated session.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        false
    }

    /// Registers a new user and returns the authenticated client, email, and password.
    ///
    /// If `email`, `password`, or `username` are not provided, they will be randomly generated.
    ///
    /// Only email+password registration **IS** supported.
    ///
    /// Google OAuth registration is **NOT** supported.
    ///
    /// Facebook registration is **NOT** supported.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or Streamable rejects the registration.
    pub async fn register(
        self,
        email: Option<String>,
        password: Option<String>,
        username: Option<String>,
    ) -> Result<(AuthenticatedStreamableClient, String, String)> {
        let email = email.unwrap_or_else(utils::generate_random_username);
        let password = password.unwrap_or_else(utils::generate_random_password);
        let username = username.unwrap_or_else(|| email.clone());

        let request = models::CreateUserRequest::new(email.clone(), password.clone(), username);
        let user = self.execute(&request).await?;

        Ok((self.into_authenticated(user), email, password))
    }

    /// Logs in an existing user.
    ///
    /// Only email+password login **IS** supported.
    ///
    /// Google OAuth login is **NOT** supported.
    ///
    /// Facebook login is **NOT** supported.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or Streamable rejects the credentials.
    pub async fn login(
        self,
        email: String,
        password: String,
    ) -> Result<AuthenticatedStreamableClient> {
        let request = models::LoginRequest::new(email, password);
        let user = self.execute(&request).await?;

        Ok(self.into_authenticated(user))
    }

    fn into_authenticated(self, user: models::AuthenticatedUser) -> AuthenticatedStreamableClient {
        StreamableClient {
            client: self.client,
            cookie_jar: self.cookie_jar,
            auth_base_url: self.auth_base_url,
            api_base_url: self.api_base_url,
            state: Authenticated { user },
        }
    }
}

impl StreamableClient<Authenticated> {
    /// The currently authenticated user's data.
    #[must_use]
    pub const fn user(&self) -> &models::AuthenticatedUser {
        &self.state.user
    }

    /// Checks whether the client has an authenticated session.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        true
    }

    /// Changes the authenticated user's privacy settings.
    ///
    /// Settings passed as `None` are omitted from the PATCH body, leaving those server-side
    /// values unchanged. The response replaces the user data stored by this client.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or Streamable rejects the session or settings.
    pub async fn change_privacy_settings(
        &mut self,
        allow_download: Option<bool>,
        allow_sharing: Option<bool>,
        visibility: Option<models::Visibility>,
    ) -> Result<&models::PrivacySettings> {
        let request =
            models::PrivacySettingsRequest::new(allow_download, allow_sharing, visibility);

        let user = self.execute_and_update_user(&request).await?;
        Ok(&user.privacy_settings)
    }

    /// Changes the authenticated account's password.
    ///
    /// # Errors
    ///
    /// Returns an error when the session cookie is missing, the current password is incorrect,
    /// the new password fails validation, or the request fails.
    pub async fn change_password(&self, current_password: &str, new_password: &str) -> Result<()> {
        let session = self.session_cookie()?;
        let request = models::ChangePasswordRequest::new(&session, current_password, new_password);

        self.execute(&request).await
    }

    /// Logs out the currently authenticated user.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement HTTP client cannot be built.
    pub fn logout(self) -> Result<UnauthenticatedStreamableClient> {
        StreamableClient::with_base_urls(self.auth_base_url, self.api_base_url)
    }

    async fn execute_and_update_user<Req>(
        &mut self,
        request: &Req,
    ) -> Result<&models::AuthenticatedUser>
    where
        Req: ApiRequest<Response = models::AuthenticatedUser> + Sync,
    {
        self.state.user = self.execute(request).await?;
        Ok(&self.state.user)
    }

    fn session_cookie(&self) -> Result<String> {
        let cookies = self
            .cookie_jar
            .cookies(&self.auth_base_url)
            .ok_or_else(|| StreamableError::InvalidSession {
                message: "No session cookie found. Are you logged in?".to_string(),
            })?;

        let cookies = cookies
            .to_str()
            .map_err(|_| StreamableError::InvalidSession {
                message: "The session cookie is not valid UTF-8.".to_string(),
            })?;

        cookies
            .split(';')
            .find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "session").then(|| value.to_string())
            })
            .filter(|session| !session.is_empty())
            .ok_or_else(|| StreamableError::InvalidSession {
                message: "No session cookie found. Are you logged in?".to_string(),
            })
    }
}

impl<State: Sync> StreamableClient<State> {
    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        let request_url = Url::parse(req.url())?;
        let mut endpoint_url = if req.url().starts_with(API_BASE_URL) {
            self.api_base_url.clone()
        } else {
            self.auth_base_url.clone()
        };
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
mod tests;
