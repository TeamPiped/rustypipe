mod date;
mod protobuf;

pub mod dictionary;

pub use date::{month_from_n, now_sec, shift_months, shift_years};
pub use protobuf::{string_from_pb, ProtoBuilder};

use std::{
    borrow::{Borrow, Cow},
    collections::BTreeMap,
    str::FromStr,
};

use base64::Engine;
use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use url::Url;

use crate::{error::Error, param::Language};

pub static VIDEO_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9_-]{11}$").unwrap());
pub static CHANNEL_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^UC[A-Za-z0-9_-]{22}$").unwrap());
pub static PLAYLIST_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:PL|RDCLAK|OLAK)[A-Za-z0-9_-]{30,50}$").unwrap());
pub static ALBUM_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^MPREb_[A-Za-z0-9_-]{11}$").unwrap());
pub static VANITY_PATH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^/?(?:(?:c/|user/)?[A-z0-9]{1,100})|(?:@[A-z0-9-_.]{1,100})$").unwrap()
});

/// Separator string for YouTube Music subtitles
pub const DOT_SEPARATOR: &str = " • ";
/// YouTube Music name (author of official playlists)
pub const YT_MUSIC_NAME: &str = "YouTube Music";
pub const VARIOUS_ARTISTS: &str = "Various Artists";
pub const PLAYLIST_ID_ALBUM_PREFIX: &str = "OLAK";

const CONTENT_PLAYBACK_NONCE_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Internal error
#[derive(thiserror::Error, Debug)]
#[error("mapping error: {0}")]
pub struct MappingError(pub(crate) Cow<'static, str>);

/// Return the given capture group that matches first in a list of regexes
pub fn get_cg_from_regexes<'a, I>(mut regexes: I, text: &str, cg: usize) -> Option<String>
where
    I: Iterator<Item = &'a Regex>,
{
    regexes
        .find_map(|pattern| pattern.captures(text))
        .map(|c| c.get(cg).unwrap().as_str().to_owned())
}

/// Return the given capture group that matches first in a list of fancy regexes
pub fn get_cg_from_fancy_regexes<'a, I>(mut regexes: I, text: &str, cg: usize) -> Option<String>
where
    I: Iterator<Item = &'a FancyRegex>,
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
pub fn url_to_params(url: &str) -> Result<(Url, BTreeMap<String, String>), Error> {
    let mut parsed_url = Url::parse(url)
        .map_err(|e| Error::Other(format!("could not parse url `{url}` err: {e}").into()))?;
    let url_params: BTreeMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    parsed_url.set_query(None);

    Ok((parsed_url, url_params))
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

/// Parse textual video length (e.g. `0:49`, `2:02` or `1:48:18`)
/// and return the duration in seconds.
pub fn parse_video_length(text: &str) -> Option<u32> {
    static VIDEO_LENGTH_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(?:(\d+):)?(\d{1,2}):(\d{2})"#).unwrap());
    VIDEO_LENGTH_REGEX.captures(text).map(|cap| {
        let hrs = cap
            .get(1)
            .and_then(|x| x.as_str().parse::<u32>().ok())
            .unwrap_or_default();
        let min = cap
            .get(2)
            .and_then(|x| x.as_str().parse::<u32>().ok())
            .unwrap_or_default();
        let sec = cap
            .get(3)
            .and_then(|x| x.as_str().parse::<u32>().ok())
            .unwrap_or_default();

        hrs * 3600 + min * 60 + sec
    })
}

pub fn parse_numeric_or_warn<F>(string: &str, warnings: &mut Vec<String>) -> Option<F>
where
    F: FromStr,
{
    let res = parse_numeric::<F>(string);
    if res.is_err() {
        warnings.push(format!("could not parse number `{string}`"));
    }
    res.ok()
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

/// Convert YouTube redirect URLs (`https://www.youtube.com/redirect?`) into regular URLs.
///
/// Also strips google analytics tracking parameters
/// (`utm_source`, `utm_medium`, `utm_campaign`, `utm_content`) because google analytics is bad.
pub fn sanitize_yt_url(url: &str) -> String {
    let mut parsed_url = ok_or_bail!(Url::parse(url), url.to_owned());

    // Convert redirect url
    if parsed_url.host_str().unwrap_or_default() == "www.youtube.com"
        && parsed_url.path() == "/redirect"
    {
        if let Some((_, url)) = parsed_url.query_pairs().find(|(k, _)| k == "q") {
            parsed_url = ok_or_bail!(Url::parse(url.as_ref()), url.to_string());
        }
    }

    // Remove GA tracking params
    if parsed_url.query().is_some() {
        let params = parsed_url
            .query_pairs()
            .filter_map(|(k, v)| match k.borrow() {
                "utm_source" | "utm_medium" | "utm_campaign" | "utm_content" => None,
                _ => Some((k.to_string(), v.to_string())),
            })
            .collect::<Vec<_>>();

        // Set empty query string if there are no parameters to prevent urls from ending with /?
        if params.is_empty() {
            parsed_url.set_query(None);
        } else {
            parsed_url
                .query_pairs_mut()
                .clear()
                .extend_pairs(params)
                .finish();
        }
    }

    parsed_url.to_string()
}

pub trait TryRemove<T> {
    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// Returns None if the index is out of bounds.
    ///
    /// Note: Because this shifts over the remaining elements, it has a
    /// worst-case performance of *O*(*n*). If you don't need the order of elements
    /// to be preserved, use [`vec_try_swap_remove`] instead.
    fn try_remove(&mut self, index: usize) -> Option<T>;

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// Returns None if the index is out of bounds.
    ///
    /// This does not preserve ordering, but is *O*(1).
    /// If you need to preserve the element order, use [`vec_try_remove`] instead.
    fn try_swap_remove(&mut self, index: usize) -> Option<T>;
}

impl<T> TryRemove<T> for Vec<T> {
    fn try_remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(self.remove(index))
        } else {
            None
        }
    }

    fn try_swap_remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(self.swap_remove(index))
        } else {
            None
        }
    }
}

/// Parse a large, textual number (e.g. `1.4M subscribers`, `22K views`)
pub fn parse_large_numstr<F>(string: &str, lang: Language) -> Option<F>
where
    F: TryFrom<u64>,
{
    let dict_entry = dictionary::entry(lang);
    let decimal_point = match dict_entry.comma_decimal {
        true => ',',
        false => '.',
    };

    let (num, mut exp, filtered) = {
        let mut buf = String::new();
        let mut filtered = String::new();
        let mut exp = 0;
        let mut after_point = false;
        for c in string.chars() {
            if c.is_ascii_digit() {
                buf.push(c);

                if after_point {
                    exp -= 1;
                }
            } else if c == decimal_point {
                after_point = true;
            } else if !matches!(c, '\u{200b}' | '.' | ',') {
                filtered.push(c);
            }
        }
        (ok_or_bail!(buf.parse::<u64>(), None), exp, filtered)
    };

    let lookup_token = |token: &str| match token {
        "K" | "k" => Some(3),
        _ => dict_entry.number_tokens.get(token).map(|t| *t as i32),
    };

    if dict_entry.by_char {
        exp += filtered
            .chars()
            .filter_map(|token| lookup_token(&token.to_string()))
            .sum::<i32>();
    } else {
        exp += filtered
            .split_whitespace()
            .filter_map(lookup_token)
            .sum::<i32>();
    }

    F::try_from(some_or_bail!(
        num.checked_mul(some_or_bail!(
            (10_u64).checked_pow(ok_or_bail!(exp.try_into(), None)),
            None
        )),
        None
    ))
    .ok()
}

/// Replace all html control characters to make a string safe for inserting into HTML.
pub fn escape_html(input: &str) -> String {
    let mut buf = String::with_capacity(input.len());

    for c in input.chars() {
        match c {
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '&' => buf.push_str("&amp;"),
            '"' => buf.push_str("&quot;"),
            '\'' => buf.push_str("&#x27;"),
            '\n' => buf.push_str("<br>"),
            _ => buf.push(c),
        };
    }
    buf
}

pub fn video_id_from_thumbnail_url(url: &str) -> Option<String> {
    static URL_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^https://i.ytimg.com/vi/([A-Za-z0-9_-]{11})/").unwrap());
    URL_REGEX
        .captures(url)
        .and_then(|cap| cap.get(1).map(|x| x.as_str().to_owned()))
}

pub fn b64_encode<T: AsRef<[u8]>>(input: T) -> String {
    base64::engine::general_purpose::STANDARD.encode(input)
}

pub fn b64_decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(input)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{fs::File, io::BufReader, path::PathBuf};

    use path_macro::path;
    use rstest::rstest;

    use super::*;

    /// Get the path of the `testfiles` directory
    pub static TESTFILES: Lazy<PathBuf> =
        Lazy::new(|| path!(env!("CARGO_MANIFEST_DIR") / "testfiles"));

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
    #[case("0:49", Some(49))]
    #[case("bla 2:02 h3llo w0rld", Some(122))]
    #[case("18:22", Some(1102))]
    #[case("1:48:18", Some(6498))]
    #[case("102:12:39", Some(367959))]
    #[case("42", None)]
    fn t_parse_video_length(#[case] text: &str, #[case] expect: Option<u32>) {
        let n = parse_video_length(text);
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
            "res: {res} not within {expect_min} and {expect_max}"
        );
    }

    #[test]
    fn t_vec_try_remove() {
        let mut v = vec![1, 2, 3];
        assert_eq!(v.try_remove(0).unwrap(), 1);
        assert_eq!(v.try_remove(1).unwrap(), 3);
        assert_eq!(v.try_remove(1), None);
    }

    #[test]
    fn t_vec_try_swap_remove() {
        let mut v = vec![1, 2, 3];
        assert_eq!(v.try_swap_remove(0).unwrap(), 1);
        assert_eq!(v.try_swap_remove(1).unwrap(), 2);
        assert_eq!(v.try_swap_remove(1), None);
    }

    #[rstest]
    #[case(
        "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbXFjbjZ6bWdHc1VFLVNBN1NiRGR1QmRuR0lGZ3xBQ3Jtc0trcG1fWHpRNlE2eGNER0ZGczFlZXM5ZlctZzFSbl8wcHdieTlTb1ktSUc5OTZxVDVQamcxdS0yRjJJelFWTGdOS09nUk8xRExqbWhOSG5MTm83WG1QQzJqZTJuT2d6cGp0cEZTWmdsal80ODk0WkNESQ&q=http%3A%2F%2Fincompetech.com%2Fmusic%2Froyalty-free%2F&v=86YLFOog4GM",
        "http://incompetech.com/music/royalty-free/",
    )]
    #[case("https://www.gnu.org", "https://www.gnu.org/")]
    #[case(
        "https://www.youtube.com/watch?v=Rp2V7d69hyM",
        "https://www.youtube.com/watch?v=Rp2V7d69hyM"
    )]
    #[case(
        "https://www.youtube.com/redirect?event=product_shelf&redir_token=QUFFLUhqbDVUMUF3SndkcDFJbzMxYkNIMDRWSzRVQU84QXxBQ3Jtc0tsQWdpaUlaMzFUQmQwSGYwR3dDRDhHWld1bFFtUmlmMng0MmxtN19iVW1EeV9oSk1Xb1VlQ1UyT2xUOWhPdUZvVEZ6UWE4Unlia3pwZXhpUmd4RVg4eWZtcHFId2RJVkMyMUFIMDhiUVUzc2x6ZVNxbw&q=https%3A%2F%2Flttstore.com%2F%3Futm_medium%3Dproduct_shelf%26utm_source%3Dyoutube%26utm_content%3DYT-AERwsnLS3vZeiqL7_mR16DPg7FPBWvP7OW-zX2M1UIPlexPS8-gpk-2c3epSZ8lJ5NYbLof0MXDKhRLCSyfOn9BYJrcG8YtpTA9VU2VXUVhhl9AKi87G_-vFhj6jcGN1CWcYYvmZYbIqA93kwkeFuUh46ntDZR1Y8p5WygwVlhfxy_BZiNbzkWw%253D&v=nFDBxBUfE74",
        "https://lttstore.com/",
    )]
    fn t_sanitize_yt_url(#[case] url: &str, #[case] expect: &str) {
        let res = sanitize_yt_url(url);
        assert_eq!(res, expect);
    }

    #[test]
    fn t_parse_large_numstr_samples() {
        let json_path = path!(*TESTFILES / "dict" / "large_number_samples.json");
        let json_file = File::open(json_path).unwrap();
        let number_samples: BTreeMap<Language, BTreeMap<u8, (String, u64)>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        number_samples.iter().for_each(|(lang, entry)| {
            entry.iter().for_each(|(_, (txt, expect))| {
                testcase_parse_large_numstr(txt, *lang, *expect);
            });
        });
    }

    #[test]
    fn t_parse_large_numstr_samples2() {
        let json_path = path!(*TESTFILES / "dict" / "large_number_samples_all.json");
        let json_file = File::open(json_path).unwrap();
        let number_samples: BTreeMap<Language, BTreeMap<String, u64>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        number_samples.iter().for_each(|(lang, entry)| {
            entry.iter().for_each(|(txt, expect)| {
                testcase_parse_large_numstr(txt, *lang, *expect);
            });
        });
    }

    fn testcase_parse_large_numstr(string: &str, lang: Language, expect: u64) {
        // Round the expected number to the amount of significant digits included
        // in the string.
        let rounded = {
            let n_significant_d = string.chars().filter(char::is_ascii_digit).count();
            let mag = (expect as f64).log10().floor();
            let factor = 10_u64.pow(1 + mag as u32 - n_significant_d as u32);
            (((expect as f64) / factor as f64).floor() as u64) * factor
        };

        let res = parse_large_numstr::<u64>(string, lang).expect(string);
        assert_eq!(res, rounded, "{string} (lang: {lang}, exact: {expect})");
    }
}
