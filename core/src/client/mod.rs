use crate::{
    constants::AUTH_BASE_URL,
    errors::{Result, StreamableError},
    models::{self, ApiRequest},
    response::ApiResponse,
    transport::{Body, DefaultTransport, HttpTransport, Request as TransportRequest},
    utils,
};
use cookie_store::CookieStore;
use file_format::FileFormat;
use http::{
    HeaderMap, HeaderValue, Method,
    header::{CONTENT_TYPE, COOKIE},
};
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use url::Url;

#[cfg(all(test, feature = "reqwest"))]
mod tests;

#[cfg(all(test, not(feature = "reqwest")))]
mod no_default_tests;

mod resources;

pub use resources::{
    Collection, CollectionDetails, CollectionPage, CollectionSummary, Label, Registration, Video,
};

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
pub type UnauthenticatedStreamableClient<T = DefaultTransport> =
    StreamableClient<Unauthenticated, T>;

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
pub type AuthenticatedStreamableClient<T = DefaultTransport> = StreamableClient<Authenticated, T>;

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Default)]
struct SessionState {
    cookies: CookieStore,
    generation: u64,
}

impl SessionState {
    fn invalidate(&mut self) {
        self.cookies = CookieStore::default();
        self.generation = self.generation.wrapping_add(1);
    }
}

#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        http.method = %method,
        url = %endpoint_url,
        request.body.kind = body.kind(),
        request.body.length = body.in_memory_len(),
    )
)]
async fn send_request<T: HttpTransport>(
    transport: &T,
    session: &Mutex<SessionState>,
    resource_generation: Option<u64>,
    method: Method,
    endpoint_url: Url,
    mut headers: HeaderMap,
    body: Body,
) -> Result<ApiResponse> {
    let (request_generation, cookie) = {
        let session = lock_unpoisoned(session);
        if resource_generation.is_some_and(|generation| generation != session.generation) {
            return Err(StreamableError::ResourceInvalidated);
        }
        (
            session.generation,
            cookie_header(&session.cookies, &endpoint_url)?,
        )
    };
    tracing::debug!(cookie.attached = cookie.is_some(), "prepared API request");
    if let Some(cookie) = cookie {
        headers.insert(COOKIE, cookie);
    }
    if matches!(body, Body::Bytes(_)) && !headers.contains_key(CONTENT_TYPE) {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }
    let request = TransportRequest {
        method,
        url: endpoint_url.clone(),
        headers,
        body,
    };
    let response = transport
        .execute(request)
        .await
        .map_err(StreamableError::transport)?;

    tracing::debug!(
        http.status = response.status.as_u16(),
        response.body.length = response.body.len(),
        "received API response"
    );

    store_response_cookies(
        session,
        request_generation,
        &endpoint_url,
        &response.headers,
    );
    Ok(ApiResponse::new(
        response.status,
        endpoint_url,
        response.body,
    ))
}

fn cookie_header(cookies: &CookieStore, url: &Url) -> Result<Option<HeaderValue>> {
    let value = cookies
        .get_request_values(url)
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    if value.is_empty() {
        return Ok(None);
    }

    HeaderValue::from_str(&value)
        .map(Some)
        .map_err(StreamableError::InvalidHeader)
}

fn store_response_cookies(
    session: &Mutex<SessionState>,
    request_generation: u64,
    url: &Url,
    headers: &HeaderMap,
) {
    let response_cookies = headers
        .get_all(http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let received = response_cookies.len();
    let mut session = lock_unpoisoned(session);
    if session.generation != request_generation {
        tracing::debug!(received, stored = 0, "ignored stale response cookies");
        return;
    }

    let stored = response_cookies
        .into_iter()
        .filter(|cookie| session.cookies.parse(cookie, url).is_ok())
        .count();
    drop(session);

    tracing::debug!(received, stored, "processed response cookies");
}

#[derive(Debug)]
enum EndpointRouting {
    Production,
    #[cfg(all(test, feature = "reqwest"))]
    Override(Url),
}

impl EndpointRouting {
    fn resolve(&self, url: &str) -> Result<Url> {
        let requested_url = Url::parse(url)?;
        match self {
            Self::Production => Ok(requested_url),
            #[cfg(all(test, feature = "reqwest"))]
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
            #[cfg(all(test, feature = "reqwest"))]
            Self::Override(url) => Some(url),
        }
    }
}

struct ClientCore<T> {
    transport: T,
    session: Mutex<SessionState>,
    endpoint_routing: EndpointRouting,
}

impl<T: HttpTransport> ClientCore<T> {
    fn invalidate_session(&self) {
        lock_unpoisoned(&self.session).invalidate();
    }

    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        self.execute_for_generation(req, None).await
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(request.type = std::any::type_name::<Req>())
    )]
    async fn execute_for_generation<Req>(
        &self,
        req: &Req,
        resource_generation: Option<u64>,
    ) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        let result = async {
            let endpoint_url = self.endpoint_routing.resolve(req.url())?;
            let response = send_request(
                &self.transport,
                &self.session,
                resource_generation,
                req.method(),
                endpoint_url,
                req.headers(),
                req.body()?,
            )
            .await?;

            req.decode_response(response)
        }
        .await;

        match &result {
            Ok(_) => tracing::debug!("completed API request"),
            Err(error) => tracing::debug!(error.kind = error.kind(), "API request failed"),
        }

        result
    }
}

struct ResourceCore<T> {
    core: Arc<ClientCore<T>>,
    generation: u64,
}

impl<T> Clone for ResourceCore<T> {
    fn clone(&self) -> Self {
        Self {
            core: Arc::clone(&self.core),
            generation: self.generation,
        }
    }
}

impl<T> ResourceCore<T> {
    fn new(core: Arc<ClientCore<T>>) -> Self {
        let generation = lock_unpoisoned(&core.session).generation;
        Self { core, generation }
    }
}

impl<T: HttpTransport> ResourceCore<T> {
    fn ensure_valid(&self) -> Result<()> {
        if lock_unpoisoned(&self.core.session).generation == self.generation {
            Ok(())
        } else {
            Err(StreamableError::ResourceInvalidated)
        }
    }

    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        self.ensure_valid()?;
        self.core
            .execute_for_generation(req, Some(self.generation))
            .await
    }
}

async fn upload_video_thumbnail<T: HttpTransport>(
    core: &ResourceCore<T>,
    shortcode: &str,
    image_file: &Path,
) -> Result<models::Video> {
    core.ensure_valid()?;
    let image_file = std::fs::canonicalize(image_file).inspect_err(|_| {
        tracing::debug!(
            error.kind = "io",
            operation = "canonicalize",
            "thumbnail upload setup failed"
        );
    })?;

    if !utils::is_image_file(&image_file) {
        tracing::debug!(
            error.kind = "invalid_image_file",
            "thumbnail upload setup failed"
        );
        return Err(StreamableError::InvalidImageFile { path: image_file });
    }

    let file_name = image_file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| StreamableError::InvalidImageFile {
            path: image_file.clone(),
        })?;
    let media_type = FileFormat::from_file(&image_file)?.media_type().to_string();

    core.execute(&models::UploadVideoThumbnailRequest::new(
        shortcode, image_file, file_name, media_type,
    ))
    .await
}

async fn cancel_video_upload<T: HttpTransport>(
    core: &ClientCore<T>,
    shortcode: &str,
) -> Result<()> {
    core.execute(&models::CancelVideoUploadRequest::new(shortcode))
        .await
}

async fn initialize_video_upload<T: HttpTransport>(
    core: &ClientCore<T>,
    upload_info: &models::UploadInfo,
    size: u64,
    original_name: String,
    title: String,
) -> Result<()> {
    core.execute(&models::InitializeVideoUploadRequest::new(
        &upload_info.shortcode,
        size,
        original_name,
        title,
    ))
    .await
}

#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(shortcode = %upload_info.shortcode, upload.bytes = size)
)]
async fn upload_video_file_to_s3<T: HttpTransport>(
    core: &ClientCore<T>,
    upload_info: &models::UploadInfo,
    size: u64,
    video_file: &Path,
) -> Result<()> {
    let signed_put = core
        .endpoint_routing
        .override_url()
        .map_or_else(
            || utils::s3::build_s3_put(upload_info, size),
            |base_url| utils::s3::build_s3_put_for_base_url(upload_info, size, base_url),
        )
        .map_err(|error| {
            tracing::debug!(
                error.kind = error.kind(),
                "object storage request signing failed"
            );
            StreamableError::UploadSigning
        })?;

    let endpoint = signed_put.url.clone();
    let request = TransportRequest {
        method: http::Method::PUT,
        url: signed_put.url,
        headers: signed_put.headers,
        body: Body::File(video_file.to_owned()),
    };
    tracing::debug!(url = %endpoint, "uploading video file to object storage");
    let response = core
        .transport
        .execute(request)
        .await
        .map_err(StreamableError::transport)?;
    tracing::debug!(
        http.status = response.status.as_u16(),
        response.body.length = response.body.len(),
        "received object storage response"
    );
    ApiResponse::new(response.status, endpoint, response.body).into_empty()
}

async fn transcode_video_after_upload<T: HttpTransport>(
    core: &ClientCore<T>,
    upload_info: &models::UploadInfo,
) -> Result<models::Video> {
    core.execute(&models::TranscodeVideoRequest::new(upload_info))
        .await
}

#[tracing::instrument(level = "debug", skip_all, fields(shortcode))]
async fn finish_upload_or_rollback<T: HttpTransport, U>(
    core: &ClientCore<T>,
    shortcode: &str,
    result: Result<U>,
) -> Result<U> {
    match result {
        Ok(value) => {
            tracing::debug!("completed video upload");
            Ok(value)
        }
        Err(error) => {
            tracing::debug!(
                error.kind = error.kind(),
                "video upload failed; cancelling allocation"
            );
            match cancel_video_upload(core, shortcode).await {
                Ok(()) => {
                    tracing::debug!("cancelled failed video upload allocation");
                    Err(error)
                }
                Err(rollback) => Err(StreamableError::UploadRollback {
                    shortcode: shortcode.to_string(),
                    source: Box::new(error),
                    rollback: Box::new(rollback),
                }),
            }
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
pub struct StreamableClient<State = Unauthenticated, T = DefaultTransport> {
    core: Arc<ClientCore<T>>,
    state: State,
}

/// An allocated Streamable upload awaiting initialization, file transfer, and transcoding.
///
/// Create one with [`StreamableClient::begin_video_upload`]. Call [`Self::complete`] for normal
/// completion or [`Self::cancel`] for explicit cleanup.
///
/// ```no_run
/// use streamable::{Result, StreamableClient, VideoUpload};
///
/// # async fn run() -> Result<()> {
/// let client = StreamableClient::new()?;
/// let upload: VideoUpload = client.begin_video_upload("video.mp4", None).await?;
/// println!("{}", upload.shortcode());
/// # Ok(()) }
/// ```
#[must_use = "the upload must be completed or explicitly cancelled"]
pub struct VideoUpload<State = Unauthenticated, T = DefaultTransport> {
    core: Arc<ClientCore<T>>,
    upload_info: models::UploadInfo,
    video_file: PathBuf,
    size: u64,
    original_name: String,
    title: String,
    state: PhantomData<State>,
}

impl<State, T> std::fmt::Debug for VideoUpload<State, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoUpload")
            .field("shortcode", &self.upload_info.shortcode)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl<State: Sync, T: HttpTransport> VideoUpload<State, T> {
    /// Returns the allocated Streamable shortcode.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// assert!(!upload.shortcode().is_empty());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn shortcode(&self) -> &str {
        &self.upload_info.shortcode
    }

    /// Creates an independent handle for cancelling this upload.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// let handle = upload.handle();
    /// assert_eq!(handle.shortcode(), upload.shortcode());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn handle(&self) -> VideoUploadHandle<State, T> {
        VideoUploadHandle {
            core: Arc::clone(&self.core),
            shortcode: self.upload_info.shortcode.clone(),
            state: PhantomData,
        }
    }

    /// Initializes the upload, streams the file to S3, and asks Streamable to transcode it.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// let video = upload.complete().await?;
    /// println!("{}", video.shortcode);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the upload failure after attempting `/cancel`. If cleanup also fails,
    /// [`StreamableError::UploadRollback`] preserves both errors.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(shortcode = %self.upload_info.shortcode, upload.bytes = self.size)
    )]
    pub async fn complete(self) -> Result<Video<State, T>> {
        let Self {
            core,
            upload_info,
            video_file,
            size,
            original_name,
            title,
            state: _,
        } = self;
        let shortcode = upload_info.shortcode.clone();
        tracing::debug!("starting allocated video upload");
        let result = async {
            initialize_video_upload(&core, &upload_info, size, original_name, title).await?;
            upload_video_file_to_s3(&core, &upload_info, size, &video_file).await?;
            transcode_video_after_upload(&core, &upload_info).await
        }
        .await;

        let data = finish_upload_or_rollback(&core, &shortcode, result).await?;
        Ok(Video::new(ResourceCore::new(core), data))
    }

    /// Cancels this allocated upload on Streamable.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// upload.cancel().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the cancellation request.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(shortcode = %self.upload_info.shortcode)
    )]
    pub async fn cancel(self) -> Result<()> {
        cancel_video_upload(&self.core, &self.upload_info.shortcode).await
    }
}

/// Lightweight cancellation handle for an allocated [`VideoUpload`].
///
/// ```no_run
/// use streamable::{Result, StreamableClient, VideoUploadHandle};
///
/// # async fn run() -> Result<()> {
/// let client = StreamableClient::new()?;
/// let upload = client.begin_video_upload("video.mp4", None).await?;
/// let handle: VideoUploadHandle = upload.handle();
/// println!("{}", handle.shortcode());
/// # Ok(()) }
/// ```
pub struct VideoUploadHandle<State = Unauthenticated, T = DefaultTransport> {
    core: Arc<ClientCore<T>>,
    shortcode: String,
    state: PhantomData<State>,
}

impl<State, T> std::fmt::Debug for VideoUploadHandle<State, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoUploadHandle")
            .field("shortcode", &self.shortcode)
            .finish_non_exhaustive()
    }
}

impl<State, T> Clone for VideoUploadHandle<State, T> {
    fn clone(&self) -> Self {
        Self {
            core: Arc::clone(&self.core),
            shortcode: self.shortcode.clone(),
            state: PhantomData,
        }
    }
}

impl<State: Sync, T: HttpTransport> VideoUploadHandle<State, T> {
    /// Returns the allocated Streamable shortcode.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// assert_eq!(upload.handle().shortcode(), upload.shortcode());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn shortcode(&self) -> &str {
        &self.shortcode
    }

    /// Cancels the allocated upload on Streamable.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// upload.handle().cancel().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the cancellation request.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(shortcode = %self.shortcode)
    )]
    pub async fn cancel(&self) -> Result<()> {
        cancel_video_upload(&self.core, &self.shortcode).await
    }
}

#[cfg(feature = "reqwest")]
impl StreamableClient<Unauthenticated, crate::transport::ReqwestTransport> {
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
        let transport =
            crate::transport::ReqwestTransport::new().map_err(StreamableError::transport)?;
        Ok(Self::with_transport_and_routing(
            transport,
            endpoint_routing,
        ))
    }
}

impl<T> StreamableClient<Unauthenticated, T> {
    /// Creates a signed-out client using a caller-supplied HTTP transport.
    ///
    /// ```
    /// use streamable::{StreamableClient, transport::ReqwestTransport};
    ///
    /// let transport = ReqwestTransport::new()?;
    /// let client = StreamableClient::with_transport(transport);
    /// assert!(!client.is_authenticated());
    /// # Ok::<(), streamable::transport::ReqwestTransportError>(())
    /// ```
    #[must_use]
    pub fn with_transport(transport: T) -> Self {
        Self::with_transport_and_routing(transport, EndpointRouting::Production)
    }

    fn with_transport_and_routing(transport: T, endpoint_routing: EndpointRouting) -> Self {
        tracing::debug!(
            transport.type = std::any::type_name::<T>(),
            endpoint.overridden = endpoint_routing.override_url().is_some(),
            "created Streamable client"
        );
        Self {
            core: Arc::new(ClientCore {
                transport,
                session: Mutex::new(SessionState::default()),
                endpoint_routing,
            }),
            state: Unauthenticated { user: None },
        }
    }
}

impl<T: HttpTransport> StreamableClient<Unauthenticated, T> {
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
    /// println!("{}", client.refresh_user().await?.total_videos);
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
    /// let registration = streamable::StreamableClient::new()?
    ///     .register(None, None, None)
    ///     .await?;
    /// println!("registered {}", registration.email());
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
    ) -> Result<Registration<T>> {
        let email = email.unwrap_or_else(utils::generate_random_username);
        let password = password.unwrap_or_else(utils::generate_random_password);
        let username = username.unwrap_or_else(|| email.clone());

        let request = models::CreateUserRequest::new(email.clone(), password.clone(), username);
        let user = self.execute(&request).await?;

        Ok(Registration::new(
            self.into_authenticated(user),
            email,
            password,
        ))
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
    ) -> Result<AuthenticatedStreamableClient<T>> {
        let request = models::LoginRequest::new(email, password);
        let user = self.execute(&request).await?;

        Ok(self.into_authenticated(user))
    }

    fn into_authenticated(
        self,
        user: models::AuthenticatedUser,
    ) -> AuthenticatedStreamableClient<T> {
        StreamableClient {
            core: self.core,
            state: Authenticated { user },
        }
    }
}

impl<T: HttpTransport> StreamableClient<Authenticated, T> {
    /// Returns the signed-in user.
    ///
    /// ```no_run
    /// fn email(client: &streamable::AuthenticatedStreamableClient) {
    ///     println!("{} has {} videos", client.user().email, client.user().total_videos);
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
    pub async fn create_label(&self, name: &str) -> Result<Label<T>> {
        let data = self.execute(&models::CreateLabelRequest::new(name)).await?;
        Ok(self.bind_label(data))
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
    pub async fn rename_label(&self, id: u64, new_name: &str) -> Result<Label<T>> {
        let data = self
            .execute(&models::RenameLabelRequest::new(id, new_name))
            .await?;
        Ok(self.bind_label(data))
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
    /// fn logout(client: streamable::AuthenticatedStreamableClient) {
    ///     let client = client.logout();
    ///     assert!(!client.is_authenticated());
    /// }
    /// ```
    #[must_use]
    pub fn logout(self) -> UnauthenticatedStreamableClient<T> {
        self.core.invalidate_session();
        StreamableClient {
            core: self.core,
            state: Unauthenticated { user: None },
        }
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
        let url = self.core.endpoint_routing.resolve(AUTH_BASE_URL)?;
        lock_unpoisoned(&self.core.session)
            .cookies
            .get_request_values(&url)
            .find_map(|(name, value)| (name == "session").then(|| value.to_string()))
            .filter(|session| !session.is_empty())
            .ok_or_else(|| StreamableError::InvalidSession {
                message: "No session cookie found. Are you logged in?".to_string(),
            })
    }
}

impl<State: Sync, T: HttpTransport> StreamableClient<State, T> {
    /// Creates a collection with the videos in the given order. Works without signing in.
    ///
    /// `None` omits the optional title from the request.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let shortcodes = vec!["first".to_string(), "second".to_string()];
    /// let collection = client.create_collection(&shortcodes, Some("Highlights")).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the videos or title, the request fails, or the
    /// response cannot be decoded.
    pub async fn create_collection(
        &self,
        shortcodes: &[String],
        title: Option<&str>,
    ) -> Result<Collection<State, T>> {
        let data = self
            .execute(&models::CreateCollectionRequest::new(shortcodes, title))
            .await?;
        let shortcode = data.shortcode.clone();
        Ok(Collection::new(
            ResourceCore::new(Arc::clone(&self.core)),
            shortcode,
            data,
        ))
    }

    /// Counts collections belonging to the current client session. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// println!("{} collections", client.count_collections().await?);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the request or the response cannot be decoded.
    pub async fn count_collections(&self) -> Result<u64> {
        self.execute(&models::CountCollectionsRequest).await
    }

    /// Lists collections belonging to the current client session. Works without signing in.
    ///
    /// `None` omits the corresponding query parameter. Passing `None` for both uses the service
    /// defaults.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let page = client.list_collections(Some(1), Some(20)).await?;
    /// println!("{} collections on this page", page.collections.len());
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the pagination or the response cannot be decoded.
    pub async fn list_collections(
        &self,
        page: Option<u32>,
        count: Option<u32>,
    ) -> Result<CollectionPage<State, T>> {
        let page = self
            .execute(&models::ListCollectionsRequest::new(page, count))
            .await?;
        let collections = page
            .collections
            .into_iter()
            .map(|data| {
                let shortcode = data.shortcode.clone();
                Collection::new(ResourceCore::new(Arc::clone(&self.core)), shortcode, data)
            })
            .collect();
        Ok(CollectionPage::new(collections))
    }

    /// Gets public collection details. An authenticated owner also receives owner fields.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let collection = client.get_collection("shared1").await?;
    /// println!("{} videos", collection.videos.len());
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the collection does not exist, Streamable rejects the request, or the
    /// response cannot be decoded.
    pub async fn get_collection(&self, shortcode: &str) -> Result<CollectionDetails<State, T>> {
        let data = self
            .execute(&models::GetCollectionRequest::new(shortcode))
            .await?;
        Ok(Collection::new(
            ResourceCore::new(Arc::clone(&self.core)),
            shortcode.to_string(),
            data,
        ))
    }

    /// Replaces a collection title. Works without signing in.
    ///
    /// The collection's video membership is omitted and remains unchanged.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
    /// client.update_collection_title("shared1", "Highlights").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the collection does not exist, Streamable rejects the title, or the
    /// response cannot be decoded.
    pub async fn update_collection_title(
        &self,
        shortcode: &str,
        title: &str,
    ) -> Result<Collection<State, T>> {
        let data = self
            .execute(&models::UpdateCollectionTitleRequest::new(shortcode, title))
            .await?;
        Ok(Collection::new(
            ResourceCore::new(Arc::clone(&self.core)),
            shortcode.to_string(),
            data,
        ))
    }

    /// Replaces a collection's complete video membership in the given order. Works without
    /// signing in.
    ///
    /// The collection title is omitted and remains unchanged. An empty slice sends an empty
    /// replacement.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
    /// let shortcodes = vec!["second".to_string(), "first".to_string()];
    /// client.replace_collection_videos("shared1", &shortcodes).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the collection does not exist, Streamable rejects the membership, or
    /// the response cannot be decoded.
    pub async fn replace_collection_videos(
        &self,
        shortcode: &str,
        shortcodes: &[String],
    ) -> Result<Collection<State, T>> {
        let data = self
            .execute(&models::ReplaceCollectionVideosRequest::new(
                shortcode, shortcodes,
            ))
            .await?;
        Ok(Collection::new(
            ResourceCore::new(Arc::clone(&self.core)),
            shortcode.to_string(),
            data,
        ))
    }

    /// Deletes a collection without deleting its member videos. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
    /// client.delete_collection("shared1").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the collection does not exist or Streamable rejects the deletion.
    pub async fn delete_collection(&self, shortcode: &str) -> Result<()> {
        self.execute(&models::DeleteCollectionRequest::new(shortcode))
            .await
    }

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
    pub async fn get_video(&self, shortcode: &str) -> Result<Video<State, T>> {
        let data = self
            .execute(&models::GetVideoRequest::new(shortcode))
            .await?;
        Ok(Video::new(ResourceCore::new(Arc::clone(&self.core)), data))
    }

    /// Deletes a video. Works without signing in.
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

    /// Uploads a video. Works without signing in; the file name is the default title.
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
    ) -> Result<Video<State, T>> {
        self.begin_video_upload(video_file, title)
            .await?
            .complete()
            .await
    }

    /// Allocates a video upload without transferring the file. Works without signing in.
    ///
    /// ```no_run
    /// # async fn run() -> streamable::Result<()> {
    /// let client = streamable::StreamableClient::new()?;
    /// let upload = client.begin_video_upload("video.mp4", None).await?;
    /// println!("allocated {}", upload.shortcode());
    /// upload.cancel().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the path is invalid or shortcode allocation fails.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn begin_video_upload(
        &self,
        video_file: impl AsRef<Path>,
        title: Option<String>,
    ) -> Result<VideoUpload<State, T>> {
        let video_file = std::fs::canonicalize(video_file.as_ref()).inspect_err(|_| {
            tracing::debug!(
                error.kind = "io",
                operation = "canonicalize",
                "upload setup failed"
            );
        })?;
        let metadata = std::fs::metadata(&video_file).inspect_err(|_| {
            tracing::debug!(
                error.kind = "io",
                operation = "metadata",
                "upload setup failed"
            );
        })?;

        if !utils::is_video_file(&video_file) {
            tracing::debug!(error.kind = "invalid_video_file", "upload setup failed");
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
        tracing::debug!(upload.bytes = size, "validated video upload file");
        let upload_info = self.generate_shortcode(size).await?;

        Ok(VideoUpload {
            core: Arc::clone(&self.core),
            upload_info,
            video_file,
            size,
            original_name,
            title,
            state: PhantomData,
        })
    }

    /// Cancels an upload by its shortcode. Works without signing in.
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
        cancel_video_upload(&self.core, shortcode).await
    }

    async fn generate_shortcode(&self, size: u64) -> Result<models::UploadInfo> {
        self.execute(&models::ShortcodeRequest::new(size)).await
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(request.type = std::any::type_name::<Req>())
    )]
    async fn execute<Req>(&self, req: &Req) -> Result<Req::Response>
    where
        Req: ApiRequest + Sync,
    {
        self.core.execute(req).await
    }
}
