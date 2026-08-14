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

/// Marks a signed-out client.
///
/// ```
/// use streamable::{StreamableClient, Unauthenticated};
///
/// let client: StreamableClient<Unauthenticated> = StreamableClient::new()?;
/// # Ok::<(), streamable::StreamableError>(())
/// ```
#[derive(Debug)]
pub struct Unauthenticated {
    user: Option<models::UnauthenticatedUser>,
}

/// Marks a signed-in client.
///
/// ```no_run
/// use streamable::{Authenticated, StreamableClient};
///
/// fn print_account(client: &StreamableClient<Authenticated>) {
///     println!("{}", client.user().email);
/// }
/// ```
#[derive(Debug)]
pub struct Authenticated {
    user: models::AuthenticatedUser,
}

/// A signed-out client.
///
/// ```
/// use streamable::{StreamableClient, UnauthenticatedStreamableClient};
///
/// let client: UnauthenticatedStreamableClient = StreamableClient::new()?;
/// assert!(!client.is_authenticated());
/// # Ok::<(), streamable::StreamableError>(())
/// ```
pub type UnauthenticatedStreamableClient = StreamableClient<Unauthenticated>;

/// A client returned by login or registration.
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

/// A shared upload stop signal.
///
/// ```
/// use streamable::UploadCancellationToken;
///
/// let token = UploadCancellationToken::new();
/// let other_task = token.clone();
/// other_task.cancel();
/// assert!(token.is_cancelled());
/// ```
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
    /// Creates a token that has not been cancelled.
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

    /// Cancels this token and every clone of it.
    ///
    /// ```
    /// use streamable::UploadCancellationToken;
    ///
    /// let token = UploadCancellationToken::new();
    /// let clone = token.clone();
    /// clone.cancel();
    /// assert!(token.is_cancelled());
    /// ```
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notify.notify_waiters();
        }
    }

    #[must_use]
    /// Returns `true` after this token or one of its clones is cancelled.
    ///
    /// ```
    /// use streamable::UploadCancellationToken;
    ///
    /// let token = UploadCancellationToken::new();
    /// assert!(!token.is_cancelled());
    /// token.cancel();
    /// assert!(token.is_cancelled());
    /// ```
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

/// A Streamable API client.
///
/// ```
/// use streamable::StreamableClient;
///
/// let client = StreamableClient::new()?;
/// assert!(!client.is_authenticated());
/// # Ok::<(), streamable::StreamableError>(())
/// ```
pub struct StreamableClient<State = Unauthenticated> {
    client: reqwest::Client,
    cookie_jar: Arc<Jar>,
    endpoint_routing: EndpointRouting,
    state: State,
}

impl StreamableClient<Unauthenticated> {
    /// Creates a signed-out client.
    ///
    /// ```
    /// let client = streamable::StreamableClient::new()?;
    /// assert!(!client.is_authenticated());
    /// # Ok::<(), streamable::StreamableError>(())
    /// ```
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

    /// Returns the last fetched signed-out user data.
    ///
    /// ```
    /// let client = streamable::StreamableClient::new()?;
    /// assert!(client.user().is_none());
    /// # Ok::<(), streamable::StreamableError>(())
    /// ```
    #[must_use]
    pub const fn user(&self) -> Option<&models::UnauthenticatedUser> {
        self.state.user.as_ref()
    }

    /// Fetches and stores signed-out user data.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let mut client = streamable::StreamableClient::new()?;
    /// println!("{}", client.refresh_user().await?.total_uploads);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response does not match the expected model.
    pub async fn refresh_user(&mut self) -> Result<&models::UnauthenticatedUser> {
        let user = self.execute(&models::MeRequest::new()).await?;
        Ok(&*self.state.user.insert(user))
    }

    /// Returns `false` for a signed-out client.
    ///
    /// ```
    /// let client = streamable::StreamableClient::new()?;
    /// assert!(!client.is_authenticated());
    /// # Ok::<(), streamable::StreamableError>(())
    /// ```
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        false
    }

    /// Registers with email and password. Missing values are generated.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let (client, email, password) = client.register(None, None, None).await?;
    /// # Ok(()) }
    /// ```
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

    /// Signs in with email and password.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?
    ///     .login("me@example.com".into(), "password".into()).await?;
    /// assert!(client.is_authenticated());
    /// # Ok(()) }
    /// ```
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
    /// Returns the signed-in user.
    ///
    /// ```no_run
    /// fn email(client: &streamable::AuthenticatedStreamableClient) {
    ///     println!("{}", client.user().email);
    /// }
    /// ```
    #[must_use]
    pub const fn user(&self) -> &models::AuthenticatedUser {
        &self.state.user
    }

    /// Returns `true` for a signed-in client.
    ///
    /// ```no_run
    /// fn check(client: &streamable::AuthenticatedStreamableClient) {
    ///     assert!(client.is_authenticated());
    /// }
    /// ```
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        true
    }

    /// Fetches and stores the signed-in user.
    ///
    /// ```no_run
    /// # async fn run(mut client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// println!("{}", client.refresh_user().await?.email);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the request fails, or the response does not
    /// match the expected model.
    pub async fn refresh_user(&mut self) -> Result<&models::AuthenticatedUser> {
        self.execute_and_update_user(&models::MeRequest::new())
            .await
    }

    /// Changes the account's default privacy settings.
    ///
    /// ```no_run
    /// # async fn run(mut client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// use streamable::models::Visibility;
    /// client.change_privacy_settings(Some(false), None, Some(Visibility::Private)).await?;
    /// # Ok(()) }
    /// ```
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

    /// Changes the account password.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// client.change_password("old password", "new password").await?;
    /// # Ok(()) }
    /// ```
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

    /// Creates an account label. Spaces around the name are removed.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// let label = client.create_label("reviewed").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label already exists, the name is
    /// rejected, or the request fails.
    pub async fn create_label(&self, name: &str) -> Result<models::Label> {
        self.execute(&models::CreateLabelRequest::new(name)).await
    }

    /// Deletes an account label.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// client.delete_label(42).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label does not exist, or the request
    /// fails.
    pub async fn delete_label(&self, id: u64) -> Result<()> {
        self.execute(&models::DeleteLabelRequest::new(id)).await
    }

    /// Renames an account label. Spaces around the name are removed.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// let label = client.rename_label(42, "done").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, the label does not exist, the name is
    /// rejected, or the request fails.
    pub async fn rename_label(&self, id: u64, new_name: &str) -> Result<models::Label> {
        self.execute(&models::RenameLabelRequest::new(id, new_name))
            .await
    }

    /// Replaces a video's labels in the given order. An empty slice removes all labels.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    /// client.set_video_labels("abc123", &[3, 1]).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the session is invalid, Streamable rejects the assignment, or the
    /// request fails.
    pub async fn set_video_labels(&self, shortcode: &str, label_ids: &[u64]) -> Result<()> {
        self.execute(&models::SetVideoLabelsRequest::new(shortcode, label_ids))
            .await
    }

    /// Returns a new signed-out client.
    ///
    /// ```no_run
    /// fn logout(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
    ///     let client = client.logout()?;
    ///     assert!(!client.is_authenticated());
    ///     Ok(())
    /// }
    /// ```
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
    /// Gets a video's analytics summary. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let analytics = client.get_video_analytics("abc123").await?;
    /// println!("{} date groups", analytics.plays.len());
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the request or the response cannot be decoded.
    pub async fn get_video_analytics(
        &self,
        shortcode: &str,
    ) -> Result<models::VideoAnalyticsSummary> {
        self.execute(&models::GetVideoAnalyticsRequest::new(shortcode))
            .await
    }

    /// Gets a video's current live view count. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// println!("{}", client.get_video_live_views("abc123").await?.count);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the request or the response cannot be decoded.
    pub async fn get_video_live_views(&self, shortcode: &str) -> Result<models::VideoLiveViews> {
        self.execute(&models::GetVideoLiveViewsRequest::new(shortcode))
            .await
    }

    /// Changes the given video privacy fields. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// use streamable::{models::VideoPrivacySettingsUpdate, StreamableClient};
    /// let client = StreamableClient::new()?;
    /// client.update_video_privacy("abc123", &VideoPrivacySettingsUpdate {
    ///     allow_download: Some(false), ..Default::default()
    /// }).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the update or the request fails.
    pub async fn update_video_privacy(
        &self,
        shortcode: &str,
        settings: &models::VideoPrivacySettingsUpdate,
    ) -> Result<()> {
        self.execute(&models::UpdateVideoPrivacyRequest::new(shortcode, settings))
            .await
    }

    /// Restores a video's default privacy settings. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// client.reset_video_privacy("abc123").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the reset or the request fails.
    pub async fn reset_video_privacy(&self, shortcode: &str) -> Result<()> {
        self.execute(&models::ResetVideoPrivacyRequest::new(shortcode))
            .await
    }

    /// Gets a video. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// println!("{}", client.get_video("abc123").await?.url);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response does not match the expected video
    /// model.
    pub async fn get_video(&self, shortcode: &str) -> Result<models::Video> {
        self.execute(&models::GetVideoRequest::new(shortcode)).await
    }

    /// Deletes a video.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// client.delete_video("abc123").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, Streamable rejects the deletion, or the successful
    /// response body is not exactly `true`.
    pub async fn delete_video(&self, shortcode: &str) -> Result<()> {
        self.execute(&models::DeleteVideoRequest::new(shortcode))
            .await
    }

    /// Uploads a video. The file name is the default title.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let video = client.upload_video("video.mp4", Some("Demo".into())).await?;
    /// # Ok(()) }
    /// ```
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

    /// Uploads a video that an [`UploadCancellationToken`] can stop.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// use streamable::{StreamableClient, UploadCancellationToken};
    /// let client = StreamableClient::new()?;
    /// let token = UploadCancellationToken::new();
    /// token.cancel();
    /// let result = client.upload_video_with_cancellation("video.mp4", None, token).await;
    /// # Ok(()) }
    /// ```
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

    /// Cancels an upload by its shortcode.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// client.cancel_video_upload("abc123").await?;
    /// # Ok(()) }
    /// ```
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
