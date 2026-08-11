use crate::constants::{LOGIN_URL, REGISTER_URL, SETTINGS_URL};
use crate::{
    errors::{Result as StreamableResult, StreamableError},
    response::ApiResponse,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, ser::SerializeStruct};

#[cfg(test)]
mod tests;

pub trait ApiRequest: Serialize {
    /// The specific response model expected from this request
    type Response;

    fn url(&self) -> &'static str;

    fn method(&self) -> reqwest::Method;

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

#[derive(Debug, Clone, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub username: String,
}

impl CreateUserRequest {
    #[must_use]
    pub const fn new(email: String, password: String, username: String) -> Self {
        Self {
            email,
            password,
            username,
        }
    }
}

impl Serialize for CreateUserRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut request = serializer.serialize_struct("CreateUserRequest", 4)?;
        request.serialize_field("email", &self.email)?;
        request.serialize_field("password", &self.password)?;
        request.serialize_field("username", &self.username)?;
        request.serialize_field(
            "verification_redirect",
            "https://streamable.com?alert=verified",
        )?;
        request.end()
    }
}

impl ApiRequest for CreateUserRequest {
    type Response = AuthenticatedUser;

    fn url(&self) -> &'static str {
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

    fn url(&self) -> &'static str {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    fn url(&self) -> &'static str {
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
