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

use std::ops::Mul;

use serde::{Deserialize, Serialize};
use time::{Date, Duration, Month, OffsetDateTime, UtcOffset};

use crate::{
    param::Language,
    util::{self, dictionary, SplitTokens},
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
    LastWeek,
    LastWeekday,
}

/// Value of a parsed TimeAgo token, used in the dictionary
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaToken {
    pub n: u8,
    pub unit: Option<TimeUnit>,
}

impl TimeUnit {
    pub fn secs(self) -> u32 {
        match self {
            TimeUnit::Second => 1,
            TimeUnit::Minute => 60,
            TimeUnit::Hour => 3600,
            TimeUnit::Day => 24 * 3600,
            TimeUnit::Week => 7 * 24 * 3600,
            TimeUnit::Month => 30 * 24 * 3600,
            TimeUnit::Year => 365 * 24 * 3600,
            TimeUnit::LastWeekday | TimeUnit::LastWeek => 0,
        }
    }
}

impl TaToken {
    fn into_timeago(self) -> Option<TimeAgo> {
        self.unit.map(|unit| TimeAgo { n: self.n, unit })
    }
}

impl TimeAgo {
    fn secs(self) -> u32 {
        u32::from(self.n) * self.unit.secs()
    }

    fn into_datetime(self, utc_offset: UtcOffset) -> OffsetDateTime {
        let ts = util::now_sec().to_offset(utc_offset);
        match self.unit {
            TimeUnit::Month => ts.replace_date(util::shift_months(ts.date(), -i32::from(self.n))),
            TimeUnit::Year => ts.replace_date(util::shift_years(ts.date(), -i32::from(self.n))),
            TimeUnit::LastWeek => {
                ts.replace_date(util::shift_weeks_monday(ts.date(), -i32::from(self.n)))
            }
            TimeUnit::LastWeekday => ts.replace_date(
                Date::from_iso_week_date(
                    ts.year(),
                    ts.iso_week(),
                    time::Weekday::Monday.nth_next(self.n),
                )
                .unwrap(),
            ),
            _ => ts - Duration::from(self),
        }
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

impl From<TimeAgo> for Duration {
    fn from(ta: TimeAgo) -> Self {
        Duration::seconds(ta.secs().into())
    }
}

impl ParsedDate {
    fn into_datetime(self, utc_offset: UtcOffset) -> OffsetDateTime {
        match self {
            ParsedDate::Absolute(date) => date.with_hms(0, 0, 0).unwrap().assume_offset(utc_offset),
            ParsedDate::Relative(timeago) => timeago.into_datetime(utc_offset),
        }
    }
}

/// Prepare the datestring for parsing: lowercase and filter out unnecessary punctuation
fn filter_datestr(string: &str) -> String {
    string
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if matches!(c, '\u{200b}' | '.' | ',') || c.is_ascii_digit() {
                None
            } else if c == '-' {
                Some(' ')
            } else {
                Some(c)
            }
        })
        .collect()
}

struct TaTokenParser<'a> {
    iter: SplitTokens<'a>,
    tokens: &'a phf::Map<&'static str, TaToken>,
}

impl<'a> TaTokenParser<'a> {
    fn new(entry: &'a dictionary::Entry, by_char: bool, nd: bool, filtered_str: &'a str) -> Self {
        let tokens = if nd {
            &entry.timeago_nd_tokens
        } else {
            &entry.timeago_tokens
        };
        Self {
            iter: SplitTokens::new(filtered_str, by_char),
            tokens,
        }
    }
}

impl Iterator for TaTokenParser<'_> {
    type Item = TimeAgo;

    fn next(&mut self) -> Option<Self::Item> {
        // Quantity for parsing separate quantity + unit tokens
        let mut qu = 1;
        self.iter.find_map(|word| {
            self.tokens.get(word).and_then(|t| match t.unit {
                Some(unit) => Some(TimeAgo { n: t.n * qu, unit }),
                None => {
                    qu = t.n;
                    None
                }
            })
        })
    }
}

fn parse_textual_month(lang: Language, filtered_str: &str) -> Option<u8> {
    let entry = dictionary::entry(lang);
    filtered_str
        .split_whitespace()
        .find_map(|word| entry.months.get(word).copied())
        .map(|mon| {
            // Mongolian has an extra number word that adds 10 to a month
            if lang == Language::Mn && filtered_str.split_whitespace().any(|s| s == "арван") {
                mon + 10
            } else {
                mon
            }
        })
}

/// Parse a TimeAgo string (e.g. "29 minutes ago") into a TimeAgo object.
///
/// Returns [`None`] if the date could not be parsed.
pub fn parse_timeago(lang: Language, textual_date: &str) -> Option<TimeAgo> {
    let entry = dictionary::entry(lang);
    let filtered_str = filter_datestr(textual_date);

    let qu: u8 = util::parse_numeric_prod(textual_date).unwrap_or(1);

    // French uses 'a' as a short form of years.
    // Since 'a' is also a word in French, it cannot be parsed as a token.
    if matches!(
        lang,
        Language::Fr | Language::FrCa | Language::Es | Language::Es419 | Language::EsUs
    ) && textual_date.ends_with(" a")
    {
        return Some(TimeAgo {
            n: qu,
            unit: TimeUnit::Year,
        });
    }

    TaTokenParser::new(&entry, util::lang_by_char(lang), false, &filtered_str)
        .next()
        .map(|ta| ta * qu)
}

/// Parse a TimeAgo string (e.g. "29 minutes ago") into a Chrono DateTime object.
///
/// Returns [`None`] if the date could not be parsed.
pub fn parse_timeago_dt(lang: Language, textual_date: &str) -> Option<OffsetDateTime> {
    parse_timeago(lang, textual_date).map(|t| t.into_datetime(UtcOffset::UTC))
}

pub fn parse_timeago_dt_or_warn(
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
/// Returns [`None`] if the date could not be parsed.
pub fn parse_textual_date(
    lang: Language,
    utc_offset: UtcOffset,
    textual_date: &str,
) -> Option<ParsedDate> {
    let entry = dictionary::entry(lang);
    let by_char = util::lang_by_char(lang);
    let filtered_str = filter_datestr(textual_date);

    let nums = util::parse_numeric_vec::<u16>(textual_date);

    if nums.is_empty() {
        entry
            .timeago_nd_tokens
            .get(&filtered_str)
            .and_then(|t| t.into_timeago())
            .or_else(|| TaTokenParser::new(&entry, by_char, true, &filtered_str).next())
            .or_else(|| TaTokenParser::new(&entry, by_char, false, &filtered_str).next())
            .map(ParsedDate::Relative)
    } else {
        if nums.len() == 1 && nums[0] < 2000 {
            if let Some(timeago) = TaTokenParser::new(&entry, by_char, false, &filtered_str).next()
            {
                return Some(ParsedDate::Relative(timeago * nums[0] as u8));
            }
        }

        let mut y: Option<u16> = None;
        let mut m = parse_textual_month(lang, &filtered_str).map(u16::from);
        let mut d: Option<u16> = None;

        for num in nums {
            if num > 31 {
                if y.is_none() {
                    y = Some(num);
                } else {
                    return None;
                }
            } else if m.is_none() && (entry.month_before_day || d.is_some()) {
                m = Some(num);
            } else if d.is_none() {
                d = Some(num);
            } else {
                return None;
            }
        }
        if m.is_none() && d.is_some() {
            m = d;
            d = None;
        }

        match (y, m, d) {
            (y, Some(m), d) => Month::try_from(m as u8)
                .ok()
                .and_then(|m| {
                    Date::from_calendar_date(
                        y.map(i32::from).unwrap_or_else(|| {
                            OffsetDateTime::now_utc().to_offset(utc_offset).year()
                        }),
                        m,
                        d.unwrap_or(1) as u8,
                    )
                    .ok()
                })
                .map(ParsedDate::Absolute),
            _ => None,
        }
    }
}

/// Parse a textual date (e.g. "29 minutes ago" or "Jul 2, 2014") into a OffsetDateTime object.
///
/// Returns None if the date could not be parsed.
pub fn parse_textual_date_to_dt(
    lang: Language,
    utc_offset: UtcOffset,
    textual_date: &str,
) -> Option<OffsetDateTime> {
    parse_textual_date(lang, utc_offset, textual_date).map(|t| t.into_datetime(utc_offset))
}

/// Parse a textual date (e.g. "29 minutes ago" "Jul 2, 2014") into a Date object.
///
/// Returns None if the date could not be parsed.
#[cfg(feature = "userdata")]
pub fn parse_textual_date_to_d(
    lang: Language,
    utc_offset: UtcOffset,
    textual_date: &str,
    warnings: &mut Vec<String>,
) -> Option<Date> {
    parse_textual_date_or_warn(lang, utc_offset, textual_date, warnings)
        .map(|d| d.to_offset(utc_offset).date())
}

pub fn parse_textual_date_or_warn(
    lang: Language,
    utc_offset: UtcOffset,
    textual_date: &str,
    warnings: &mut Vec<String>,
) -> Option<OffsetDateTime> {
    let res = parse_textual_date_to_dt(lang, utc_offset, textual_date);
    if res.is_none() {
        warnings.push(format!("could not parse textual date `{textual_date}`"));
    }
    res
}

/// Parse a textual video duration (e.g. "11 minutes, 20 seconds")
///
/// Returns None if the duration could not be parsed
pub fn parse_video_duration(lang: Language, video_duration: &str) -> Option<u32> {
    let entry = dictionary::entry(lang);
    let by_char = util::lang_by_char(lang);

    let parts = split_duration_txt(video_duration, matches!(lang, Language::Si | Language::Sw));
    let mut secs = 0;

    if parts.is_empty() {
        return None;
    }

    for part in parts {
        let mut n = if part.digits.is_empty() {
            1
        } else {
            part.digits.parse::<u32>().ok()?
        };
        let mut tokens = TaTokenParser::new(&entry, by_char, false, &part.word).peekable();
        tokens.peek()?;

        tokens.for_each(|ta| {
            secs += n * ta.secs();
            n = 1;
        });
    }

    Some(secs)
}

pub fn parse_video_duration_or_warn(
    lang: Language,
    video_duration: &str,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    let res = parse_video_duration(lang, video_duration);
    if res.is_none() {
        warnings.push(format!("could not parse video duration `{video_duration}`"));
    }
    res
}

#[derive(Default)]
struct DurationTxtSegment {
    digits: String,
    word: String,
}

/// Split a video duration string into its segments.
///
/// Each segment consists of a word and a string of digits (one of them may be empty).
///
/// The `start_word` parameter determines whether the segments should start with a word
/// instead of a number. This is the case in Swahili and Singhalese.
///
/// Example (start_word=false):
/// - `1 minute, 13 seconds` -> `{1;minute} {13;seconds}`
/// - `foo 1 minute, 13 seconds bar` -> `{foo} {1;minute} {13;seconds bar}`
///
/// Example (start_word=true):
/// - `dakika 1 na sekunde 1` -> `{1;dakika} {1;na sekunde}`
/// - `foo dakika 1 na sekunde 1 bar` -> `{1;foo dakika} {1;na sekunde} {bar}`
fn split_duration_txt(txt: &str, start_word: bool) -> Vec<DurationTxtSegment> {
    let mut segments = Vec::new();

    // 1: parse digits, 2: parse word
    let mut state: u8 = 0;
    let mut seg = DurationTxtSegment::default();

    for c in txt.trim().chars() {
        if c.is_ascii_digit() {
            if state == 2 && (!seg.digits.is_empty() || (!start_word && segments.is_empty())) {
                segments.push(seg);
                seg = DurationTxtSegment::default();
            }
            seg.digits.push(c);
            state = 1;
        } else {
            if (state == 1) && (!seg.word.is_empty() || (start_word && segments.is_empty())) {
                segments.push(seg);
                seg = DurationTxtSegment::default();
            }
            if !matches!(c, '.' | ',') {
                c.to_lowercase().for_each(|c| seg.word.push(c));
            }
            state = 2;
        }
    }
    if !seg.word.is_empty() || !seg.digits.is_empty() {
        segments.push(seg);
    }

    segments
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader, str::FromStr};

    use path_macro::path;
    use rstest::rstest;
    use time::macros::{date, datetime};

    use super::*;
    use crate::util::tests::TESTFILES;

    #[rstest]
    #[case::de(Language::De, "vor 1 Sekunde", Some(TimeAgo { n: 1, unit: TimeUnit::Second }))]
    #[case::ar(Language::Ar, "قبل ساعة واحدة", Some(TimeAgo { n: 1, unit: TimeUnit::Hour }))]
    // No-break space
    #[case::nbsp(Language::De, "Vor 3\u{a0}Tagen aktualisiert", Some(TimeAgo { n: 3, unit: TimeUnit::Day }))]
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

        for (lang, strings) in &strings_map {
            assert_eq!(strings.len(), expect.len());
            strings.iter().enumerate().for_each(|(n, s)| {
                assert_eq!(
                    parse_timeago(*lang, s),
                    Some(expect[n]),
                    "Language: {lang}, txt: `{s}`"
                );
            });
        }
    }

    #[test]
    fn t_testfile_short() {
        let json_path = path!(*TESTFILES / "dict" / "timeago_samples_short.json");

        let expect = [
            TimeAgo {
                n: 35,
                unit: TimeUnit::Minute,
            },
            TimeAgo {
                n: 50,
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
                n: 3,
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
                n: 12,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 17,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 18,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 19,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 20,
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
                n: 13,
                unit: TimeUnit::Hour,
            },
            TimeAgo {
                n: 1,
                unit: TimeUnit::Day,
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
                n: 4,
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
                n: 11,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 12,
                unit: TimeUnit::Day,
            },
            TimeAgo {
                n: 13,
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
                n: 1,
                unit: TimeUnit::Month,
            },
            TimeAgo {
                n: 4,
                unit: TimeUnit::Week,
            },
            TimeAgo {
                n: 7,
                unit: TimeUnit::Month,
            },
            TimeAgo {
                n: 10,
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
            TimeAgo {
                n: 5,
                unit: TimeUnit::Year,
            },
        ];

        let json_file = File::open(json_path).unwrap();
        let strings_map: BTreeMap<Language, Vec<String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, strings) in &strings_map {
            assert_eq!(strings.len(), expect.len(), "Language: {lang}");
            strings.iter().enumerate().for_each(|(n, s)| {
                let mut exp = expect[n];
                if *lang == Language::Mn && exp.unit == TimeUnit::Week {
                    exp.unit = TimeUnit::Day;
                    exp.n *= 7;
                }

                assert_eq!(
                    parse_timeago(*lang, s),
                    Some(exp),
                    "Language: {lang}, txt: `{s}`"
                );
            });
        }
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
            for (t, entry) in entries {
                entry.cases.iter().for_each(|(txt, n)| {
                    let timeago = parse_timeago(*lang, txt);
                    let textual_date = parse_textual_date(*lang, UtcOffset::UTC, txt);
                    assert_eq!(
                        timeago,
                        Some(TimeAgo { n: *n, unit: *t }),
                        "lang: {lang}, txt: {txt}"
                    );
                    assert_eq!(
                        textual_date,
                        Some(ParsedDate::Relative(TimeAgo { n: *n, unit: *t })),
                        "textual_date lang: {lang}, txt: {txt}"
                    );

                    n_cases += 1;
                });
            }
        });

        assert_eq!(n_cases, 1065);
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
        "যোগ দিয়েছেন 24 সেপ, 2013",
        Some(ParsedDate::Absolute(date!(2013-9-24)))
    )]
    #[case(Language::Ja, "2023年7月", Some(ParsedDate::Absolute(date!(2023-07-01))))]
    #[case(Language::De, "Juli 2023", Some(ParsedDate::Absolute(date!(2023-07-01))))]
    fn t_parse_date(
        #[case] lang: Language,
        #[case] textual_date: &str,
        #[case] expect: Option<ParsedDate>,
    ) {
        let parsed_date = parse_textual_date(lang, UtcOffset::UTC, textual_date);
        assert_eq!(parsed_date, expect);
    }

    #[rstest]
    #[case(Language::En, "Jan 5", date!(0000-01-05))]
    fn t_parse_date_this_year(
        #[case] lang: Language,
        #[case] textual_date: &str,
        #[case] expect: Date,
    ) {
        let parsed_date = parse_textual_date(lang, UtcOffset::UTC, textual_date);
        let expected_date = expect
            .replace_year(OffsetDateTime::now_utc().year())
            .unwrap();
        assert_eq!(parsed_date, Some(ParsedDate::Absolute(expected_date)));
    }

    #[test]
    fn t_parse_date_samples() {
        let json_path = path!(*TESTFILES / "dict" / "playlist_samples.json");
        let json_file = File::open(json_path).unwrap();
        let date_samples: BTreeMap<Language, BTreeMap<String, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, samples) in &date_samples {
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Today").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 0,
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Yesterday").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 1,
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Ago").unwrap()),
                Some(ParsedDate::Relative(TimeAgo {
                    n: 5,
                    unit: TimeUnit::Day
                })),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Jan").unwrap()),
                Some(ParsedDate::Absolute(date!(2020 - 1 - 3))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Feb").unwrap()),
                Some(ParsedDate::Absolute(date!(2016 - 2 - 7))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Mar").unwrap()),
                Some(ParsedDate::Absolute(date!(2015 - 3 - 9))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Apr").unwrap()),
                Some(ParsedDate::Absolute(date!(2017 - 4 - 2))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("May").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 5 - 22))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Jun").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 6 - 28))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Jul").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 7 - 2))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Aug").unwrap()),
                Some(ParsedDate::Absolute(date!(2015 - 8 - 23))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Sep").unwrap()),
                Some(ParsedDate::Absolute(date!(2018 - 9 - 16))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Oct").unwrap()),
                Some(ParsedDate::Absolute(date!(2014 - 10 - 31))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Nov").unwrap()),
                Some(ParsedDate::Absolute(date!(2016 - 11 - 3))),
                "lang: {lang}"
            );
            assert_eq!(
                parse_textual_date(*lang, UtcOffset::UTC, samples.get("Dec").unwrap()),
                Some(ParsedDate::Absolute(date!(2021 - 12 - 24))),
                "lang: {lang}"
            );
        }
    }

    #[test]
    fn t_parse_history_date_samples() {
        let json_path = path!(*TESTFILES / "dict" / "history_date_samples.json");
        let json_file = File::open(json_path).unwrap();
        let date_samples: BTreeMap<Language, BTreeMap<String, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, samples) in date_samples {
            for (k, v) in samples {
                let expected = match k.as_str() {
                    "this_week" => ParsedDate::Relative(TimeAgo {
                        n: 0,
                        unit: TimeUnit::LastWeek,
                    }),
                    "last_week" => ParsedDate::Relative(TimeAgo {
                        n: 1,
                        unit: TimeUnit::LastWeek,
                    }),
                    _ => {
                        if let Ok(wd) = time::Weekday::from_str(&k) {
                            ParsedDate::Relative(TimeAgo {
                                n: wd.number_days_from_monday(),
                                unit: TimeUnit::LastWeekday,
                            })
                        } else {
                            let mut date_nums = k.split('-');
                            let mut y = date_nums.next().unwrap().parse::<i32>().unwrap();
                            if y == 0 {
                                y = OffsetDateTime::now_utc().date().year();
                            }
                            let m = date_nums.next().unwrap().parse::<u8>().unwrap();
                            let d = date_nums.next().unwrap().parse::<u8>().unwrap();
                            ParsedDate::Absolute(
                                Date::from_calendar_date(y, m.try_into().unwrap(), d).unwrap(),
                            )
                        }
                    }
                };
                assert_eq!(
                    parse_textual_date(lang, UtcOffset::UTC, &v),
                    Some(expected),
                    "lang={lang}; {k}"
                );
            }
        }
    }

    #[test]
    fn t_parse_video_duration() {
        let json_path = path!(*TESTFILES / "dict" / "video_duration_samples.json");
        let json_file = File::open(json_path).unwrap();
        let date_samples: BTreeMap<Language, BTreeMap<String, u32>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, samples) in &date_samples {
            for (txt, duration) in samples {
                assert_eq!(
                    parse_video_duration(*lang, txt),
                    Some(*duration),
                    "lang: {lang}; txt: `{txt}`"
                );
            }
        }
    }

    #[rstest]
    #[case(Language::Ar, "19 دقيقة وثانيتان", 1142)]
    #[case(Language::Ar, "دقيقة و13 ثانية", 73)]
    #[case(Language::Sw, "dakika 1 na sekunde 13", 73)]
    #[case(Language::Ar, "1 س و41 د", 6060)]
    #[case(Language::Ar, "4 د و33 ث", 273)]
    fn t_parse_video_duration2(
        #[case] lang: Language,
        #[case] video_duration: &str,
        #[case] expect: u32,
    ) {
        assert_eq!(parse_video_duration(lang, video_duration), Some(expect));
    }

    #[test]
    fn t_to_datetime() {
        // Absolute date
        let date =
            parse_textual_date_to_dt(Language::En, UtcOffset::UTC, "Last updated on Jan 3, 2020")
                .unwrap();
        assert_eq!(date, datetime!(2020-1-3 0:00 +0));

        // Relative date
        let date = parse_textual_date_to_dt(Language::En, UtcOffset::UTC, "1 year ago").unwrap();
        let now = OffsetDateTime::now_utc();
        assert_eq!(date.year(), now.year() - 1);
    }
}
