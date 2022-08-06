use fancy_regex::Regex;
use rand::Rng;

const CONTENT_PLAYBACK_NONCE_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Return the given capture group that matches first in a list of regexes
pub fn get_cg_from_regexes<'a, I>(mut regexes: I, text: &str, cg: usize) -> Option<String>
where
    I: Iterator<Item = &'a Regex>,
{
    regexes
        .find_map(|pattern| pattern.captures(text).ok().flatten())
        .map(|c| c.get(cg).unwrap().as_str().to_owned())
}

/// Generates a random string with given length and byte charset.
pub fn random_string(charset: &[u8], length: usize) -> String {
    let mut result = String::with_capacity(length);
    let mut rng = rand::thread_rng();

    unsafe {
        for _ in 0..length {
            result.push(char::from(
                *charset.get_unchecked(rng.gen_range(0..charset.len())),
            ));
        }
    }

    result
}

pub fn generate_content_playback_nonce() -> String {
    random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 16)
}

pub fn generate_t_parameter() -> String {
    random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 12)
}
