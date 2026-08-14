use super::{
    CancelVideoUploadRequest, ChangePasswordRequest, CreateLabelRequest, CreateUserRequest,
    DomainRestrictions, InitializeVideoUploadRequest, PrivacySettingsRequest, RenameLabelRequest,
    SetVideoLabelsRequest, TranscodeVideoRequest, UploadInfo, VideoAnalyticsSummary,
    VideoPasswordUpdate, VideoPrivacySettingsUpdate, Visibility,
};

#[test]
fn video_analytics_summary_deserializes_wire_fields() {
    let summary: VideoAnalyticsSummary = serde_json::from_value(serde_json::json!({
        "countries": [{ "source": "US", "count": 3 }],
        "platforms": [{ "source": "desktop", "count": 2 }],
        "referrers": [{ "source": "direct", "count": 1 }],
        "group": "day",
        "plays": [
            { "date": "2026-08-13", "count": 0 },
            { "date": "2026-08-14", "count": 3 }
        ],
        "from_date": "2026-08-13",
        "to_date": "2026-08-14"
    }))
    .expect("video analytics should deserialize");

    assert_eq!(summary.countries[0].source, "US");
    assert_eq!(summary.platforms[0].count, 2);
    assert_eq!(summary.referrers[0].source, "direct");
    assert_eq!(summary.plays[0].count, 0);
    assert_eq!(summary.from_date, "2026-08-13");
    assert_eq!(summary.to_date, "2026-08-14");
}

#[test]
fn create_user_request_serializes_static_verification_redirect() {
    let request = CreateUserRequest::new(
        "user@example.com".to_string(),
        "Password1".to_string(),
        "user".to_string(),
    );

    assert_eq!(
        serde_json::to_value(request).expect("create user request should serialize"),
        serde_json::json!({
            "email": "user@example.com",
            "password": "Password1",
            "username": "user",
            "verification_redirect": "https://streamable.com?alert=verified"
        })
    );
}

#[test]
fn visibility_serializes_as_lowercase_strings() {
    assert_eq!(
        serde_json::to_string(&Visibility::Public).expect("public visibility should serialize"),
        r#""public""#
    );
    assert_eq!(
        serde_json::to_string(&Visibility::HiddenOnStreamable)
            .expect("hidden visibility should serialize"),
        r#""hidden_on_streamable""#
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
fn video_privacy_update_serializes_only_supplied_fields() {
    let update = VideoPrivacySettingsUpdate {
        visibility: Some(Visibility::HiddenOnStreamable),
        allow_download: Some(true),
        allow_sharing: Some(false),
        domain_restrictions: Some(DomainRestrictions::Allowlist),
        allowed_domain: Some("site1.com,site2.com".to_string()),
        password: Some(VideoPasswordUpdate::Set("secret".to_string())),
        hide_view_count: Some(true),
    };

    assert_eq!(
        serde_json::to_value(update).expect("video privacy update should serialize"),
        serde_json::json!({
            "visibility": "hidden_on_streamable",
            "allow_download": true,
            "allow_sharing": false,
            "domain_restrictions": "allowlist",
            "allowed_domain": "site1.com,site2.com",
            "password": "secret",
            "hide_view_count": true
        })
    );
}

#[test]
fn video_privacy_password_removal_serializes_as_null() {
    let update = VideoPrivacySettingsUpdate {
        password: Some(VideoPasswordUpdate::Remove),
        ..VideoPrivacySettingsUpdate::default()
    };

    assert_eq!(
        serde_json::to_value(update).expect("password removal should serialize"),
        serde_json::json!({ "password": null })
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
fn set_video_labels_request_preserves_id_order_and_serializes_only_labels() {
    let request = SetVideoLabelsRequest::new("abc123", &[42, 7, 18]);

    assert_eq!(
        serde_json::to_value(request).expect("video labels request should serialize"),
        serde_json::json!({ "labels": [42, 7, 18] })
    );
}

#[test]
fn set_video_labels_request_preserves_empty_replacement() {
    let request = SetVideoLabelsRequest::new("abc123", &[]);

    assert_eq!(
        serde_json::to_value(request).expect("empty video labels request should serialize"),
        serde_json::json!({ "labels": [] })
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
            "key": "key",
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
    assert_eq!(serialized["transcoder_options"]["key"], "key");
}

#[test]
fn upload_flow_requests_preserve_live_web_fields() {
    let initialize =
        InitializeVideoUploadRequest::new("abc", 42, "video.webm".to_string(), "video".to_string());
    assert_eq!(
        serde_json::to_value(initialize).expect("initialize request should serialize"),
        serde_json::json!({
            "original_size": 42,
            "original_name": "video.webm",
            "upload_source": "web",
            "title": "video"
        })
    );

    let upload_info: UploadInfo = serde_json::from_value(serde_json::json!({
        "accelerated": false,
        "bucket": "bucket",
        "credentials": {
            "accessKeyId": "access",
            "secretAccessKey": "secret",
            "sessionToken": "session"
        },
        "fields": {
            "key": "upload/abc",
            "bucket": "bucket",
            "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
            "X-Amz-Credential": "access/20260812/us-east-1/s3/aws4_request",
            "X-Amz-Date": "20260812T100000Z",
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
        "key": "upload/abc",
        "time": 1,
        "transcoder": null,
        "transcoder_options": {
            "key": "upload/abc",
            "token": "token",
            "shortcode": "abc",
            "size": 42
        }
    }))
    .expect("upload info should deserialize");
    let cancellation = CancelVideoUploadRequest::new("abc");
    assert_eq!(
        serde_json::to_value(cancellation).expect("cancellation request should serialize"),
        serde_json::json!({})
    );
    assert_eq!(
        serde_json::to_value(TranscodeVideoRequest::new(&upload_info))
            .expect("transcode request should serialize"),
        serde_json::json!({
            "upload_source": "web",
            "key": "upload/abc",
            "token": "token",
            "shortcode": "abc",
            "size": 42
        })
    );
}
