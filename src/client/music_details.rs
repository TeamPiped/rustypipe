use std::borrow::Cow;

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{ArtistId, Lyrics, MusicRelated, Paginator, TrackDetails, TrackItem},
    param::Language,
    serializer::MapResult,
};

use super::{
    response::{
        self,
        music_item::{map_queue_item, MusicListMapper},
    },
    ClientType, MapResponse, QBrowse, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
struct QMusicDetails<'a> {
    context: YTContext<'a>,
    video_id: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

#[derive(Debug, Serialize)]
struct QRadio<'a> {
    context: YTContext<'a>,
    playlist_id: &'a str,
    params: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

impl RustyPipeQuery {
    pub async fn music_details(&self, video_id: &str) -> Result<TrackDetails, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QMusicDetails {
            context,
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

    pub async fn music_lyrics(&self, lyrics_id: &str) -> Result<Lyrics, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
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

    pub async fn music_related(&self, related_id: &str) -> Result<MusicRelated, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
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

    pub async fn music_radio(&self, radio_id: &str) -> Result<Paginator<TrackItem>, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QRadio {
            context,
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

    pub async fn music_radio_track(&self, video_id: &str) -> Result<Paginator<TrackItem>, Error> {
        self.music_radio(&format!("RDAMVM{}", video_id)).await
    }

    pub async fn music_radio_playlist(
        &self,
        playlist_id: &str,
    ) -> Result<Paginator<TrackItem>, Error> {
        self.music_radio(&format!("RDAMPL{}", playlist_id)).await
    }
}

impl MapResponse<TrackDetails> for response::MusicDetails {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
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

        let content = content.ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
            "track not found",
        )))?;
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
        let track = map_queue_item(track_item, lang);

        if track.id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong video id {}, expected {}",
                track.id, id
            )));
        }

        Ok(MapResult {
            c: TrackDetails {
                track,
                lyrics_id,
                related_id,
            },
            warnings: content.contents.warnings,
        })
    }
}

impl MapResponse<Paginator<TrackItem>> for response::MusicDetails {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
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
            .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
                "radio unavailable",
            )))?
            .music_queue_renderer
            .content
            .playlist_panel_renderer;

        let tracks = content
            .contents
            .c
            .into_iter()
            .filter_map(|item| match item {
                response::music_item::PlaylistPanelVideo::PlaylistPanelVideoRenderer(item) => {
                    Some(map_queue_item(item, lang))
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
                crate::param::ContinuationEndpoint::MusicNext,
            ),
            warnings: content.contents.warnings,
        })
    }
}

impl MapResponse<Lyrics> for response::MusicLyrics {
    fn map_response(
        self,
        _id: &str,
        _lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Lyrics>, ExtractionError> {
        let lyrics = self
            .contents
            .section_list_renderer
            .and_then(|sl| {
                sl.contents
                    .into_iter()
                    .find_map(|item| item.music_description_shelf_renderer)
            })
            .ok_or(match self.contents.message_renderer {
                Some(msg) => ExtractionError::ContentUnavailable(Cow::Owned(msg.text)),
                None => ExtractionError::InvalidData(Cow::Borrowed("no content")),
            })?;

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
        _id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicRelated>, ExtractionError> {
        // Find artist
        let artist_id = self
            .contents
            .section_list_renderer
            .contents
            .iter()
            .find_map(|section| match section {
                response::music_item::ItemSection::MusicShelfRenderer(_) => None,
                response::music_item::ItemSection::MusicCarouselShelfRenderer {
                    header, ..
                } => header.as_ref().and_then(|h| {
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
                }),
                response::music_item::ItemSection::None => None,
            });

        let mut mapper_tracks = MusicListMapper::new(lang);
        let mut mapper = match artist_id {
            Some(artist_id) => MusicListMapper::with_artist(lang, artist_id),
            None => MusicListMapper::new(lang),
        };

        let mut sections = self.contents.section_list_renderer.contents.into_iter();
        if let Some(response::music_item::ItemSection::MusicCarouselShelfRenderer {
            contents,
            ..
        }) = sections.next()
        {
            mapper_tracks.map_response(contents);
        }

        sections.for_each(|section| match section {
            response::music_item::ItemSection::MusicShelfRenderer(shelf) => {
                mapper.map_response(shelf.contents);
            }
            response::music_item::ItemSection::MusicCarouselShelfRenderer { contents, .. } => {
                mapper.map_response(contents);
            }
            response::music_item::ItemSection::None => {}
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
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;
    use crate::{model, param::Language};

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::track("track", "7nigXQS1Xb0")]
    fn map_music_details(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/music_details/details_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let details: response::MusicDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::TrackDetails> =
            details.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_details_{}", name), map_res.c);
    }

    #[rstest]
    #[case::mv("mv", "RDAMVMZeerrnuLi5E")]
    #[case::track("track", "RDAMVM7nigXQS1Xb0")]
    fn map_music_radio(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/music_details/radio_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let radio: response::MusicDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<TrackItem>> =
            radio.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_radio_{}", name), map_res.c);
    }

    #[test]
    fn map_lyrics() {
        let json_path = Path::new("testfiles/music_details/lyrics.json");
        let json_file = File::open(json_path).unwrap();

        let lyrics: response::MusicLyrics =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Lyrics> = lyrics.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_lyrics"), map_res.c);
    }

    #[test]
    fn map_related() {
        let json_path = Path::new("testfiles/music_details/related.json");
        let json_file = File::open(json_path).unwrap();

        let lyrics: response::MusicRelated =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicRelated> = lyrics.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_related"), map_res.c);
    }
}
