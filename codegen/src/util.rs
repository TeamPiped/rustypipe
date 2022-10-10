use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path, str::FromStr};

use rustypipe::param::Language;
use serde::{Deserialize, Serialize};

const DICT_PATH: &str = "testfiles/dict/dictionary.json";

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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub simple_text: String,
}

pub fn read_dict(project_root: &Path) -> Dictionary {
    let mut json_path = project_root.to_path_buf();
    json_path.push(DICT_PATH);
    let json_file = File::open(json_path).unwrap();
    serde_json::from_reader(BufReader::new(json_file)).unwrap()
}

pub fn write_dict(project_root: &Path, dict: &Dictionary) {
    let mut json_path = project_root.to_path_buf();
    json_path.push(DICT_PATH);
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
        .filter(|c| !matches!(c, '\u{200b}' | '.' | ',') && !c.is_ascii_digit())
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
