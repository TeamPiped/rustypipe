#![cfg(test)]

use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::BufReader,
    path::Path,
};

use fancy_regex::Regex;
use futures::{stream, StreamExt};
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use log::{error, info};
use once_cell::sync::Lazy;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

use crate::{
    client::{response, ClientType, ContextYT, RustyTube},
    model::{Country, Language},
    timeago::{self, TimeUnit, TimeagoPattern, LANGUAGES},
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
                video.published_time_text.to_owned()
            }
            response::VideoListItem::ContinuationItemRenderer { .. } => None,
        })
        .collect::<Vec<_>>()
}

async fn get_comment_initial_ctoken(rp: &RustyTube, video_id: &str, latest: bool) -> String {
    let video_response = rp.get_video_response(video_id).await.unwrap();

    match latest {
        true => video_response
            .engagement_panels
            .iter()
            .find_map(|p| {
                p.engagement_panel_section_list_renderer
                    .header
                    .engagement_panel_title_header_renderer
                    .menu
                    .sort_filter_sub_menu_renderer
                    .sub_menu_items
                    .get(1)
                    .map(|i| i.service_endpoint.continuation_command.token.to_owned())
            })
            .unwrap(),
        false => video_response
            .contents
            .two_column_watch_next_results
            .results
            .results
            .contents
            .iter()
            .find_map(|c| match c {
                response::video::VideoResultsItem::ItemSectionRenderer {
                    contents,
                    section_identifier,
                } => match section_identifier == "comment-item-section" {
                    true => match &contents[0] {
                        response::video::ItemSection::ContinuationItemRenderer {
                            continuation_endpoint,
                        } => Some(continuation_endpoint.continuation_command.token.to_owned()),
                        _ => None,
                    },
                    false => None,
                },
                _ => None,
            })
            .unwrap(),
    }
}

async fn get_comment_datestrings(rp: &RustyTube, ctoken: &str) -> (Vec<String>, Option<String>) {
    let comments_response = rp.get_comments_response(ctoken).await.unwrap();

    let mut next_ctoken: Option<String> = None;
    let datestrings = comments_response
        .on_response_received_endpoints
        /*
        .iter()
        .find(|e| {
            !e.append_continuation_items_action
                .continuation_items
                .is_empty()
                && matches!(
                    &e.append_continuation_items_action.continuation_items[0],
                    CommentListItem::CommentsHeaderRenderer { count_text }
                )
        })
        .unwrap()
        */
        .iter()
        .rev()
        .next()
        .unwrap()
        .append_continuation_items_action
        .continuation_items
        .iter()
        .filter_map(|itm| match itm {
            response::video::CommentListItem::CommentThreadRenderer { comment, .. } => {
                Some(comment.comment_renderer.published_time_text.to_owned())
            }
            response::video::CommentListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                next_ctoken = Some(continuation_endpoint.continuation_command.token.to_owned());
                None
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    (datestrings, next_ctoken)
}

// #[test_log::test(tokio::test)]
#[allow(dead_code)]
async fn download_timeago_testfiles() {
    let json_path = Path::new("testfiles/date/timeago_samples.json").to_path_buf();
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

#[derive(Debug, Clone, Deserialize)]
struct PluralRulesData {
    supplemental: PluralRulesInner,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PluralRulesInner {
    plurals_type_cardinal: BTreeMap<String, Ruleset>,
}

#[derive(Debug, Clone, Deserialize)]
struct Ruleset {
    #[serde(rename = "pluralRule-count-one")]
    one: Option<String>,
    #[serde(rename = "pluralRule-count-two")]
    two: Option<String>,
    #[serde(rename = "pluralRule-count-few")]
    few: Option<String>,
    #[serde(rename = "pluralRule-count-many")]
    many: Option<String>,
    #[serde(rename = "pluralRule-count-other")]
    other: Option<String>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum PluralCat {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl From<PluralCategory> for PluralCat {
    fn from(value: PluralCategory) -> Self {
        match value {
            PluralCategory::ZERO => Self::Zero,
            PluralCategory::ONE => Self::One,
            PluralCategory::TWO => Self::Two,
            PluralCategory::FEW => Self::Few,
            PluralCategory::MANY => Self::Many,
            PluralCategory::OTHER => Self::Other,
        }
    }
}

static PLURAL_RULES: Lazy<BTreeMap<String, HashSet<PluralCat>>> = Lazy::new(|| {
    let json_path = Path::new("testfiles/date/cldr_pluralrules_cardinals.json");
    let json_file = File::open(json_path).unwrap();

    serde_json::from_reader::<_, PluralRulesData>(BufReader::new(json_file))
        .unwrap()
        .supplemental
        .plurals_type_cardinal
        .iter()
        .map(|(lang, rules)| {
            let mut hs: HashSet<PluralCat> = HashSet::new();

            if rules.one.is_some() {
                hs.insert(PluralCat::One);
            }
            if rules.two.is_some() {
                hs.insert(PluralCat::Two);
            }
            if rules.few.is_some() {
                hs.insert(PluralCat::Few);
            }
            if rules.many.is_some() {
                hs.insert(PluralCat::Many);
            }
            if rules.other.is_some() {
                hs.insert(PluralCat::Other);
            }

            (lang.to_owned(), hs)
        })
        .collect::<BTreeMap<_, _>>()
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeagoTable {
    entries: BTreeMap<Language, BTreeMap<TimeUnit, TimeagoTableEntry>>,
    errors: BTreeMap<Language, HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeagoTableEntry {
    cases: BTreeMap<String, u8>,
    missing_plurals: HashSet<PluralCat>,
}

const TIME_UNITS: [TimeUnit; 7] = [
    TimeUnit::Second,
    TimeUnit::Minute,
    TimeUnit::Hour,
    TimeUnit::Day,
    TimeUnit::Week,
    TimeUnit::Month,
    TimeUnit::Year,
];

fn new_timeago_table() -> TimeagoTable {
    TimeagoTable {
        entries: LANGUAGES
            .iter()
            .filter_map(|lang| {
                // Check if language is redundant
                match lang {
                    Language::EnGb
                    | Language::EnIn
                    | Language::FrCa
                    | Language::EsUs
                    | Language::Es419 => None,
                    _ => {
                        let cldr_lang_str = match lang {
                            Language::SrLatn => "sr".to_owned(),
                            Language::ZhCn | Language::ZhHk | Language::ZhTw => "zh".to_owned(),
                            _ => lang.to_string(),
                        };

                        let m = TIME_UNITS
                            .iter()
                            .map(|t| {
                                let missing_plurals = if t == &TimeUnit::Week {
                                    // Week only has 3 valid values (2-4)
                                    let mut mp = HashSet::new();

                                    let l_id = cldr_lang_str.parse::<LanguageIdentifier>().unwrap();
                                    let pr = PluralRules::create(l_id, PluralRuleType::CARDINAL)
                                        .unwrap();

                                    mp.insert(PluralCat::from(pr.select(2).unwrap()));
                                    mp.insert(PluralCat::from(pr.select(3).unwrap()));
                                    mp.insert(PluralCat::from(pr.select(4).unwrap()));

                                    mp
                                } else {
                                    PLURAL_RULES.get(&cldr_lang_str).unwrap().clone()
                                };

                                (
                                    t.to_owned(),
                                    TimeagoTableEntry {
                                        cases: BTreeMap::new(),
                                        missing_plurals,
                                    },
                                )
                            })
                            .collect();

                        Some((lang.to_owned(), m))
                    }
                }
            })
            .collect(),
        errors: BTreeMap::new(),
    }
}

fn read_timeago_table() -> TimeagoTable {
    let json_path = Path::new("testfiles/date/timeago_table.json").to_path_buf();
    if json_path.exists() {
        let file = File::open(json_path).unwrap();
        serde_json::from_reader(BufReader::new(file)).unwrap()
    } else {
        new_timeago_table()
    }
}

fn write_timeago_table(timeago_table: &TimeagoTable) {
    let json_path = Path::new("testfiles/date/timeago_table.json").to_path_buf();
    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, timeago_table).unwrap();
}

fn insert_timeago_table(
    timeago_table: &mut TimeagoTable,
    lang: &Language,
    date_str: &str,
    limit: Option<TimeUnit>,
    ignore_1s: bool,
) -> bool {
    let pattern = TimeagoPattern::from(lang.to_owned());
    match pattern.parse(date_str) {
        Some(timeago) => {
            let entry = timeago_table
                .entries
                .get_mut(lang)
                .unwrap()
                .get_mut(&timeago.unit)
                .unwrap();

            let cldr_lang_str = &lang.to_string()[0..2];
            let l_id: LanguageIdentifier = cldr_lang_str.parse().unwrap();
            let pl_pat = PluralRules::create(l_id, PluralRuleType::CARDINAL).unwrap();
            let pl = PluralCat::from(pl_pat.select(timeago.n).unwrap());

            // Collect the case if its plural type is missing
            if entry.missing_plurals.remove(&pl) {
                entry.cases.insert(date_str.to_owned(), timeago.n);
                info!(
                    "Collected `{}` ({} {:?})",
                    date_str, timeago.n, timeago.unit
                );
            }

            timeago_table
                .entries
                .get(lang)
                .unwrap()
                .iter()
                .all(|(t, entry)| {
                    (limit.is_some() && t > &limit.unwrap())
                        || entry.missing_plurals.is_empty()
                        || (ignore_1s
                            && t == &TimeUnit::Second
                            && entry.missing_plurals.len() == 1
                            && entry.missing_plurals.contains(&PluralCat::One))
                })
        }
        None => {
            error!("Could not parse `{}`", date_str);
            let errors = timeago_table
                .errors
                .entry(*lang)
                .or_insert_with(|| HashSet::new());
            errors.insert(date_str.to_owned());
            false
        }
    }
}

async fn insert_timeago_table_datestrings(
    rp: &RustyTube,
    timeago_table: &mut TimeagoTable,
    video_id: &str,
    latest: bool,
    limit: Option<TimeUnit>,
    ignore_1s: bool,
) {
    let mut ctoken = get_comment_initial_ctoken(&rp, video_id, latest).await;
    let brace_pattern = Regex::new(r"\(.+\)").unwrap();
    let lang = &rp.localization.language;
    let err_baseline = timeago_table.errors.len();

    for _ in 0..40 {
        let (strings, new_ctoken) = get_comment_datestrings(&rp, &ctoken).await;

        let res = strings
            .iter()
            .map(|s| {
                // Remove zero-width space characters
                let s = s.replace('\u{200b}', "");

                // Remove braces
                let s = brace_pattern.replace(&s, "");

                let s = s.trim();
                s.to_owned()
            })
            .find(|s| insert_timeago_table(timeago_table, lang, &s, limit, ignore_1s));

        if res.is_some() {
            break;
        }

        if timeago_table.errors.len() > err_baseline {
            return;
        }

        if let Some(new_ctoken) = new_ctoken {
            ctoken = new_ctoken.to_owned();
        } else {
            error!("end of comments");
            break;
        }
    }
}

async fn insert_timeago_table_datestrings_channel(
    rp: &RustyTube,
    timeago_table: &mut TimeagoTable,
    channel_id: &str,
) {
    let lang = &rp.localization.language;

    let strings = get_channel_datestrings(rp, channel_id).await;

    strings
        .iter()
        .map(|s| {
            // Remove zero-width space characters
            let s = s.replace('\u{200b}', "");

            let s = s.trim();
            s.to_owned()
        })
        .for_each(|s| {
            insert_timeago_table(timeago_table, lang, &s, None, false);
        });
}

// #[test_log::test(tokio::test)]
#[allow(dead_code)]
async fn t_build_timeago_table() {
    let mut timeago_table = read_timeago_table();
    let ignore_1s = false;
    let langs = timeago_table
        .entries
        .keys()
        .map(|k| k.to_owned())
        .collect::<Vec<_>>();

    for lang in langs {
        if timeago_table
            .entries
            .get(&lang)
            .unwrap()
            .iter()
            .all(|(t, entry)| {
                entry.missing_plurals.is_empty()
                    || (ignore_1s
                        && t == &TimeUnit::Second
                        && entry.missing_plurals.len() == 1
                        && entry.missing_plurals.contains(&PluralCat::One))
            })
        {
            continue;
        }

        let rp = RustyTube::new_with_ua(lang, Country::Us, None);

        println!("{}: 1s!", lang);
        {
            let ctoken = get_comment_initial_ctoken(&rp, "gQlMMD8auMs", true).await;
            // let ctoken = get_comment_initial_ctoken(&rp, "k6jqx9kZgPM", true).await;
            let brace_pattern = Regex::new(r"\(.+\)").unwrap();
            let lang = &rp.localization.language;
            let err_baseline = timeago_table.errors.len();

            loop {
                let (strings, _) = get_comment_datestrings(&rp, &ctoken).await;
                println!("{}", strings[0]);

                let res = strings
                    .iter()
                    .map(|s| {
                        // Remove zero-width space characters
                        let s = s.replace('\u{200b}', "");

                        // Remove braces
                        let s = brace_pattern.replace(&s, "");

                        let s = s.trim();
                        s.to_owned()
                    })
                    .find(|s| {
                        insert_timeago_table(
                            &mut timeago_table,
                            lang,
                            &s,
                            Some(TimeUnit::Second),
                            ignore_1s,
                        )
                    });

                if res.is_some() {
                    break;
                }

                if timeago_table.errors.len() > err_baseline {
                    break;
                }
            }
        }

        println!("{}: 2s - n min", lang);
        insert_timeago_table_datestrings(
            &rp,
            &mut timeago_table,
            "gQlMMD8auMs",
            true,
            Some(TimeUnit::Minute),
            ignore_1s,
        )
        .await;
        println!("{}: x hr", lang);
        insert_timeago_table_datestrings(
            &rp,
            &mut timeago_table,
            "TohrPm3ICJE",
            true,
            Some(TimeUnit::Hour),
            ignore_1s,
        )
        .await;
        println!("{}: 1 hr - n day", lang);
        insert_timeago_table_datestrings(
            &rp,
            &mut timeago_table,
            "J9NQFACZYEU",
            true,
            Some(TimeUnit::Day),
            ignore_1s,
        )
        .await;
        println!("{}: week", lang);
        insert_timeago_table_datestrings(
            &rp,
            &mut timeago_table,
            "-zPDx6HQ_9w",
            true,
            Some(TimeUnit::Week),
            ignore_1s,
        )
        .await;

        println!("{}: 1 yr - n yr", lang);
        insert_timeago_table_datestrings_channel(
            &rp,
            &mut timeago_table,
            "UCEOXxzW2vU0P-0THehuIIeg",
        )
        .await;

        println!("{}: 11 mon", lang);
        insert_timeago_table_datestrings_channel(
            &rp,
            &mut timeago_table,
            "UCY1kMZp36IQSyNx_9h4mpCg",
        )
        .await;

        println!("{}: 13 yr", lang);
        insert_timeago_table_datestrings_channel(
            &rp,
            &mut timeago_table,
            "UCfw6qEAJMDbmgqQbuoB5moA",
        )
        .await;

        write_timeago_table(&timeago_table);
    }
}
