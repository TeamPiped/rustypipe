use std::{borrow::Cow, fmt::Debug};

use serde::Serialize;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        AlbumItem, ArtistItem, MusicItem, MusicPlaylistItem, MusicSearchResult,
        MusicSearchSuggestion, TrackItem, UserItem,
    },
    param::search_filter::MusicSearchFilter,
    serializer::MapResult,
};

use super::{response, ClientType, MapRespCtx, MapResponse, RustyPipeQuery};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearch<'a> {
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearchSuggestion<'a> {
    input: &'a str,
}

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
        let request_body = QSearch {
            query,
            params: filter.map(MusicSearchFilter::params),
        };

        self.execute_request::<response::MusicSearch, _, _>(
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
        let request_body = QSearchSuggestion { input: query };

        self.execute_request::<response::MusicSearchSuggestion, _, _>(
            ClientType::DesktopMusic,
            "music_search_suggestion",
            query,
            "music/get_search_suggestions",
            &request_body,
        )
        .await
    }
}

impl<T: FromYtItem> MapResponse<MusicSearchResult<T>> for response::MusicSearch {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchResult<T>>, ExtractionError> {
        let tabs = self.contents.tabbed_search_results_renderer.contents;
        let sections = tabs
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no tab")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut corrected_query = None;
        let mut ctoken = None;
        let mut mapper = MusicListMapper::new(ctx.lang);

        sections.into_iter().for_each(|section| match section {
            response::music_search::ItemSection::MusicShelfRenderer(shelf) => {
                mapper.map_response(shelf.contents);
                if let Some(cont) = shelf.continuations.into_iter().next() {
                    ctoken = Some(cont.next_continuation_data.continuation);
                }
            }
            response::music_search::ItemSection::MusicCardShelfRenderer(card) => {
                mapper.map_card(card);
            }
            response::music_search::ItemSection::ItemSectionRenderer { contents } => {
                if let Some(corrected) = contents.into_iter().next() {
                    corrected_query = Some(corrected.showing_results_for_renderer.corrected_query);
                }
            }
            response::music_search::ItemSection::None => {}
        });

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
    }
}

impl MapResponse<MusicSearchSuggestion> for response::MusicSearchSuggestion {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicSearchSuggestion>, ExtractionError> {
        let mut mapper = MusicListMapper::new_search_suggest(ctx.lang);
        let mut terms = Vec::new();

        for section in self.contents {
            for item in section.search_suggestions_section_renderer.contents {
                match item {
                    response::music_search::SearchSuggestionItem::SearchSuggestionRenderer {
                        suggestion,
                    } => {
                        terms.push(suggestion);
                    },
                    response::music_search::SearchSuggestionItem::MusicResponsiveListItemRenderer(item) => {
                        mapper.add_response_item(response::music_item::MusicResponseItem::MusicResponsiveListItemRenderer(*item));
                    }
                    response::music_search::SearchSuggestionItem::None => {},
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
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapRespCtx, MapResponse},
        model::{
            AlbumItem, ArtistItem, MusicItem, MusicPlaylistItem, MusicSearchResult,
            MusicSearchSuggestion, TrackItem,
        },
        serializer::MapResult,
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
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult<MusicItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult<TrackItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult<AlbumItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult<ArtistItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let search: response::MusicSearch =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchResult<MusicPlaylistItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let suggestion: response::MusicSearchSuggestion =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicSearchSuggestion> =
            suggestion.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_suggestion_{name}"), map_res.c);
    }
}
