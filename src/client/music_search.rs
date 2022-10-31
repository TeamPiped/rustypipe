use std::borrow::Cow;

use serde::Serialize;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    model::MusicSearchResult,
    serializer::MapResult,
    util::TryRemove,
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearch<'a> {
    context: YTContext<'a>,
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Params>,
}

#[derive(Debug, Serialize)]
enum Params {
    #[serde(rename = "EgWKAQIIAWoMEAMQBBAJEA4QChAF")]
    Tracks,
    #[serde(rename = "EgWKAQIQAWoMEAMQBBAJEA4QChAF")]
    Videos,
    #[serde(rename = "EgWKAQIYAWoMEAMQBBAJEA4QChAF")]
    Albums,
    #[serde(rename = "EgWKAQIgAWoMEAMQBBAJEA4QChAF")]
    Artists,
    #[serde(rename = "EgeKAQQoADgBagwQAxAEEAkQDhAKEAU%3D")]
    FeaturedPlaylists,
    #[serde(rename = "EgeKAQQoAEABagwQAxAEEAkQDhAKEAU%3D")]
    CommunityPlaylists,
}

impl RustyPipeQuery {
    pub async fn music_search(&self, query: &str) -> Result<MusicSearchResult, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearch {
            context,
            query,
            params: None,
        };

        self.execute_request::<response::MusicSearch, _, _>(
            ClientType::DesktopMusic,
            "music_search",
            query,
            "search",
            &request_body,
        )
        .await
    }
}

impl MapResponse<MusicSearchResult> for response::MusicSearch {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicSearchResult>, crate::error::ExtractionError> {
        // dbg!(&self);

        let mut tabs = self.contents.tabbed_search_results_renderer.contents;
        let sections = tabs
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no tab")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut corrected_query = None;
        // let mut ctoken = None;
        let mut mapper = MusicListMapper::new(lang);

        sections.into_iter().for_each(|section| match section {
            response::music_search::ItemSection::MusicShelfRenderer(shelf) => {
                mapper.map_response(shelf.contents);
                // if let Some(cont) = shelf.continuations.try_swap_remove(0) {
                //     ctoken = Some(cont.next_continuation_data.continuation);
                // }
            }
            response::music_search::ItemSection::ItemSectionRenderer { mut contents } => {
                if let Some(corrected) = contents.try_swap_remove(0) {
                    corrected_query = Some(corrected.showing_results_for_renderer.corrected_query)
                }
            }
            response::music_search::ItemSection::None => {}
        });

        Ok(MapResult {
            c: MusicSearchResult {
                tracks: mapper.tracks,
                albums: mapper.albums,
                artists: mapper.artists,
                playlists: mapper.playlists,
                corrected_query,
            },
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use crate::{
        client::{response, MapResponse},
        model::MusicSearchResult,
        param::Language,
        serializer::MapResult,
    };

    use rstest::rstest;

    #[rstest]
    #[case::default("default")]
    #[case::typo("typo")]
    fn map_music_search(#[case] name: &str) {
        let filename = format!("testfiles/music_search/{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult> =
            search.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_{}", name), map_res.c);
    }
}
