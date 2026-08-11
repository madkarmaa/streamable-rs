use super::{PrivacySettingsRequest, Visibility};

#[test]
fn visibility_serializes_as_lowercase_strings() {
    assert_eq!(
        serde_json::to_string(&Visibility::Public).expect("public visibility should serialize"),
        r#""public""#
    );
    assert_eq!(
        serde_json::to_string(&Visibility::Private).expect("private visibility should serialize"),
        r#""private""#
    );
}

#[test]
fn privacy_settings_request_omits_none_fields() {
    let request = PrivacySettingsRequest::new(Some(false), None, None);

    assert_eq!(
        serde_json::to_value(request).expect("privacy settings request should serialize"),
        serde_json::json!({ "allow_download": false })
    );
}
