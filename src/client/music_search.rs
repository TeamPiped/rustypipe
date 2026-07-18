use std::fmt::Debug;

use crate::{
    client::response::music_item::{MusicCardShelf, MusicListMapper, MusicResponseItem},
    error::{Error, ExtractionError},
    json::{yt_continuation, ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        AlbumItem, ArtistItem, MusicItem, MusicPlaylistItem, MusicSearchResult,
        MusicSearchSuggestion, TrackItem, UserItem,
    },
    param::search_filter::MusicSearchFilter,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{ClientType, MapJsonResponse, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct MusicSearchJson;

#[derive(Debug)]
struct MusicSearchSuggestionJson;

impl RustyPipeQuery {
    /// Search YouTube Music.
    ///
    /// This is a generic implementation which casts items to the given type or filters
    /// them out.
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

        self.execute_request::<MusicSearchJson, _, _>(
            ClientType::DesktopMusic,
            "music_search_tracks",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Search YouTube Music and return items of all types
    pub async fn music_search_main<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<MusicItem>, Error> {
        self.music_search(query, None).await
    }

    /// Search YouTube Music artists
    pub async fn music_search_artists<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<ArtistItem>, Error> {
        self.music_search(query, Some(MusicSearchFilter::Artists))
            .await
    }

    /// Search YouTube Music albums
    pub async fn music_search_albums<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<AlbumItem>, Error> {
        self.music_search(query, Some(MusicSearchFilter::Albums))
            .await
    }

    /// Search YouTube Music tracks
    pub async fn music_search_tracks<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<TrackItem>, Error> {
        self.music_search(query, Some(MusicSearchFilter::Tracks))
            .await
    }

    /// Search YouTube Music videos
    pub async fn music_search_videos<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<TrackItem>, Error> {
        self.music_search(query, Some(MusicSearchFilter::Videos))
            .await
    }

    /// Search YouTube Music playlists
    ///
    /// Playlists are filtered whether they are created by users
    /// (`community=true`) or by YouTube Music (`community=false`)
    pub async fn music_search_playlists<S: AsRef<str> + Debug>(
        &self,
        query: S,
        community: bool,
    ) -> Result<MusicSearchResult<MusicPlaylistItem>, Error> {
        self.music_search(
            query,
            Some(if community {
                MusicSearchFilter::CommunityPlaylists
            } else {
                MusicSearchFilter::YtmPlaylists
            }),
        )
        .await
    }

    /// Search YouTube Music users
    pub async fn music_search_users<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchResult<UserItem>, Error> {
        self.music_search(query, Some(MusicSearchFilter::Users))
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

        self.execute_request::<MusicSearchSuggestionJson, _, _>(
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
    root.first_of(&[
        ytq!(
            .contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content
                .sectionListRenderer.contents
        ),
        ytq!(
            .contents.tabbedSearchResultsRenderer.contents[0].tabRenderer.content
                .sectionListRenderer.contents
        ),
    ])
    .ok_or_else(|| ExtractionError::InvalidData("missing music search sections".into()))
}

impl<T: FromYtItem> MapJsonResponse<MusicSearchResult<T>> for MusicSearchJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchResult<T>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_music_search_sections(&root)?;
            let mut corrected_query = None;
            let mut ctoken = None;
            let mut mapper = MusicListMapper::new(ctx.lang);

            for section in sections.items() {
                if let Some(shelf) = section.query(ytq!(.musicShelfRenderer)) {
                    if let Some(contents) = shelf.query(ytq!(.contents)) {
                        mapper.map_response_node(&contents);
                    }
                    if ctoken.is_none() {
                        ctoken = shelf
                            .query(ytq!(.continuations[0]))
                            .and_then(|cont| yt_continuation(&cont));
                    }
                } else if let Some(card) = section.query(ytq!(.musicCardShelfRenderer)) {
                    if let Ok(card) = card.deserialize::<MusicCardShelf>() {
                        mapper.map_card(card);
                    }
                } else if let Some(corrected) = section.query(ytq!(
                    .itemSectionRenderer.contents[0].showingResultsForRenderer.correctedQuery
                )) {
                    corrected_query = corrected.text();
                }
            }

            let ctoken = ctoken.or(mapper.ctoken.clone());
            let map_res = mapper.conv_items();

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

impl MapJsonResponse<MusicSearchSuggestion> for MusicSearchSuggestionJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchSuggestion>, ExtractionError> {
        json.with_root(|root| {
            let mut mapper = MusicListMapper::new_search_suggest(ctx.lang);
            let mut terms = Vec::new();

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
                        } else if let Ok(response_item) = item.deserialize::<MusicResponseItem>() {
                            mapper.add_response_item(response_item);
                        }
                    }
                }
            }

            let map_res = mapper.conv_items();

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
            MusicSearchJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicSearchJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicSearchJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicSearchJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicSearchJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicSearchSuggestionJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_suggestion_{name}"), map_res.c);
    }
}
