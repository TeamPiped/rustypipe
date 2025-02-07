mod date;
mod protobuf;
mod visitor_data;

pub mod dictionary;
pub mod timeago;

pub use date::{now_sec, shift_months, shift_weeks_monday, shift_years};
pub use protobuf::{string_from_pb, ProtoBuilder};
pub use visitor_data::VisitorDataCache;

use std::{
    collections::BTreeMap,
    str::{FromStr, SplitWhitespace},
};

use fancy_regex::RegexBuilder;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use url::Url;

use crate::{
    error::Error,
    param::{Country, Language, COUNTRIES},
    serializer::text::TextComponent,
};

pub static VIDEO_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9_-]{11}$").unwrap());
pub static CHANNEL_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^UC[A-Za-z0-9_-]{22}$").unwrap());
pub static PLAYLIST_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:PL|RD|OLAK|UU)[A-Za-z0-9_-]{5,50}$").unwrap());
pub static ALBUM_ID_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^MPREb_[A-Za-z0-9_-]{11}$").unwrap());
pub static VANITY_PATH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^/?(?:(?:c/|user/)?[A-z0-9]{1,100})|(?:@[\w\-\.·]{1,30})$").unwrap());
pub static CHANNEL_HANDLE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^@[\w\-\.·]{1,30}$"#).unwrap());

/// Separator string for YouTube Music subtitles
pub const DOT_SEPARATOR: &str = " • ";
pub const VARIOUS_ARTISTS: &str = "Various Artists";
pub const PLAYLIST_ID_ALBUM_PREFIX: &str = "OLAK";
pub const ARTIST_DISCOGRAPHY_PREFIX: &str = "MPAD";
pub const PODCAST_PLAYLIST_PREFIX: &str = "MPSP";
pub const PODCAST_EPISODE_PREFIX: &str = "MPED";
/// Builtin user-specific playlists (Watch later, Liked videos, Liked tracks, Episodes for later)
pub static USER_PLAYLIST_IDS: [&str; 4] = ["WL", "LL", "LM", "SE"];

const CONTENT_PLAYBACK_NONCE_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Return the given capture group that matches the regex
pub fn get_cg_from_regex(regex: &Regex, text: &str, cg: usize) -> Option<String> {
    regex
        .captures(text)
        .and_then(|c| c.get(cg).map(|c| c.as_str().to_owned()))
}

/// Return the given capture group that matches first in a list of fancy regexes
pub fn get_cg_from_fancy_regexes(regexes: &[&str], text: &str, cg_name: &str) -> Option<String> {
    regexes
        .iter()
        .find_map(|pattern| {
            let re = RegexBuilder::new(pattern)
                .backtrack_limit(10_000_000)
                .build()
                .unwrap();
            re.captures(text).ok().flatten()
        })
        .and_then(|c| c.name(cg_name).map(|c| c.as_str().to_owned()))
}

/// Generate a random string with given length and byte charset.
fn random_string(charset: &[u8], length: usize) -> String {
    let mut result = String::with_capacity(length);
    let mut rng = rand::rng();

    for _ in 0..length {
        result.push(char::from(charset[rng.random_range(0..charset.len())]));
    }

    result
}

/// Generate a 16 characters long random string used as a CPN (Content Playback Nonce)
pub fn generate_content_playback_nonce() -> String {
    random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 16)
}

pub fn random_uuid() -> String {
    let mut rng = rand::rng();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rng.random::<u32>(),
        rng.random::<u16>(),
        rng.random::<u16>(),
        rng.random::<u16>(),
        rng.random::<u64>() & 0xffff_ffff_ffff,
    )
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

/// Parse a string after removing all non-numeric characters.
///
/// If the string contains multiple numbers, it returns the product of them.
pub fn parse_numeric_prod<F>(string: &str) -> Option<F>
where
    F: FromStr + Copy + std::ops::Mul<Output = F>,
{
    let mut n = None;
    let mut buf = String::new();

    for c in string.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if !buf.is_empty() {
            if let Ok(x) = buf.parse::<F>() {
                n = n.map(|n| n * x).or(Some(x));
            }
            buf.clear();
        }
    }
    if !buf.is_empty() {
        if let Ok(x) = buf.parse::<F>() {
            n = n.map(|n| n * x).or(Some(x));
        }
    }
    n
}

/// Parse all numbers occurring in a string and return them as a vec
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
            if let Ok(n) = buf.parse::<F>() {
                numbers.push(n);
            }
            buf.clear();
        }
    }
    if !buf.is_empty() {
        if let Ok(n) = buf.parse::<F>() {
            numbers.push(n);
        }
    }

    numbers
}

/// Parse textual video length (e.g. `0:49`, `2:02` or `1:48:18`)
/// and return the duration in seconds.
pub fn parse_video_length(text: &str) -> Option<u32> {
    static VIDEO_LENGTH_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?:(\d+)[:.])?(\d{1,2})[:.](\d{2})").unwrap());
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
    let jitter_factor = rand::rng().random_range(800..1500);
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
    fn try_sanitize_yt_url(url: &str) -> Option<String> {
        let mut parsed_url = Url::parse(url).ok()?;

        // Convert redirect url
        if parsed_url.host_str().unwrap_or_default() == "www.youtube.com"
            && parsed_url.path() == "/redirect"
        {
            if let Some((_, url)) = parsed_url.query_pairs().find(|(k, _)| k == "q") {
                parsed_url = Url::parse(url.as_ref()).ok()?;
            }
        }

        // Remove GA tracking params
        if parsed_url.query().is_some() {
            let params = parsed_url
                .query_pairs()
                .filter_map(|(k, v)| match k.as_ref() {
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
        Some(parsed_url.to_string())
    }

    try_sanitize_yt_url(url).unwrap_or_else(|| url.to_string())
}

pub fn div_ceil(a: u32, b: u32) -> u32 {
    let d = a / b;
    let r = a % b;

    if r > 0 && b > 0 {
        d + 1
    } else {
        d
    }
}

#[allow(dead_code)]
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

/// Check if a channel name equals "YouTube Music"
/// (the author of original YouTube Music playlists)
pub(crate) fn is_ytm(text: &TextComponent) -> bool {
    if let TextComponent::Text { text, .. } = text {
        text.starts_with("YouTube")
    } else {
        false
    }
}

/// Check if a language should be parsed by character
pub fn lang_by_char(lang: Language) -> bool {
    matches!(
        lang,
        Language::Ja | Language::ZhCn | Language::ZhHk | Language::ZhTw
    )
}

/// Parse a large, textual number (e.g. `1.4M subscribers`, `22K views`)
pub fn parse_large_numstr<F>(string: &str, lang: Language) -> Option<F>
where
    F: TryFrom<u64>,
{
    // Special case for Gujarati: the "no views" text does not contain
    // any parseable tokens: the 2 words occur in any view count text.
    // This may be a translation error.
    if lang == Language::Gu && string == "જોવાયાની સંખ્યા" {
        return 0.try_into().ok();
    }

    let dict_entry = dictionary::entry(lang);
    let by_char = lang_by_char(lang) || lang == Language::Ko;
    let decimal_point = if dict_entry.comma_decimal { ',' } else { '.' };

    let mut digits = String::new();
    let mut filtered = String::new();
    let mut exp = 0;
    let mut after_point = false;
    let mut last_number = false;

    for c in string.chars() {
        if c.is_ascii_digit() {
            digits.push(c);

            if after_point {
                exp -= 1;
            }
            if !last_number {
                filtered.push(' ');
                last_number = true;
            }
        } else if c == decimal_point && !digits.is_empty() {
            after_point = true;
        } else if !matches!(
            c,
            '\u{200b}' | '\u{202b}' | '\u{202c}' | '\u{202e}' | '\u{200e}' | '\u{200f}' | '.' | ','
        ) {
            c.to_lowercase().for_each(|c| filtered.push(c));
            last_number = false;
        }
    }

    if digits.is_empty() {
        SplitTokens::new(&filtered, by_char)
            .find_map(|token| dict_entry.number_nd_tokens.get(token))
            .and_then(|n| (u64::from(*n)).try_into().ok())
    } else {
        let num = digits.parse::<u64>().ok()?;

        exp += SplitTokens::new(&filtered, by_char)
            .filter_map(|token| match token {
                "k" => Some(3),
                _ => dict_entry.number_tokens.get(token).map(|t| i32::from(*t)),
            })
            .sum::<i32>();

        F::try_from(num.checked_mul((10_u64).checked_pow(exp.try_into().ok()?)?)?).ok()
    }
}

pub fn parse_large_numstr_or_warn<F>(
    string: &str,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Option<F>
where
    F: TryFrom<u64>,
{
    let res = parse_large_numstr::<F>(string, lang);
    if res.is_none() {
        warnings.push(format!("could not parse numstr `{string}`"));
    }
    res
}

/// Replace all html control characters to make a string safe for inserting into HTML.
pub fn escape_html(input: &str) -> String {
    let mut buf = String::with_capacity(input.len());
    escape_html_append(input, &mut buf);
    buf
}

pub fn escape_html_append(input: &str, buf: &mut String) {
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
}

/// Replace all markdown control characters to make a string safe for
/// inserting into Markdown.
pub fn escape_markdown(input: &str) -> String {
    let mut buf = String::with_capacity(input.len());
    escape_markdown_append(input, &mut buf);
    buf
}

pub fn escape_markdown_append(input: &str, buf: &mut String) {
    for c in input.chars() {
        match c {
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '\n' => buf.push_str("<br>"),
            '*' | '#' | '(' | ')' | '[' | ']' | '_' | '`' | '~' | '$' | '^' | '=' | ':' | '+'
            | '\\' => {
                buf.push('\\');
                buf.push(c);
            }
            _ => buf.push(c),
        };
    }
}

pub fn video_id_from_thumbnail_url(url: &str) -> Option<String> {
    static URL_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^https://i.ytimg.com/vi/([A-Za-z0-9_-]{11})/").unwrap());
    URL_REGEX
        .captures(url)
        .and_then(|cap| cap.get(1).map(|x| x.as_str().to_owned()))
}

pub fn b64_encode<T: AsRef<[u8]>>(input: T) -> String {
    data_encoding::BASE64URL.encode(input.as_ref())
}

pub fn b64_decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, data_encoding::DecodeError> {
    data_encoding::BASE64URL.decode(input.as_ref())
}

/// Get the country from its English name
pub fn country_from_name(name: &str) -> Option<Country> {
    COUNTRIES
        .binary_search_by_key(&name, Country::name)
        .ok()
        .map(|i| COUNTRIES[i])
}

/// Strip prefix from string if presend
pub fn strip_prefix(s: &str, prefix: &str) -> String {
    s.strip_prefix(prefix).unwrap_or(s).to_string()
}

/// An iterator over the chars in a string (in str format)
pub struct SplitChar<'a> {
    txt: &'a str,
    index: usize,
}

impl<'a> From<&'a str> for SplitChar<'a> {
    fn from(value: &'a str) -> Self {
        Self {
            txt: value,
            index: 0,
        }
    }
}

impl<'a> Iterator for SplitChar<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.txt
            .get(self.index..)
            .and_then(|txt| txt.chars().next())
            .map(|c| {
                let start = self.index;
                self.index += c.len_utf8();
                &self.txt[start..self.index]
            })
    }
}

/// An iterator for parsing strings. It can either iterate over words or characters.
pub enum SplitTokens<'a> {
    Word(SplitWhitespace<'a>),
    Char(SplitChar<'a>),
}

impl<'a> SplitTokens<'a> {
    pub fn new(s: &'a str, by_char: bool) -> Self {
        if by_char {
            Self::Char(SplitChar::from(s))
        } else {
            Self::Word(s.split_whitespace())
        }
    }
}

impl<'a> Iterator for SplitTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SplitTokens::Word(iter) => iter.next(),
            SplitTokens::Char(iter) => iter.next(),
        }
    }
}

/// Applies function to the elements of iterator and returns the first successful result
/// or the last error if the function fails on all elements. If the iterator is empty, e_empty
/// is returned.
pub fn find_map_or_last_err<I, T, P, O, E>(mut iter: I, e_empty: E, mut f: P) -> Result<O, E>
where
    I: Iterator<Item = T>,
    P: FnMut(T) -> Result<O, E>,
{
    let res = iter.try_fold(e_empty, |_, itm| match f(itm) {
        Ok(o) => Err(o),
        Err(e) => Ok(e),
    });
    match res {
        Ok(e) => Err(e),
        Err(o) => Ok(o),
    }
}

/// Map error when fetching an internal playlist
///
/// If no user is logged in, YouTube returns a "NotFound" error. This has to be corrected
/// into a NoLogin error.
#[cfg(feature = "userdata")]
pub fn map_internal_playlist_err(e: Error) -> Error {
    if let Error::Extraction(crate::error::ExtractionError::NotFound { .. }) = e {
        Error::Auth(crate::error::AuthError::NoLogin)
    } else {
        e
    }
}

/// Parse cookies from a Netscape HTTP Cookie File and return a cookie header value
pub fn parse_netscape_cookies(cookies: &str, filter_domain: &str) -> Result<String, Error> {
    let mut res = cookies
        .lines()
        .enumerate()
        .map(|(line_n, line)| parse_netscape_cookie_line(line, line_n + 1, filter_domain))
        .try_fold(String::new(), |mut acc, itm| {
            if let Some((k, v)) = itm? {
                acc += k;
                acc.push('=');
                acc += v;
                acc += "; ";
            }
            Ok::<_, Error>(acc)
        })?;

    if !res.is_empty() {
        res.truncate(res.len() - 2);
    }
    Ok(res)
}

fn parse_netscape_cookie_line<'a>(
    line: &'a str,
    line_n: usize,
    filter_domain: &str,
) -> Result<Option<(&'a str, &'a str)>, Error> {
    let mut line = line.trim();

    if let Some(s) = line.strip_prefix("#HttpOnly_") {
        line = s;
    } else if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }

    let mkerr = || Error::Other(format!("line {line_n}: too few fields, expected 7").into());

    let mut cols = line.split('\t');
    let domain = cols.next().ok_or_else(mkerr)?;
    if domain != filter_domain {
        return Ok(None);
    }
    let include_subdomains = cols.next().ok_or_else(mkerr)?.eq_ignore_ascii_case("true");
    if !include_subdomains {
        return Ok(None);
    }
    let path = cols.next().ok_or_else(mkerr)?;
    if path != "/" {
        return Ok(None);
    }
    // skip secure, expire
    let name = cols.nth(2).ok_or_else(mkerr)?;
    let value = cols.next().ok_or_else(mkerr)?;

    if cols.next().is_some() {
        return Err(Error::Other(
            format!("line {line_n}: too many fields, expected 7").into(),
        ));
    }

    Ok(Some((name, value)))
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
    #[case("102:12:39", Some(367_959))]
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

    #[rstest]
    #[case(
        Language::Iw,
        "\u{200f}\u{202b}3.36M\u{200f}\u{202c}\u{200f} \u{200f}מנויים\u{200f}",
        3_360_000
    )]
    #[case(Language::As, "১ জন গ্ৰাহক", 1)]
    #[case(Language::Ru, "Зрителей, ожидающих начала трансляции: 6", 6)]
    #[case(Language::Si, "වාදන මි4.6ක්", 4_600_000)]
    #[case(Language::As, "3.7 শঃ কোঃ বাৰ প্লে’ কৰা হৈছে", 370_000)]
    #[case(Language::Bs, "3,3 mlrd. pregleda", 3_300_000_000)]
    #[case(Language::It, "3,73 Mio di iscritti", 3_730_000)]
    fn t_parse_large_numstr(#[case] lang: Language, #[case] string: &str, #[case] expect: u64) {
        let res = parse_large_numstr::<u64>(string, lang).unwrap();
        assert_eq!(res, expect);
    }

    #[test]
    fn t_parse_large_numstr_samples() {
        let json_path = path!(*TESTFILES / "dict" / "large_number_samples.json");
        let json_file = File::open(json_path).unwrap();
        let number_samples: BTreeMap<Language, BTreeMap<String, u64>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, entry) in &number_samples {
            for (txt, expect) in entry {
                testcase_parse_large_numstr(txt, *lang, *expect);
            }
        }
    }

    fn testcase_parse_large_numstr(string: &str, lang: Language, expect: u64) {
        // Round the expected number to the amount of significant digits included
        // in the string.
        let rounded = {
            let n_significant_d = string.chars().filter(char::is_ascii_digit).count();
            if n_significant_d == 0 {
                expect
            } else {
                let mag = (expect as f64).log10().floor();
                let factor = 10_u64.pow(1 + mag as u32 - n_significant_d as u32);
                (((expect as f64) / factor as f64).floor() as u64) * factor
            }
        };

        let emsg = format!("{string} (lang: {lang}, exact: {expect})");

        let res = parse_large_numstr::<u64>(string, lang).expect(&emsg);
        assert_eq!(res, rounded, "{emsg}");
    }

    #[test]
    fn split_char() {
        let teststr = "abc今天更新def";
        let res = SplitTokens::new(teststr, true).collect::<Vec<_>>();
        assert_eq!(res.len(), 10);
        let res_str = res.into_iter().collect::<String>();
        assert_eq!(res_str, teststr);
    }

    #[test]
    fn split_words() {
        let teststr = "abc 今天更新 ghi";
        let res = SplitTokens::new(teststr, false).collect::<Vec<_>>();
        assert_eq!(res.len(), 3);
        let res_str = res.join(" ");
        assert_eq!(res_str, teststr);
    }

    #[rstest]
    #[case("en", Some(Language::En))]
    #[case("en-GB", Some(Language::EnGb))]
    #[case("en-US", Some(Language::En))]
    #[case("en-ZZ", Some(Language::En))]
    #[case("xy", None)]
    #[case("xy-ZZ", None)]
    fn parse_language(#[case] s: &str, #[case] expect: Option<Language>) {
        let res = Language::from_str(s).ok();
        assert_eq!(res, expect);
    }

    #[rstest]
    #[case("United States", Some(Country::Us))]
    #[case("Zimbabwe", Some(Country::Zw))]
    #[case("foobar", None)]
    fn t_country_from_name(#[case] name: &str, #[case] expect: Option<Country>) {
        let res = country_from_name(name);
        assert_eq!(res, expect);
    }

    #[test]
    fn t_find_map_or_last_err() {
        // Success
        let res = find_map_or_last_err([1, 2, 3].into_iter(), 0, |x: i32| {
            if x > 2 {
                Ok(true)
            } else {
                Err(x)
            }
        });
        assert_eq!(res, Ok(true));

        // Error
        let res = find_map_or_last_err([1, 2, 3].into_iter(), 0, |x: i32| Err::<(), _>(x));
        assert_eq!(res, Err(3));

        // Empty iterator
        assert_eq!(
            find_map_or_last_err(std::iter::empty(), 0, |_: i32| Ok(true)),
            Err(0)
        );
    }

    #[test]
    fn t_parse_netscape_cookies() {
        let cookies = r#"# Netscape HTTP Cookie File
# http://curl.haxx.se/rfc/cookie_spec.html
# This is a generated file! Do not edit.

# Domain	Subdomain	Path	Secure	Expire	Name	Value
.www.youtube.com	TRUE	/	FALSE	1769704561	yt-dev.storage-integrity	true
.youtube.com	TRUE	/	TRUE	1763481937	SOCS	Abcdefg
.youtube.com	TRUE	/	TRUE	1744905937	__Secure-BUCKET	IE7E
"#;
        let filter_domain = ".youtube.com";
        let parsed = parse_netscape_cookies(cookies, filter_domain).unwrap();
        assert_eq!(&parsed, "SOCS=Abcdefg; __Secure-BUCKET=IE7E");

        let cookies_too_few_cols = r#".youtube.com	TRUE	/	TRUE	1763481937	SOCS"#;
        let cookies_too_many_cols = r#".youtube.com	TRUE	/	TRUE	1763481937	SOCS	Abcdefg	foo"#;
        assert_eq!(
            parse_netscape_cookies(cookies_too_few_cols, filter_domain)
                .unwrap_err()
                .to_string(),
            "error: line 1: too few fields, expected 7"
        );
        assert_eq!(
            parse_netscape_cookies(cookies_too_many_cols, filter_domain)
                .unwrap_err()
                .to_string(),
            "error: line 1: too many fields, expected 7"
        );
    }
}
