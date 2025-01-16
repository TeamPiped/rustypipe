use std::{borrow::Cow, fmt::Debug};

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ArtistId, Lyrics, MusicRelated, TrackDetails, TrackItem,
    },
    serializer::MapResult,
};

use super::{
    response::{
        self,
        music_item::{map_queue_item, MusicListMapper},
    },
    ClientType, MapRespCtx, MapResponse, QBrowse, RustyPipeQuery,
};

#[derive(Debug, Serialize)]
struct QMusicDetails<'a> {
    video_id: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

#[derive(Debug, Serialize)]
struct QRadio<'a> {
    playlist_id: &'a str,
    params: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

impl RustyPipeQuery {
    /// Get the metadata of a YouTube Music track
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_details<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
    ) -> Result<TrackDetails, Error> {
        let video_id = video_id.as_ref();
        let request_body = QMusicDetails {
            video_id,
            enable_persistent_playlist_panel: true,
            is_audio_only: true,
            tuner_setting_value: "AUTOMIX_SETTING_NORMAL",
        };

        self.execute_request::<response::MusicDetails, _, _>(
            ClientType::DesktopMusic,
            "music_details",
            video_id,
            "next",
            &request_body,
        )
        .await
    }

    /// Get the lyrics of a YouTube Music track
    ///
    /// The `lyrics_id` has to be obtained using [`RustyPipeQuery::music_details`].
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_lyrics<S: AsRef<str> + Debug>(&self, lyrics_id: S) -> Result<Lyrics, Error> {
        let lyrics_id = lyrics_id.as_ref();
        let request_body = QBrowse {
            browse_id: lyrics_id,
        };

        self.execute_request::<response::MusicLyrics, _, _>(
            ClientType::DesktopMusic,
            "music_lyrics",
            lyrics_id,
            "browse",
            &request_body,
        )
        .await
    }

    /// Get related items (tracks, playlists, artists) to a YouTube Music track
    ///
    /// The `related_id` has to be obtained using [`RustyPipeQuery::music_details`].
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_related<S: AsRef<str> + Debug>(
        &self,
        related_id: S,
    ) -> Result<MusicRelated, Error> {
        let related_id = related_id.as_ref();
        let request_body = QBrowse {
            browse_id: related_id,
        };

        self.execute_request::<response::MusicRelated, _, _>(
            ClientType::DesktopMusic,
            "music_related",
            related_id,
            "browse",
            &request_body,
        )
        .await
    }

    /// Get a YouTube Music radio (a dynamically generated playlist)
    ///
    /// The `radio_id` can be obtained using [`RustyPipeQuery::music_artist`] to get an artist's radio.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_radio<S: AsRef<str> + Debug>(
        &self,
        radio_id: S,
    ) -> Result<Paginator<TrackItem>, Error> {
        let radio_id = radio_id.as_ref();
        let request_body = QRadio {
            playlist_id: radio_id,
            params: "wAEB8gECeAE%3D",
            enable_persistent_playlist_panel: true,
            is_audio_only: true,
            tuner_setting_value: "AUTOMIX_SETTING_NORMAL",
        };

        self.execute_request::<response::MusicDetails, _, _>(
            ClientType::DesktopMusic,
            "music_radio",
            radio_id,
            "next",
            &request_body,
        )
        .await
    }

    /// Get a YouTube Music radio (a dynamically generated playlist) for a track
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_radio_track<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
    ) -> Result<Paginator<TrackItem>, Error> {
        self.music_radio(&format!("RDAMVM{}", video_id.as_ref()))
            .await
    }

    /// Get a YouTube Music radio (a dynamically generated playlist) for a playlist
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_radio_playlist<S: AsRef<str> + Debug>(
        &self,
        playlist_id: S,
    ) -> Result<Paginator<TrackItem>, Error> {
        self.music_radio(&format!("RDAMPL{}", playlist_id.as_ref()))
            .await
    }
}

impl MapResponse<TrackDetails> for response::MusicDetails {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<TrackDetails>, ExtractionError> {
        let tabs = self
            .contents
            .single_column_music_watch_next_results_renderer
            .tabbed_renderer
            .watch_next_tabbed_results_renderer
            .tabs;

        let mut content = None;
        let mut lyrics_id = None;
        let mut related_id = None;

        for t in tabs {
            match (t.tab_renderer.content, t.tab_renderer.endpoint) {
                (Some(tc), _) => {
                    content = Some(tc.music_queue_renderer.content.playlist_panel_renderer);
                }
                (_, Some(endpoint)) => {
                    match endpoint
                        .browse_endpoint
                        .browse_endpoint_context_supported_configs
                        .browse_endpoint_context_music_config
                        .page_type
                    {
                        response::music_details::TabType::Lyrics => {
                            lyrics_id = Some(endpoint.browse_endpoint.browse_id);
                        }
                        response::music_details::TabType::Related => {
                            related_id = Some(endpoint.browse_endpoint.browse_id);
                        }
                    }
                }
                (None, None) => {}
            }
        }

        let content = content.ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no content".into(),
        })?;
        let track_item = content
            .contents
            .c
            .into_iter()
            .find_map(|item| match item {
                response::music_item::PlaylistPanelVideo::PlaylistPanelVideoRenderer(track) => {
                    Some(track)
                }
                response::music_item::PlaylistPanelVideo::None => None,
            })
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no video item")))?;
        let mut track = map_queue_item(track_item, ctx.lang);

        let mut warnings = content.contents.warnings;
        warnings.append(&mut track.warnings);

        Ok(MapResult {
            c: TrackDetails {
                track: track.c,
                lyrics_id,
                related_id,
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<TrackItem>> for response::MusicDetails {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<TrackItem>>, ExtractionError> {
        let tabs = self
            .contents
            .single_column_music_watch_next_results_renderer
            .tabbed_renderer
            .watch_next_tabbed_results_renderer
            .tabs;

        let content = tabs
            .into_iter()
            .find_map(|t| t.tab_renderer.content)
            .ok_or_else(|| ExtractionError::NotFound {
                id: ctx.id.to_owned(),
                msg: "no content".into(),
            })?
            .music_queue_renderer
            .content
            .playlist_panel_renderer;

        let mut warnings = content.contents.warnings;

        let tracks = content
            .contents
            .c
            .into_iter()
            .filter_map(|item| match item {
                response::music_item::PlaylistPanelVideo::PlaylistPanelVideoRenderer(item) => {
                    let mut track = map_queue_item(item, ctx.lang);
                    warnings.append(&mut track.warnings);
                    Some(track.c)
                }
                response::music_item::PlaylistPanelVideo::None => None,
            })
            .collect::<Vec<_>>();

        let ctoken = content
            .continuations
            .into_iter()
            .next()
            .map(|c| c.next_continuation_data.continuation);

        Ok(MapResult {
            c: Paginator::new_ext(
                None,
                tracks,
                ctoken,
                None,
                ContinuationEndpoint::MusicNext,
                false,
            ),
            warnings,
        })
    }
}

impl MapResponse<Lyrics> for response::MusicLyrics {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<Lyrics>, ExtractionError> {
        let lyrics = self
            .contents
            .into_res()
            .map_err(|msg| ExtractionError::NotFound {
                id: ctx.id.to_owned(),
                msg: msg.into(),
            })?
            .into_iter()
            .find_map(|item| item.music_description_shelf_renderer)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?;

        Ok(MapResult {
            c: Lyrics {
                body: lyrics.description,
                footer: lyrics.footer,
            },
            warnings: Vec::new(),
        })
    }
}

impl MapResponse<MusicRelated> for response::MusicRelated {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicRelated>, ExtractionError> {
        let contents = self
            .contents
            .into_res()
            .map_err(|msg| ExtractionError::NotFound {
                id: ctx.id.to_owned(),
                msg: msg.into(),
            })?;

        // Find artist
        let artist_id = contents.iter().find_map(|section| match section {
            response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf) => {
                shelf.header.as_ref().and_then(|h| {
                    h.music_carousel_shelf_basic_header_renderer
                        .title
                        .0
                        .iter()
                        .find_map(|c| {
                            let artist = ArtistId::from(c.clone());
                            if artist.id.is_some() {
                                Some(artist)
                            } else {
                                None
                            }
                        })
                })
            }
            _ => None,
        });

        let mut mapper_tracks = MusicListMapper::new(ctx.lang);
        let mut mapper = match artist_id {
            Some(artist_id) => MusicListMapper::with_artist(ctx.lang, artist_id),
            None => MusicListMapper::new(ctx.lang),
        };

        let mut sections = contents.into_iter();
        if let Some(response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf)) =
            sections.next()
        {
            mapper_tracks.map_response(shelf.contents);
        }

        sections.for_each(|section| match section {
            response::music_item::ItemSection::MusicShelfRenderer(shelf) => {
                mapper.map_response(shelf.contents);
            }
            response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf) => {
                mapper.map_response(shelf.contents);
            }
            _ => {}
        });

        let mapped_tracks = mapper_tracks.conv_items();
        let mut mapped = mapper.group_items();

        let mut warnings = mapped_tracks.warnings;
        warnings.append(&mut mapped.warnings);

        Ok(MapResult {
            c: MusicRelated {
                tracks: mapped_tracks.c,
                other_versions: mapped.c.tracks,
                albums: mapped.c.albums,
                artists: mapped.c.artists,
                playlists: mapped.c.playlists,
            },
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{model, util::tests::TESTFILES};

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::track("track", "7nigXQS1Xb0")]
    fn map_music_details(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_details" / format!("details_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let details: response::MusicDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::TrackDetails> =
            details.map_response(&MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_details_{name}"), map_res.c);
    }

    #[rstest]
    #[case::mv("mv", "RDAMVMZeerrnuLi5E")]
    #[case::track("track", "RDAMVM7nigXQS1Xb0")]
    fn map_music_radio(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_details" / format!("radio_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let radio: response::MusicDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<TrackItem>> =
            radio.map_response(&MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_radio_{name}"), map_res.c);
    }

    #[test]
    fn map_lyrics() {
        let json_path = path!(*TESTFILES / "music_details" / "lyrics.json");
        let json_file = File::open(json_path).unwrap();

        let lyrics: response::MusicLyrics =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Lyrics> = lyrics.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_lyrics"), map_res.c);
    }

    #[test]
    fn map_related() {
        let json_path = path!(*TESTFILES / "music_details" / "related.json");
        let json_file = File::open(json_path).unwrap();

        let lyrics: response::MusicRelated =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicRelated> = lyrics.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_related"), map_res.c);
    }
}
