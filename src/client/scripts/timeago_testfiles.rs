#![cfg(test)]

use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    path::Path,
};

use futures::{stream, StreamExt};
use reqwest::Method;
use serde::Serialize;

use crate::{
    client::{response, ClientType, ContextYT, RustyTube},
    model::{Country, Language},
    timeago,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel {
    context: ContextYT,
    browse_id: String,
    params: String,
}

async fn get_channel_datestrings(rp: &RustyTube, channel_id: &str) -> Vec<String> {
    let client = rp.get_ytclient(ClientType::Desktop);
    let context = client.get_context(true).await;

    let request_body = QChannel {
        context,
        browse_id: channel_id.to_owned(),
        params: "EgZ2aWRlb3PyBgQKAjoA".to_owned(),
    };

    let resp = client
        .request_builder(Method::POST, "browse")
        .await
        .json(&request_body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let channel_response = resp.json::<response::Channel>().await.unwrap();

    channel_response
        .contents
        .two_column_browse_results_renderer
        .tabs[0]
        .tab_renderer
        .content
        .section_list_renderer
        .contents[0]
        .item_section_renderer
        .contents[0]
        .grid_renderer
        .items
        .iter()
        .filter_map(|itm| match itm {
            response::VideoListItem::GridVideoRenderer { video } => {
                Some(video.published_time_text.to_owned())
            }
            response::VideoListItem::ContinuationItemRenderer { .. } => None,
        })
        .collect::<Vec<_>>()
}

#[test_log::test(tokio::test)]
async fn download_timeago_testfiles() {
    let json_path = Path::new("testfiles/date/timeago.json").to_path_buf();
    if json_path.exists() {
        return;
    }

    let channel_ids = [
        "UCeY0bbntWzzVIaj2z3QigXg",
        "UCcmpeVbSSQlZRvHfdC-CRwg",
        "UC65afEgL62PGFWXY7n6CUbA",
        "UCEOXxzW2vU0P-0THehuIIeg",
    ];

    // Get strings of all languages
    let mut lang_strings: BTreeMap<Language, Vec<String>> = BTreeMap::new();
    for lang in timeago::LANGUAGES {
        let rp = RustyTube::new_with_ua(lang, Country::Us, None);
        let strings = stream::iter(channel_ids)
            .map(|id| get_channel_datestrings(&rp, id))
            .buffered(4)
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
