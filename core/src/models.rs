use crate::constants::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeStruct};

pub trait ApiRequest: Serialize {
    /// The specific response model expected from this request
    type Response: DeserializeOwned;

    fn url(&self) -> &'static str;
    fn method(&self) -> reqwest::Method;
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub username: String,
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
        CREATE_USER_URL
    }

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

impl LoginRequest {
    pub fn new(email: String, password: String) -> Self {
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
pub struct PrivacySettings {
    pub allow_download: bool,
    pub allow_sharing: bool,
    pub hide_view_count: bool,
    pub visibility: String,
}
