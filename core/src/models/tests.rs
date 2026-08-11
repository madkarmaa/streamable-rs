use super::{
    ChangePasswordRequest, CreateLabelRequest, PrivacySettingsRequest, RenameLabelRequest,
    UploadInfo, Visibility,
};

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

#[test]
fn change_password_request_serializes_session_and_trimmed_passwords() {
    let request = ChangePasswordRequest::new(" mock-session ", " Password1 ", " NewPassword2 ");

    assert_eq!(
        serde_json::to_value(request).expect("change password request should serialize"),
        serde_json::json!({
            "session": "mock-session",
            "current_password": "Password1",
            "new_password": "NewPassword2"
        })
    );
}

#[test]
fn create_label_request_trims_and_serializes_name() {
    let request = CreateLabelRequest::new("  important  ");

    assert_eq!(
        serde_json::to_value(request).expect("create label request should serialize"),
        serde_json::json!({ "name": "important" })
    );
}

#[test]
fn rename_label_request_trims_and_serializes_only_name() {
    let request = RenameLabelRequest::new(174_172, "  renamed  ");

    assert_eq!(
        serde_json::to_value(request).expect("rename label request should serialize"),
        serde_json::json!({ "name": "renamed" })
    );
}

#[test]
fn upload_info_preserves_aws_wire_names() {
    let upload_info: UploadInfo = serde_json::from_value(serde_json::json!({
        "accelerated": false,
        "bucket": "bucket",
        "credentials": {
            "accessKeyId": "access",
            "secretAccessKey": "secret",
            "sessionToken": "session"
        },
        "fields": {
            "key": "key",
            "acl": "private",
            "bucket": "bucket",
            "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
            "X-Amz-Credential": "access/20250929/eu-west-1/s3/aws4_request",
            "X-Amz-Date": "20250929T151031Z",
            "X-Amz-Security-Token": "session",
            "Policy": "policy",
            "X-Amz-Signature": "signature"
        },
        "url": "url",
        "video": {
            "shortcode": "abc",
            "date_added": 1,
            "url": "video-url"
        },
        "options": { "preset": "mp4", "shortcode": "abc", "screenshot": true },
        "shortcode": "abc",
        "key": "key",
        "time": 1,
        "transcoder": null,
        "transcoder_options": {
            "url": "transcoder-url",
            "token": "token",
            "shortcode": "abc",
            "size": 42
        }
    }))
    .expect("upload info should deserialize");

    let serialized = serde_json::to_value(upload_info).expect("upload info should serialize");
    assert_eq!(serialized["credentials"]["accessKeyId"], "access");
    assert_eq!(serialized["credentials"]["secretAccessKey"], "secret");
    assert_eq!(serialized["credentials"]["sessionToken"], "session");
    assert_eq!(
        serialized["fields"]["X-Amz-Credential"],
        "access/20250929/eu-west-1/s3/aws4_request"
    );
    assert_eq!(serialized["fields"]["X-Amz-Security-Token"], "session");
    assert_eq!(serialized["fields"]["Policy"], "policy");
}
