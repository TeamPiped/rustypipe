#![cfg(test)]

use std::{collections::BTreeMap, fs::File, io::BufReader};

use serde::{Serialize, Deserialize};

use crate::model::Language;
mod collect_playlist_dates;
mod gen_dictionary;
mod gen_locales;

const DICT_PATH: &str = "testfiles/date/dictionary.json";

type Dictionary = BTreeMap<Language, DictEntry>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct DictEntry {
    equivalent: Vec<Language>,
    by_char: bool,
    timeago_tokens: BTreeMap<String, String>,
    date_order: String,
    months: BTreeMap<String, u8>,
    timeago_nd_tokens: BTreeMap<String, String>,
}

fn read_dict() -> Dictionary {
    let json_file = File::open(DICT_PATH).unwrap();
    serde_json::from_reader(BufReader::new(json_file)).unwrap()
}

fn write_dict(dict: &Dictionary) {
    let json_file = File::create(DICT_PATH).unwrap();
    serde_json::to_writer_pretty(json_file, dict).unwrap();
}
