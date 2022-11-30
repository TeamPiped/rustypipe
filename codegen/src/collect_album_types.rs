use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use futures::stream::{self, StreamExt};
use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe, RustyPipeQuery, YTContext},
    model::AlbumType,
    param::{locale::LANGUAGES, Language},
};
use serde::{Deserialize, Serialize};

use crate::util::{self, TextRuns};

pub async fn collect_album_types(project_root: &Path, concurrency: usize) {
    let json_path = path!(project_root / "testfiles" / "dict" / "album_type_samples.json");

    let album_types = [
        (AlbumType::Album, "MPREb_nlBWQROfvjo"),
        (AlbumType::Single, "MPREb_bHfHGoy7vuv"),
        (AlbumType::Ep, "MPREb_u1I69lSAe5v"),
        (AlbumType::Audiobook, "MPREb_gaoNzsQHedo"),
        (AlbumType::Show, "MPREb_cwzk8EUwypZ"),
    ];

    let rp = RustyPipe::new();

    let collected_album_types = stream::iter(LANGUAGES)
        .map(|lang| {
            let rp = rp.clone();
            async move {
                let query = rp.query().lang(lang);
                let mut data: BTreeMap<AlbumType, String> = BTreeMap::new();

                for (album_type, id) in album_types {
                    let atype_txt = get_album_type(&query, id).await;
                    println!("collected {}-{:?} ({})", lang, album_type, &atype_txt);
                    data.insert(album_type, atype_txt);
                }

                (lang, data)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<BTreeMap<_, _>>()
        .await;

    let file = File::create(json_path).unwrap();
    serde_json::to_writer_pretty(file, &collected_album_types).unwrap();
}

pub fn write_samples_to_dict(project_root: &Path) {
    let json_path = path!(project_root / "testfiles" / "dict" / "album_type_samples.json");

    let json_file = File::open(json_path).unwrap();
    let collected: BTreeMap<Language, BTreeMap<AlbumType, String>> =
        serde_json::from_reader(BufReader::new(json_file)).unwrap();
    let mut dict = util::read_dict(project_root);
    let langs = dict.keys().map(|k| k.to_owned()).collect::<Vec<_>>();

    for lang in langs {
        let dict_entry = dict.entry(lang).or_default();

        let mut e_langs = dict_entry.equivalent.clone();
        e_langs.push(lang);

        e_langs.iter().for_each(|lang| {
            collected.get(lang).unwrap().iter().for_each(|(t, v)| {
                dict_entry
                    .album_types
                    .insert(v.to_lowercase().trim().to_owned(), *t);
            });
        });
    }

    util::write_dict(project_root, &dict);
}

#[derive(Debug, Deserialize)]
struct AlbumData {
    header: Header,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    music_detail_header_renderer: HeaderRenderer,
}

#[derive(Debug, Deserialize)]
struct HeaderRenderer {
    subtitle: TextRuns,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowse<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
}

async fn get_album_type(query: &RustyPipeQuery, id: &str) -> String {
    let context = query
        .get_context(ClientType::DesktopMusic, true, None)
        .await;
    let body = QBrowse {
        context,
        browse_id: id,
    };
    let response_txt = query
        .raw(ClientType::DesktopMusic, "browse", &body)
        .await
        .unwrap();
    let album = serde_json::from_str::<AlbumData>(&response_txt).unwrap();

    album
        .header
        .music_detail_header_renderer
        .subtitle
        .runs
        .into_iter()
        .next()
        .unwrap()
        .text
}
