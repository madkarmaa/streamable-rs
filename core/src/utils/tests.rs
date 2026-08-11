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
