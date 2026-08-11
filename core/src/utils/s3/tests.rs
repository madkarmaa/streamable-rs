use super::*;
use crate::models::UploadInfo;

fn upload_info() -> UploadInfo {
    serde_json::from_value(serde_json::json!({
        "accelerated": false,
        "bucket": "example-bucket",
        "credentials": {
            "accessKeyId": "AKIDEXAMPLE",
            "secretAccessKey": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "sessionToken": "session-token"
        },
        "fields": {
            "key": "uploads/test.mp4",
            "acl": "private",
            "bucket": "example-bucket",
            "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
            "X-Amz-Credential": "AKIDEXAMPLE/20250929/eu-west-1/s3/aws4_request",
            "X-Amz-Date": "20250929T151031Z",
            "X-Amz-Security-Token": "session-token",
            "Policy": "policy",
            "X-Amz-Signature": "original",
            "futureField": "ignored"
        },
        "url": "https://example.invalid/upload",
        "video": {
            "shortcode": "abc",
            "date_added": 1,
            "url": "https://streamable.com/abc",
            "futureField": "ignored"
        },
        "options": { "preset": "mp4", "shortcode": "abc", "screenshot": true },
        "shortcode": "abc",
        "key": "uploads/test.mp4",
        "time": 1,
        "transcoder": null,
        "transcoder_options": {
            "url": "https://example.invalid/transcode",
            "token": "token",
            "shortcode": "abc",
            "size": 42
        },
        "futureField": "ignored"
    }))
    .expect("the upload fixture should deserialize")
}

#[test]
fn uri_encoding_matches_aws_rules() {
    assert_eq!(uri_encode("a b/c+~", true), "a%20b%2Fc%2B~");
    assert_eq!(uri_encode("a b/c+~", false), "a%20b/c%2B~");
    assert_eq!(uri_encode("café", true), "caf%C3%A9");
}

#[test]
fn signing_key_matches_python_reference() {
    let signing_key = get_signature_key(
        "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        "20250929",
        "eu-west-1",
        "s3",
    )
    .expect("the HMAC key should be accepted");

    assert_eq!(
        encode_hex(&signing_key),
        "05f0db29773f507257fd5c201ba6144d5e22649594a55586a9704c20915d1172"
    );
}

#[test]
fn canonical_request_and_string_to_sign_match_python_reference() {
    let canonical_headers = concat!(
        "host:example-bucket.s3.amazonaws.com\n",
        "x-amz-acl:private\n",
        "x-amz-content-sha256:UNSIGNED-PAYLOAD\n",
        "x-amz-date:20250929T151031Z\n",
        "x-amz-security-token:session-token\n",
        "x-amz-user-agent:aws-sdk-js/2.1530.0 callback\n"
    );
    let signed_headers = concat!(
        "host;x-amz-acl;x-amz-content-sha256;x-amz-date;",
        "x-amz-security-token;x-amz-user-agent"
    );
    let canonical_request = create_canonical_request(
        "PUT",
        "/uploads/test.mp4",
        "",
        canonical_headers,
        signed_headers,
        UNSIGNED_PAYLOAD,
    );

    assert_eq!(
        create_string_to_sign(
            "20250929T151031Z",
            "20250929/eu-west-1/s3/aws4_request",
            &canonical_request
        ),
        concat!(
            "AWS4-HMAC-SHA256\n",
            "20250929T151031Z\n",
            "20250929/eu-west-1/s3/aws4_request\n",
            "5586760cfbb3bd8458b097cf9d943d5cf09e541e3ab066e49ceac8f0d497d607"
        )
    );
}

#[test]
fn signature_sorts_and_encodes_query_and_extra_headers() {
    let query = StringMap::from([
        ("z".to_string(), "last value".to_string()),
        ("a".to_string(), "first/value".to_string()),
    ]);
    let extra_headers = StringMap::from([
        ("X-Amz-ACL".to_string(), "private".to_string()),
        (
            "x-amz-user-agent".to_string(),
            AWS_SDK_USER_AGENT.to_string(),
        ),
    ]);
    let (authorization, signed_headers, credential_scope) = calculate_aws_s3_v4_signature(
        "PUT",
        "example-bucket.s3.amazonaws.com",
        "/uploads/test video.mp4",
        "AKIDEXAMPLE",
        "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        "session-token",
        "us-east-1",
        "20250929T151031Z",
        None,
        Some(&query),
        Some(&extra_headers),
    )
    .expect("the signature should be created");

    assert_eq!(credential_scope, "20250929/us-east-1/s3/aws4_request");
    assert_eq!(
        signed_headers,
        concat!(
            "host;x-amz-acl;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent"
        )
    );
    assert_eq!(
        authorization,
        concat!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20250929/us-east-1/s3/aws4_request, ",
            "SignedHeaders=host;x-amz-acl;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent, ",
            "Signature=0ce0a529035f94ae2988386af87a024d6cca52502ba98bbd808826ad82daace3"
        )
    );
}

#[test]
fn upload_headers_match_python_reference_with_initialized_timestamp() {
    let headers = build_s3_upload_headers(&upload_info(), 42, false)
        .expect("the upload headers should be created");

    assert_eq!(headers.len(), 9);
    assert_eq!(headers["Host"], "example-bucket.s3.amazonaws.com");
    assert_eq!(headers["Content-Type"], "application/octet-stream");
    assert_eq!(headers["Content-Length"], "42");
    assert_eq!(headers["x-amz-content-sha256"], UNSIGNED_PAYLOAD);
    assert_eq!(headers["x-amz-date"], "20250929T151031Z");
    assert_eq!(headers["x-amz-security-token"], "session-token");
    assert_eq!(headers["x-amz-acl"], "private");
    assert_eq!(headers["x-amz-user-agent"], AWS_SDK_USER_AGENT);
    assert_eq!(
        headers["Authorization"],
        concat!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20250929/eu-west-1/s3/aws4_request, ",
            "SignedHeaders=host;x-amz-acl;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent, ",
            "Signature=169e611285f92cbaf712cb4b04b2f14e5efb02ffbaf28e8b370b08d7c54510b4"
        )
    );
}

#[test]
fn upload_headers_can_refresh_the_timestamp() {
    let info = upload_info();
    let headers =
        build_s3_upload_headers(&info, 42, true).expect("the upload headers should be created");
    let timestamp = &headers["x-amz-date"];

    assert_eq!(timestamp.len(), 16);
    assert!(timestamp.ends_with('Z'));
    assert_ne!(timestamp, &info.fields.x_amz_date);

    let extra_headers = StringMap::from([
        ("x-amz-acl".to_string(), info.fields.acl.clone()),
        (
            "x-amz-user-agent".to_string(),
            AWS_SDK_USER_AGENT.to_string(),
        ),
    ]);
    let (expected_authorization, _, _) = calculate_aws_s3_v4_signature(
        "PUT",
        "example-bucket.s3.amazonaws.com",
        "/uploads/test.mp4",
        &info.credentials.access_key_id,
        &info.credentials.secret_access_key,
        &info.credentials.session_token,
        "eu-west-1",
        timestamp,
        Some(UNSIGNED_PAYLOAD),
        None,
        Some(&extra_headers),
    )
    .expect("the refreshed timestamp should produce a valid signature");

    assert_eq!(headers["Authorization"], expected_authorization);
}

#[test]
fn malformed_credential_returns_a_specific_error() {
    let mut info = upload_info();
    info.fields.x_amz_credential = "AKIDEXAMPLE/20250929".to_string();

    assert_eq!(
        build_s3_upload_headers(&info, 42, false),
        Err(S3Error::InvalidCredential {
            credential: "AKIDEXAMPLE/20250929".to_string()
        })
    );
}
