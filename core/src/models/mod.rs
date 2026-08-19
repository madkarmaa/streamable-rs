use crate::constants::{
    API_BASE_URL, CHANGE_PASSWORD_URL, LABELS_URL, LOGIN_URL, ME_URL, REGISTER_URL, SETTINGS_URL,
    UPLOAD_SHORTCODE_URL,
};
use crate::{
    errors::{Result as StreamableResult, StreamableError},
    response::ApiResponse,
    transport::Body,
};
use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{CACHE_CONTROL, CONTENT_TYPE, PRAGMA},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

#[cfg(test)]
mod tests;

pub(crate) trait ApiRequest: Serialize + Sized {
    type Response;

    fn url(&self) -> &str;

    fn method(&self) -> http::Method;

    fn headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    fn body(&self) -> StreamableResult<Body> {
        serde_json::to_vec(self)
            .map(|body| Body::Bytes(body.into()))
            .map_err(StreamableError::RequestEncode)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response>;
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    #[serde(default)]
    pub error: String,
    pub message: String,
}

fn common_api_error(response: &ApiResponse) -> Option<StreamableError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Some(StreamableError::RateLimitExceeded {
            endpoint: response.endpoint().to_string(),
        });
    }

    let api_error = response.api_error();
    if response.status() == StatusCode::UNAUTHORIZED
        || response.status() == StatusCode::FORBIDDEN
        || api_error
            .as_ref()
            .is_some_and(|error| error.error == "InvalidSessionError")
    {
        let message = api_error
            .map(|error| error.message)
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "Session is missing or expired.".to_string());

        return Some(StreamableError::InvalidSession { message });
    }

    None
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MeRequest<Response> {
    #[serde(skip)]
    response: PhantomData<Response>,
}

impl<Response> MeRequest<Response> {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            response: PhantomData,
        }
    }
}

impl<Response> Default for MeRequest<Response> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Response> ApiRequest for MeRequest<Response>
where
    Response: DeserializeOwned,
{
    type Response = Response;

    fn url(&self) -> &str {
        ME_URL
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreateUserRequest {
    pub(crate) email: String,
    pub(crate) password: String,
    pub(crate) username: String,
    #[serde(skip_deserializing, default = "verification_redirect")]
    verification_redirect: &'static str,
}

const fn verification_redirect() -> &'static str {
    "https://streamable.com?alert=verified"
}

impl CreateUserRequest {
    #[must_use]
    pub(crate) const fn new(email: String, password: String, username: String) -> Self {
        Self {
            email,
            password,
            username,
            verification_redirect: verification_redirect(),
        }
    }
}

impl ApiRequest for CreateUserRequest {
    type Response = AuthenticatedUser;

    fn url(&self) -> &str {
        REGISTER_URL
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if response.status() == StatusCode::BAD_REQUEST
            && response.text().contains("Email already in use")
        {
            return Err(StreamableError::EmailAlreadyInUse {
                message: response.text().into_owned(),
            });
        }

        if response.status() == StatusCode::BAD_REQUEST
            && response.text().starts_with("Password must ")
        {
            return Err(StreamableError::PasswordValidation {
                message: response.text().into_owned(),
            });
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

impl LoginRequest {
    #[must_use]
    pub(crate) const fn new(email: String, password: String) -> Self {
        Self {
            username: email,
            password,
        }
    }
}

impl ApiRequest for LoginRequest {
    type Response = AuthenticatedUser;

    fn url(&self) -> &str {
        LOGIN_URL
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if let Some(error) = response.api_error()
            && error.error == "AuthError"
            && error.message.contains("Invalid username or password")
        {
            return Err(StreamableError::InvalidCredentials {
                message: error.message,
            });
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChangePasswordRequest {
    pub(crate) session: String,
    pub(crate) current_password: String,
    pub(crate) new_password: String,
}

impl ChangePasswordRequest {
    #[must_use]
    pub(crate) fn new(session: &str, current_password: &str, new_password: &str) -> Self {
        Self {
            session: session.trim().to_string(),
            current_password: current_password.trim().to_string(),
            new_password: new_password.trim().to_string(),
        }
    }
}

impl ApiRequest for ChangePasswordRequest {
    type Response = ();

    fn url(&self) -> &str {
        CHANGE_PASSWORD_URL
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if let Some(error) = response.api_error() {
            if response.status() == StatusCode::BAD_REQUEST && error.error == "ValidationError" {
                return Err(StreamableError::PasswordValidation {
                    message: error.message,
                });
            }

            if error.error == "AuthError" {
                return Err(StreamableError::InvalidCredentials {
                    message: "Current password is incorrect.".to_string(),
                });
            }
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateLabelRequest {
    pub(crate) name: String,
}

impl CreateLabelRequest {
    #[must_use]
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.trim().to_string(),
        }
    }
}

impl ApiRequest for CreateLabelRequest {
    type Response = Label;

    fn url(&self) -> &str {
        LABELS_URL
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if response.status() == StatusCode::CONFLICT {
            return Err(StreamableError::LabelAlreadyExists {
                name: self.name.clone(),
            });
        }

        response.json()
    }
}

/// Label owned by a Streamable account.
///
/// ```
/// let label = streamable::models::Label { name: "reviewed".into(), id: 42 };
/// assert_eq!(label.id, 42);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// Label name.
    pub name: String,
    /// Server-assigned label identifier.
    pub id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeleteLabelRequest {
    #[serde(skip)]
    url: String,
    id: u64,
}

impl DeleteLabelRequest {
    #[must_use]
    pub(crate) fn new(id: u64) -> Self {
        Self {
            url: format!("{LABELS_URL}/{id}"),
            id,
        }
    }
}

impl ApiRequest for DeleteLabelRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::DELETE
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if response.status() == StatusCode::NOT_FOUND {
            return Err(StreamableError::LabelNotFound { id: self.id });
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RenameLabelRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    id: u64,
    pub(crate) name: String,
}

impl RenameLabelRequest {
    #[must_use]
    pub(crate) fn new(id: u64, name: &str) -> Self {
        Self {
            url: format!("{LABELS_URL}/{id}"),
            id,
            name: name.trim().to_string(),
        }
    }
}

impl ApiRequest for RenameLabelRequest {
    type Response = Label;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::PATCH
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if response.status() == StatusCode::NOT_FOUND {
            return Err(StreamableError::LabelNotFound { id: self.id });
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetVideoLabelsRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
    pub(crate) labels: Vec<u64>,
}

impl SetVideoLabelsRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str, label_ids: &[u64]) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/labels"),
            shortcode: shortcode.to_string(),
            labels: label_ids.to_vec(),
        }
    }
}

impl ApiRequest for SetVideoLabelsRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if !response.status().is_success() {
            return Err(StreamableError::VideoLabelAssignmentFailed {
                shortcode: self.shortcode.clone(),
                status: response.status().as_u16(),
            });
        }

        response.into_empty()
    }
}

/// User totals available without signing in.
///
/// ```
/// let user = streamable::models::UnauthenticatedUser {
///     socket: "socket-id".into(), total_plays: 10, total_uploads: 3, total_videos: 2,
/// };
/// assert_eq!(user.total_videos, 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnauthenticatedUser {
    /// Streamable socket ID.
    pub socket: String,
    /// Visible play count.
    pub total_plays: u32,
    /// Visible upload count.
    pub total_uploads: u32,
    /// Visible video count.
    pub total_videos: u32,
}

/// Data for a signed-in user.
///
/// ```no_run
/// fn show(user: &streamable::models::AuthenticatedUser) {
///     println!("{} has {} videos", user.user_name, user.total_videos);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    #[serde(flatten)]
    /// Signed-out user totals.
    pub unauthenticated: UnauthenticatedUser,

    /// Account ID.
    pub id: u64,
    /// Account username.
    pub user_name: String,
    /// Account email address.
    pub email: String,
    /// Account creation time.
    pub date_added: f64,
    /// Profile color.
    pub color: String,
    /// Profile biography.
    pub bio: String,
    /// Whether the account is restricted.
    pub restricted: bool,

    /// Current plan name.
    pub plan_name: String,
    /// Plan video length limit.
    pub plan_max_length: u32,
    /// Plan upload size limit.
    pub plan_max_size: f64,

    /// Current account privacy settings.
    pub privacy_settings: PrivacySettings,
}

impl std::ops::Deref for AuthenticatedUser {
    type Target = UnauthenticatedUser;

    fn deref(&self) -> &Self::Target {
        &self.unauthenticated
    }
}

/// Who can see a video.
///
/// ```
/// use streamable::models::Visibility;
/// let visibility = Visibility::Private;
/// assert_eq!(visibility, Visibility::Private);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Anyone can find the video.
    Public,
    /// The video is hidden on Streamable but can be embedded.
    HiddenOnStreamable,
    /// The video is private.
    Private,
}

/// Account-level privacy settings.
///
/// ```
/// use streamable::models::{PrivacySettings, Visibility};
/// let settings = PrivacySettings {
///     allow_download: false, allow_sharing: true, visibility: Visibility::Private,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    /// Whether viewers may download videos.
    pub allow_download: bool,
    /// Whether viewers may share videos.
    pub allow_sharing: bool,
    /// Default video visibility.
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PrivacySettingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allow_download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allow_sharing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) visibility: Option<Visibility>,
}

impl PrivacySettingsRequest {
    #[must_use]
    pub(crate) const fn new(
        allow_download: Option<bool>,
        allow_sharing: Option<bool>,
        visibility: Option<Visibility>,
    ) -> Self {
        Self {
            allow_download,
            allow_sharing,
            visibility,
        }
    }
}

impl ApiRequest for PrivacySettingsRequest {
    type Response = AuthenticatedUser;

    fn url(&self) -> &str {
        SETTINGS_URL
    }

    fn method(&self) -> http::Method {
        http::Method::PATCH
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}

/// Where a video can play.
///
/// ```
/// use streamable::models::DomainRestrictions;
/// let mode = DomainRestrictions::Allowlist;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainRestrictions {
    /// Allow every domain.
    Off,
    /// Allow listed domains only.
    Allowlist,
}

/// A video password change.
///
/// ```
/// use streamable::models::VideoPasswordUpdate;
/// let password = VideoPasswordUpdate::Set("secret".into());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum VideoPasswordUpdate {
    /// Set or replace the password.
    Set(String),
    /// Remove the password.
    Remove,
}

/// Privacy fields to change. `None` leaves a field unchanged.
///
/// ```
/// use streamable::models::{VideoPrivacySettingsUpdate, Visibility};
/// let update = VideoPrivacySettingsUpdate {
///     visibility: Some(Visibility::Private),
///     allow_download: Some(false),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VideoPrivacySettingsUpdate {
    /// New video visibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    /// Whether viewers may download the video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_download: Option<bool>,
    /// Whether viewers may share the video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_sharing: Option<bool>,
    /// New domain rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_restrictions: Option<DomainRestrictions>,
    /// Allowed domains, sent unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domain: Option<String>,
    /// Password to leave, set, or remove.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<VideoPasswordUpdate>,
    /// Whether the player hides the view count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_view_count: Option<bool>,
}

/// A video's current privacy settings.
///
/// ```no_run
/// fn show(settings: &streamable::models::VideoPrivacySettings) {
///     println!("private settings: {}", settings.is_custom);
/// }
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPrivacySettings {
    /// Whether viewers may download the video.
    pub allow_download: bool,
    /// Whether viewers may share the video.
    pub allow_sharing: bool,
    /// Video visibility.
    pub visibility: Visibility,
    /// Domain rule.
    pub domain_restrictions: DomainRestrictions,
    /// Allowed domains.
    pub allowed_domain: String,
    /// Whether the video has a password.
    pub password_protected: bool,
    /// Whether the player hides the view count.
    pub hide_view_count: bool,
    /// Whether these settings replace account defaults.
    pub is_custom: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateVideoPrivacyRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
    #[serde(flatten)]
    settings: VideoPrivacySettingsUpdate,
}

impl UpdateVideoPrivacyRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str, settings: &VideoPrivacySettingsUpdate) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/settings"),
            shortcode: shortcode.to_string(),
            settings: settings.clone(),
        }
    }
}

impl ApiRequest for UpdateVideoPrivacyRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::PATCH
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .api_error()
                .map(|error| error.message)
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| response.text().into_owned());
            return Err(StreamableError::VideoPrivacyUpdateFailed {
                shortcode: self.shortcode.clone(),
                status,
                message,
            });
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResetVideoPrivacyRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
}

impl ResetVideoPrivacyRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/settings"),
            shortcode: shortcode.to_string(),
        }
    }
}

impl ApiRequest for ResetVideoPrivacyRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::DELETE
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .api_error()
                .map(|error| error.message)
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| response.text().into_owned());
            return Err(StreamableError::VideoPrivacyResetFailed {
                shortcode: self.shortcode.clone(),
                status,
                message,
            });
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GetVideoRequest {
    #[serde(skip)]
    url: String,
}

impl GetVideoRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}"),
        }
    }
}

impl ApiRequest for GetVideoRequest {
    type Response = Video;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}

/// A source and its play count.
///
/// ```
/// use streamable::models::VideoAnalyticsSource;
/// let country = VideoAnalyticsSource { source: "US".into(), count: 4 };
/// assert_eq!(country.count, 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoAnalyticsSource {
    /// Source name or code.
    pub source: String,
    /// Number of plays.
    pub count: u64,
}

/// Plays recorded on one date.
///
/// ```
/// use streamable::models::VideoAnalyticsPlay;
/// let play = VideoAnalyticsPlay { date: "2026-08-14".into(), count: 2 };
/// assert_eq!(play.date, "2026-08-14");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoAnalyticsPlay {
    /// Date returned by Streamable.
    pub date: String,
    /// Number of plays.
    pub count: u64,
}

/// A video's analytics summary.
///
/// ```no_run
/// fn print_total(summary: &streamable::models::VideoAnalyticsSummary) {
///     let total: u64 = summary.plays.iter().map(|play| play.count).sum();
///     println!("{total}");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoAnalyticsSummary {
    /// Plays grouped by country.
    pub countries: Vec<VideoAnalyticsSource>,
    /// Plays grouped by platform.
    pub platforms: Vec<VideoAnalyticsSource>,
    /// Plays grouped by referrer.
    pub referrers: Vec<VideoAnalyticsSource>,
    /// Time grouping returned by Streamable.
    pub group: String,
    /// Plays grouped by date.
    pub plays: Vec<VideoAnalyticsPlay>,
    /// Start date returned by Streamable.
    pub from_date: String,
    /// End date returned by Streamable.
    pub to_date: String,
}

/// A video's current live view count.
///
/// ```
/// use streamable::models::VideoLiveViews;
/// let live = VideoLiveViews { count: 3 };
/// assert_eq!(live.count, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoLiveViews {
    /// Current live viewers.
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GetVideoAnalyticsRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
}

impl GetVideoAnalyticsRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/analytics"),
            shortcode: shortcode.to_string(),
        }
    }
}

impl ApiRequest for GetVideoAnalyticsRequest {
    type Response = VideoAnalyticsSummary;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .api_error()
                .map(|error| error.message)
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| response.text().into_owned());
            return Err(StreamableError::VideoAnalyticsFailed {
                shortcode: self.shortcode.clone(),
                status,
                message,
            });
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GetVideoLiveViewsRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
}

impl GetVideoLiveViewsRequest {
    #[must_use]
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/analytics/live"),
            shortcode: shortcode.to_string(),
        }
    }
}

impl ApiRequest for GetVideoLiveViewsRequest {
    type Response = VideoLiveViews;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .api_error()
                .map(|error| error.message)
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| response.text().into_owned());
            return Err(StreamableError::VideoLiveViewsFailed {
                shortcode: self.shortcode.clone(),
                status,
                message,
            });
        }

        response.json()
    }
}

/// Temporary AWS credentials returned for an S3 upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Credentials {
    pub(crate) access_key_id: String,
    pub(crate) secret_access_key: String,
    pub(crate) session_token: String,
}

/// S3 form fields returned while initializing an upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Fields {
    pub(crate) key: String,
    pub(crate) bucket: String,
    #[serde(rename = "X-Amz-Algorithm")]
    pub(crate) x_amz_algorithm: String,
    #[serde(rename = "X-Amz-Credential")]
    pub(crate) x_amz_credential: String,
    #[serde(rename = "X-Amz-Date")]
    pub(crate) x_amz_date: String,
    #[serde(rename = "X-Amz-Security-Token")]
    pub(crate) x_amz_security_token: String,
    #[serde(rename = "Policy")]
    pub(crate) policy: String,
    #[serde(rename = "X-Amz-Signature")]
    pub(crate) x_amz_signature: String,
}

/// Video data returned by Streamable.
///
/// ```no_run
/// fn show(video: &streamable::models::Video) {
///     println!("{}: {}", video.shortcode, video.url);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// Short ID used in URLs.
    pub shortcode: String,
    #[serde(default)]
    /// Processing status code.
    pub status: u8,
    #[serde(default)]
    /// Processing completion percentage.
    pub percent: u8,
    /// Video creation time.
    pub date_added: i64,
    /// Video URL.
    pub url: String,
    /// Original file name, when known.
    pub original_name: Option<String>,
    /// Duration in seconds when known.
    pub duration: Option<f64>,
    /// Pixel width when known.
    pub width: Option<u32>,
    /// Pixel height when known.
    pub height: Option<u32>,
    /// Video privacy, when included.
    #[serde(default)]
    pub privacy_settings: Option<VideoPrivacySettings>,
}

/// Video processing options returned while initializing an upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Options {
    pub(crate) preset: String,
    pub(crate) shortcode: String,
    pub(crate) screenshot: bool,
}

/// Transcoder configuration returned while initializing an upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TranscoderOptions {
    pub(crate) key: String,
    pub(crate) token: String,
    pub(crate) shortcode: String,
    pub(crate) size: u64,
}

/// Complete S3 upload configuration returned by Streamable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct UploadInfo {
    pub(crate) accelerated: bool,
    pub(crate) bucket: String,
    pub(crate) credentials: Credentials,
    pub(crate) fields: Fields,
    pub(crate) url: String,
    pub(crate) video: Video,
    pub(crate) options: Options,
    pub(crate) shortcode: String,
    pub(crate) key: String,
    pub(crate) time: i64,
    pub(crate) transcoder: Option<String>,
    pub(crate) transcoder_options: TranscoderOptions,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ShortcodeRequest {
    #[serde(skip)]
    url: String,
}

impl ShortcodeRequest {
    pub(crate) fn new(size: u64) -> Self {
        Self {
            url: format!("{UPLOAD_SHORTCODE_URL}?size={size}&version=unknown"),
        }
    }
}

impl ApiRequest for ShortcodeRequest {
    type Response = UploadInfo;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InitializeVideoUploadRequest {
    #[serde(skip)]
    url: String,
    original_size: u64,
    original_name: String,
    upload_source: &'static str,
    title: String,
}

impl InitializeVideoUploadRequest {
    pub(crate) fn new(
        shortcode: &str,
        original_size: u64,
        original_name: String,
        title: String,
    ) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/initialize"),
            original_size,
            original_name,
            upload_source: "web",
            title,
        }
    }
}

impl ApiRequest for InitializeVideoUploadRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CancelVideoUploadRequest {
    #[serde(skip)]
    url: String,
}

impl CancelVideoUploadRequest {
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}/cancel"),
        }
    }
}

impl ApiRequest for CancelVideoUploadRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.into_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeleteVideoRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    shortcode: String,
}

impl DeleteVideoRequest {
    pub(crate) fn new(shortcode: &str) -> Self {
        Self {
            url: format!("{API_BASE_URL}/videos/{shortcode}"),
            shortcode: shortcode.to_string(),
        }
    }
}

impl ApiRequest for DeleteVideoRequest {
    type Response = ();

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::DELETE
    }

    fn body(&self) -> StreamableResult<Body> {
        Ok(Body::Empty)
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        if response.status().is_client_error() || response.status().is_server_error() {
            return response.into_empty();
        }

        let response_body = response.text();
        if response_body != "true" {
            return Err(StreamableError::UnexpectedVideoDeletionResponse {
                shortcode: self.shortcode.clone(),
                response: response_body.into_owned(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TranscodeVideoRequest {
    #[serde(skip)]
    url: String,
    upload_source: &'static str,
    key: String,
    token: String,
    shortcode: String,
    size: u64,
}

impl TranscodeVideoRequest {
    pub(crate) fn new(upload_info: &UploadInfo) -> Self {
        let options = &upload_info.transcoder_options;
        Self {
            url: format!("{API_BASE_URL}/transcode/{}", upload_info.shortcode),
            upload_source: "web",
            key: options.key.clone(),
            token: options.token.clone(),
            shortcode: options.shortcode.clone(),
            size: options.size,
        }
    }
}

impl ApiRequest for TranscodeVideoRequest {
    type Response = Video;

    fn url(&self) -> &str {
        &self.url
    }

    fn method(&self) -> http::Method {
        http::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}
