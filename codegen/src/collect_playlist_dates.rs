use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    hash::Hash,
    io::BufReader,
    path::Path,
};

use futures::{stream, StreamExt};
use path_macro::path;
use rustypipe::{
    client::RustyPipe,
    param::{locale::LANGUAGES, Language},
    timeago::{self, TimeAgo},
};
use serde::{Deserialize, Serialize};

use crate::util;

type CollectedDates = BTreeMap<Language, BTreeMap<DateCase, String>>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
enum DateCase {
    Today,
    Yesterday,
    Ago,
    Jan,
    Feb,
    Mar,
    Apr,
    May,
    Jun,
    Jul,
    Aug,
    Sep,
    Oct,
    Nov,
    Dec,
}

/// Collect 'Playlist updated' dates in every supported language
/// and write them to `testfiles/dict/playlist_samples.json`.
///
/// YouTube's API outputs the update date of playlists only in a
/// textual format (e.g. *Last updated on Jan 3, 2020*), which varies
/// by language.
///
/// For recently updated playlists YouTube shows 'today', 'yesterday'
/// and 'x<=7 days ago' instead of the literal date.
///
/// To parse these dates correctly we need to collect a sample set
/// in every language.
///
/// This set includes
/// - one playlist updated today
/// - one playlist updated yesterday
/// - one playlist updated 2-7 days ago
/// - one playlist from every month. Note that there should not
///   be any dates which include the same number twice (e.g. 01.01.2020).
///
/// **IMPORTANT:**
///
/// Because the relative dates change with time, the first three playlists
/// have to checked and eventually changed before running the program.
pub async fn collect_dates(project_root: &Path, concurrency: usize) {
    let json_path = path!(project_root / "testfiles" / "dict" / "playlist_samples.json");

    // These are the sample playlists
    let cases = [
        (
            DateCase::Today,
            "RDCLAK5uy_kj3rhiar1LINmyDcuFnXihEO0K1NQa2jI",
        ),
        (DateCase::Yesterday, "PL7zsB-C3aNu2yRY2869T0zj1FhtRIu5am"),
        (DateCase::Ago, "PLmB6td997u3kUOrfFwkULZ910ho44oQSy"),
        (DateCase::Jan, "PL1J-6JOckZtFjcni6Xj1pLYglJp6JCpKD"),
        (DateCase::Feb, "PL1J-6JOckZtETrbzwZE7mRIIK6BzWNLAs"),
        (DateCase::Mar, "PL1J-6JOckZtG3AVdvBXhMO64mB2k3BtKi"),
        (DateCase::Apr, "PL1J-6JOckZtE_rUpK24S6X5hOE4eQoprN"),
        (DateCase::May, "PL1J-6JOckZtG1ThBxoSLFL-Jg4sa2iX_a"),
        (DateCase::Jun, "PL1J-6JOckZtF_wSzkXBl91pit9d6Fh0QF"),
        (DateCase::Jul, "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
        (DateCase::Aug, "PL1J-6JOckZtFFQeWx-ZC0ubpJCEWmGWRx"),
        (DateCase::Sep, "PL1J-6JOckZtHVs0JhBW_qfsW-dtXuM0mQ"),
        (DateCase::Oct, "PL1J-6JOckZtE4g-XgZkL_N0kkoKui5Eys"),
        (DateCase::Nov, "PL1J-6JOckZtEzjMUEyPyPpG836pjeIapw"),
        (DateCase::Dec, "PL1J-6JOckZtHo91uApeb10Qlf2XhkfM-9"),
    ];

    let rp = RustyPipe::new();
    let collected_dates = stream::iter(LANGUAGES)
        .map(|lang| {
            let rp = rp.clone();
            async move {
                let mut map: BTreeMap<DateCase, String> = BTreeMap::new();

                for (case, pl_id) in cases {
                    let playlist = rp.query().lang(lang).playlist(pl_id).await.unwrap();
                    map.insert(case, playlist.last_update_txt.unwrap());
                }

                (lang, map)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<BTreeMap<_, _>>()
        .await;

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &collected_dates).unwrap();
}

/// Attempt to parse the dates collected by `collect-playlist-dates`
/// and write the results to `dictionary.json`.
///
/// The ND (no digit) tokens (today, tomorrow) of some languages cannot be
/// parsed automatically and require manual work.
pub fn write_samples_to_dict(project_root: &Path) {
    let json_path = path!(project_root / "testfiles" / "dict" / "playlist_samples.json");

    let json_file = File::open(json_path).unwrap();
    let collected_dates: CollectedDates =
        serde_json::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict(project_root);
    let langs = dict.keys().map(|k| k.to_owned()).collect::<Vec<_>>();

    let months = [
        DateCase::Jan,
        DateCase::Feb,
        DateCase::Mar,
        DateCase::Apr,
        DateCase::May,
        DateCase::Jun,
        DateCase::Jul,
        DateCase::Aug,
        DateCase::Sep,
        DateCase::Oct,
        DateCase::Nov,
        DateCase::Dec,
    ];

    let dates: [(u32, u32, u32); 12] = [
        (2020, 1, 3),
        (2016, 2, 7),
        (2015, 3, 9),
        (2017, 4, 2),
        (2014, 5, 22),
        (2014, 6, 28),
        (2014, 7, 2),
        (2015, 8, 23),
        (2018, 9, 16),
        (2014, 10, 31),
        (2016, 11, 3),
        (2021, 12, 24),
    ];

    for lang in langs {
        let mut datestr_tables = vec![collected_dates.get(&lang).unwrap()];
        dict.get(&lang)
            .unwrap()
            .equivalent
            .iter()
            .for_each(|l| datestr_tables.push(collected_dates.get(l).unwrap()));

        let dict_entry = dict.entry(lang).or_default();
        let mut num_order = "".to_owned();

        let collect_nd_tokens = !matches!(
            lang,
            // ND tokens of these languages must be edited manually
            Language::Ja
            | Language::ZhCn
            | Language::ZhHk
            | Language::ZhTw
            | Language::Ko
            | Language::Gu
            | Language::Pa
            | Language::Ur
            | Language::Uz
            | Language::Te
            | Language::PtPt
            // Singhalese YT translation has an error (today == tomorrow)
            | Language::Si
        );

        dict_entry.months = BTreeMap::new();

        if collect_nd_tokens {
            dict_entry.timeago_nd_tokens = BTreeMap::new();
        }

        for datestr_table in &datestr_tables {
            let mut month_words: HashMap<String, usize> = HashMap::new();
            let mut td_words: HashMap<String, i8> = HashMap::new();

            // Today/Yesterday
            {
                let mut parse = |string: &str, n: i8| {
                    util::filter_datestr(string)
                        .split_whitespace()
                        .for_each(|word| {
                            td_words
                                .entry(word.to_owned())
                                .and_modify(|e| *e = 0)
                                .or_insert(n);
                        });
                };

                parse(datestr_table.get(&DateCase::Today).unwrap(), 1);
                parse(datestr_table.get(&DateCase::Yesterday).unwrap(), 2);
                parse(datestr_table.get(&DateCase::Ago).unwrap(), 0);
                parse(datestr_table.get(&DateCase::Jan).unwrap(), 0);
            }

            // n days ago
            {
                let datestr = datestr_table.get(&DateCase::Ago).unwrap();
                let tago = timeago::parse_timeago(lang, datestr);
                assert_eq!(
                    tago,
                    Some(TimeAgo {
                        n: 3,
                        unit: timeago::TimeUnit::Day
                    }),
                    "lang: {}, txt: {}",
                    lang,
                    datestr
                );
            }

            // Absolute dates (Jan 3, 2020)
            months.iter().enumerate().for_each(|(n, m)| {
                let datestr = datestr_table.get(m).unwrap();

                // Get order of numbers
                let nums = util::parse_numeric_vec::<u32>(datestr);
                let date = dates[n];

                let this_num_order = nums
                    .iter()
                    .map(|n| {
                        if n == &date.0 {
                            "Y"
                        } else if n == &date.1 {
                            "M"
                        } else if n == &date.2 {
                            "D"
                        } else {
                            panic!("invalid number {} in {}", n, datestr);
                        }
                    })
                    .collect::<String>();

                if num_order.is_empty() {
                    num_order = this_num_order;
                } else {
                    assert_eq!(this_num_order, num_order, "lang: {}", lang);
                }

                // Insert words into the map
                util::filter_datestr(datestr)
                    .split_whitespace()
                    .for_each(|word| {
                        month_words
                            .entry(word.to_owned())
                            .and_modify(|e| *e = 0)
                            .or_insert(n + 1);
                    });
            });

            month_words.iter().for_each(|(word, m)| {
                if *m != 0 {
                    dict_entry.months.insert(word.to_owned(), *m as u8);
                };
            });

            if collect_nd_tokens {
                td_words.iter().for_each(|(word, n)| {
                    match n {
                        // Today
                        1 => {
                            dict_entry
                                .timeago_nd_tokens
                                .insert(word.to_owned(), "0D".to_owned());
                        }
                        // Yesterday
                        2 => {
                            dict_entry
                                .timeago_nd_tokens
                                .insert(word.to_owned(), "1D".to_owned());
                        }
                        _ => {}
                    };
                });

                if datestr_tables.len() == 1 {
                    assert_eq!(
                        dict_entry.timeago_nd_tokens.len(),
                        2,
                        "lang: {}, nd_tokens: {:?}",
                        lang,
                        &dict_entry.timeago_nd_tokens
                    );
                }
            }
        }
        dict_entry.date_order = num_order;
    }

    util::write_dict(project_root, &dict);
}
