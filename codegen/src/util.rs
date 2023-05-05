use std::{
    collections::BTreeMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
};

use once_cell::sync::Lazy;
use path_macro::path;
use rustypipe::{client::YTContext, model::AlbumType, param::Language};
use serde::{Deserialize, Serialize};

static DICT_PATH: Lazy<PathBuf> = Lazy::new(|| path!("testfiles" / "dict" / "dictionary.json"));

type Dictionary = BTreeMap<Language, DictEntry>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DictEntry {
    /// List of languages that should be treated equally (e.g. EnUs/EnGb/EnIn)
    pub equivalent: Vec<Language>,
    /// Should the language be parsed by character instead of by word?
    /// (e.g. Chinese/Japanese)
    pub by_char: bool,
    /// Tokens for parsing timeago strings.
    ///
    /// Format: Parsed token -> \[Quantity\] Identifier
    ///
    /// Identifiers: `Y`(ear), `M`(month), `W`(eek), `D`(ay),
    /// `h`(our), `m`(inute), `s`(econd)
    pub timeago_tokens: BTreeMap<String, String>,
    /// Order in which to parse numeric date components. Formatted as
    /// a string of date identifiers (Y, M, D).
    ///
    /// Examples:
    ///
    /// - 03.01.2020 => `"DMY"`
    /// - Jan 3, 2020 => `"DY"`
    pub date_order: String,
    /// Tokens for parsing month names.
    ///
    /// Format: Parsed token -> Month number (starting from 1)
    pub months: BTreeMap<String, u8>,
    /// Tokens for parsing date strings with no digits (e.g. Today, Tomorrow)
    ///
    /// Format: Parsed token -> \[Quantity\] Identifier
    pub timeago_nd_tokens: BTreeMap<String, String>,
    /// Are commas (instead of points) used as decimal separators?
    pub comma_decimal: bool,
    /// Tokens for parsing decimal prefixes (K, M, B, ...)
    ///
    /// Format: Parsed token -> decimal power
    pub number_tokens: BTreeMap<String, u8>,
    /// Names of album types (Album, Single, ...)
    ///
    /// Format: Parsed text -> Album type
    pub album_types: BTreeMap<String, AlbumType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QBrowse<'a> {
    pub context: YTContext<'a>,
    pub browse_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QCont<'a> {
    pub context: YTContext<'a>,
    pub continuation: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TextRuns {
    pub runs: Vec<Text>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
    #[serde(alias = "simpleText")]
    pub text: String,
}

pub fn read_dict(project_root: &Path) -> Dictionary {
    let json_path = path!(project_root / *DICT_PATH);
    let json_file = File::open(json_path).unwrap();
    serde_json::from_reader(BufReader::new(json_file)).unwrap()
}

pub fn write_dict(project_root: &Path, dict: &Dictionary) {
    let json_path = path!(project_root / *DICT_PATH);
    let json_file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(json_file, dict).unwrap();
}

pub fn filter_datestr(string: &str) -> String {
    string
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c == '\u{200b}' || c.is_ascii_digit() {
                None
            } else if c == '-' {
                Some(' ')
            } else {
                Some(c)
            }
        })
        .collect()
}

pub fn filter_largenumstr(string: &str) -> String {
    string
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200b}'
                    | '\u{202b}'
                    | '\u{202c}'
                    | '\u{202e}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '.'
                    | ','
            ) && !c.is_ascii_digit()
        })
        .collect()
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

pub fn parse_largenum_en(string: &str) -> Option<u64> {
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
            } else if c == '.' {
                after_point = true;
            } else if !matches!(c, '\u{200b}' | '.' | ',') {
                filtered.push(c);
            }
        }
        (buf.parse::<u64>().ok()?, exp, filtered)
    };

    let lookup_token = |token: &str| match token {
        "K" => Some(3),
        "M" => Some(6),
        "B" => Some(9),
        _ => None,
    };

    exp += filtered
        .split_whitespace()
        .filter_map(lookup_token)
        .sum::<i32>();

    num.checked_mul((10_u64).checked_pow(exp.try_into().ok()?)?)
}
