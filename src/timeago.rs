use std::{cmp::Ordering, ops::Mul};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{dictionary, model::Language, util};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TimeAgo {
    pub n: u8,
    pub unit: TimeUnit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaToken {
    pub n: u8,
    pub unit: Option<TimeUnit>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ParsedDate {
    Absolute(NaiveDate),
    Relative(TimeAgo),
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

pub enum DateCmp {
    Y,
    M,
    D,
}

impl TimeUnit {
    fn seconds(&self) -> u64 {
        match self {
            TimeUnit::Second => 1,
            TimeUnit::Minute => 60,
            TimeUnit::Hour => 3600,
            TimeUnit::Day => 24 * 3600,
            TimeUnit::Week => 7 * 24 * 3600,
            TimeUnit::Month => 30 * 24 * 3600,
            TimeUnit::Year => 365 * 24 * 3600,
        }
    }
}

impl TimeAgo {
    fn seconds(&self) -> u64 {
        self.n as u64 * self.unit.seconds()
    }
}

impl PartialEq for TimeAgo {
    fn eq(&self, other: &Self) -> bool {
        self.seconds() == other.seconds()
    }
}

impl Ord for TimeAgo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.seconds().cmp(&other.seconds())
    }
}

impl PartialOrd for TimeAgo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Mul<u8> for TimeAgo {
    type Output = Self;

    fn mul(self, rhs: u8) -> Self::Output {
        TimeAgo {
            n: self.n * rhs,
            unit: self.unit,
        }
    }
}

pub fn filter_str(string: &str) -> String {
    string
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c == '\u{200b}' || c.is_ascii_digit() {
                None
            } else if c == '-' {
                Some(' ')
            } else {
                Some(c)
            }
        })
        .collect()
}

fn parse_ta_token(entry: &dictionary::Entry, nd: bool, filtered_str: &str) -> Option<TimeAgo> {
    let tokens = match nd {
        true => &entry.timeago_nd_tokens,
        false => &entry.timeago_tokens,
    };
    let mut qu = 1;

    if entry.by_char {
        filtered_str.chars().find_map(|word| {
            tokens
                .get(&word.to_string())
                .map(|t| match t.unit {
                    Some(unit) => Some(TimeAgo { n: t.n * qu, unit }),
                    None => {
                        qu = t.n;
                        None
                    }
                })
                .flatten()
        })
    } else {
        filtered_str.split_whitespace().find_map(|word| {
            tokens
                .get(word)
                .map(|t| match t.unit {
                    Some(unit) => Some(TimeAgo { n: t.n * qu, unit }),
                    None => {
                        qu = t.n;
                        None
                    }
                })
                .flatten()
        })
    }
}

fn parse_textual_month(entry: &dictionary::Entry, filtered_str: &str) -> Option<u8> {
    if entry.by_char {
        // Chinese/Japanese dont use textual months
        None
    } else {
        filtered_str
            .split_whitespace()
            .find_map(|word| entry.months.get(word).map(|n| *n))
    }
}

pub fn parse(lang: Language, textual_date: &str) -> Option<TimeAgo> {
    let entry = dictionary::entry(lang);
    let filtered_str = filter_str(textual_date);

    let qu: u8 = util::parse_numeric(&textual_date).unwrap_or(1);

    parse_ta_token(&entry, false, &filtered_str).map(|ta| ta * qu)
}

fn parse_date(lang: Language, textual_date: &str) -> Option<ParsedDate> {
    let entry = dictionary::entry(lang);
    let filtered_str = filter_str(textual_date);

    let nums = util::parse_numeric_vec::<u16>(textual_date);

    match nums.len() {
        0 => match parse_ta_token(&entry, true, &filtered_str) {
            Some(timeago) => Some(ParsedDate::Relative(timeago)),
            None => parse_ta_token(&entry, false, &filtered_str)
                .map(|timeago| ParsedDate::Relative(timeago)),
        },
        1 => parse_ta_token(&entry, false, &filtered_str)
            .map(|timeago| ParsedDate::Relative(timeago * nums[0] as u8)),
        2..=3 => {
            if nums.len() == entry.date_order.len() {
                let mut y: Option<u16> = None;
                let mut m: Option<u16> = None;
                let mut d: Option<u16> = None;

                nums.iter()
                    .enumerate()
                    .for_each(|(i, n)| match entry.date_order[i] {
                        DateCmp::Y => y = Some(*n),
                        DateCmp::M => m = Some(*n),
                        DateCmp::D => d = Some(*n),
                    });

                if m.is_none() {
                    m = parse_textual_month(&entry, &filtered_str).map(|n| n as u16);
                }

                match (y, m, d) {
                    (Some(y), Some(m), Some(d)) => Some(ParsedDate::Absolute(NaiveDate::from_ymd(
                        y.into(),
                        m.into(),
                        d.into(),
                    ))),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Language::De, "vor 1 Sekunde", Some(TimeAgo { n: 1, unit: TimeUnit::Second }))]
    #[case(Language::Ar, "قبل ساعة واحدة", Some(TimeAgo { n: 1, unit: TimeUnit::Hour }))]
    // No-break space
    #[case(Language::De, "Vor 3\u{a0}Tagen aktualisiert", Some(TimeAgo { n: 3, unit: TimeUnit::Day }))]
    fn t_parse(
        #[case] lang: Language,
        #[case] textual_date: &str,
        #[case] expect: Option<TimeAgo>,
    ) {
        let time_ago = parse(lang, textual_date);
        assert_eq!(time_ago, expect);
    }

    #[test]
    fn t_testfile() {
        let json_path = Path::new("testfiles/date/timeago_samples.json");

        let expect = [
            TimeAgo {
                n: 10,
                unit: TimeUnit::Minute,
            },
            TimeAgo {
                n: 20,
                unit: TimeUnit::Minute,
            },
            TimeAgo {
                n: 1,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 2,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 7,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 8,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 9,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 10,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 11,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 12,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 13,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 14,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 15,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 3,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 4,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 4,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 5,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 6,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 6,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 20,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 2,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 3,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 5,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 6,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 8,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 10,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 12,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 2,
                unit: TimeUnit::Week,
            },
            TimeAgo {
                n: 3,
                unit: TimeUnit::Week,
            },
            TimeAgo {
                n: 4,
                unit: TimeUnit::Week,
            },
            TimeAgo {
                n: 1,
                unit: TimeUnit::Month,
            },
            TimeAgo {
                n: 8,
                unit: TimeUnit::Month,
            },
            TimeAgo {
                n: 11,
                unit: TimeUnit::Month,
            },
            TimeAgo {
                n: 1,
                unit: TimeUnit::Year,
            },
            TimeAgo {
                n: 2,
                unit: TimeUnit::Year,
            },
            TimeAgo {
                n: 3,
                unit: TimeUnit::Year,
            },
            TimeAgo {
                n: 4,
                unit: TimeUnit::Year,
            },
        ];

        let json_file = File::open(json_path).unwrap();
        let strings_map: BTreeMap<Language, Vec<String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        strings_map.iter().for_each(|(lang, strings)| {
            assert_eq!(strings.len(), expect.len());
            strings.iter().enumerate().for_each(|(n, s)| {
                assert_eq!(
                    parse(*lang, s),
                    Some(expect[n]),
                    "Language: {}, n: {}",
                    lang,
                    n
                );
            });
        })
    }

    #[test]
    fn t_timeago_table() {
        #[derive(Debug, Clone, Deserialize)]
        struct TimeagoTable {
            entries: BTreeMap<Language, BTreeMap<TimeUnit, TimeagoTableEntry>>,
        }

        #[derive(Debug, Clone, Deserialize)]
        struct TimeagoTableEntry {
            cases: BTreeMap<String, u8>,
        }

        let json_path = Path::new("testfiles/date/timeago_table.json");
        let json_file = File::open(json_path).unwrap();
        let timeago_table: TimeagoTable =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let mut n_cases = 0;

        timeago_table.entries.iter().for_each(|(lang, entries)| {
            entries.iter().for_each(|(t, entry)| {
                entry.cases.iter().for_each(|(txt, n)| {
                    let timeago = parse(*lang, txt);
                    assert_eq!(
                        timeago,
                        Some(TimeAgo { n: *n, unit: *t }),
                        "lang: {}, txt: {}",
                        lang,
                        txt
                    );

                    n_cases += 1;
                })
            });
        });

        assert_eq!(n_cases, 1065)
    }

    #[rstest]
    #[case(Language::En, "Updated today", Some(ParsedDate::Relative(TimeAgo { n: 0, unit: TimeUnit::Day })))]
    #[case(Language::En, "Updated yesterday", Some(ParsedDate::Relative(TimeAgo { n: 1, unit: TimeUnit::Day })))]
    #[case(Language::En, "Updated 2 days ago", Some(ParsedDate::Relative(TimeAgo { n: 2, unit: TimeUnit::Day })))]
    #[case(Language::Si, "ඊයේ යාවත්කාලීන කරන ලදී", Some(ParsedDate::Relative(TimeAgo { n: 1, unit: TimeUnit::Day })))]
    #[case(
        Language::En,
        "Last updated on Jun 04, 2003",
        Some(ParsedDate::Absolute(NaiveDate::from_ymd(2003, 6, 4)))
    )]
    fn t_parse_date(
        #[case] lang: Language,
        #[case] textual_date: &str,
        #[case] expect: Option<ParsedDate>,
    ) {
        let parsed_date = parse_date(lang, textual_date);
        assert_eq!(parsed_date, expect);
    }

    #[test]
    fn t_parse_date_samples() {
        let json_path = Path::new("testfiles/date/playlist_samples.json");
        let json_file = File::open(json_path).unwrap();
        let date_samples: BTreeMap<Language, BTreeMap<String, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        date_samples.iter().for_each(|(lang, samples)| {
            assert_eq!(
                parse_date(*lang, samples.get("Today").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 0,
                    unit: TimeUnit::Day
                })),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Yesterday").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    // YT's Singhalese translation has an error (yesterday == today)
                    n: match lang {
                        Language::Si => 0,
                        _ => 1,
                    },
                    unit: TimeUnit::Day
                })),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Ago").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 3,
                    unit: TimeUnit::Day
                })),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Jan").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2020, 1, 3))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Feb").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2016, 2, 7))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Mar").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2015, 3, 9))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Apr").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2017, 4, 2))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("May").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2014, 5, 22))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Jun").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2014, 6, 28))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Jul").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2014, 7, 2))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Aug").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2015, 8, 23))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Sep").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2018, 9, 16))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Oct").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2014, 10, 31))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Nov").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2016, 11, 3))),
                "lang: {}",
                lang
            );
            assert_eq!(
                parse_date(*lang, samples.get("Dec").unwrap()),
                Some(ParsedDate::Absolute(NaiveDate::from_ymd(2021, 12, 24))),
                "lang: {}",
                lang
            );
        })
    }
}
