use std::{collections::BTreeMap, fs::File, io::BufReader};

use futures_util::stream::{self, StreamExt};
use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe, RustyPipeQuery},
    model::AlbumType,
    param::{Language, LANGUAGES},
};
use serde::{Deserialize, Serialize};
use serde_with::rust::deserialize_ignore_any;

use crate::{
    model::{QBrowse, TextRuns},
    util::{self, DICT_DIR},
};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum AlbumTypeX {
    Album,
    Ep,
    Single,
    Audiobook,
    Show,
    AlbumRow,
    SingleRow,
}

pub async fn collect_album_types(concurrency: usize) {
    let json_path = path!(*DICT_DIR / "album_type_samples.json");

    let album_types = [
        (AlbumTypeX::Album, "MPREb_nlBWQROfvjo"),
        (AlbumTypeX::Single, "MPREb_bHfHGoy7vuv"),
        (AlbumTypeX::Ep, "MPREb_u1I69lSAe5v"),
        (AlbumTypeX::Audiobook, "MPREb_gaoNzsQHedo"),
        (AlbumTypeX::Show, "MPREb_cwzk8EUwypZ"),
    ];

    let rp = RustyPipe::new();

    let collected_album_types = stream::iter(LANGUAGES)
        .map(|lang| {
            let rp = rp.clone();
            async move {
                let query = rp.query().lang(lang);
                let mut data: BTreeMap<AlbumTypeX, String> = BTreeMap::new();

                for (album_type, id) in album_types {
                    let atype_txt = get_album_type(&query, id).await;
                    println!("collected {}-{:?} ({})", lang, album_type, &atype_txt);
                    data.insert(album_type, atype_txt);
                }

                let (albums_txt, singles_txt) = get_album_groups(&query).await;
                println!(
                    "collected {}-{:?} ({})",
                    lang,
                    AlbumTypeX::AlbumRow,
                    &albums_txt
                );
                println!(
                    "collected {}-{:?} ({})",
                    lang,
                    AlbumTypeX::SingleRow,
                    &singles_txt
                );
                data.insert(AlbumTypeX::AlbumRow, albums_txt);
                data.insert(AlbumTypeX::SingleRow, singles_txt);

                (lang, data)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<BTreeMap<_, _>>()
        .await;

    let file = File::create(json_path).unwrap();
    flexon::to_writer_pretty(file, &collected_album_types).unwrap();
}

pub fn write_samples_to_dict() {
    let json_path = path!(*DICT_DIR / "album_type_samples.json");

    let json_file = File::open(json_path).unwrap();
    let collected: BTreeMap<Language, BTreeMap<String, String>> =
        flexon::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict();
    let langs = dict.keys().copied().collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let mut e_langs = dict_entry.equivalent.clone();
        e_langs.push(lang);

        for lang in &e_langs {
            collected.get(lang).unwrap().iter().for_each(|(t_str, v)| {
                let t =
                    serde_plain::from_str::<AlbumType>(t_str.split('_').next().unwrap()).unwrap();
                dict_entry
                    .album_types
                    .insert(v.to_lowercase().trim().to_owned(), t);
            });
        }
    }

    util::write_dict(dict);
}

#[derive(Debug, Deserialize)]
struct AlbumData {
    contents: AlbumContents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumContents {
    two_column_browse_results_renderer: AlbumTabs,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumTabs {
    contents: Vec<AlbumTab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumTab {
    tab_renderer: AlbumTabRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumTabRenderer {
    content: AlbumSectionList,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumSectionList {
    section_list_renderer: AlbumHeaderSections,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumHeaderSections {
    contents: Vec<AlbumHeader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumHeader {
    music_responsive_header_renderer: HeaderRenderer,
}

#[derive(Debug, Deserialize)]
struct HeaderRenderer {
    subtitle: TextRuns,
}

async fn get_album_type(query: &RustyPipeQuery, id: &str) -> String {
    let body = QBrowse {
        browse_id: id,
        params: None,
    };
    let response_txt = query
        .raw(ClientType::DesktopMusic, "browse", &body)
        .await
        .unwrap();
    let album = flexon::from_str::<AlbumData>(&response_txt).unwrap();

    album
        .contents
        .two_column_browse_results_renderer
        .contents
        .into_iter()
        .next()
        .unwrap()
        .tab_renderer
        .content
        .section_list_renderer
        .contents
        .into_iter()
        .next()
        .unwrap()
        .music_responsive_header_renderer
        .subtitle
        .runs
        .into_iter()
        .next()
        .unwrap()
        .text
}

async fn get_album_groups(query: &RustyPipeQuery) -> (String, String) {
    let body = QBrowse {
        browse_id: "UCOR4_bSVIXPsGa4BbCSt60Q",
        params: None,
    };
    let response_txt = query
        .clone()
        .visitor_data("CgtwbzJZcS1XZWc1QSjM2JG8BjIKCgJERRIEEgAgCw%3D%3D")
        .raw(ClientType::DesktopMusic, "browse", &body)
        .await
        .unwrap();
    let artist = flexon::from_str::<ArtistData>(&response_txt).unwrap();

    let sections = artist
        .contents
        .single_column_browse_results_renderer
        .contents
        .into_iter()
        .next()
        .map(|c| c.tab_renderer.content.section_list_renderer.contents)
        .unwrap();
    let titles = sections
        .into_iter()
        .filter_map(|s| {
            if let ItemSection::MusicCarouselShelfRenderer(r) = s {
                r.header
            } else {
                None
            }
        })
        .map(|h| {
            h.music_carousel_shelf_basic_header_renderer
                .title
                .runs
                .into_iter()
                .next()
                .unwrap()
                .text
        })
        .collect::<Vec<_>>();
    assert!(titles.len() >= 2, "too few sections");

    let mut titles_it = titles.into_iter();
    (titles_it.next().unwrap(), titles_it.next().unwrap())
}

#[derive(Debug, Deserialize)]
struct ArtistData {
    contents: ArtistDataContents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistDataContents {
    single_column_browse_results_renderer: ArtistTabs,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistTabs {
    contents: Vec<ArtistTab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistTab {
    tab_renderer: ArtistTabRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistTabRenderer {
    content: ArtistSectionList,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistSectionList {
    section_list_renderer: ArtistSections,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistSections {
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
