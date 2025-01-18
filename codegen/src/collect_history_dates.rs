use std::{collections::BTreeMap, fs::File, io::BufReader};

use path_macro::path;
use rustypipe::{
    client::RustyPipe,
    param::{Language, LANGUAGES},
};

use crate::util::{self, DICT_DIR};

type CollectedDates = BTreeMap<Language, BTreeMap<String, String>>;

const THIS_WEEK: &str = "this_week";
const LAST_WEEK: &str = "last_week";

pub async fn collect_dates_music() {
    let json_path = path!(*DICT_DIR / "history_date_samples.json");
    let rp = RustyPipe::builder()
        .storage_dir(path!(env!("CARGO_MANIFEST_DIR") / ".."))
        .build()
        .unwrap();

    let mut res: CollectedDates = {
        let json_file = File::open(&json_path).unwrap();
        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    };

    for lang in LANGUAGES {
        println!("{lang}");
        let history = rp.query().lang(lang).music_history().await.unwrap();
        if history.items.len() < 3 {
            panic!("{lang} empty history")
        }

        // The indexes have to be adapted before running
        let entry = res.entry(lang).or_default();
        entry.insert(
            THIS_WEEK.to_owned(),
            history.items[0].playback_date_txt.clone().unwrap(),
        );
        entry.insert(
            LAST_WEEK.to_owned(),
            history.items[18].playback_date_txt.clone().unwrap(),
        );
    }

    let file = File::create(&json_path).unwrap();
    serde_json::to_writer_pretty(file, &res).unwrap();
}

pub async fn collect_dates() {
    let json_path = path!(*DICT_DIR / "history_date_samples.json");
    let rp = RustyPipe::builder()
        .storage_dir(path!(env!("CARGO_MANIFEST_DIR") / ".."))
        .build()
        .unwrap();

    let mut res: CollectedDates = {
        let json_file = File::open(&json_path).unwrap();
        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    };

    for lang in LANGUAGES {
        println!("{lang}");
        let history = rp.query().lang(lang).history().await.unwrap();
        if history.items.len() < 3 {
            panic!("{lang} empty history")
        }

        let entry = res.entry(lang).or_default();
        entry.insert(
            "tuesday".to_owned(),
            history.items[0].playback_date_txt.clone().unwrap(),
        );
        entry.insert(
            "0000-01-06".to_owned(),
            history.items[1].playback_date_txt.clone().unwrap(),
        );
        entry.insert(
            "2024-12-28".to_owned(),
            history.items[15].playback_date_txt.clone().unwrap(),
        );
    }

    let file = File::create(&json_path).unwrap();
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
            .insert(util::filter_datestr(&cd[THIS_WEEK]), "0Wl".to_owned());
        dict_entry
            .timeago_nd_tokens
            .insert(util::filter_datestr(&cd[LAST_WEEK]), "1Wl".to_owned());
    }

    util::write_dict(dict);
}
