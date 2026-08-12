use crate::constants::{
    API_BASE_URL, CHANGE_PASSWORD_URL, LABELS_URL, LOGIN_URL, ME_URL, REGISTER_URL, SETTINGS_URL,
    UPLOAD_SHORTCODE_URL,
};
use crate::{
    errors::{Result as StreamableResult, StreamableError},
    response::ApiResponse,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

#[cfg(test)]
mod tests;

pub trait ApiRequest: Serialize {
    /// The specific response model expected from this request
    type Response;

    fn url(&self) -> &str;

    fn method(&self) -> reqwest::Method;

    fn prepare_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.json(self)
    }

    /// Decodes the HTTP response expected by this request.
    ///
    /// # Errors
    ///
    /// Returns a request-specific API, transport, or response decoding error.
    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response>;
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
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
pub struct MeRequest<Response> {
    #[serde(skip)]
    response: PhantomData<Response>,
}

impl<Response> MeRequest<Response> {
    #[must_use]
    pub const fn new() -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::GET
    }

    fn prepare_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub username: String,
    #[serde(skip_deserializing, default = "verification_redirect")]
    verification_redirect: &'static str,
}

const fn verification_redirect() -> &'static str {
    "https://streamable.com?alert=verified"
}

impl CreateUserRequest {
    #[must_use]
    pub const fn new(email: String, password: String, username: String) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
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
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

impl LoginRequest {
    #[must_use]
    pub const fn new(email: String, password: String) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
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
pub struct ChangePasswordRequest {
    pub session: String,
    pub current_password: String,
    pub new_password: String,
}

impl ChangePasswordRequest {
    #[must_use]
    pub fn new(session: &str, current_password: &str, new_password: &str) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
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
pub struct CreateLabelRequest {
    pub name: String,
}

impl CreateLabelRequest {
    #[must_use]
    pub fn new(name: &str) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteLabelRequest {
    #[serde(skip)]
    url: String,
    id: u64,
}

impl DeleteLabelRequest {
    #[must_use]
    pub fn new(id: u64) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::DELETE
    }

    fn prepare_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
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
pub struct RenameLabelRequest {
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    id: u64,
    pub name: String,
}

impl RenameLabelRequest {
    #[must_use]
    pub fn new(id: u64, name: &str) -> Self {
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::PATCH
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnauthenticatedUser {
    pub socket: String,
    pub total_plays: u32,
    pub total_uploads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    #[serde(flatten)]
    pub unauthenticated: UnauthenticatedUser,

    pub id: u64,
    pub user_name: String,
    pub email: String,
    pub date_added: f64,
    pub color: String,
    pub bio: String,
    pub restricted: bool,

    pub plan_name: String,
    pub plan_max_length: u32,
    pub plan_max_size: f64,

    pub privacy_settings: PrivacySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    pub allow_download: bool,
    pub allow_sharing: bool,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrivacySettingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_sharing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
}

impl PrivacySettingsRequest {
    #[must_use]
    pub const fn new(
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::PATCH
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
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

/// Video metadata returned after Streamable accepts or processes an upload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub shortcode: String,
    #[serde(default)]
    pub status: u8,
    #[serde(default)]
    pub percent: u8,
    pub date_added: i64,
    pub url: String,
    pub original_name: Option<String>,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::GET
    }

    fn prepare_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn prepare_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.into_empty()
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

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn decode_response(&self, response: ApiResponse) -> StreamableResult<Self::Response> {
        if let Some(error) = common_api_error(&response) {
            return Err(error);
        }

        response.json()
    }
}
