use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
};

use futures::{stream, StreamExt};
use path_macro::path;
use rustypipe::{
    client::{RustyPipe, RustyPipeQuery},
    param::{Language, LANGUAGES},
};

use crate::util::DICT_DIR;

pub async fn collect_video_dates(concurrency: usize) {
    let json_path = path!(*DICT_DIR / "timeago_samples_short.json");
    let rp = RustyPipe::builder()
        .visitor_data("Cgtwel9tMkh2eHh0USiyzc6jBg%3D%3D")
        .build()
        .unwrap();

    let channels = [
        "UCeY0bbntWzzVIaj2z3QigXg",
        "UCcmpeVbSSQlZRvHfdC-CRwg",
        "UC65afEgL62PGFWXY7n6CUbA",
        "UCEOXxzW2vU0P-0THehuIIeg",
    ];

    let mut lang_strings: BTreeMap<Language, Vec<String>> = BTreeMap::new();
    for lang in LANGUAGES {
        println!("{lang}");
        let query = rp.query().lang(lang);
        let strings = stream::iter(channels)
            .map(|id| get_channel_datestrings(&query, id))
            .buffered(concurrency)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        lang_strings.insert(lang, strings);
    }

    let mut en_strings_uniq: HashSet<&str> = HashSet::new();
    let mut uniq_ids: HashSet<usize> = HashSet::new();

    lang_strings[&Language::En]
        .iter()
        .enumerate()
        .for_each(|(n, s)| {
            if en_strings_uniq.insert(s) {
                uniq_ids.insert(n);
            }
        });

    let strings_map = lang_strings
        .iter()
        .map(|(lang, strings)| {
            (
                lang,
                strings
                    .iter()
                    .enumerate()
                    .filter(|(n, _)| uniq_ids.contains(n))
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &strings_map).unwrap();
}

async fn get_channel_datestrings(rp: &RustyPipeQuery, id: &str) -> Vec<String> {
    let channel = rp.channel_videos(id).await.unwrap();

    channel
        .content
        .items
        .into_iter()
        .filter_map(|itm| itm.publish_date_txt)
        .collect()
}
