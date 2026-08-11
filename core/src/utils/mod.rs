use rand::{Rng, RngExt, seq::SliceRandom};

use crate::constants::EMAIL_DOMAINS;

const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const MIN_CREDENTIAL_LENGTH: usize = 8;
const MAX_CREDENTIAL_LENGTH: usize = 20;

/// Generates a username as a random email address.
pub fn generate_random_username() -> String {
    let mut rng = rand::rng();
    let length = rng.random_range(MIN_CREDENTIAL_LENGTH..=MAX_CREDENTIAL_LENGTH);
    let mut local_part = Vec::with_capacity(length);

    local_part.push(random_char(&mut rng, LOWERCASE));
    local_part.push(random_char(&mut rng, DIGITS));

    let allowed = [LOWERCASE, DIGITS].concat();
    local_part.extend((local_part.len()..length).map(|_| random_char(&mut rng, &allowed)));
    local_part.shuffle(&mut rng);

    let local_part =
        String::from_utf8(local_part).expect("credential character sets contain only ASCII");
    let domain = EMAIL_DOMAINS[rng.random_range(0..EMAIL_DOMAINS.len())];

    format!("{local_part}@{domain}")
}

/// Generates an 8-20 character password with uppercase, lowercase, and numeric characters.
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

    String::from_utf8(password).expect("credential character sets contain only ASCII")
}

fn random_char(rng: &mut impl Rng, characters: &[u8]) -> u8 {
    characters[rng.random_range(0..characters.len())]
}

#[cfg(test)]
mod tests;
