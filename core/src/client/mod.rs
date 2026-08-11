use crate::{
    constants::*,
    errors::Result,
    models::{self, ApiRequest},
    response::ApiResponse,
    utils,
};
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
    auth_base_url: Url,
    state: State,
}

impl StreamableClient<Unauthenticated> {
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

    /// Registers a new user and returns the authenticated client, email, and password.
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
            auth_base_url: self.auth_base_url,
            state: Authenticated { user },
        }
    }
}

impl StreamableClient<Authenticated> {
    /// The currently authenticated user's data.
    pub fn user(&self) -> &models::AuthenticatedUser {
        &self.state.user
    }

    /// Checks whether the client has an authenticated session.
    pub fn is_authenticated(&self) -> bool {
        true
    }

    /// Logs out the currently authenticated user.
    pub fn logout(self) -> Result<UnauthenticatedStreamableClient> {
        StreamableClient::with_auth_base_url(self.auth_base_url)
    }
}

impl<State> StreamableClient<State> {
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
mod tests;
