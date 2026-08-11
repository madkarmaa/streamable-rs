//! AWS S3 Signature Version 4 helpers used by Streamable uploads.
//!
//! Streamable returns temporary AWS credentials and upload fields. This module rebuilds the
//! signed `PUT` headers expected by the target S3 bucket.

use crate::models::UploadInfo;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write;
use thiserror::Error;
use time::{OffsetDateTime, macros::format_description};

type HmacSha256 = Hmac<Sha256>;

pub const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
pub const AWS_SDK_USER_AGENT: &str = "aws-sdk-js/2.1530.0 callback";

/// String map used for query parameters, extra signing headers, and completed upload headers.
pub type StringMap = BTreeMap<String, String>;

/// Errors produced while creating S3 upload headers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum S3Error {
    #[error("the AWS signing key could not be initialized")]
    InvalidSigningKey,

    #[error("X-Amz-Credential does not contain a region: {credential}")]
    InvalidCredential { credential: String },

    #[error("the current UTC timestamp could not be formatted for AWS")]
    TimestampFormatting,
}

pub type Result<T> = std::result::Result<T, S3Error>;

fn sign(key: &[u8], message: &str) -> Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| S3Error::InvalidSigningKey)?;
    mac.update(message.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn get_signature_key(
    secret_key: &str,
    date_stamp: &str,
    region: &str,
    service: &str,
) -> Result<Vec<u8>> {
    let k_secret = format!("AWS4{secret_key}");
    let k_date = sign(k_secret.as_bytes(), date_stamp)?;
    let k_region = sign(&k_date, region)?;
    let k_service = sign(&k_region, service)?;
    sign(&k_service, "aws4_request")
}

fn uri_encode(value: &str, encode_slash: bool) -> String {
    let mut encoded = String::new();

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
    method: &str,
    canonical_uri: &str,
    canonical_query_string: &str,
    canonical_headers: &str,
    signed_headers: &str,
    payload_hash: &str,
) -> String {
    [
        method,
        canonical_uri,
        canonical_query_string,
        canonical_headers,
        signed_headers,
        payload_hash,
    ]
    .join("\n")
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::new();
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

/// Calculates an AWS S3 Signature Version 4 authorization value.
///
/// `path` must already have the encoding required by the request URL. Passing `None` for
/// `payload_hash` selects [`UNSIGNED_PAYLOAD`], matching Streamable's browser upload flow.
///
/// # Errors
///
/// Returns [`S3Error::InvalidSigningKey`] if the HMAC implementation rejects a signing key.
#[allow(clippy::too_many_arguments)]
pub fn calculate_aws_s3_v4_signature(
    method: &str,
    host: &str,
    path: &str,
    access_key: &str,
    secret_key: &str,
    session_token: &str,
    region: &str,
    timestamp: &str,
    payload_hash: Option<&str>,
    query_params: Option<&StringMap>,
    extra_headers: Option<&StringMap>,
) -> Result<(String, String, String)> {
    let payload_hash = payload_hash.unwrap_or(UNSIGNED_PAYLOAD);
    let date_stamp = timestamp.chars().take(8).collect::<String>();
    let credential_scope = format!("{date_stamp}/{region}/s3/aws4_request");

    let canonical_query_string = query_params.map_or_else(String::new, |parameters| {
        parameters
            .iter()
            .map(|(key, value)| format!("{}={}", uri_encode(key, true), uri_encode(value, true)))
            .collect::<Vec<_>>()
            .join("&")
    });

    let mut headers = StringMap::from([
        ("host".to_string(), host.to_string()),
        ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
        ("x-amz-date".to_string(), timestamp.to_string()),
        (
            "x-amz-security-token".to_string(),
            session_token.to_string(),
        ),
    ]);

    if let Some(extra_headers) = extra_headers {
        for (name, value) in extra_headers {
            headers.insert(name.to_lowercase(), value.clone());
        }
    }

    let canonical_headers = headers
        .iter()
        .fold(String::new(), |mut output, (name, value)| {
            let _ = writeln!(output, "{}:{}", name, value.trim());
            output
        });
    let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical_request = create_canonical_request(
        method,
        path,
        &canonical_query_string,
        &canonical_headers,
        &signed_headers,
        payload_hash,
    );
    let string_to_sign = create_string_to_sign(timestamp, &credential_scope, &canonical_request);
    let signing_key = get_signature_key(secret_key, &date_stamp, region, "s3")?;
    let signature = encode_hex(&sign(&signing_key, &string_to_sign)?);
    let authorization_header = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    Ok((authorization_header, signed_headers, credential_scope))
}

/// Builds the complete header map for uploading a Streamable video to S3 with `PUT`.
///
/// When `use_current_timestamp` is `true`, the returned signature uses the current UTC time.
/// Actual uploads must use that setting because the timestamp supplied during initialization may
/// have expired.
///
/// # Errors
///
/// Returns [`S3Error::InvalidCredential`] if `X-Amz-Credential` has no region,
/// [`S3Error::TimestampFormatting`] if the current UTC time cannot be formatted, or
/// [`S3Error::InvalidSigningKey`] if the HMAC implementation rejects a signing key.
pub fn build_s3_upload_headers(
    upload_info: &UploadInfo,
    content_length: u64,
    use_current_timestamp: bool,
) -> Result<StringMap> {
    let credentials = &upload_info.credentials;
    let fields = &upload_info.fields;
    let host = format!("{}.s3.amazonaws.com", upload_info.bucket);
    let path = format!("/{}", fields.key);
    let region = fields
        .x_amz_credential
        .split('/')
        .nth(2)
        .filter(|region| !region.is_empty())
        .ok_or_else(|| S3Error::InvalidCredential {
            credential: fields.x_amz_credential.clone(),
        })?;
    let timestamp = if use_current_timestamp {
        OffsetDateTime::now_utc()
            .format(format_description!(
                "[year][month][day]T[hour][minute][second]Z"
            ))
            .map_err(|_| S3Error::TimestampFormatting)?
    } else {
        fields.x_amz_date.clone()
    };
    let extra_headers = StringMap::from([
        ("x-amz-acl".to_string(), fields.acl.clone()),
        (
            "x-amz-user-agent".to_string(),
            AWS_SDK_USER_AGENT.to_string(),
        ),
    ]);
    let (authorization, _, _) = calculate_aws_s3_v4_signature(
        "PUT",
        &host,
        &path,
        &credentials.access_key_id,
        &credentials.secret_access_key,
        &credentials.session_token,
        region,
        &timestamp,
        Some(UNSIGNED_PAYLOAD),
        None,
        Some(&extra_headers),
    )?;

    Ok(StringMap::from([
        ("Host".to_string(), host),
        ("Authorization".to_string(), authorization),
        (
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        ),
        ("Content-Length".to_string(), content_length.to_string()),
        (
            "x-amz-content-sha256".to_string(),
            UNSIGNED_PAYLOAD.to_string(),
        ),
        ("x-amz-date".to_string(), timestamp),
        (
            "x-amz-security-token".to_string(),
            credentials.session_token.clone(),
        ),
        ("x-amz-acl".to_string(), fields.acl.clone()),
        (
            "x-amz-user-agent".to_string(),
            AWS_SDK_USER_AGENT.to_string(),
        ),
    ]))
}

#[cfg(test)]
mod tests;
