use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::BufReader,
};

use anyhow::Result;
use futures::{stream, StreamExt};
use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe, RustyPipeQuery},
    param::{locale::LANGUAGES, Language},
};

use crate::{
    model::{Channel, QBrowse, TimeAgo, TimeUnit},
    util::{self, DICT_DIR},
};

type CollectedDurations = BTreeMap<Language, BTreeMap<String, u32>>;

/// Collect the video duration texts in every supported language
/// and write them to `testfiles/dict/video_duration_samples.json`.
///
/// The length of YouTube short videos is only available in textual form.
/// To parse it correctly, we need to collect samples of this text in every
/// language. We collect these samples from regular channel videos because these
/// include a textual duration in addition to the easy to parse "mm:ss"
/// duration format.
pub async fn collect_video_durations(concurrency: usize) {
    let json_path = path!(*DICT_DIR / "video_duration_samples.json");
    let rp = RustyPipe::new();

    let channels = [
        "UCq-Fj5jknLsUf-MWSy4_brA",
        "UCMcS5ITpSohfr8Ppzlo4vKw",
        "UCXuqSBlHAE6Xw-yeJA0Tunw",
    ];

    let durations: CollectedDurations = stream::iter(LANGUAGES)
        .map(|lang| {
            let rp = rp.query().lang(lang);
            async move {
                let mut map = BTreeMap::new();

                for (n, ch_id) in channels.iter().enumerate() {
                    get_channel_vlengths(&rp, ch_id, &mut map).await.unwrap();
                    println!("collected {lang}-{n}");
                }

                // Since we are only parsing shorts durations, we do not need durations >= 1h
                let map = map.into_iter().filter(|(_, v)| v < &3600).collect();
                (lang, map)
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &durations).unwrap();
}

pub fn parse_video_durations() {
    let json_path = path!(*DICT_DIR / "video_duration_samples.json");
    let json_file = File::open(json_path).unwrap();
    let durations: CollectedDurations = serde_json::from_reader(BufReader::new(json_file)).unwrap();

    let mut dict = util::read_dict();
    let langs = dict.keys().map(|k| k.to_owned()).collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let mut e_langs = dict_entry.equivalent.clone();
        e_langs.push(lang);

        for lang in e_langs {
            let mut words = HashMap::new();

            fn check_add_word(
                words: &mut HashMap<String, Option<TimeAgo>>,
                by_char: bool,
                val: u32,
                expect: u32,
                w: String,
                unit: TimeUnit,
            ) -> bool {
                let ok = val == expect || val * 2 == expect;
                if ok {
                    let mut ins = |w: &str, val: &mut TimeAgo| {
                        // Filter stop words
                        if matches!(
                            w,
                            "na" | "y"
                                | "و"
                                | "ja"
                                | "et"
                                | "e"
                                | "i"
                                | "և"
                                | "og"
                                | "en"
                                | "и"
                                | "a"
                                | "és"
                                | "ir"
                                | "un"
                                | "și"
                                | "in"
                                | "และ"
                                | "\u{0456}"
                                | "鐘"
                                | "eta"
                                | "અને"
                                | "और"
                                | "കൂടാതെ"
                                | "සහ"
                        ) {
                            return;
                        }

                        let entry = words.entry(w.to_owned()).or_insert(Some(*val));
                        if let Some(e) = entry {
                            if e != val {
                                *entry = None;
                            }
                        }
                    };

                    let mut val = TimeAgo {
                        n: (expect / val).try_into().unwrap(),
                        unit,
                    };

                    if by_char {
                        w.chars().for_each(|c| {
                            if !c.is_whitespace() {
                                ins(&c.to_string(), &mut val);
                            }
                        });
                    } else {
                        w.split_whitespace().for_each(|w| ins(w, &mut val));
                    }
                }
                ok
            }

            fn parse(
                words: &mut HashMap<String, Option<TimeAgo>>,
                lang: Language,
                by_char: bool,
                txt: &str,
                d: u32,
            ) {
                let (m, s) = split_duration(d);

                let mut parts =
                    split_duration_txt(txt, matches!(lang, Language::Si | Language::Sw))
                        .into_iter();

                let p1 = parts.next().unwrap();
                let p1_n = p1.digits.parse::<u32>().unwrap_or(1);
                let p2: Option<DurationTxtSegment> = parts.next();

                match p2 {
                    Some(p2) => {
                        let p2_n = p2.digits.parse::<u32>().unwrap_or(1);

                        assert!(
                            check_add_word(words, by_char, p1_n, m, p1.word, TimeUnit::Minute),
                            "{txt}: min parse error"
                        );
                        assert!(
                            check_add_word(words, by_char, p2_n, s, p2.word, TimeUnit::Second),
                            "{txt}: sec parse error"
                        );
                    }
                    None => {
                        if s == 0 {
                            assert!(
                                check_add_word(words, by_char, p1_n, m, p1.word, TimeUnit::Minute),
                                "{txt}: min parse error"
                            );
                        } else if m == 0 {
                            assert!(
                                check_add_word(words, by_char, p1_n, s, p1.word, TimeUnit::Second),
                                "{txt}: sec parse error"
                            );
                        } else {
                            let p = txt
                                .find([',', 'و'])
                                .unwrap_or_else(|| panic!("`{txt}`: only 1 part"));
                            parse(words, lang, by_char, &txt[0..p], m);
                            parse(words, lang, by_char, &txt[p..], s);
                        }
                    }
                }

                assert!(parts.next().is_none(), "`{txt}`: more than 2 parts");
            }

            for (txt, d) in &durations[&lang] {
                parse(&mut words, lang, dict_entry.by_char, txt, *d);
            }

            // dbg!(&words);

            words.into_iter().for_each(|(k, v)| {
                if let Some(v) = v {
                    dict_entry.timeago_tokens.insert(k, v.to_string());
                }
            });
        }
    }

    util::write_dict(dict);
}

fn split_duration(d: u32) -> (u32, u32) {
    (d / 60, d % 60)
}

#[derive(Debug, Default)]
struct DurationTxtSegment {
    digits: String,
    word: String,
}

fn split_duration_txt(txt: &str, start_c: bool) -> Vec<DurationTxtSegment> {
    let mut segments = Vec::new();

    // 1: parse digits, 2: parse word
    let mut state: u8 = 0;
    let mut seg = DurationTxtSegment::default();

    for c in txt.chars() {
        if c.is_ascii_digit() {
            if state == 2 && (!seg.digits.is_empty() || (!start_c && segments.is_empty())) {
                segments.push(seg);
                seg = DurationTxtSegment::default();
            }
            seg.digits.push(c);
            state = 1;
        } else {
            if (state == 1) && (!seg.word.is_empty() || (start_c && segments.is_empty())) {
                segments.push(seg);
                seg = DurationTxtSegment::default();
            }
            if c != ',' {
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

async fn get_channel_vlengths(
    query: &RustyPipeQuery,
    channel_id: &str,
    map: &mut BTreeMap<String, u32>,
) -> Result<()> {
    let resp = query
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context: query.get_context(ClientType::Desktop, true, None).await,
                browse_id: channel_id,
                params: Some("EgZ2aWRlb3MYASAAMAE"),
            },
        )
        .await?;

    let channel = serde_json::from_str::<Channel>(&resp)?;

    let tab = channel
        .contents
        .two_column_browse_results_renderer
        .tabs
        .into_iter()
        .next()
        .unwrap()
        .tab_renderer
        .content
        .rich_grid_renderer;

    tab.contents.into_iter().for_each(|c| {
        let lt = c.rich_item_renderer.content.video_renderer.length_text;
        let duration = util::parse_video_length(&lt.simple_text).unwrap();
        map.insert(lt.accessibility.accessibility_data.label, duration);
    });

    Ok(())
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl From<intl_pluralrules::PluralCategory> for PluralCategory {
    fn from(value: intl_pluralrules::PluralCategory) -> Self {
        match value {
            intl_pluralrules::PluralCategory::ZERO => Self::Zero,
            intl_pluralrules::PluralCategory::ONE => Self::One,
            intl_pluralrules::PluralCategory::TWO => Self::Two,
            intl_pluralrules::PluralCategory::FEW => Self::Few,
            intl_pluralrules::PluralCategory::MANY => Self::Many,
            intl_pluralrules::PluralCategory::OTHER => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::BufReader;

    use intl_pluralrules::{PluralRuleType, PluralRules};
    use unic_langid::LanguageIdentifier;

    /// Verify that the duration sample set covers all pluralization variants of the languages
    #[test]
    fn check_video_duration_samples() {
        let json_path = path!(*DICT_DIR / "video_duration_samples.json");
        let json_file = File::open(json_path).unwrap();
        let durations: CollectedDurations =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let mut failed = false;

        for (lang, durations) in durations {
            let ul: LanguageIdentifier =
                lang.to_string().split('-').next().unwrap().parse().unwrap();

            let pr = PluralRules::create(ul, PluralRuleType::CARDINAL).expect(&lang.to_string());

            let mut plurals_m: HashSet<PluralCategory> = HashSet::new();
            for n in 1..60 {
                plurals_m.insert(pr.select(n).unwrap().into());
            }
            let mut plurals_s = plurals_m.clone();

            durations.values().for_each(|v| {
                let (m, s) = split_duration(*v);
                plurals_m.remove(&pr.select(m).unwrap().into());
                plurals_s.remove(&pr.select(s).unwrap().into());
            });

            if !plurals_m.is_empty() {
                println!("{lang}: missing minutes {plurals_m:?}");
                failed = true;
            }

            if !plurals_s.is_empty() {
                println!("{lang}: missing seconds {plurals_m:?}");
                failed = true;
            }
        }

        assert!(!failed);
    }
}
