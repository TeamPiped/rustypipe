use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path, str::FromStr};

use rustypipe::model::Language;
use serde::{Deserialize, Serialize};

const DICT_PATH: &str = "testfiles/date/dictionary.json";

type Dictionary = BTreeMap<Language, DictEntry>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DictEntry {
    pub equivalent: Vec<Language>,
    pub by_char: bool,
    pub timeago_tokens: BTreeMap<String, String>,
    pub date_order: String,
    pub months: BTreeMap<String, u8>,
    pub timeago_nd_tokens: BTreeMap<String, String>,
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
