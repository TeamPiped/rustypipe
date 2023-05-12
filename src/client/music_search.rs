use std::borrow::Cow;

use serde::Serialize;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    model::{
        paginator::Paginator, traits::FromYtItem, AlbumItem, ArtistItem, MusicPlaylistItem,
        MusicSearchFiltered, MusicSearchResult, MusicSearchSuggestion, TrackItem,
    },
    serializer::MapResult,
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
#[serde(rename_all = "camelCase")]
struct QSearchSuggestion<'a> {
    context: YTContext<'a>,
    input: &'a str,
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
    #[serde(rename = "EgWKAQIoAWoMEAMQBBAJEA4QChAF")]
    Playlists,
    #[serde(rename = "EgeKAQQoADgBagwQAxAEEAkQDhAKEAU%3D")]
    YtmPlaylists,
    #[serde(rename = "EgeKAQQoAEABagwQAxAEEAkQDhAKEAU%3D")]
    CommunityPlaylists,
}

impl RustyPipeQuery {
    /// Search YouTube Music. Returns items from any type.
    pub async fn music_search<S: AsRef<str>>(&self, query: S) -> Result<MusicSearchResult, Error> {
        let query = query.as_ref();
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

    /// Search YouTube Music tracks
    pub async fn music_search_tracks<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchFiltered<TrackItem>, Error> {
        self._music_search_tracks(query, Params::Tracks).await
    }

    /// Search YouTube Music videos
    pub async fn music_search_videos<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchFiltered<TrackItem>, Error> {
        self._music_search_tracks(query, Params::Videos).await
    }

    async fn _music_search_tracks<S: AsRef<str>>(
        &self,
        query: S,
        params: Params,
    ) -> Result<MusicSearchFiltered<TrackItem>, Error> {
        let query = query.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(params),
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

    /// Search YouTube Music albums
    pub async fn music_search_albums<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchFiltered<AlbumItem>, Error> {
        let query = query.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(Params::Albums),
        };

        self.execute_request::<response::MusicSearch, _, _>(
            ClientType::DesktopMusic,
            "music_search_albums",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Search YouTube Music artists
    pub async fn music_search_artists(
        &self,
        query: &str,
    ) -> Result<MusicSearchFiltered<ArtistItem>, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(Params::Artists),
        };

        self.execute_request::<response::MusicSearch, _, _>(
            ClientType::DesktopMusic,
            "music_search_albums",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Search YouTube Music playlists
    pub async fn music_search_playlists<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchFiltered<MusicPlaylistItem>, Error> {
        self._music_search_playlists(query, Params::Playlists).await
    }

    /// Search YouTube Music playlists that were created by users
    /// (`community=true`) or by YouTube Music (`community=false`)
    pub async fn music_search_playlists_filter<S: AsRef<str>>(
        &self,
        query: S,
        community: bool,
    ) -> Result<MusicSearchFiltered<MusicPlaylistItem>, Error> {
        self._music_search_playlists(
            query,
            match community {
                true => Params::CommunityPlaylists,
                false => Params::YtmPlaylists,
            },
        )
        .await
    }

    async fn _music_search_playlists<S: AsRef<str>>(
        &self,
        query: S,
        params: Params,
    ) -> Result<MusicSearchFiltered<MusicPlaylistItem>, Error> {
        let query = query.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(params),
        };

        self.execute_request::<response::MusicSearch, _, _>(
            ClientType::DesktopMusic,
            "music_search_playlists",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Get YouTube Music search suggestions
    pub async fn music_search_suggestion<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<MusicSearchSuggestion, Error> {
        let query = query.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QSearchSuggestion {
            context,
            input: query,
        };

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

impl MapResponse<MusicSearchResult> for response::MusicSearch {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<MusicSearchResult>, crate::error::ExtractionError> {
        // dbg!(&self);

        let sections = self
            .contents
            .tabbed_search_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no tab")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut corrected_query = None;
        let mut order = Vec::new();
        let mut mapper = MusicListMapper::new(lang);

        sections.into_iter().for_each(|section| match section {
            response::music_search::ItemSection::MusicShelfRenderer(shelf) => {
                if let Some(etype) = mapper.map_response(shelf.contents) {
                    if !order.contains(&etype) {
                        order.push(etype);
                    }
                }
            }
            response::music_search::ItemSection::MusicCardShelfRenderer(card) => {
                if let Some(etype) = mapper.map_card(card) {
                    if !order.contains(&etype) {
                        order.push(etype);
                    }
                }
            }
            response::music_search::ItemSection::ItemSectionRenderer { contents } => {
                if let Some(corrected) = contents.into_iter().next() {
                    corrected_query = Some(corrected.showing_results_for_renderer.corrected_query)
                }
            }
            response::music_search::ItemSection::None => {}
        });

        mapper.check_unknown()?;
        let map_res = mapper.group_items();

        Ok(MapResult {
            c: MusicSearchResult {
                tracks: map_res.c.tracks,
                albums: map_res.c.albums,
                artists: map_res.c.artists,
                playlists: map_res.c.playlists,
                corrected_query,
                order,
            },
            warnings: map_res.warnings,
        })
    }
}

impl<T: FromYtItem> MapResponse<MusicSearchFiltered<T>> for response::MusicSearch {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<MusicSearchFiltered<T>>, ExtractionError> {
        // dbg!(&self);

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
        let mut mapper = MusicListMapper::new(lang);

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
                    corrected_query = Some(corrected.showing_results_for_renderer.corrected_query)
                }
            }
            response::music_search::ItemSection::None => {}
        });

        mapper.check_unknown()?;
        let map_res = mapper.conv_items();

        Ok(MapResult {
            c: MusicSearchFiltered {
                items: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    None,
                    crate::model::paginator::ContinuationEndpoint::MusicSearch,
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
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<MusicSearchSuggestion>, ExtractionError> {
        let mut mapper = MusicListMapper::new(lang);
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

        mapper.check_unknown()?;
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
        client::{response, MapResponse},
        model::{
            AlbumItem, ArtistItem, MusicPlaylistItem, MusicSearchFiltered, MusicSearchResult,
            MusicSearchSuggestion, TrackItem,
        },
        param::Language,
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::default("default")]
    #[case::typo("typo")]
    #[case::radio("radio")]
    #[case::artist("artist")]
    fn map_music_search_main(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_search" / format!("main_{name}.json"));
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
        let map_res: MapResult<MusicSearchFiltered<TrackItem>> =
            search.map_response("", Language::En, None).unwrap();

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
        let map_res: MapResult<MusicSearchFiltered<AlbumItem>> =
            search.map_response("", Language::En, None).unwrap();

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
        let map_res: MapResult<MusicSearchFiltered<ArtistItem>> =
            search.map_response("", Language::En, None).unwrap();

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
        let map_res: MapResult<MusicSearchFiltered<MusicPlaylistItem>> =
            search.map_response("", Language::En, None).unwrap();

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
            suggestion.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_music_search_suggestion_{name}"), map_res.c);
    }
}
