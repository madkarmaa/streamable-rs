use super::{
    ApiRequest, COLLECTIONS_URL, CancelVideoUploadRequest, ChangePasswordRequest, Collection,
    CollectionDetails, CollectionPage, CreateCollectionRequest, CreateLabelRequest,
    CreateUserRequest, DomainRestrictions, InitializeVideoUploadRequest, ListCollectionsRequest,
    PrivacySettingsRequest, RenameLabelRequest, ReplaceCollectionVideosRequest,
    SetVideoLabelsRequest, SetVideoThumbnailFrameRequest, TranscodeVideoRequest,
    UpdateCollectionTitleRequest, UploadInfo, UploadVideoThumbnailRequest, Video,
    VideoAnalyticsSummary, VideoPasswordUpdate, VideoPrivacySettingsUpdate, Visibility,
};
use crate::transport::Body;

#[test]
fn collection_response_models_deserialize_distinct_wire_shapes() {
    let collection: Collection = serde_json::from_value(serde_json::json!({
        "shortcode": "shared1",
        "title": null,
        "videos": [{ "shortcode": "first", "title": "First", "plays": 0 }]
    }))
    .expect("collection snapshot should deserialize");
    let page: CollectionPage = serde_json::from_value(serde_json::json!({
        "collections": [{
            "shortcode": "shared1",
            "title": "Highlights",
            "created_at": "2026-08-13T10:00:00Z",
            "updated_at": "2026-08-13T11:00:00Z",
            "thumbnail_url": "https://cdn.example/thumbnail.jpg"
        }]
    }))
    .expect("collection page should deserialize");
    let details: CollectionDetails = serde_json::from_value(serde_json::json!({
        "shortcode": "shared1",
        "title": "Highlights",
        "is_owner": true,
        "white_label": false,
        "show_streamable_brand": true,
        "videos": [{
            "shortcode": "first",
            "title": "First",
            "plays": 3,
            "date_added": "2026-08-13T10:00:00Z"
        }]
    }))
    .expect("collection details should deserialize");

    assert_eq!(collection.videos[0].shortcode, "first");
    assert_eq!(
        page.collections[0].thumbnail_url.as_deref(),
        Some("https://cdn.example/thumbnail.jpg")
    );
    assert!(details.is_owner);
    assert_eq!(details.videos[0].date_added, "2026-08-13T10:00:00Z");
}

#[test]
fn video_labels_deserialize_as_id_references_and_default_empty() {
    let video = |labels: Option<serde_json::Value>| {
        let mut value = serde_json::json!({
            "shortcode": "abc123",
            "status": 2,
            "percent": 100,
            "date_added": 1,
            "url": "https://streamable.com/abc123",
            "original_name": null,
            "duration": null,
            "width": null,
            "height": null,
            "thumbnail_url": null,
            "dynamic_thumbnail_url": null,
            "thumbnail_offset": null
        });
        if let Some(labels) = labels {
            value["labels"] = labels;
        }
        serde_json::from_value::<Video>(value).expect("video snapshot should deserialize")
    };

    let labeled = video(Some(serde_json::json!([
        { "id": 42, "name": "reviewed" },
        { "id": 7 }
    ])));
    let unlabeled = video(None);

    assert_eq!(
        labeled
            .labels
            .iter()
            .map(|label| label.id)
            .collect::<Vec<_>>(),
        vec![42, 7]
    );
    assert!(unlabeled.labels.is_empty());
}

#[test]
fn create_collection_request_preserves_order_and_omits_absent_title() {
    let shortcodes = vec!["second".to_string(), "first".to_string()];

    assert_eq!(
        serde_json::to_value(CreateCollectionRequest::new(&shortcodes, None))
            .expect("collection create request should serialize"),
        serde_json::json!({ "shortcodes": ["second", "first"] })
    );
    assert_eq!(
        serde_json::to_value(CreateCollectionRequest::new(
            &shortcodes,
            Some("Highlights")
        ))
        .expect("titled collection create request should serialize"),
        serde_json::json!({
            "title": "Highlights",
            "shortcodes": ["second", "first"]
        })
    );
}

#[test]
fn collection_update_requests_keep_title_and_membership_disjoint() {
    let shortcodes = vec!["second".to_string(), "first".to_string()];

    assert_eq!(
        serde_json::to_value(UpdateCollectionTitleRequest::new("shared1", "Highlights"))
            .expect("collection title update should serialize"),
        serde_json::json!({ "title": "Highlights" })
    );
    assert_eq!(
        serde_json::to_value(ReplaceCollectionVideosRequest::new("shared1", &shortcodes))
            .expect("collection membership replacement should serialize"),
        serde_json::json!({ "shortcodes": ["second", "first"] })
    );
    assert_eq!(
        serde_json::to_value(ReplaceCollectionVideosRequest::new("shared1", &[]))
            .expect("empty collection membership replacement should serialize"),
        serde_json::json!({ "shortcodes": [] })
    );
}

#[test]
fn collection_list_request_omits_absent_pagination() {
    assert_eq!(
        ListCollectionsRequest::new(None, None).url(),
        COLLECTIONS_URL
    );
    assert_eq!(
        ListCollectionsRequest::new(Some(2), None).url(),
        format!("{COLLECTIONS_URL}?page=2")
    );
    assert_eq!(
        ListCollectionsRequest::new(None, Some(50)).url(),
        format!("{COLLECTIONS_URL}?count=50")
    );
    assert_eq!(
        ListCollectionsRequest::new(Some(2), Some(50)).url(),
        format!("{COLLECTIONS_URL}?page=2&count=50")
    );
}

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
fn thumbnail_frame_request_uses_camel_case_offset_only() {
    let request = SetVideoThumbnailFrameRequest::new("abc123", 12.5);

    assert_eq!(
        serde_json::to_value(request).expect("thumbnail frame request should serialize"),
        serde_json::json!({ "thumbOffset": 12.5 })
    );
}

#[test]
fn thumbnail_upload_request_builds_exact_screenshot_file_part() {
    let path = std::path::PathBuf::from("thumbnail.png");
    let request = UploadVideoThumbnailRequest::new(
        "abc123",
        path.clone(),
        "thumbnail.png".to_string(),
        "image/png".to_string(),
    );

    let Body::MultipartFile(file) = request
        .body()
        .expect("thumbnail upload body should be constructed")
    else {
        panic!("thumbnail upload should use a multipart file body");
    };
    assert_eq!(file.field_name, "screenshot");
    assert_eq!(file.file_name, "thumbnail.png");
    assert_eq!(file.media_type, "image/png");
    assert_eq!(file.path, path);
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
