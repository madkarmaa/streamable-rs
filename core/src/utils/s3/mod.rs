use crate::models::UploadInfo;
use hmac::{Hmac, Mac};
use reqwest::{
    Method, Url,
    header::{
        AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderMap, HeaderName, HeaderValue,
    },
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write;
use thiserror::Error;
use time::{OffsetDateTime, macros::format_description};

#[cfg(test)]
mod tests;

type HmacSha256 = Hmac<Sha256>;
type HmacDigest = [u8; 32];
type QueryParameters = BTreeMap<String, String>;

const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const AWS_SDK_USER_AGENT: &str = "aws-sdk-js/2.1530.0 callback";
const AWS_SERVICE: &str = "s3";
const AWS_REQUEST_TYPE: &str = "aws4_request";
const X_AMZ_CONTENT_SHA256: HeaderName = HeaderName::from_static("x-amz-content-sha256");
const X_AMZ_DATE: HeaderName = HeaderName::from_static("x-amz-date");
const X_AMZ_SECURITY_TOKEN: HeaderName = HeaderName::from_static("x-amz-security-token");
const X_AMZ_USER_AGENT: HeaderName = HeaderName::from_static("x-amz-user-agent");

/// Errors produced while creating a signed S3 upload request.
#[derive(Debug, Error)]
pub enum S3Error {
    #[error("the AWS signing key could not be initialized")]
    InvalidSigningKey(#[source] hmac::digest::InvalidLength),

    #[error("X-Amz-Credential does not contain a region: {credential}")]
    InvalidCredential { credential: String },

    #[error("the current UTC timestamp could not be formatted for AWS")]
    TimestampFormatting(#[source] time::error::Format),

    #[error("the S3 upload URL could not be constructed")]
    InvalidUrl(#[source] url::ParseError),

    #[error("invalid value for HTTP header {name}")]
    InvalidHeaderValue {
        name: String,
        #[source]
        source: reqwest::header::InvalidHeaderValue,
    },

    #[error("required signing header is missing: {name}")]
    MissingSigningHeader { name: String },

    #[error("signing header is not valid ASCII: {name}")]
    NonAsciiSigningHeader {
        name: String,
        #[source]
        source: reqwest::header::ToStrError,
    },
}

type Result<T> = std::result::Result<T, S3Error>;

/// Complete HTTP request components for a Streamable S3 upload.
pub struct SignedS3Put {
    pub url: Url,
    pub headers: HeaderMap,
}

struct CredentialScope<'a> {
    region: &'a str,
}

impl<'a> CredentialScope<'a> {
    fn parse(credential: &'a str) -> Result<Self> {
        let region = credential
            .split('/')
            .nth(2)
            .filter(|region| !region.is_empty())
            .ok_or_else(|| S3Error::InvalidCredential {
                credential: credential.to_string(),
            })?;

        Ok(Self { region })
    }
}

struct SigningInput<'a> {
    method: &'a Method,
    canonical_uri: &'a str,
    access_key: &'a str,
    secret_key: &'a str,
    region: &'a str,
    query_parameters: &'a QueryParameters,
    headers: &'a HeaderMap,
}

struct Signature {
    authorization: String,
    signed_headers: String,
    credential_scope: String,
}

fn sign(key: &[u8], message: &str) -> Result<HmacDigest> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(S3Error::InvalidSigningKey)?;
    mac.update(message.as_bytes());
    Ok(mac.finalize().into_bytes().into())
}

fn get_signature_key(
    secret_key: &str,
    date_stamp: &str,
    region: &str,
    service: &str,
) -> Result<HmacDigest> {
    let k_secret = format!("AWS4{secret_key}");
    let k_date = sign(k_secret.as_bytes(), date_stamp)?;
    let k_region = sign(&k_date, region)?;
    let k_service = sign(&k_region, service)?;
    sign(&k_service, AWS_REQUEST_TYPE)
}

fn uri_encode(value: &str, encode_slash: bool) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'~')
            || (!encode_slash && *byte == b'/')
        {
            encoded.push(char::from(*byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }

    encoded
}

fn create_canonical_request(
    method: &Method,
    canonical_uri: &str,
    canonical_query_string: &str,
    canonical_headers: &str,
    signed_headers: &str,
    payload_hash: &str,
) -> String {
    [
        method.as_str(),
        canonical_uri,
        canonical_query_string,
        canonical_headers,
        signed_headers,
        payload_hash,
    ]
    .join("\n")
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn create_string_to_sign(
    timestamp: &str,
    credential_scope: &str,
    canonical_request: &str,
) -> String {
    let canonical_request_hash = Sha256::digest(canonical_request.as_bytes());
    [
        "AWS4-HMAC-SHA256",
        timestamp,
        credential_scope,
        &encode_hex(&canonical_request_hash),
    ]
    .join("\n")
}

fn required_header<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Result<&'a str> {
    headers
        .get(name)
        .ok_or_else(|| S3Error::MissingSigningHeader {
            name: name.as_str().to_string(),
        })?
        .to_str()
        .map_err(|source| S3Error::NonAsciiSigningHeader {
            name: name.as_str().to_string(),
            source,
        })
}

fn canonicalize_headers(headers: &HeaderMap) -> Result<(String, String)> {
    let mut sorted_headers = headers.iter().collect::<Vec<_>>();
    sorted_headers.sort_unstable_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));

    let mut canonical_headers = String::new();
    let mut signed_headers = String::new();

    for (index, (name, value)) in sorted_headers.into_iter().enumerate() {
        let value = value
            .to_str()
            .map_err(|source| S3Error::NonAsciiSigningHeader {
                name: name.as_str().to_string(),
                source,
            })?;
        let _ = writeln!(canonical_headers, "{}:{}", name.as_str(), value.trim());
        if index != 0 {
            signed_headers.push(';');
        }
        signed_headers.push_str(name.as_str());
    }

    Ok((canonical_headers, signed_headers))
}

fn calculate_aws_s3_v4_signature(input: &SigningInput<'_>) -> Result<Signature> {
    let timestamp = required_header(input.headers, &X_AMZ_DATE)?;
    let payload_hash = required_header(input.headers, &X_AMZ_CONTENT_SHA256)?;
    let date_stamp = timestamp.chars().take(8).collect::<String>();
    let credential_scope = format!(
        "{date_stamp}/{}/{AWS_SERVICE}/{AWS_REQUEST_TYPE}",
        input.region
    );
    let canonical_query_string = input
        .query_parameters
        .iter()
        .map(|(key, value)| format!("{}={}", uri_encode(key, true), uri_encode(value, true)))
        .collect::<Vec<_>>()
        .join("&");
    let (canonical_headers, signed_headers) = canonicalize_headers(input.headers)?;
    let canonical_request = create_canonical_request(
        input.method,
        input.canonical_uri,
        &canonical_query_string,
        &canonical_headers,
        &signed_headers,
        payload_hash,
    );
    let string_to_sign = create_string_to_sign(timestamp, &credential_scope, &canonical_request);
    let signing_key = get_signature_key(input.secret_key, &date_stamp, input.region, AWS_SERVICE)?;
    let signature = encode_hex(&sign(&signing_key, &string_to_sign)?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
        input.access_key
    );

    Ok(Signature {
        authorization,
        signed_headers,
        credential_scope,
    })
}

fn insert_header(headers: &mut HeaderMap, name: HeaderName, value: &str) -> Result<()> {
    let header_value =
        HeaderValue::from_str(value).map_err(|source| S3Error::InvalidHeaderValue {
            name: name.as_str().to_string(),
            source,
        })?;
    headers.insert(name, header_value);
    Ok(())
}

/// Builds a signed `PUT` request for uploading a Streamable video to S3.
///
/// # Errors
///
/// Returns [`S3Error`] when Streamable's upload fields cannot be converted into a valid signed
/// request.
pub fn build_s3_put(upload_info: &UploadInfo, content_length: u64) -> Result<SignedS3Put> {
    let timestamp = OffsetDateTime::now_utc()
        .format(format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .map_err(S3Error::TimestampFormatting)?;

    build_s3_put_at(upload_info, content_length, &timestamp)
}

fn build_s3_put_at(
    upload_info: &UploadInfo,
    content_length: u64,
    timestamp: &str,
) -> Result<SignedS3Put> {
    let credentials = &upload_info.credentials;
    let fields = &upload_info.fields;
    let credential_scope = CredentialScope::parse(&fields.x_amz_credential)?;
    let host = format!("{}.s3.amazonaws.com", upload_info.bucket);
    let mut url = Url::parse(&format!("https://{host}")).map_err(S3Error::InvalidUrl)?;
    url.set_path(&fields.key);

    // These are the exact headers included in the signature. The same values remain in the
    // returned map, preventing signed and transmitted headers from diverging.
    let mut headers = HeaderMap::with_capacity(8);
    insert_header(&mut headers, HOST, &host)?;
    insert_header(&mut headers, X_AMZ_CONTENT_SHA256, UNSIGNED_PAYLOAD)?;
    insert_header(&mut headers, X_AMZ_DATE, timestamp)?;
    insert_header(
        &mut headers,
        X_AMZ_SECURITY_TOKEN,
        &credentials.session_token,
    )?;
    insert_header(&mut headers, X_AMZ_USER_AGENT, AWS_SDK_USER_AGENT)?;

    let method = Method::PUT;
    let query_parameters = QueryParameters::new();
    let signature = calculate_aws_s3_v4_signature(&SigningInput {
        method: &method,
        canonical_uri: url.path(),
        access_key: &credentials.access_key_id,
        secret_key: &credentials.secret_access_key,
        region: credential_scope.region,
        query_parameters: &query_parameters,
        headers: &headers,
    })?;

    insert_header(&mut headers, AUTHORIZATION, &signature.authorization)?;
    insert_header(&mut headers, CONTENT_TYPE, "application/octet-stream")?;
    insert_header(&mut headers, CONTENT_LENGTH, &content_length.to_string())?;

    Ok(SignedS3Put { url, headers })
}
