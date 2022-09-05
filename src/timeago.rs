use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{dictionary, model::Language, util};

pub const LANGUAGES: [Language; 83] = [
    Language::Af,
    Language::Am,
    Language::Ar,
    Language::As,
    Language::Az,
    Language::Be,
    Language::Bg,
    Language::Bn,
    Language::Bs,
    Language::Ca,
    Language::Cs,
    Language::Da,
    Language::De,
    Language::El,
    Language::En,
    Language::EnGb,
    Language::EnIn,
    Language::Es,
    Language::Es419,
    Language::EsUs,
    Language::Et,
    Language::Eu,
    Language::Fa,
    Language::Fi,
    Language::Fil,
    Language::Fr,
    Language::FrCa,
    Language::Gl,
    Language::Gu,
    Language::Hi,
    Language::Hr,
    Language::Hu,
    Language::Hy,
    Language::Id,
    Language::Is,
    Language::It,
    Language::Iw,
    Language::Ja,
    Language::Ka,
    Language::Kk,
    Language::Km,
    Language::Kn,
    Language::Ko,
    Language::Ky,
    Language::Lo,
    Language::Lt,
    Language::Lv,
    Language::Mk,
    Language::Ml,
    Language::Mn,
    Language::Mr,
    Language::Ms,
    Language::My,
    Language::Ne,
    Language::Nl,
    Language::No,
    Language::Or,
    Language::Pa,
    Language::Pl,
    Language::Pt,
    Language::PtPt,
    Language::Ro,
    Language::Ru,
    Language::Si,
    Language::Sk,
    Language::Sl,
    Language::Sq,
    Language::Sr,
    Language::SrLatn,
    Language::Sv,
    Language::Sw,
    Language::Ta,
    Language::Te,
    Language::Th,
    Language::Tr,
    Language::Uk,
    Language::Ur,
    Language::Uz,
    Language::Vi,
    Language::ZhCn,
    Language::ZhHk,
    Language::ZhTw,
    Language::Zu,
];

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TimeAgo {
    pub n: u8,
    pub unit: TimeUnit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct TaToken {
    pub n: u8,
    pub unit: Option<TimeUnit>,
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

pub fn parse(lang: Language, textual_date: &str) -> Option<TimeAgo> {
    let mappings = dictionary::get_timeago_tokens(lang);

    let filtered_str = textual_date
        .to_lowercase()
        .chars()
        .filter(|c| c != &'\u{200b}' && !c.is_ascii_digit())
        .collect::<String>();

    let mut qu: u8 = util::parse_numeric(&textual_date).unwrap_or(1);
    filtered_str.split(' ').find_map(|word| {
        mappings
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

    use super::*;

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
}
