use std::{collections::BTreeMap, str::FromStr};

use anyhow::Result;
use fancy_regex::Regex;
use rand::Rng;
use url::Url;

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

/// Generate a random string with given length and byte charset.
fn random_string(charset: &[u8], length: usize) -> String {
    let mut result = String::with_capacity(length);
    let mut rng = rand::thread_rng();

    for _ in 0..length {
        result.push(char::from(charset[rng.gen_range(0..charset.len())]));
    }

    result
}

/// Generate a 16 characters long random string used as a CPN (Content Playback Nonce)
pub fn generate_content_playback_nonce() -> String {
    random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 16)
}

/// Split an URL into its base string and parameter map
///
/// Example:
///
/// `example.com/api?k1=v1&k2=v2 => example.com/api; {k1: v1, k2: v2}`
pub fn url_to_params(url: &str) -> Result<(String, BTreeMap<String, String>)> {
    let mut parsed_url = Url::parse(url)?;
    let url_params: BTreeMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    parsed_url.set_query(None);

    Ok((parsed_url.to_string(), url_params))
}

/// Parse a string after removing all non-numeric characters
pub fn parse_numeric<F>(string: &str) -> Result<F, F::Err>
where
    F: FromStr,
{
    let mut buf = String::new();
    for c in string.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        }
    }
    buf.parse()
}

/// Parse all numbers occurring in a string and reurn them as a vec
pub fn parse_numeric_vec<F>(string: &str) -> Vec<F>
where
    F: FromStr,
{
    let mut numbers = vec![];

    let mut buf = String::new();
    for c in string.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if !buf.is_empty() {
            buf.parse::<F>().map_or((), |n| numbers.push(n));
            buf.clear();
        }
    }
    if !buf.is_empty() {
        buf.parse::<F>().map_or((), |n| numbers.push(n));
    }

    numbers
}

pub fn retry_delay(
    n_past_retries: u32,
    min_retry_interval: u32,
    max_retry_interval: u32,
    backoff_base: u32,
) -> u32 {
    let unjittered_delay = backoff_base.checked_pow(n_past_retries).unwrap_or(u32::MAX);
    let jitter_factor = rand::thread_rng().gen_range(800..1500);
    let jittered_delay = unjittered_delay
        .checked_mul(jitter_factor)
        .unwrap_or(u32::MAX);

    min_retry_interval.max(jittered_delay.min(max_retry_interval))
}

/// Removes and returns the element at position `index` within the vector,
/// shifting all elements after it to the left.
///
/// Returns None if the index is out of bounds.
///
/// Note: Because this shifts over the remaining elements, it has a
/// worst-case performance of *O*(*n*). If you don't need the order of elements
/// to be preserved, use [`vec_try_swap_remove`] instead.
pub fn vec_try_remove<T>(vec: &mut Vec<T>, index: usize) -> Option<T> {
    if index < vec.len() {
        Some(vec.remove(index))
    } else {
        None
    }
}

/// Removes an element from the vector and returns it.
///
/// The removed element is replaced by the last element of the vector.
///
/// Returns None if the index is out of bounds.
///
/// This does not preserve ordering, but is *O*(1).
/// If you need to preserve the element order, use [`vec_try_remove`] instead.
pub fn vec_try_swap_remove<T>(vec: &mut Vec<T>, index: usize) -> Option<T> {
    if index < vec.len() {
        Some(vec.swap_remove(index))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("1.000", 1000)]
    #[case("4 Hello World 2", 42)]
    fn t_parse_num(#[case] string: &str, #[case] expect: u32) {
        let n = parse_numeric::<u32>(string).unwrap();
        assert_eq!(n, expect);
    }

    #[rstest]
    #[case("15.03.2022", vec![15, 3, 2022])]
    #[case("4 Hello World 2", vec![4, 2])]
    #[case("最后更新时间：2020年1月3日", vec![2020, 1, 3])]
    fn t_parse_numeric_vec(#[case] string: &str, #[case] expect: Vec<u32>) {
        let n = parse_numeric_vec::<u32>(string);
        assert_eq!(n, expect);
    }

    #[rstest]
    #[case(0, 800, 1500)]
    #[case(1, 2400, 4500)]
    #[case(2, 7200, 13500)]
    #[case(100, 60000, 60000)]
    fn t_retry_delay(#[case] n: u32, #[case] expect_min: u32, #[case] expect_max: u32) {
        let res = retry_delay(n, 1000, 60000, 3);
        assert!(
            res >= expect_min && res <= expect_max,
            "res: {} not within {} and {}",
            res,
            expect_min,
            expect_max
        );
    }
}
