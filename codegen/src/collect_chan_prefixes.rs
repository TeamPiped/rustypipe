use std::{collections::BTreeMap, fs::File, io::BufReader};

use path_macro::path;
use rustypipe::{
    client::RustyPipe,
    param::{Language, LANGUAGES},
};
use serde::{Deserialize, Serialize};

use crate::util::{self, DICT_DIR};

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    prefix: String,
    suffix: String,
}

pub async fn collect_chan_prefixes() {
    let cname = "kiernanchrisman";
    let json_path = path!(*DICT_DIR / "chan_prefixes.json");
    let mut res = BTreeMap::new();

    let rp = RustyPipe::new();

    for lang in LANGUAGES {
        let playlist = rp
            .query()
            .lang(lang)
            .playlist("PLZN_exA7d4RVmCQrG5VlWIjMOkMFZVVOc")
            .await
            .unwrap();
        let n = playlist.channel.unwrap().name;
        let offset = n.find(cname).unwrap();
        let prefix = &n[..offset];
        let suffix = &n[(offset + cname.len())..];

        res.insert(
            lang,
            Entry {
                prefix: prefix.to_owned(),
                suffix: suffix.to_owned(),
            },
        );
    }

    let file = File::create(json_path).unwrap();
    flexon::to_writer_pretty(file, &res).unwrap();
}

pub fn write_samples_to_dict() {
    let json_path = path!(*DICT_DIR / "chan_prefixes.json");

    let json_file = File::open(json_path).unwrap();
    let collected: BTreeMap<Language, Entry> =
        flexon::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict();
    let langs = dict.keys().copied().collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let e = collected.get(&lang).unwrap();
        dict_entry.chan_prefix = e.prefix.trim().to_owned();
        dict_entry.chan_suffix = e.suffix.trim().to_owned();

        for lang in &dict_entry.equivalent {
            let ee = collected.get(lang).unwrap();
            if ee.prefix != e.prefix || ee.suffix != e.suffix {
                panic!("equivalent lang conflict, lang: {lang}");
            }
        }
    }

    util::write_dict(dict);
}
