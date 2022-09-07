#![cfg(test)]

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    hash::Hash,
    io::BufReader,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{
    client::RustyTube,
    model::{locale::LANGUAGES, Country, Language},
    timeago::{self, TimeAgo},
    util,
};

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

// #[test_log::test(tokio::test)]
async fn collect_dates() {
    let json_path = Path::new("testfiles/date/playlist_samples.json").to_path_buf();
    if json_path.exists() {
        return;
    }

    let cases = [
        (
            DateCase::Today,
            "RDCLAK5uy_kj3rhiar1LINmyDcuFnXihEO0K1NQa2jI",
        ),
        (DateCase::Yesterday, "PLmB6td997u3kUOrfFwkULZ910ho44oQSy"),
        (DateCase::Ago, "PL7zsB-C3aNu2yRY2869T0zj1FhtRIu5am"),
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

    let mut collected_dates = CollectedDates::new();

    for lang in LANGUAGES {
        let rp = RustyTube::new_with_ua(lang, Country::Us, None);
        let mut map: BTreeMap<DateCase, String> = BTreeMap::new();

        for (case, pl_id) in cases {
            let playlist = rp.get_playlist(pl_id).await.unwrap();
            map.insert(case, playlist.last_update_txt.unwrap());
        }

        collected_dates.insert(lang, map);
    }

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &collected_dates).unwrap();
}

fn filter_str(string: &str) -> String {
    string
        .to_lowercase()
        .chars()
        .filter(|c| c != &'\u{200b}' && !c.is_ascii_digit())
        .collect()
}

#[test]
fn write_samples_to_dict() {
    let json_path = Path::new("testfiles/date/playlist_samples.json").to_path_buf();
    let json_file = File::open(json_path).unwrap();
    let collected_dates: CollectedDates =
        serde_json::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = super::read_dict();
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
        let datestr_table = collected_dates.get(&lang).unwrap();
        let mut month_words: HashMap<String, usize> = HashMap::new();
        let mut num_order = "".to_owned();

        // Today/Yesterday
        let mut td_words: HashMap<String, i8> = HashMap::new();
        {
            let mut parse = |string: &str, n: i8| {
                filter_str(string).split_whitespace().for_each(|word| {
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
            let tago = timeago::parse(lang, &datestr);
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
            let nums = util::parse_numeric_vec::<u32>(&datestr);
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

            if num_order == "" {
                num_order = this_num_order;
            } else {
                assert_eq!(this_num_order, num_order);
            }

            // Insert words into the map
            filter_str(&datestr).split_whitespace().for_each(|word| {
                month_words
                    .entry(word.to_owned())
                    .and_modify(|e| *e = 0)
                    .or_insert(n + 1);
            });
        });

        let dict_entry = dict.entry(lang).or_default();
        dict_entry.date_order = num_order;
        dict_entry.months = month_words
            .iter()
            .filter_map(|(word, m)| {
                if *m == 0 {
                    None
                } else {
                    Some((word.to_owned(), *m as u8))
                }
            })
            .collect();

        match lang {
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
            // Singhalese YT translation is broken (today == tomorrow)
            | Language::Si => {}
            _ => {
                dict_entry.timeago_nd_tokens = td_words
                    .iter()
                    .filter_map(|(word, n)| {
                        match n {
                            // Today
                            1 => Some((word.to_owned(), "0D".to_owned())),
                            // Yesterday
                            2 => Some((word.to_owned(), "1D".to_owned())),
                            _ => None,
                        }
                    })
                    .collect();

                assert_eq!(dict_entry.timeago_nd_tokens.len(), 2, "lang: {}, nd_tokens: {:?}", lang, &dict_entry.timeago_nd_tokens);
            }
        }
    }

    super::write_dict(&dict);
}
