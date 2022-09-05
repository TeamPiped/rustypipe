#![cfg(test)]

use std::{collections::BTreeMap, fs::File, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    client::RustyTube,
    model::{locale::LANGUAGES, Country, Language},
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

#[test_log::test(tokio::test)]
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
        (DateCase::Jan, "PL1J-6JOckZtHxTA3hN5SK7gBQaFfKzeXr"),
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
