use crate::constants::EMAIL_DOMAINS;
use file_format::{FileFormat, Kind};
use rand::{Rng, RngExt, prelude::IndexedRandom, seq::SliceRandom};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[cfg(test)]
mod tests;

#[allow(dead_code)]
pub(crate) mod s3;

const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const MIN_CREDENTIAL_LENGTH: usize = 8;
const MAX_CREDENTIAL_LENGTH: usize = 20;
const HEVC_PROBE_LIMIT: u64 = 8 * 1024 * 1024;

fn random_char(rng: &mut impl Rng, characters: &[u8]) -> u8 {
    characters.choose(rng).copied().unwrap_or_default()
}

/// Generates a username as a random email address.
#[must_use]
pub fn generate_random_username() -> String {
    let mut rng = rand::rng();
    let length = rng.random_range(MIN_CREDENTIAL_LENGTH..=MAX_CREDENTIAL_LENGTH);
    let mut local_part = Vec::with_capacity(length);

    local_part.push(random_char(&mut rng, LOWERCASE));
    local_part.push(random_char(&mut rng, DIGITS));

    let allowed = [LOWERCASE, DIGITS].concat();
    local_part.extend((local_part.len()..length).map(|_| random_char(&mut rng, &allowed)));
    local_part.shuffle(&mut rng);

    let local_part = local_part.into_iter().map(char::from).collect::<String>();
    let domain = EMAIL_DOMAINS
        .choose(&mut rng)
        .map_or("gmail.com", |domain| *domain);

    format!("{local_part}@{domain}")
}

/// Generates an 8-20 character password with uppercase, lowercase, and numeric characters.
#[must_use]
pub fn generate_random_password() -> String {
    let mut rng = rand::rng();
    let length = rng.random_range(MIN_CREDENTIAL_LENGTH..=MAX_CREDENTIAL_LENGTH);
    let mut password = Vec::with_capacity(length);

    password.push(random_char(&mut rng, UPPERCASE));
    password.push(random_char(&mut rng, LOWERCASE));
    password.push(random_char(&mut rng, DIGITS));

    let allowed = [UPPERCASE, LOWERCASE, DIGITS].concat();
    password.extend((password.len()..length).map(|_| random_char(&mut rng, &allowed)));
    password.shuffle(&mut rng);

    password.into_iter().map(char::from).collect()
}

#[must_use]
pub fn is_video_file(path: &Path) -> bool {
    FileFormat::from_file(path).is_ok_and(|format| {
        matches!(format.kind(), Kind::Video) || format == FileFormat::SmallWebFormat
    }) || is_raw_hevc(path)
}

fn is_raw_hevc(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };

    let mut bytes = Vec::new();
    if file.take(HEVC_PROBE_LIMIT).read_to_end(&mut bytes).is_err() {
        return false;
    }

    contains_hevc_annex_b_stream(&bytes)
}

// Sources for the Annex-B start codes, two-byte NAL header, and probe criteria:
// - https://www.rfc-editor.org/rfc/rfc7798#section-1.1.4
// - https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/hevcdec.c
// - https://stackoverflow.com/questions/73374991/how-to-understand-header-of-h-265
fn contains_hevc_annex_b_stream(bytes: &[u8]) -> bool {
    let mut has_video_parameter_set = false;
    let mut has_sequence_parameter_set = false;
    let mut has_picture_parameter_set = false;
    let mut has_random_access_picture = false;

    for window in bytes.windows(6) {
        let header = match window {
            [0, 0, 0, 1, first, second] | [0, 0, 1, first, second, _] => Some((*first, *second)),
            _ => None,
        };
        let Some((first, second)) = header else {
            continue;
        };

        let forbidden_zero_bit = first & 0x80;
        let temporal_id_plus_one = second & 0x07;
        if forbidden_zero_bit != 0 || temporal_id_plus_one == 0 {
            continue;
        }

        let nal_unit_type = (first & 0x7e) >> 1;
        match nal_unit_type {
            32 => has_video_parameter_set = true,
            33 => has_sequence_parameter_set = true,
            34 => has_picture_parameter_set = true,
            16..=23 => has_random_access_picture = true,
            _ => {}
        }

        if has_video_parameter_set
            && has_sequence_parameter_set
            && has_picture_parameter_set
            && has_random_access_picture
        {
            return true;
        }
    }

    false
}
