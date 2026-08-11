use super::*;
use serde_json::json;

fn endpoint() -> Url {
    Url::parse("https://api.example.test/resource").expect("test endpoint should be valid")
}

fn request_error() -> reqwest::Error {
    reqwest::Client::new()
        .get("://invalid")
        .build()
        .expect_err("invalid request URL should fail")
}

#[test]
fn accessors_and_text_expose_response_data() {
    let endpoint = endpoint();
    let response = ApiResponse::new(
        StatusCode::CREATED,
        endpoint.clone(),
        Bytes::from_static(b"created"),
        None,
    );

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.endpoint(), &endpoint);
    assert_eq!(response.text(), "created");
}

#[test]
fn text_replaces_invalid_utf8() {
    let response = ApiResponse::new(
        StatusCode::OK,
        endpoint(),
        Bytes::from_static(b"valid\x80text"),
        None,
    );

    assert_eq!(response.text(), "valid\u{fffd}text");
}

#[test]
fn json_deserializes_valid_body_and_reports_invalid_body() {
    let valid = ApiResponse::new(
        StatusCode::OK,
        endpoint(),
        Bytes::from_static(br#"{"name":"streamable"}"#),
        None,
    );
    let invalid = ApiResponse::new(
        StatusCode::OK,
        endpoint(),
        Bytes::from_static(b"not json"),
        None,
    );

    assert_eq!(
        valid
            .json::<serde_json::Value>()
            .expect("valid JSON should decode"),
        json!({ "name": "streamable" })
    );
    assert!(matches!(
        invalid.json::<serde_json::Value>(),
        Err(StreamableError::ResponseDecode(_))
    ));
}

#[test]
fn stored_request_errors_take_priority_over_body_decoding() {
    let response = ApiResponse::new(
        StatusCode::BAD_GATEWAY,
        endpoint(),
        Bytes::from_static(br#"{"valid":"json"}"#),
        Some(request_error()),
    );

    assert!(matches!(
        response.json::<serde_json::Value>(),
        Err(StreamableError::Request(_))
    ));
}

#[test]
fn into_empty_accepts_success_and_propagates_request_errors() {
    let success = ApiResponse::new(StatusCode::NO_CONTENT, endpoint(), Bytes::new(), None);
    let failure = ApiResponse::new(
        StatusCode::BAD_GATEWAY,
        endpoint(),
        Bytes::new(),
        Some(request_error()),
    );

    assert!(success.into_empty().is_ok());
    assert!(matches!(
        failure.into_empty(),
        Err(StreamableError::Request(_))
    ));
}

#[test]
fn api_error_parses_expected_shape_only() {
    let valid = ApiResponse::new(
        StatusCode::BAD_REQUEST,
        endpoint(),
        Bytes::from_static(br#"{"error":"ValidationError","message":"invalid input"}"#),
        None,
    );
    let invalid = ApiResponse::new(
        StatusCode::BAD_REQUEST,
        endpoint(),
        Bytes::from_static(br#"{"unexpected":true}"#),
        None,
    );

    let error = valid.api_error().expect("API error body should decode");
    assert_eq!(error.error, "ValidationError");
    assert_eq!(error.message, "invalid input");
    assert!(invalid.api_error().is_none());
}
