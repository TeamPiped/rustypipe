use std::{collections::BTreeMap, fs::File, io::BufReader, path::PathBuf, str::FromStr};

use once_cell::sync::Lazy;
use path_macro::path;
use regex::Regex;
use rustypipe::param::Language;
use serde::{Deserialize, Serialize};

use crate::model::DictEntry;

/// Get the path of the `testfiles` directory
pub static TESTFILES_DIR: Lazy<PathBuf> = Lazy::new(|| {
    path!(env!("CARGO_MANIFEST_DIR") / ".." / "testfiles")
        .canonicalize()
        .unwrap()
});
/// Get the path of the `dict` directory
pub static DICT_DIR: Lazy<PathBuf> = Lazy::new(|| path!(*TESTFILES_DIR / "dict"));
/// Get the path of the `src` directory
pub static SRC_DIR: Lazy<PathBuf> = Lazy::new(|| path!(env!("CARGO_MANIFEST_DIR") / ".." / "src"));

type Dictionary = BTreeMap<Language, DictEntry>;
type DictionaryOverride = BTreeMap<Language, DictOverrideEntry>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct DictOverrideEntry {
    number_tokens: BTreeMap<String, Option<u8>>,
    number_nd_tokens: BTreeMap<String, Option<u8>>,
}

pub fn read_dict() -> Dictionary {
    let json_path = path!(*DICT_DIR / "dictionary.json");
    let json_file = File::open(json_path).unwrap();
    serde_json::from_reader(BufReader::new(json_file)).unwrap()
}

fn read_dict_override() -> DictionaryOverride {
    let json_path = path!(*DICT_DIR / "dictionary_override.json");
    let json_file = File::open(json_path).unwrap();
    serde_json::from_reader(BufReader::new(json_file)).unwrap()
}

pub fn write_dict(dict: Dictionary) {
    let dict_override = read_dict_override();

    let json_path = path!(*DICT_DIR / "dictionary.json");
    let json_file = File::create(json_path).unwrap();

    fn apply_map<K: Clone + Ord, V: Clone>(map: &mut BTreeMap<K, V>, or: &BTreeMap<K, Option<V>>) {
        or.iter().for_each(|(key, val)| match val {
            Some(val) => {
                map.insert(key.clone(), val.clone());
            }
            None => {
                map.remove(key);
            }
        });
    }

    let dict: Dictionary = dict
        .into_iter()
        .map(|(lang, mut entry)| {
            if let Some(or) = dict_override.get(&lang) {
                apply_map(&mut entry.number_tokens, &or.number_tokens);
                apply_map(&mut entry.number_nd_tokens, &or.number_nd_tokens);
            }
            (lang, entry)
        })
        .collect();

    serde_json::to_writer_pretty(json_file, &dict).unwrap();
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
        .flat_map(char::to_lowercase)
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

/// Parse textual video length (e.g. `0:49`, `2:02` or `1:48:18`)
/// and return the duration in seconds.
pub fn parse_video_length(text: &str) -> Option<u32> {
    static VIDEO_LENGTH_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(?:(\d+)[:.])?(\d{1,2})[:.](\d{2})"#).unwrap());
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
