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
            "key": "uploads/test.mp4",
            "token": "token",
            "shortcode": "abc",
            "size": 42
        },
        "futureField": "ignored"
    }))
    .expect("the upload fixture should deserialize")
}

fn signing_headers(host: &str, timestamp: &str, payload_hash: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, HOST, host).expect("the host should be a valid header value");
    insert_header(&mut headers, X_AMZ_CONTENT_SHA256, payload_hash)
        .expect("the payload hash should be a valid header value");
    insert_header(&mut headers, X_AMZ_DATE, timestamp)
        .expect("the timestamp should be a valid header value");
    insert_header(&mut headers, X_AMZ_SECURITY_TOKEN, "session-token")
        .expect("the token should be a valid header value");
    headers
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
fn canonical_request_and_string_to_sign_match_direct_put_reference() {
    let canonical_headers = concat!(
        "host:example-bucket.s3.amazonaws.com\n",
        "x-amz-content-sha256:UNSIGNED-PAYLOAD\n",
        "x-amz-date:20250929T151031Z\n",
        "x-amz-security-token:session-token\n",
        "x-amz-user-agent:aws-sdk-js/2.1530.0 callback\n"
    );
    let signed_headers = concat!(
        "host;x-amz-content-sha256;x-amz-date;",
        "x-amz-security-token;x-amz-user-agent"
    );
    let canonical_request = create_canonical_request(
        &Method::PUT,
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
            "c8840d1a452b0d1b4a78a19854e8f86f6174a030a02d0b2355f3d6b68665c864"
        )
    );
}

#[test]
fn signature_sorts_and_encodes_query_and_put_headers() {
    let query = QueryParameters::from([
        ("z".to_string(), "last value".to_string()),
        ("a".to_string(), "first/value".to_string()),
    ]);
    let mut headers = signing_headers(
        "example-bucket.s3.amazonaws.com",
        "20250929T151031Z",
        UNSIGNED_PAYLOAD,
    );
    insert_header(&mut headers, X_AMZ_USER_AGENT, AWS_SDK_USER_AGENT)
        .expect("the user agent should be a valid header value");
    let method = Method::PUT;
    let signature = calculate_aws_s3_v4_signature(&SigningInput {
        method: &method,
        canonical_uri: "/uploads/test video.mp4",
        access_key: "AKIDEXAMPLE",
        secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        region: "us-east-1",
        query_parameters: &query,
        headers: &headers,
    })
    .expect("the signature should be created");

    assert_eq!(
        signature.credential_scope,
        "20250929/us-east-1/s3/aws4_request"
    );
    assert_eq!(
        signature.signed_headers,
        concat!(
            "host;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent"
        )
    );
    assert_eq!(
        signature.authorization,
        concat!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20250929/us-east-1/s3/aws4_request, ",
            "SignedHeaders=host;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent, ",
            "Signature=8c9d28f6ae4768506f7c70e2e4002aec4c0a85c98edac4a6de348cdae0470540"
        )
    );
}

#[test]
fn upload_headers_match_direct_put_reference_with_initialized_timestamp() {
    let request = build_s3_put_at(&upload_info(), 42, "20250929T151031Z")
        .expect("the signed upload request should be created");
    let headers = request.headers;

    assert_eq!(
        request.url.as_str(),
        "https://example-bucket.s3.amazonaws.com/uploads/test.mp4"
    );
    assert_eq!(headers.len(), 8);
    assert_eq!(headers["Host"], "example-bucket.s3.amazonaws.com");
    assert_eq!(headers["Content-Type"], "application/octet-stream");
    assert_eq!(headers["Content-Length"], "42");
    assert_eq!(headers["x-amz-content-sha256"], UNSIGNED_PAYLOAD);
    assert_eq!(headers["x-amz-date"], "20250929T151031Z");
    assert_eq!(headers["x-amz-security-token"], "session-token");
    assert_eq!(headers["x-amz-user-agent"], AWS_SDK_USER_AGENT);
    assert_eq!(
        headers["Authorization"],
        concat!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20250929/eu-west-1/s3/aws4_request, ",
            "SignedHeaders=host;x-amz-content-sha256;x-amz-date;",
            "x-amz-security-token;x-amz-user-agent, ",
            "Signature=c391ba59cea0d2010bcfdab0b8bfa9f9391b133012d3463bcd24c249df84f4c1"
        )
    );
}

#[test]
fn upload_headers_can_refresh_the_timestamp() {
    let info = upload_info();
    let request = build_s3_put(&info, 42).expect("the signed upload request should be created");
    let timestamp = request.headers["x-amz-date"]
        .to_str()
        .expect("the timestamp should be valid ASCII");

    assert_eq!(timestamp.len(), 16);
    assert!(timestamp.ends_with('Z'));
    assert_ne!(timestamp, info.fields.x_amz_date);

    let expected = build_s3_put_at(&info, 42, timestamp)
        .expect("the refreshed timestamp should produce a valid request");

    assert_eq!(
        request.headers["Authorization"],
        expected.headers["Authorization"]
    );
}

#[test]
fn malformed_credential_returns_a_specific_error() {
    let mut info = upload_info();
    info.fields.x_amz_credential = "AKIDEXAMPLE/20250929".to_string();

    assert!(matches!(
        build_s3_put_at(&info, 42, "20250929T151031Z"),
        Err(S3Error::InvalidCredential { credential })
            if credential == "AKIDEXAMPLE/20250929"
    ));
}

#[test]
fn request_url_and_signature_share_the_same_encoded_path() {
    let mut info = upload_info();
    info.fields.key = "uploads/café video.mp4".to_string();
    let request = build_s3_put_at(&info, 42, "20250929T151031Z")
        .expect("the encoded upload request should be created");

    assert_eq!(request.url.path(), "/uploads/caf%C3%A9%20video.mp4");

    let mut signing_headers = request.headers.clone();
    signing_headers.remove(AUTHORIZATION);
    signing_headers.remove(CONTENT_TYPE);
    signing_headers.remove(CONTENT_LENGTH);
    let method = Method::PUT;
    let expected = calculate_aws_s3_v4_signature(&SigningInput {
        method: &method,
        canonical_uri: request.url.path(),
        access_key: &info.credentials.access_key_id,
        secret_key: &info.credentials.secret_access_key,
        region: "eu-west-1",
        query_parameters: &QueryParameters::new(),
        headers: &signing_headers,
    })
    .expect("the URL path should produce the transmitted signature");

    assert_eq!(request.headers[AUTHORIZATION], expected.authorization);
}
