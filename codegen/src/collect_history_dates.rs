use std::{collections::BTreeMap, fs::File, io::BufReader};

use path_macro::path;
use rustypipe::{
    client::RustyPipe,
    param::{Language, LANGUAGES},
};
use serde::{Deserialize, Serialize};

use crate::util::{self, DICT_DIR};

type CollectedDates = BTreeMap<Language, HistoryDates>;

#[derive(Debug, Serialize, Deserialize)]
struct HistoryDates {
    this_week: String,
    last_week: String,
}

pub async fn collect_dates() {
    let json_path = path!(*DICT_DIR / "history_date_samples.json");
    let rp = RustyPipe::builder()
        .storage_dir("/home/thetadev/Documents/Programmieren/Rust/rustypipe")
        .build()
        .unwrap();

    let mut res: CollectedDates = BTreeMap::new();

    for lang in LANGUAGES {
        println!("{lang}");
        let history = rp.query().lang(lang).music_history().await.unwrap();
        if history.items.len() < 3 {
            panic!("{lang} empty history")
        }

        // The indexes have to be adapted before running
        let d = HistoryDates {
            this_week: history.items[0].playback_date_txt.clone().unwrap(),
            last_week: history.items[18].playback_date_txt.clone().unwrap(),
        };
        res.insert(lang, d);
    }

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &res).unwrap();
}

pub fn write_samples_to_dict() {
    let json_path = path!(*DICT_DIR / "history_date_samples.json");

    let json_file = File::open(json_path).unwrap();
    let collected_dates: CollectedDates =
        serde_json::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict();
    let langs = dict.keys().copied().collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();
        let cd = &collected_dates[&lang];
        dict_entry
            .timeago_nd_tokens
            .insert(util::filter_datestr(&cd.this_week), "0Wl".to_owned());
        dict_entry
            .timeago_nd_tokens
            .insert(util::filter_datestr(&cd.last_week), "1Wl".to_owned());
    }

    util::write_dict(dict);
}
