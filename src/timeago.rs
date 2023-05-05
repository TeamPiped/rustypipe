//! Parser for textual dates and times.
//!
//! The YouTube API mostly outputs pre-formatted dates and times
//! like "18 minutes ago" or "Jul 2, 2014" instead of standardized
//! machine-readable date and time formats.
//!
//! Additionally these formats are localized, meaning they depend
//! on the configured language.
//!
//! This module can parse these dates using an embedded dictionary which
//! contains date/time unit tokens for all supported languages.
//!
//! Note that this module is public so it can be tested from outside
//! the crate, which is important for including new languages, too.
//!
//! It is not intended to be used to parse textual dates that are not from YouTube.

use std::ops::Mul;

use serde::{Deserialize, Serialize};
use time::{Date, Duration, Month, OffsetDateTime};

use crate::{
    param::Language,
    util::{self, dictionary},
};

/// Parsed TimeAgo string, contains amount and time unit.
///
/// Example: "14 hours ago" => `TimeAgo {n: 14, unit: TimeUnit::Hour}`
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeAgo {
    /// Number of time units
    pub n: u8,
    /// Time unit
    pub unit: TimeUnit,
}

/// Parsed date string that may be relative or absolute.
///
/// Examples:
///
/// - "Jul 2, 2014" => `ParsedDate::Absolute("2014-07-02")`
/// - "2 months ago" => `ParsedDate::Relative(TimeAgo {n: 2, unit: TimeUnit::Month})`
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ParsedDate {
    /// Absolute date
    ///
    /// Example: "Jul 2, 2014"
    Absolute(Date),
    /// Relative date
    ///
    /// Example: "2 months ago"
    Relative(TimeAgo),
}

/// Parsed time unit
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Value of a parsed TimeAgo token, used in the dictionary
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct TaToken {
    pub n: u8,
    pub unit: Option<TimeUnit>,
}

pub(crate) enum DateCmp {
    Y,
    M,
    D,
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

impl From<TimeAgo> for Duration {
    fn from(ta: TimeAgo) -> Self {
        match ta.unit {
            TimeUnit::Second => Duration::seconds(ta.n as i64),
            TimeUnit::Minute => Duration::minutes(ta.n as i64),
            TimeUnit::Hour => Duration::hours(ta.n as i64),
            TimeUnit::Day => Duration::days(ta.n as i64),
            TimeUnit::Week => Duration::weeks(ta.n as i64),
            TimeUnit::Month => Duration::days(ta.n as i64 * 30),
            TimeUnit::Year => Duration::days(ta.n as i64 * 365),
        }
    }
}

impl From<TimeAgo> for OffsetDateTime {
    fn from(ta: TimeAgo) -> Self {
        let ts = util::now_sec();
        match ta.unit {
            TimeUnit::Month => ts.replace_date(util::shift_months(ts.date(), -(ta.n as i32))),
            TimeUnit::Year => ts.replace_date(util::shift_years(ts.date(), -(ta.n as i32))),
            _ => ts - Duration::from(ta),
        }
    }
}

impl From<ParsedDate> for OffsetDateTime {
    fn from(date: ParsedDate) -> Self {
        match date {
            ParsedDate::Absolute(date) => date.with_hms(0, 0, 0).unwrap().assume_utc(),
            ParsedDate::Relative(timeago) => timeago.into(),
        }
    }
}

fn filter_str(string: &str) -> String {
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
            tokens.get(&word.to_string()).and_then(|t| match t.unit {
                Some(unit) => Some(TimeAgo { n: t.n * qu, unit }),
                None => {
                    qu = t.n;
                    None
                }
            })
        })
    } else {
        filtered_str.split_whitespace().find_map(|word| {
            tokens.get(word).and_then(|t| match t.unit {
                Some(unit) => Some(TimeAgo { n: t.n * qu, unit }),
                None => {
                    qu = t.n;
                    None
                }
            })
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
            .find_map(|word| entry.months.get(word).copied())
    }
}

/// Parse a TimeAgo string (e.g. "29 minutes ago") into a TimeAgo object.
///
/// Returns None if the date could not be parsed.
pub fn parse_timeago(lang: Language, textual_date: &str) -> Option<TimeAgo> {
    let entry = dictionary::entry(lang);
    let filtered_str = filter_str(textual_date);

    let qu: u8 = util::parse_numeric(textual_date).unwrap_or(1);

    parse_ta_token(&entry, false, &filtered_str).map(|ta| ta * qu)
}

/// Parse a TimeAgo string (e.g. "29 minutes ago") into a Chrono DateTime object.
///
/// Returns None if the date could not be parsed.
pub fn parse_timeago_dt(lang: Language, textual_date: &str) -> Option<OffsetDateTime> {
    parse_timeago(lang, textual_date).map(|ta| ta.into())
}

pub(crate) fn parse_timeago_or_warn(
    lang: Language,
    textual_date: &str,
    warnings: &mut Vec<String>,
) -> Option<TimeAgo> {
    let res = parse_timeago(lang, textual_date);
    if res.is_none() {
        warnings.push(format!("could not parse timeago `{textual_date}`"));
    }
    res
}

pub(crate) fn parse_timeago_dt_or_warn(
    lang: Language,
    textual_date: &str,
    warnings: &mut Vec<String>,
) -> Option<OffsetDateTime> {
    let res = parse_timeago_dt(lang, textual_date);
    if res.is_none() {
        warnings.push(format!("could not parse timeago `{textual_date}`"));
    }
    res
}

/// Parse a textual date (e.g. "29 minutes ago" or "Jul 2, 2014") into a ParsedDate object.
///
/// Returns None if the date could not be parsed.
pub fn parse_textual_date(lang: Language, textual_date: &str) -> Option<ParsedDate> {
    let entry = dictionary::entry(lang);
    let filtered_str = filter_str(textual_date);

    let nums = util::parse_numeric_vec::<u16>(textual_date);

    match nums.len() {
        0 => match parse_ta_token(&entry, true, &filtered_str) {
            Some(timeago) => Some(ParsedDate::Relative(timeago)),
            None => parse_ta_token(&entry, false, &filtered_str).map(ParsedDate::Relative),
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
                    (Some(y), Some(m), Some(d)) => Month::try_from(m as u8)
                        .ok()
                        .and_then(|m| Date::from_calendar_date(y.into(), m, d as u8).ok())
                        .map(ParsedDate::Absolute),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse a textual date (e.g. "29 minutes ago" or "Jul 2, 2014") into a Chrono DateTime object.
///
/// Returns None if the date could not be parsed.
pub fn parse_textual_date_to_dt(lang: Language, textual_date: &str) -> Option<OffsetDateTime> {
    parse_textual_date(lang, textual_date).map(|ta| ta.into())
}

pub(crate) fn parse_textual_date_or_warn(
    lang: Language,
    textual_date: &str,
    warnings: &mut Vec<String>,
) -> Option<OffsetDateTime> {
    let res = parse_textual_date_to_dt(lang, textual_date);
    if res.is_none() {
        warnings.push(format!("could not parse textual date `{textual_date}`"));
    }
    res
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;
    use time::macros::{date, datetime};

    use super::*;
    use crate::util::tests::TESTFILES;

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
        let time_ago = parse_timeago(lang, textual_date);
        assert_eq!(time_ago, expect);
    }

    #[test]
    fn t_testfile() {
        let json_path = path!(*TESTFILES / "dict" / "timeago_samples.json");

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
                    parse_timeago(*lang, s),
                    Some(expect[n]),
                    "Language: {lang}, n: {n}"
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

        let json_path = path!(*TESTFILES / "dict" / "timeago_table.json");
        let json_file = File::open(json_path).unwrap();
        let timeago_table: TimeagoTable =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let mut n_cases = 0;

        timeago_table.entries.iter().for_each(|(lang, entries)| {
            entries.iter().for_each(|(t, entry)| {
                entry.cases.iter().for_each(|(txt, n)| {
                    let timeago = parse_timeago(*lang, txt);
                    assert_eq!(
                        timeago,
                        Some(TimeAgo { n: *n, unit: *t }),
                        "lang: {lang}, txt: {txt}"
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
        Some(ParsedDate::Absolute(date!(2003-6-4)))
    )]
    #[case(
        Language::Bn,
        "যোগ দিয়েছেন 24 সেপ, 2013",
        Some(ParsedDate::Absolute(date!(2013-9-24)))
    )]
    fn t_parse_date(
        #[case] lang: Language,
        #[case] textual_date: &str,
        #[case] expect: Option<ParsedDate>,
    ) {
        let parsed_date = parse_textual_date(lang, textual_date);
        assert_eq!(parsed_date, expect);
    }

    #[test]
    #[ignore]
    fn t_parse_date_samples() {
        let json_path = path!(*TESTFILES / "dict" / "playlist_samples.json");
        let json_file = File::open(json_path).unwrap();
        let date_samples: BTreeMap<Language, BTreeMap<String, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        date_samples.iter().for_each(|(lang, samples)| {
            assert_eq!(
                parse_textual_date(*lang, samples.get("Today").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 0,
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Yesterday").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    // YT's Singhalese translation has an error (yesterday == today)
                    n: match lang {
                        Language::Si => 0,
                        _ => 1,
                    },
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Ago").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 3,
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Jan").unwrap()),
                Some(ParsedDate::Absolute(date!(2020 - 1 - 3))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Feb").unwrap()),
                Some(ParsedDate::Absolute(date!(2016 - 2 - 7))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Mar").unwrap()),
                Some(ParsedDate::Absolute(date!(2015 - 3 - 9))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Apr").unwrap()),
                Some(ParsedDate::Absolute(date!(2017 - 4 - 2))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("May").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 5 - 22))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Jun").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 6 - 28))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Jul").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 7 - 2))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Aug").unwrap()),
                Some(ParsedDate::Absolute(date!(2015 - 8 - 23))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Sep").unwrap()),
                Some(ParsedDate::Absolute(date!(2018 - 9 - 16))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Oct").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 10 - 31))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Nov").unwrap()),
                Some(ParsedDate::Absolute(date!(2016 - 11 - 3))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, samples.get("Dec").unwrap()),
                Some(ParsedDate::Absolute(date!(2021 - 12 - 24))),
                "lang: {lang}"
            );
        })
    }

    #[test]
    fn t_to_datetime() {
        // Absolute date
        let date = parse_textual_date_to_dt(Language::En, "Last updated on Jan 3, 2020").unwrap();
        assert_eq!(date, datetime!(2020-1-3 0:00 +0));

        // Relative date
        let date = parse_textual_date_to_dt(Language::En, "1 year ago").unwrap();
        let now = OffsetDateTime::now_utc();
        assert_eq!(date.year(), now.year() - 1);
    }
}
