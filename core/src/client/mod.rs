use crate::{
    constants::AUTH_BASE_URL,
    errors::{Result, StreamableError},
    models::{self, ApiRequest},
    response::ApiResponse,
    utils,
};
use reqwest::cookie::{CookieStore, Jar};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Notify;
use url::Url;

#[cfg(test)]
mod tests;

/// Marker for a client without an authenticated session.
#[derive(Debug)]
pub struct Unauthenticated {
    user: Option<models::UnauthenticatedUser>,
}

/// Marker for a client with an authenticated session.
#[derive(Debug)]
pub struct Authenticated {
    user: models::AuthenticatedUser,
}

/// Client returned by [`StreamableClient::new`] and
/// [`AuthenticatedStreamableClient::logout`].
///
/// Its [`StreamableClient::user`] method exposes only the basic user data available without an
/// authenticated session.
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

/// Cooperative cancellation token for an in-flight video upload.
///
/// Clone this token before starting [`StreamableClient::upload_video_with_cancellation`], then call
/// [`UploadCancellationToken::cancel`] from another task. Cancellation aborts the current request
/// and, after Streamable assigns a shortcode, reports cancellation to Streamable.
#[derive(Clone, Debug)]
pub struct UploadCancellationToken {
    inner: Arc<UploadCancellationState>,
}

#[derive(Debug)]
struct UploadCancellationState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl UploadCancellationToken {
    /// Creates a token in the active (not cancelled) state.
    ///
    /// ```
    /// use streamable::UploadCancellationToken;
    ///
    /// let token = UploadCancellationToken::new();
    /// assert!(!token.is_cancelled());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(UploadCancellationState {
                cancelled: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }

    /// Marks this token and all its clones as cancelled and wakes upload tasks.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notify.notify_waiters();
        }
    }

    #[must_use]
    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    async fn cancelled(&self) {
        let notified = self.inner.notify.notified();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

impl Default for UploadCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
enum EndpointRouting {
    Production,
    #[cfg(test)]
    Override(Url),
}

impl EndpointRouting {
    fn resolve(&self, url: &str) -> Result<Url> {
        let requested_url = Url::parse(url)?;
        match self {
            Self::Production => Ok(requested_url),
            #[cfg(test)]
            Self::Override(endpoint_url) => {
                let mut endpoint_url = endpoint_url.clone();
                endpoint_url.set_path(requested_url.path());
                endpoint_url.set_query(requested_url.query());
                Ok(endpoint_url)
            }
        }
    }

    const fn override_url(&self) -> Option<&Url> {
        match self {
            Self::Production => None,
            #[cfg(test)]
            Self::Override(url) => Some(url),
        }
    }
}

/// Streamable API client.
pub struct StreamableClient<State = Unauthenticated> {
    client: reqwest::Client,
    cookie_jar: Arc<Jar>,
    endpoint_routing: EndpointRouting,
    state: State,
}

impl StreamableClient<Unauthenticated> {
    /// Creates a client using the production authentication and API base URLs.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be built.
    pub fn new() -> Result<Self> {
        Self::with_endpoint_routing(EndpointRouting::Production)
    }

    #[cfg(test)]
    fn with_base_url(base_url: Url) -> Result<Self> {
        Self::with_endpoint_routing(EndpointRouting::Override(base_url))
    }

    fn with_endpoint_routing(endpoint_routing: EndpointRouting) -> Result<Self> {
        let cookie_jar = Arc::new(Jar::default());
        let client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .build()?;

        Ok(Self {
            client,
            cookie_jar,
            endpoint_routing,
            state: Unauthenticated { user: None },
        })
    }

    /// Basic user data previously fetched without an authenticated session.
    ///
    /// **NOTE**: the returned [`Option`] is `None` until [`UnauthenticatedStreamableClient::refresh_user`] is called.
    #[must_use]
    pub const fn user(&self) -> Option<&models::UnauthenticatedUser> {
        self.state.user.as_ref()
    }

    /// Fetches current unauthenticated user data and stores it in this client.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response does not match the expected model.
    pub async fn refresh_user(&mut self) -> Result<&models::UnauthenticatedUser> {
        let user = self.execute(&models::MeRequest::new()).await?;
        Ok(&*self.state.user.insert(user))
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
            endpoint_routing: self.endpoint_routing,
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

    /// Fetches current authenticated user data and stores it in this client.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the request fails, or the response does not
    /// match the expected model.
    pub async fn refresh_user(&mut self) -> Result<&models::AuthenticatedUser> {
        self.execute_and_update_user(&models::MeRequest::new())
            .await
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

    /// Creates a label for the authenticated user.
    ///
    /// Leading and trailing whitespace is removed from `name` before sending it.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label already exists, the name is
    /// rejected, or the request fails.
    pub async fn create_label(&self, name: &str) -> Result<models::Label> {
        self.execute(&models::CreateLabelRequest::new(name)).await
    }

    /// Deletes a label belonging to the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label does not exist, or the request
    /// fails.
    pub async fn delete_label(&self, id: u64) -> Result<()> {
        self.execute(&models::DeleteLabelRequest::new(id)).await
    }

    /// Renames a label belonging to the authenticated user.
    ///
    /// Leading and trailing whitespace is removed from `new_name` before sending it.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label does not exist, the name is
    /// rejected, or the request fails.
    pub async fn rename_label(&self, id: u64, new_name: &str) -> Result<models::Label> {
        self.execute(&models::RenameLabelRequest::new(id, new_name))
            .await
    }

    /// Logs out the currently authenticated user.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement HTTP client cannot be built.
    pub fn logout(self) -> Result<UnauthenticatedStreamableClient> {
        StreamableClient::with_endpoint_routing(self.endpoint_routing)
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
            .cookies(&self.endpoint_routing.resolve(AUTH_BASE_URL)?)
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
    /// Permanently deletes a video by shortcode.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, Streamable rejects the deletion, or the successful
    /// response body is not exactly `true`.
    pub async fn delete_video(&self, shortcode: &str) -> Result<()> {
        self.execute(&models::DeleteVideoRequest::new(shortcode))
            .await
    }

    /// Uploads a local video and starts Streamable transcoding.
    ///
    /// `title` is sent as the video's title when provided. Otherwise, the title defaults to the
    /// file stem.
    ///
    /// # Errors
    ///
    /// Returns an error when the path cannot be read, is not a recognized video, any API or S3
    /// request fails, the upload configuration cannot be signed, or a response cannot be decoded.
    pub async fn upload_video(
        &self,
        video_file: impl AsRef<Path>,
        title: Option<String>,
    ) -> Result<models::Video> {
        self.upload_video_with_cancellation(video_file, title, UploadCancellationToken::new())
            .await
    }

    /// Uploads a local video with cooperative cancellation.
    ///
    /// `title` is sent as the video's title when provided. Otherwise, the title defaults to the
    /// file stem.
    ///
    /// Calling [`UploadCancellationToken::cancel`] aborts the active upload request.
    ///
    /// # Errors
    ///
    /// Returns [`StreamableError::UploadCancelled`] after successful cancellation, or another
    /// request, file, signing, or response error when that operation fails.
    pub async fn upload_video_with_cancellation(
        &self,
        video_file: impl AsRef<Path>,
        title: Option<String>,
        cancellation: UploadCancellationToken,
    ) -> Result<models::Video> {
        let video_file = tokio::fs::canonicalize(video_file.as_ref()).await?;
        let metadata = tokio::fs::metadata(&video_file).await?;

        if !utils::is_video_file(&video_file) {
            return Err(StreamableError::InvalidVideoFile { path: video_file });
        }

        let original_name = video_file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| StreamableError::InvalidVideoFile {
                path: video_file.clone(),
            })?;

        let title = title.map_or_else(
            || {
                video_file
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .ok_or_else(|| StreamableError::InvalidVideoFile {
                        path: video_file.clone(),
                    })
            },
            Ok,
        )?;

        let size = metadata.len();

        let upload_info = self.generate_shortcode(size, &cancellation).await?;
        self.initialize_video_upload(&upload_info, size, original_name, title, &cancellation)
            .await?;
        self.upload_video_file_to_s3(&upload_info, size, &video_file, &cancellation)
            .await?;
        self.transcode_video_after_upload(&upload_info, &cancellation)
            .await
    }

    /// Cancels an upload already known by its Streamable shortcode.
    ///
    /// # Errors
    ///
    /// Returns an error when the cancellation request fails or Streamable rejects it.
    pub async fn cancel_video_upload(&self, shortcode: &str) -> Result<()> {
        self.execute(&models::CancelVideoUploadRequest::new(shortcode))
            .await
    }

    async fn generate_shortcode(
        &self,
        size: u64,
        cancellation: &UploadCancellationToken,
    ) -> Result<models::UploadInfo> {
        let request = models::ShortcodeRequest::new(size);
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(StreamableError::UploadCancelled { shortcode: None }),
            result = self.execute(&request) => result,
        }
    }

    async fn initialize_video_upload(
        &self,
        upload_info: &models::UploadInfo,
        size: u64,
        original_name: String,
        title: String,
        cancellation: &UploadCancellationToken,
    ) -> Result<()> {
        let shortcode = &upload_info.shortcode;
        let request =
            models::InitializeVideoUploadRequest::new(shortcode, size, original_name, title);
        tokio::select! {
            biased;
            () = cancellation.cancelled() => self.cancel_initialized_upload(shortcode).await,
            result = self.execute(&request) => result,
        }
    }

    async fn upload_video_file_to_s3(
        &self,
        upload_info: &models::UploadInfo,
        size: u64,
        video_file: &Path,
        cancellation: &UploadCancellationToken,
    ) -> Result<()> {
        let signed_put = self
            .endpoint_routing
            .override_url()
            .map_or_else(
                || utils::s3::build_s3_put(upload_info, size),
                |base_url| utils::s3::build_s3_put_for_base_url(upload_info, size, base_url),
            )
            .map_err(|error| StreamableError::UploadSigning {
                message: error.to_string(),
            })?;

        let file = tokio::fs::File::open(video_file).await?;

        let upload = async {
            self.client
                .put(signed_put.url)
                .headers(signed_put.headers)
                .body(reqwest::Body::from(file))
                .send()
                .await?
                .error_for_status()?;
            Ok(())
        };

        tokio::select! {
            biased;
            () = cancellation.cancelled() => self.cancel_initialized_upload(&upload_info.shortcode).await,
            result = upload => result,
        }
    }

    async fn transcode_video_after_upload(
        &self,
        upload_info: &models::UploadInfo,
        cancellation: &UploadCancellationToken,
    ) -> Result<models::Video> {
        let request = models::TranscodeVideoRequest::new(upload_info);
        tokio::select! {
            biased;
            () = cancellation.cancelled() => self.cancel_initialized_upload(&upload_info.shortcode).await,
            result = self.execute(&request) => result,
        }
    }

    async fn cancel_initialized_upload<T>(&self, shortcode: &str) -> Result<T> {
        self.cancel_video_upload(shortcode).await?;
        Err(StreamableError::UploadCancelled {
            shortcode: Some(shortcode.to_string()),
        })
    }

    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        let endpoint_url = self.endpoint_routing.resolve(req.url())?;
        let request = self.client.request(req.method(), endpoint_url.clone());
        let request = req.prepare_request(request);
        let response = request.send().await?;

        let status = response.status();
        let status_error = response.error_for_status_ref().err();
        let body = response.bytes().await?;

        req.decode_response(ApiResponse::new(status, endpoint_url, body, status_error))
    }
}
