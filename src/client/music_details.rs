use std::{borrow::Cow, fmt::Debug};

use crate::{
    error::{Error, ExtractionError},
    json::{ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        AlbumType, ArtistId, Lyrics, MusicRelated, TrackDetails, TrackItem,
    },
    request_body::ytbody,
    serializer::MapResult,
};

use super::{
    response::{
        self,
        music_item::{map_grouped_music_items_values, map_music_items_value, map_queue_item},
    },
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

struct MusicDetailsEndpoint;
struct MusicRadioEndpoint;
struct MusicLyricsEndpoint;
struct MusicRelatedEndpoint;

impl RustyPipeQuery {
    /// Get the metadata of a YouTube Music track
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_details<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
    ) -> Result<TrackDetails, Error> {
        let video_id = video_id.as_ref();
        let request_body = ytbody!({
            "videoId": video_id,
            "enablePersistentPlaylistPanel": true,
            "isAudioOnly": true,
            "tunerSettingValue": "AUTOMIX_SETTING_NORMAL",
        });

        self.execute_request_json::<MusicDetailsEndpoint, _, _>(
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
        let request_body = ytbody!({
            "browseId": lyrics_id,
        });

        self.execute_request_json::<MusicLyricsEndpoint, _, _>(
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
        let request_body = ytbody!({
            "browseId": related_id,
        });

        self.execute_request_json::<MusicRelatedEndpoint, _, _>(
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
        let request_body = ytbody!({
            "playlistId": radio_id,
            "params": "wAEB8gECeAE%3D",
            "enablePersistentPlaylistPanel": true,
            "isAudioOnly": true,
            "tunerSettingValue": "AUTOMIX_SETTING_NORMAL",
        });

        self.execute_request_json::<MusicRadioEndpoint, _, _>(
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

fn inspect_music_tabs<'a>(
    root: &JsonNode<'a>,
    ctx: &MapRespCtx<'_>,
) -> Result<(JsonNode<'a>, Option<String>, Option<String>), ExtractionError> {
    let tabs = root.require(
        ytq!(.contents.singleColumnMusicWatchNextResultsRenderer.tabbedRenderer.watchNextTabbedResultsRenderer.tabs),
        "music tabs",
    )?;

    let mut panel = None;
    let mut lyrics_id = None;
    let mut related_id = None;

    for tab in tabs.items() {
        if let Some(content) =
            tab.query(ytq!(.tabRenderer.content.musicQueueRenderer.content.playlistPanelRenderer))
        {
            panel = Some(content);
        }

        if let Some(endpoint) = tab.query(ytq!(.tabRenderer.endpoint.browseEndpoint)) {
            match endpoint
                .query(ytq!(.browseEndpointContextSupportedConfigs.browseEndpointContextMusicConfig.pageType))
                .and_then(|value| value.as_str())
                .as_deref()
            {
                Some("MUSIC_PAGE_TYPE_TRACK_LYRICS") => {
                    lyrics_id = endpoint.query(ytq!(.browseId)).and_then(|value| value.as_str());
                }
                Some("MUSIC_PAGE_TYPE_TRACK_RELATED") => {
                    related_id = endpoint.query(ytq!(.browseId)).and_then(|value| value.as_str());
                }
                _ => {}
            }
        }
    }

    let panel = panel.ok_or_else(|| ExtractionError::NotFound {
        id: ctx.id.to_owned(),
        msg: "no content".into(),
    })?;

    Ok((panel, lyrics_id, related_id))
}

type PlaylistPanelData = (
    Vec<response::music_item::QueueMusicItem>,
    Vec<String>,
    Option<String>,
);

fn deserialize_playlist_panel(panel: &JsonNode<'_>) -> Result<PlaylistPanelData, ExtractionError> {
    let contents = panel.require(ytq!(.contents), "playlist panel contents")?;
    let mut warnings = Vec::new();
    let items = contents
        .items()
        .into_iter()
        .filter_map(|item| {
            item.try_deserialize::<response::music_item::QueueMusicItem>(
                ytq!(.playlistPanelVideoRenderer),
                &mut warnings,
            )
        })
        .collect();
    let ctoken = panel
        .query(ytq!(.continuations[0].nextRadioContinuationData.continuation))
        .and_then(|node| node.as_str());

    Ok((items, warnings, ctoken))
}

impl MapEndpoint<TrackDetails> for MusicDetailsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<TrackDetails>, ExtractionError> {
        json.with_root(|root| {
            let (panel, lyrics_id, related_id) = inspect_music_tabs(&root, ctx)?;
            let (items, mut warnings, _) = deserialize_playlist_panel(&panel)?;
            let track_item = items
                .into_iter()
                .next()
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no video item")))?;

            let mut track = map_queue_item(track_item, ctx.lang);
            warnings.append(&mut track.warnings);

            Ok(MapResult {
                c: TrackDetails {
                    track: track.c,
                    lyrics_id,
                    related_id,
                },
                warnings,
            })
        })
    }
}

impl MapEndpoint<Paginator<TrackItem>> for MusicRadioEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<TrackItem>>, ExtractionError> {
        json.with_root(|root| {
            let (panel, _, _) = inspect_music_tabs(&root, ctx)?;
            let (items, mut warnings, ctoken) = deserialize_playlist_panel(&panel)?;

            let tracks = items
                .into_iter()
                .map(|item| {
                    let mut track = map_queue_item(item, ctx.lang);
                    warnings.append(&mut track.warnings);
                    track.c
                })
                .collect::<Vec<_>>();

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
        })
    }
}

impl MapEndpoint<Lyrics> for MusicLyricsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Lyrics>, ExtractionError> {
        json.with_root(|root| {
            if let Some(msg) =
                root.text_at(ytq!(.contents.messageRenderer.text || .contents.messageRenderer))
            {
                return Err(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: msg.into(),
                });
            }

            let contents = root.require(
                ytq!(.contents.sectionListRenderer.contents),
                "lyrics contents",
            )?;
            let shelf = contents
                .items()
                .into_iter()
                .find_map(|item| item.query(ytq!(.musicDescriptionShelfRenderer)))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?;

            let body = shelf
                .text_at(ytq!(.description))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "missing lyrics body",
                )))?;
            let footer = shelf
                .text_at(ytq!(.footer))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "missing lyrics footer",
                )))?;

            Ok(MapResult {
                c: Lyrics { body, footer },
                warnings: Vec::new(),
            })
        })
    }
}

impl MapEndpoint<MusicRelated> for MusicRelatedEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicRelated>, ExtractionError> {
        json.with_root(|root| {
            if let Some(msg) =
                root.text_at(ytq!(.contents.messageRenderer.text || .contents.messageRenderer))
            {
                return Err(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: msg.into(),
                });
            }

            let contents_node = root.require(
                ytq!(.contents.sectionListRenderer.contents),
                "related contents",
            )?;
            let contents = contents_node.items();
            let mut warnings = Vec::new();

            let artist_id = contents.iter().find_map(|section| {
                response::music_item::music_carousel_node(section)
                    .and_then(|node| {
                        node.deserialize::<response::music_item::MusicCarouselShelf>()
                            .ok()
                    })
                    .and_then(|shelf| {
                        shelf.header.and_then(|h| {
                            h.title.0.into_iter().find_map(|c| {
                                let artist = ArtistId::from(c);
                                artist.id.is_some().then_some(artist)
                            })
                        })
                    })
            });

            let mut sections = contents.into_iter();
            let mut mapped_tracks: MapResult<Vec<TrackItem>> = MapResult::default();
            if let Some(shelf) = sections.next().and_then(|section| {
                response::music_item::music_carousel_node(&section).and_then(|node| {
                    node.deserialize::<response::music_item::MusicCarouselShelf>()
                        .ok()
                })
            }) {
                mapped_tracks = map_music_items_value(shelf.contents, ctx.lang).0;
            }

            let mut grouped_values = Vec::new();
            sections.for_each(|section| {
                if let Some(shelf) = response::music_item::music_shelf_node(&section)
                    .and_then(|node| node.deserialize::<response::music_item::MusicShelf>().ok())
                {
                    grouped_values.push((shelf.contents, AlbumType::Single));
                } else if let Some(shelf) = response::music_item::music_carousel_node(&section)
                    .and_then(|node| {
                        node.deserialize::<response::music_item::MusicCarouselShelf>()
                            .ok()
                    })
                {
                    grouped_values.push((shelf.contents, AlbumType::Single));
                }
            });

            let mut mapped = map_grouped_music_items_values(grouped_values, ctx.lang, artist_id).0;

            warnings.append(&mut mapped_tracks.warnings);
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
        })
    }
}

#[cfg(test)]
mod tests {
    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{model, util::tests::TESTFILES};

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::track("track", "7nigXQS1Xb0")]
    fn map_music_details(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_details" / format!("details_{name}.json"));
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<model::TrackDetails> =
            MusicDetailsEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<TrackItem>> =
            MusicRadioEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Lyrics> =
            MusicLyricsEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicRelated> =
            MusicRelatedEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_related"), map_res.c);
    }
}
