use std::fmt::Debug;

use crate::{
    client::response::music_item::{
        map_music_item_card, map_music_items, map_search_suggestion_item, MusicCardShelf,
    },
    error::{Error, ExtractionError},
    json::{yt_continuation, ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        MusicItem, MusicSearchResult, MusicSearchSuggestion,
    },
    param::search_filter::MusicSearchFilter,
    request_body::ytbody,
    serializer::{ItemsAccumulator, MapResult},
};

use super::{ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct MusicSearchEndpoint;

#[derive(Debug)]
struct MusicSearchSuggestionEndpoint;

impl RustyPipeQuery {
    /// Search YouTube Music.
    ///
    /// This is a generic implementation which casts items to the given type or
    /// filters them out. Pass `filter = None` to search all item types, or
    /// pass a [`MusicSearchFilter`] to restrict to a single category.
    pub async fn music_search<T: FromYtItem, S: AsRef<str>>(
        &self,
        query: S,
        filter: Option<MusicSearchFilter>,
    ) -> Result<MusicSearchResult<T>, Error> {
        let query = query.as_ref();
        let request_body = ytbody!({
            "query": query,
            ? "params": filter.map(MusicSearchFilter::params),
        });

        self.execute_request::<MusicSearchEndpoint, _, _>(
            ClientType::DesktopMusic,
            "music_search",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Get YouTube Music search suggestions
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_search_suggestion<S: AsRef<str> + Debug>(
        &self,
        query: S,
    ) -> Result<MusicSearchSuggestion, Error> {
        let query = query.as_ref();
        let request_body = ytbody!({
            "input": query,
        });

        self.execute_request::<MusicSearchSuggestionEndpoint, _, _>(
            ClientType::DesktopMusic,
            "music_search_suggestion",
            query,
            "music/get_search_suggestions",
            &request_body,
        )
        .await
    }
}

fn yt_music_search_sections<'a>(root: &'a JsonNode<'a>) -> Result<JsonNode<'a>, ExtractionError> {
    root.query(ytq!(
        .contents.tabbedSearchResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content
            .sectionListRenderer.contents
    ))
    .ok_or_else(|| ExtractionError::InvalidData("missing music search sections".into()))
}

impl<T: FromYtItem> MapEndpoint<MusicSearchResult<T>> for MusicSearchEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchResult<T>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_music_search_sections(&root)?;
            let mut corrected_query = None;
            let mut acc = ItemsAccumulator::<T>::new();

            for section in sections.items() {
                if let Some(shelf) = section.query(ytq!(.musicShelfRenderer)) {
                    if let Some(contents) = shelf.query(ytq!(.contents)) {
                        acc.add_mapped_vec(map_music_items(&contents, ctx.lang).0, None);
                    }
                    if acc.ctoken.is_none() {
                        acc.ctoken = shelf
                            .query(ytq!(.continuations[0]))
                            .and_then(|cont| yt_continuation(&cont));
                    }
                } else if let Some(card) = section.query(ytq!(.musicCardShelfRenderer)) {
                    if let Ok(card) = card.deserialize::<MusicCardShelf>() {
                        let (mapped, card_ctoken, _) = map_music_item_card(card, ctx.lang);
                        acc.add_warnings(mapped.warnings);
                        acc.items
                            .extend(mapped.c.into_iter().filter_map(T::from_ytm_item));
                        if acc.ctoken.is_none() {
                            acc.ctoken = card_ctoken;
                        }
                    }
                } else if let Some(corrected) = section.query(ytq!(
                    .itemSectionRenderer.contents[0].showingResultsForRenderer.correctedQuery
                )) {
                    corrected_query = corrected.text();
                }
            }

            let (map_res, ctoken) = acc.finish();

            Ok(MapResult {
                c: MusicSearchResult {
                    items: Paginator::new_ext(
                        None,
                        map_res.c,
                        ctoken,
                        ctx.visitor_data.map(str::to_owned),
                        ContinuationEndpoint::MusicSearch,
                        false,
                    ),
                    corrected_query,
                },
                warnings: map_res.warnings,
            })
        })
    }
}

impl MapEndpoint<MusicSearchSuggestion> for MusicSearchSuggestionEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchSuggestion>, ExtractionError> {
        json.with_root(|root| {
            let mut terms = Vec::new();
            let mut map_res: MapResult<Vec<MusicItem>> = MapResult::default();

            if let Some(sections) = root.query(ytq!(.contents)) {
                for section in sections.items() {
                    let Some(contents) =
                        section.query(ytq!(.searchSuggestionsSectionRenderer.contents))
                    else {
                        continue;
                    };

                    for item in contents.items() {
                        if let Some(suggestion) =
                            item.query(ytq!(.searchSuggestionRenderer.suggestion))
                        {
                            if let Some(term) = suggestion.text() {
                                terms.push(term);
                            }
                        } else {
                            let (mut mapped, _, _) = map_search_suggestion_item(&item, ctx.lang);
                            map_res.extend_vec(mapped);
                        }
                    }
                }
            }

            Ok(MapResult {
                c: MusicSearchSuggestion {
                    terms,
                    items: map_res.c,
                },
                warnings: map_res.warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{
        model::{
            AlbumItem, ArtistItem, MusicItem, MusicPlaylistItem, MusicSearchResult,
            MusicSearchSuggestion, TrackItem,
        },
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::default("default")]
    #[case::typo("typo")]
    #[case::radio("radio")]
    #[case::artist("artist")]
    #[case::live("live")]
    fn map_music_search_main(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_search" / format!("main_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchResult<MusicItem>> =
            MusicSearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_main_{name}"), map_res.c);
    }

    #[rstest]
    #[case::default("default")]
    #[case::typo("typo")]
    #[case::videos("videos")]
    #[case::no_artist_link("no_artist_link")]
    fn map_music_search_tracks(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_search" / format!("tracks_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchResult<TrackItem>> =
            MusicSearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_tracks_{name}"), map_res.c);
    }

    #[test]
    fn map_music_search_albums() {
        let json_path = path!(*TESTFILES / "music_search" / "albums.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchResult<AlbumItem>> =
            MusicSearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_music_search_albums", map_res.c);
    }

    #[test]
    fn map_music_search_artists() {
        let json_path = path!(*TESTFILES / "music_search" / "artists.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchResult<ArtistItem>> =
            MusicSearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_music_search_artists", map_res.c);
    }

    #[rstest]
    #[case::ytm("ytm")]
    #[case::community("community")]
    fn map_music_search_playlists(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_search" / format!("playlists_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchResult<MusicPlaylistItem>> =
            MusicSearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_playlists_{name}"), map_res.c);
    }

    #[rstest]
    #[case::default("default")]
    #[case::empty("empty")]
    fn map_music_search_suggestion(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_search" / format!("suggestion_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicSearchSuggestion> =
            MusicSearchSuggestionEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_suggestion_{name}"), map_res.c);
    }
}
