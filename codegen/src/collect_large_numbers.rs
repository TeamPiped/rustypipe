use std::collections::{HashMap, HashSet};
use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};
use fancy_regex::Regex;
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use reqwest::{header, Client};
use rustypipe::model::{locale::LANGUAGES, Language};
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

use crate::util::{self, Text};

type CollectedNumbers = BTreeMap<Language, BTreeMap<u8, (String, u64)>>;

/// Collect video view count texts in every supported language
/// and write them to `testfiles/dict/large_number_samples.json`.
///
/// YouTube's API outputs the subscriber count of a channel only in a
/// approximated format (e.g *880K subscribers*), which varies
/// by language.
///
/// To parse these numbers correctly we need to collect textual numbers
/// of different orders of magnitude in every language. This script extracts
/// the view count texts from the most popular videos of different channels.
///
/// We extract these instead of subscriber counts because the YouTube API
/// outputs view counts both in approximated and exact format, so we can use
/// the exact counts to figure out the tokens.
pub async fn collect_large_numbers(project_root: &Path, concurrency: usize) {
    let mut json_path = project_root.to_path_buf();
    json_path.push("testfiles/dict/large_number_samples.json");

    let mut json_path_all = project_root.to_path_buf();
    json_path_all.push("testfiles/dict/large_number_samples_all.json");

    let channels = [
        "UCq-Fj5jknLsUf-MWSy4_brA", // 10e8 (225M)
        "UCcdwLMPsaU2ezNSJU1nFoBQ", // 10e7 (60M)
        "UC6mIxFTvXkWQVEHPsEdflzQ", // 10e6 (1.7M)
        "UCD0y51PJfvkZNe3y3FR5riw", // 10e5 (125K)
        "UCNcN0dW43zE0Om3278fjY8A", // 10e4 (27K)
        "UC0QEucPrn0-Ddi3JBTcs5Kw", // 10e3 (5K)
        "UCXvtcj9xUQhaqPaitFf2DqA", // (170)
        "UCq-XMc01T641v-4P3hQYJWg", // (636)
    ];

    let collected_numbers_all: BTreeMap<Language, BTreeMap<String, u64>> = stream::iter(LANGUAGES)
        .map(|lang| async move {
            let mut entry = BTreeMap::new();

            for (n, ch_id) in channels.iter().enumerate() {
                let channel = get_channel(ch_id, lang)
                    .await
                    .context(format!("{}-{}", lang, n))
                    .unwrap();

                channel.view_counts.iter().for_each(|(num, txt)| {
                    entry.insert(txt.to_owned(), *num);
                });

                println!("collected {}-{}", lang, n);
            }

            (lang, entry)
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let collected_numbers: CollectedNumbers = collected_numbers_all
        .iter()
        .map(|(lang, entry)| {
            let mut e2 = BTreeMap::new();
            entry.iter().for_each(|(txt, num)| {
                e2.insert(get_mag(*num), (txt.to_owned(), *num));
            });
            (*lang, e2)
        })
        .collect();

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &collected_numbers).unwrap();

    let file = File::create(json_path_all).unwrap();
    serde_json::to_writer_pretty(file, &collected_numbers_all).unwrap();
}

/// Attempt to parse the numbers collected by `collect-large-numbers`
/// and write the results to `dictionary.json`.
pub fn write_samples_to_dict(project_root: &Path) {
    /*
    Manual corrections:
    as
    "কোঃটা": 9,
    "নিঃটা": 6,
    "নিযুতটা": 6,
    "লাখটা": 5,
    "হাজাৰটা": 3

    ar
    "ألف": 3,
    "آلاف": 3,
    "مليار": 9,
    "مليون": 6

    bn
    "লাটি": 5,
    "শত": 2,
    "হাটি": 3,
    "কোটি": 7

    es/es-US
    "mil": 3,
    "M": 6
    */

    let mut json_path = project_root.to_path_buf();
    json_path.push("testfiles/dict/large_number_samples.json");

    let json_file = File::open(json_path).unwrap();
    let collected_nums: CollectedNumbers =
        serde_json::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict(project_root);
    let langs = dict.keys().map(|k| k.to_owned()).collect::<Vec<_>>();

    static POINT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d(\.|,)\d{1,3}(?:\D|$)").unwrap());

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let mut e_langs = dict_entry.equivalent.clone();
        e_langs.push(lang);

        let comma_decimal = collected_nums
            .get(&lang)
            .unwrap()
            .iter()
            .find_map(|(mag, (txt, _))| {
                let point = POINT_REGEX
                    .captures(txt)
                    .unwrap()
                    .map(|c| c.get(1).unwrap().as_str());

                if let Some(point) = point {
                    let num_all = util::parse_numeric::<u64>(txt).unwrap();
                    // If the number parsed from all digits has the same order of
                    // magnitude as the actual number, it must be a separator.
                    // Otherwise it is a decimal point
                    return Some((get_mag(num_all) == *mag) ^ (point == ","));
                }
                None
            })
            .unwrap();

        let decimal_point = match comma_decimal {
            true => ",",
            false => ".",
        };

        // Search for tokens

        // This map holds all the tokens we encounter while parsing the language
        // If a new token is found, it is stored in this map with the derived order of
        // magnitude.
        // If the token is found again with a different derived order of magnitude,
        // its value in the map is set to None.
        let mut found_tokens: HashMap<String, Option<u8>> = HashMap::new();

        let mut insert_token = |token: String, mag: u8| {
            let found_token = found_tokens.entry(token).or_insert(match mag {
                0 => None,
                x => Some(x),
            });

            if let Some(f) = found_token {
                if *f != mag {
                    *found_token = None;
                }
            }
        };

        for lang in e_langs {
            let entry = collected_nums.get(&lang).unwrap();

            entry.iter().for_each(|(mag, (txt, _))| {
                let filtered = util::filter_largenumstr(txt);

                let tokens: Vec<String> = match dict_entry.by_char {
                    true => filtered.chars().map(|c| c.to_string()).collect(),
                    false => filtered.split_whitespace().map(|c| c.to_string()).collect(),
                };

                let num_before_point =
                    util::parse_numeric::<u64>(txt.split(decimal_point).next().unwrap()).unwrap();
                let mag_before_point = get_mag(num_before_point);
                let mut mag_remaining = mag - mag_before_point;

                tokens.iter().for_each(|t| {
                    // These tokens are correct in all languages
                    // and are used to parse combined prefixes like `1.1K crore` (en-IN)
                    let known_tmag: u8 = if t.len() == 1 {
                        match t.as_str() {
                            "K" | "k" => 3,
                            // 'm' means 10^3 in Catalan, 'B' means 10^3 in Turkish
                            // 'M' means 10^9 in Indonesian
                            _ => 0,
                        }
                    } else {
                        0
                    };

                    // K/M/B
                    if known_tmag > 0 {
                        mag_remaining = mag_remaining
                            .checked_sub(known_tmag)
                            .expect("known magnitude incorrect");
                    } else {
                        insert_token(t.to_owned(), mag_remaining);
                    }
                });
            });
        }

        // Insert collected data into dictionary
        dict_entry.number_tokens = found_tokens
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect();
        dict_entry.comma_decimal = comma_decimal;

        // Check for duplicates
        let mut uniq = HashSet::new();
        if !dict_entry.number_tokens.values().all(|x| uniq.insert(x)) {
            println!("Warning: collected duplicate tokens for {}", lang);
        }
    }

    util::write_dict(project_root, &dict);
}

fn get_mag(n: u64) -> u8 {
    (n as f64).log10().floor() as u8
}

/*
YouTube channel videos response
*/

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Channel {
    contents: Contents,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contents {
    two_column_browse_results_renderer: TabsRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    tabs: Vec<TabRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TabRendererWrap {
    tab_renderer: TabRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TabRenderer {
    content: SectionListRendererWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionListRendererWrap {
    section_list_renderer: SectionListRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionListRenderer {
    contents: Vec<ItemSectionRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemSectionRendererWrap {
    item_section_renderer: ItemSectionRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemSectionRenderer {
    contents: Vec<GridRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridRendererWrap {
    grid_renderer: GridRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    items: Vec<VideoListItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoListItem {
    grid_video_renderer: GridVideoRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridVideoRenderer {
    /// `24,194 views`
    view_count_text: Text,
    /// `19K views`
    short_view_count_text: Text,
}

#[derive(Clone, Debug)]
struct ChannelData {
    view_counts: Vec<(u64, String)>,
}

async fn get_channel(channel_id: &str, lang: Language) -> Result<ChannelData> {
    let client = Client::new();

    let body = format!(
        "{}{}{}{}{}",
        r##"{"context":{"client":{"clientName":"WEB","clientVersion":"2.20220914.06.00","platform":"DESKTOP","originalUrl":"https://www.youtube.com/","hl":""##,
        lang,
        r##"","gl":"US"},"request":{"internalExperimentFlags":[],"useSsl":true},"user":{"lockedSafetyMode":false}},"params":"EgZ2aWRlb3MYASAAMAE%3D","browseId":""##,
        channel_id,
        "\"}"
    );

    let resp = client
        .post("https://www.youtube.com/youtubei/v1/browse?key=AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8&prettyPrint=false")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .send().await?
        .error_for_status()?;

    let channel = resp.json::<Channel>().await?;

    Ok(ChannelData {
        view_counts: channel
            .contents
            .two_column_browse_results_renderer
            .tabs
            .get(0)
            .map(|tab| {
                tab.tab_renderer.content.section_list_renderer.contents[0]
                    .item_section_renderer
                    .contents[0]
                    .grid_renderer
                    .items
                    .iter()
                    .map(|itm| {
                        (
                            util::parse_numeric(
                                &itm.grid_video_renderer.view_count_text.simple_text,
                            )
                            .unwrap(),
                            itm.grid_video_renderer
                                .short_view_count_text
                                .simple_text
                                .to_owned(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}
