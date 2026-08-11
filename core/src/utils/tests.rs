use super::*;

#[test]
fn generated_usernames_match_contract() {
    for _ in 0..256 {
        let username = generate_random_username();
        let (local_part, domain) = username
            .split_once('@')
            .expect("generated username must be an email address");

        assert!((MIN_CREDENTIAL_LENGTH..=MAX_CREDENTIAL_LENGTH).contains(&local_part.len()));
        assert!(local_part.bytes().any(|byte| byte.is_ascii_lowercase()));
        assert!(local_part.bytes().any(|byte| byte.is_ascii_digit()));
        assert!(
            local_part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        );
        assert!(EMAIL_DOMAINS.contains(&domain));
    }
}

#[test]
fn generated_passwords_meet_streamable_requirements() {
    for _ in 0..256 {
        let password = generate_random_password();

        assert!(password.len() >= 8);
        assert!(password.bytes().any(|byte| byte.is_ascii_uppercase()));
        assert!(password.bytes().any(|byte| byte.is_ascii_lowercase()));
        assert!(password.bytes().any(|byte| byte.is_ascii_digit()));
    }
}

#[test]
fn every_media_fixture_is_recognized_as_video() {
    let media_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../media");
    let entries = std::fs::read_dir(&media_dir).expect("media fixture directory should exist");
    let mut file_count = 0_usize;
    let mut unsupported = Vec::new();

    for entry in entries {
        let path = entry.expect("media fixture should be readable").path();
        if path.is_file() {
            file_count = file_count
                .checked_add(1)
                .expect("media fixture count should fit usize");
            if !is_video_file(&path) {
                unsupported.push(path);
            }
        }
    }

    assert!(
        file_count > 0,
        "media fixture directory should not be empty"
    );
    assert!(
        unsupported.is_empty(),
        "media fixtures should be detected as video: {unsupported:?}"
    );
}

#[test]
fn raw_hevc_is_detected_without_hevc_extension() {
    let media_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../media");
    let source = media_dir.join("hevc.hevc");
    let disguised =
        std::env::temp_dir().join(format!("streamable-rs-raw-hevc-{}.txt", std::process::id()));

    std::fs::copy(&source, &disguised).expect("HEVC fixture should copy to a .txt path");
    let detected = is_video_file(&disguised);
    std::fs::remove_file(&disguised).expect("temporary HEVC fixture should be removed");

    assert!(
        detected,
        "raw HEVC content should be detected through bytes"
    );
}
