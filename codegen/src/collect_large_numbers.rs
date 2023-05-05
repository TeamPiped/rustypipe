use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use path_macro::path;
use regex::Regex;
use rustypipe::client::{ClientType, RustyPipe, RustyPipeQuery};
use rustypipe::param::{locale::LANGUAGES, Language};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::util::{self, QBrowse, QCont, Text};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
enum NumKey {
    Mag(u8),
    S(NumKeyS),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
enum NumKeyS {
    Zero,
    One,
}

type CollectedNumbers = BTreeMap<Language, BTreeMap<NumKey, (String, u64)>>;

/// Collect video view count texts in every supported language
/// and write them to `testfiles/dict/large_number_samples.json`.
///
/// YouTube's API outputs subscriber and view counts only in a
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
    let json_path = path!(project_root / "testfiles" / "dict" / "large_number_samples.json");
    let json_path_all =
        path!(project_root / "testfiles" / "dict" / "large_number_samples_all.json");
    let rp = RustyPipe::new();

    let channels = [
        "UCq-Fj5jknLsUf-MWSy4_brA", // 10e8 (241M)
        "UCcdwLMPsaU2ezNSJU1nFoBQ", // 10e7 (67M)
        "UC6mIxFTvXkWQVEHPsEdflzQ", // 10e6 (1.8M)
        "UCD0y51PJfvkZNe3y3FR5riw", // 10e5 (126K)
        "UCNcN0dW43zE0Om3278fjY8A", // 10e4 (33K)
        "UC0QEucPrn0-Ddi3JBTcs5Kw", // 10e3 (5K)
        "UCXvtcj9xUQhaqPaitFf2DqA", // (275)
        "UCq-XMc01T641v-4P3hQYJWg", // (695)
        "UCaZL4eLD7a30Fa8QI-sRi_g", // (31K)
        "UCO-dylEoJozPTxGYd8fTQxA", // (5)
        "UCQXYK94vDqOEkPbTCyL0OjA", // (1)
    ];

    // Build a lookup table for the channel's subscriber counts
    let subscriber_counts: Arc<BTreeMap<String, u64>> = stream::iter(channels)
        .map(|c| {
            let rp = rp.query();
            async move {
                let channel = get_channel(&rp, c).await.unwrap();

                let n = util::parse_largenum_en(&channel.subscriber_count).unwrap();
                (c.to_owned(), n)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<BTreeMap<_, _>>()
        .await
        .into();

    let collected_numbers_all: BTreeMap<Language, BTreeMap<String, u64>> = stream::iter(LANGUAGES)
        .map(|lang| {
            let rp = rp.query().lang(lang);
            let subscriber_counts = subscriber_counts.clone();
            async move {
                let mut entry = BTreeMap::new();

                for (n, ch_id) in channels.iter().enumerate() {
                    let channel = get_channel(&rp, ch_id)
                        .await
                        .context(format!("{lang}-{n}"))
                        .unwrap();

                    channel.view_counts.iter().for_each(|(num, txt)| {
                        entry.insert(txt.to_owned(), *num);
                    });
                    entry.insert(channel.subscriber_count, subscriber_counts[*ch_id]);

                    println!("collected {lang}-{n}");
                }

                (lang, entry)
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let collected_numbers: CollectedNumbers = collected_numbers_all
        .iter()
        .map(|(lang, entry)| {
            let mut e2 = BTreeMap::new();
            entry.iter().for_each(|(txt, num)| {
                let key = if num == &0 {
                    NumKey::S(NumKeyS::Zero)
                } else if num == &1 {
                    NumKey::S(NumKeyS::One)
                } else {
                    NumKey::Mag(get_mag(*num))
                };

                e2.insert(key, (txt.to_owned(), *num));
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

    let json_path = path!(project_root / "testfiles" / "dict" / "large_number_samples.json");

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
            .find_map(|(key, (txt, _))| {
                match key {
                    NumKey::Mag(mag) => {
                        let point = POINT_REGEX
                            .captures(txt)
                            .map(|c| c.get(1).unwrap().as_str());

                        if let Some(point) = point {
                            let num_all = util::parse_numeric::<u64>(txt).unwrap();
                            // If the number parsed from all digits has the same order of
                            // magnitude as the actual number, it must be a separator.
                            // Otherwise it is a decimal point
                            return Some((get_mag(num_all) == *mag) ^ (point == ","));
                        }
                    }
                    NumKey::S(_) => {}
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

            entry.iter().for_each(|(key, (txt, _))| {
                match key {
                    NumKey::Mag(mag) => {
                        let filtered = util::filter_largenumstr(txt);

                        let tokens: Vec<String> = match dict_entry.by_char {
                            true => filtered.chars().map(|c| c.to_string()).collect(),
                            false => filtered.split_whitespace().map(|c| c.to_string()).collect(),
                        };

                        let num_before_point =
                            util::parse_numeric::<u64>(txt.split(decimal_point).next().unwrap())
                                .unwrap();
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
                    }
                    NumKey::S(_) => {}
                }
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
            println!("Warning: collected duplicate tokens for {lang}");
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
    header: ChannelHeader,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelHeader {
    c4_tabbed_header_renderer: HeaderRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeaderRenderer {
    subscriber_count_text: Text,
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
    content: RichGridRendererWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichGridRendererWrap {
    rich_grid_renderer: RichGridRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichGridRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    contents: Vec<RichItemRendererWrap>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    header: Option<RichGridHeader>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichItemRendererWrap {
    rich_item_renderer: RichItemRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichItemRenderer {
    content: VideoRendererWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRendererWrap {
    video_renderer: VideoRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRenderer {
    /// `24,194 views`
    view_count_text: Text,
    /// `19K views`
    short_view_count_text: Text,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichGridHeader {
    feed_filter_chip_bar_renderer: ChipBar,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChipBar {
    contents: Vec<Chip>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Chip {
    chip_cloud_chip_renderer: ChipRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChipRenderer {
    navigation_endpoint: NavigationEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigationEndpoint {
    continuation_command: ContinuationCommand,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationCommand {
    token: String,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationResponse {
    // #[serde_as(as = "VecSkipError<_>")]
    on_response_received_actions: Vec<ContinuationAction>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationAction {
    reload_continuation_items_command: ContinuationItemsWrap,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationItemsWrap {
    #[serde_as(as = "VecSkipError<_>")]
    continuation_items: Vec<RichItemRendererWrap>,
}

#[derive(Clone, Debug)]
struct ChannelData {
    view_counts: BTreeMap<u64, String>,
    subscriber_count: String,
}

async fn get_channel(query: &RustyPipeQuery, channel_id: &str) -> Result<ChannelData> {
    let resp = query
        .raw(
            ClientType::DesktopMusic,
            "browse",
            &QBrowse {
                context: query.get_context(ClientType::Desktop, true, None).await,
                browse_id: channel_id,
                params: Some("EgZ2aWRlb3MYASAAMAE"),
            },
        )
        .await?;

    let channel = serde_json::from_str::<Channel>(&resp)?;

    let tab = &channel.contents.two_column_browse_results_renderer.tabs[0]
        .tab_renderer
        .content
        .rich_grid_renderer;

    let popular_token = tab.header.as_ref().and_then(|h| {
        h.feed_filter_chip_bar_renderer.contents.get(1).map(|c| {
            c.chip_cloud_chip_renderer
                .navigation_endpoint
                .continuation_command
                .token
                .to_owned()
        })
    });

    let mut view_counts: BTreeMap<u64, String> = tab
        .contents
        .iter()
        .map(|itm| {
            let v = &itm.rich_item_renderer.content.video_renderer;
            (
                util::parse_numeric(&v.view_count_text.text).unwrap_or_default(),
                v.short_view_count_text.text.to_owned(),
            )
        })
        .collect();

    if let Some(popular_token) = popular_token {
        let resp = query
            .raw(
                ClientType::Desktop,
                "browse",
                &QCont {
                    context: query.get_context(ClientType::Desktop, true, None).await,
                    continuation: &popular_token,
                },
            )
            .await?;

        let continuation = serde_json::from_str::<ContinuationResponse>(&resp)?;

        continuation
            .on_response_received_actions
            .iter()
            .for_each(|a| {
                a.reload_continuation_items_command
                    .continuation_items
                    .iter()
                    .for_each(|itm| {
                        let v = &itm.rich_item_renderer.content.video_renderer;
                        view_counts.insert(
                            util::parse_numeric(&v.view_count_text.text).unwrap(),
                            v.short_view_count_text.text.to_owned(),
                        );
                    })
            });
    }

    Ok(ChannelData {
        view_counts,
        subscriber_count: channel
            .header
            .c4_tabbed_header_renderer
            .subscriber_count_text
            .text,
    })
}

#[cfg(test)]
mod tests {
    use rustypipe::client::RustyPipe;

    use super::*;

    #[tokio::test]
    async fn t() {
        let rp = RustyPipe::new();
        let x = get_channel(&rp.query(), "UCQXYK94vDqOEkPbTCyL0OjA")
            .await
            .unwrap();
        dbg!(&x);
    }
}
