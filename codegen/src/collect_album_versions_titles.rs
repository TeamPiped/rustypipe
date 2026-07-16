use std::{collections::BTreeMap, fs::File, io::BufReader};

use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe},
    param::{Language, LANGUAGES},
};
use serde::Deserialize;
use serde_with::rust::deserialize_ignore_any;

use crate::{
    model::{QBrowse, TextRuns},
    util::{self, DICT_DIR},
};

pub async fn collect_album_versions_titles() {
    let json_path = path!(*DICT_DIR / "other_versions_titles.json");
    let mut res = BTreeMap::new();

    let rp = RustyPipe::new();

    for lang in LANGUAGES {
        let query = QBrowse {
            browse_id: "MPREb_nlBWQROfvjo",
            params: None,
        };
        let raw_resp = rp
            .query()
            .lang(lang)
            .raw(ClientType::DesktopMusic, "browse", &query)
            .await
            .unwrap();
        let data = flexon::from_str::<AlbumData>(&raw_resp).unwrap();
        let title = data
            .contents
            .two_column_browse_results_renderer
            .secondary_contents
            .contents
            .into_iter()
            .find_map(|x| match x {
                ItemSection::MusicCarouselShelfRenderer(music_carousel_shelf) => {
                    Some(music_carousel_shelf)
                }
                ItemSection::None => None,
            })
            .expect("other versions")
            .header
            .expect("header")
            .music_carousel_shelf_basic_header_renderer
            .title
            .runs
            .into_iter()
            .next()
            .unwrap()
            .text;
        println!("{lang}: {title}");
        res.insert(lang, title);
    }

    let file = File::create(json_path).unwrap();
    flexon::to_writer_pretty(file, &res).unwrap();
}

pub fn write_samples_to_dict() {
    let json_path = path!(*DICT_DIR / "other_versions_titles.json");
    let json_file = File::open(json_path).unwrap();
    let collected: BTreeMap<Language, String> =
        flexon::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict();
    let langs = dict.keys().copied().collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let e = collected.get(&lang).unwrap();
        assert_eq!(e, e.trim());
        dict_entry.album_versions_title = e.to_owned();

        for lang in &dict_entry.equivalent {
            let ee = collected.get(lang).unwrap();
            if ee != e {
                panic!("equivalent lang conflict, lang: {lang}");
            }
        }
    }

    util::write_dict(dict);
}

#[derive(Debug, Deserialize)]
struct AlbumData {
    contents: AlbumDataContents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumDataContents {
    two_column_browse_results_renderer: X1,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct X1 {
    secondary_contents: AlbumVersionSections,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumVersionSections {
    contents: Vec<ItemSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ItemSection {
    MusicCarouselShelfRenderer(MusicCarouselShelf),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
struct MusicCarouselShelf {
    header: Option<MusicCarouselShelfHeader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MusicCarouselShelfHeader {
    music_carousel_shelf_basic_header_renderer: MusicCarouselShelfHeaderRenderer,
}

#[derive(Debug, Deserialize)]
struct MusicCarouselShelfHeaderRenderer {
    title: TextRuns,
}
