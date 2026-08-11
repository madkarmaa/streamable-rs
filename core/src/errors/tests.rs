use super::*;

#[test]
fn rate_limit_error_message_matches() {
    let error = StreamableError::RateLimitExceeded {
        endpoint: "https://ajax.streamable.com/check".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "Rate limit exceeded for https://ajax.streamable.com/check. Try again later."
    );
}
