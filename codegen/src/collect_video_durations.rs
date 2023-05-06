use std::{collections::BTreeMap, fs::File};

use anyhow::Result;
use futures::{stream, StreamExt};
use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe, RustyPipeQuery},
    param::{locale::LANGUAGES, Language},
};

use crate::{
    model::{Channel, QBrowse},
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

    fn split_duration(d: u32) -> (u32, u32) {
        (d / 60, d % 60)
    }

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
