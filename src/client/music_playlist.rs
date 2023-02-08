use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::{paginator::Paginator, AlbumId, ChannelId, MusicAlbum, MusicPlaylist, TrackItem},
    serializer::MapResult,
    util::{self, TryRemove},
};

use super::{
    response::{
        self,
        music_item::{map_album_type, map_artist_id, map_artists, MusicListMapper},
    },
    ClientType, MapResponse, QBrowse, RustyPipeQuery,
};

impl RustyPipeQuery {
    /// Get a playlist from YouTube Music
    pub async fn music_playlist<S: AsRef<str>>(
        &self,
        playlist_id: S,
    ) -> Result<MusicPlaylist, Error> {
        let playlist_id = playlist_id.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: &format!("VL{playlist_id}"),
        };

        self.execute_request::<response::MusicPlaylist, _, _>(
            ClientType::DesktopMusic,
            "music_playlist",
            playlist_id,
            "browse",
            &request_body,
        )
        .await
    }

    /// Get an album from YouTube Music
    pub async fn music_album<S: AsRef<str>>(&self, album_id: S) -> Result<MusicAlbum, Error> {
        let album_id = album_id.as_ref();
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: album_id,
        };

        let mut album = self
            .execute_request::<response::MusicPlaylist, MusicAlbum, _>(
                ClientType::DesktopMusic,
                "music_album",
                album_id,
                "browse",
                &request_body,
            )
            .await?;

        // YouTube Music is replacing album tracks with their respective music videos. To get the original
        // tracks, we have to fetch the album as a playlist and replace the offending track ids.
        if let Some(playlist_id) = &album.playlist_id {
            // Get a list of music videos in the album
            let to_replace = album
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(i, track)| {
                    if track.is_video {
                        Some((i, track.name.to_owned()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if !to_replace.is_empty() {
                match self.music_playlist(playlist_id).await {
                    Ok(playlist) => {
                        for (i, title) in to_replace {
                            let found_track = playlist.tracks.items.iter().find_map(|track| {
                                if track.name == title && !track.is_video {
                                    Some((track.id.to_owned(), track.duration))
                                } else {
                                    None
                                }
                            });
                            if let Some((track_id, duration)) = found_track {
                                album.tracks[i].id = track_id;
                                if let Some(duration) = duration {
                                    album.tracks[i].duration = Some(duration);
                                }
                                album.tracks[i].is_video = false;
                            }
                        }
                    }
                    Err(Error::Extraction(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(album)
    }
}

impl MapResponse<MusicPlaylist> for response::MusicPlaylist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
        // dbg!(&self);

        let mut content = self.contents.single_column_browse_results_renderer.contents;
        let mut music_contents = content
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer;
        let mut shelf = music_contents
            .contents
            .into_iter()
            .find_map(|section| match section {
                response::music_item::ItemSection::MusicShelfRenderer(shelf) => Some(shelf),
                _ => None,
            })
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "no sectionListRenderer content",
            )))?;

        if let Some(playlist_id) = shelf.playlist_id {
            if playlist_id != id {
                return Err(ExtractionError::WrongResult(format!(
                    "got wrong playlist id {playlist_id}, expected {id}"
                )));
            }
        }

        let mut mapper = MusicListMapper::new(lang);
        mapper.map_response(shelf.contents);
        let map_res = mapper.conv_items();

        let ctoken = shelf
            .continuations
            .try_swap_remove(0)
            .map(|cont| cont.next_continuation_data.continuation);

        let track_count = match ctoken {
            Some(_) => self.header.as_ref().and_then(|h| {
                h.music_detail_header_renderer
                    .second_subtitle
                    .first()
                    .and_then(|txt| util::parse_numeric::<u64>(txt).ok())
            }),
            None => Some(map_res.c.len() as u64),
        };

        let related_ctoken = music_contents
            .continuations
            .try_swap_remove(0)
            .map(|c| c.next_continuation_data.continuation);

        let (from_ytm, channel, name, thumbnail, description) = match self.header {
            Some(header) => {
                let h = header.music_detail_header_renderer;

                let from_ytm = h
                    .subtitle
                    .0
                    .iter()
                    .any(|c| c.as_str() == util::YT_MUSIC_NAME);
                let channel = h
                    .subtitle
                    .0
                    .into_iter()
                    .find_map(|c| ChannelId::try_from(c).ok());

                (
                    from_ytm,
                    channel,
                    h.title,
                    h.thumbnail.into(),
                    h.description,
                )
            }
            None => {
                // Album playlists fetched via the playlist method dont include a header
                let (album, cover) = map_res
                    .c
                    .first()
                    .and_then(|t: &TrackItem| {
                        t.album.as_ref().map(|a| (a.clone(), t.cover.clone()))
                    })
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                        "playlist without header or album items",
                    )))?;

                if !map_res.c.iter().all(|t| {
                    t.album
                        .as_ref()
                        .map(|a| a.id == album.id)
                        .unwrap_or_default()
                }) {
                    return Err(ExtractionError::InvalidData(Cow::Borrowed(
                        "album playlist containing items from different albums",
                    )));
                }

                (true, None, album.name, cover, None)
            }
        };

        Ok(MapResult {
            c: MusicPlaylist {
                id: id.to_owned(),
                name,
                thumbnail,
                channel,
                description,
                track_count,
                from_ytm,
                tracks: Paginator::new_ext(
                    track_count,
                    map_res.c,
                    ctoken,
                    None,
                    crate::model::paginator::ContinuationEndpoint::MusicBrowse,
                ),
                related_playlists: Paginator::new_ext(
                    None,
                    Vec::new(),
                    related_ctoken,
                    None,
                    crate::model::paginator::ContinuationEndpoint::MusicBrowse,
                ),
            },
            warnings: map_res.warnings,
        })
    }
}

impl MapResponse<MusicAlbum> for response::MusicPlaylist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<MusicAlbum>, ExtractionError> {
        // dbg!(&self);

        let header = self
            .header
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no header")))?
            .music_detail_header_renderer;

        let mut content = self.contents.single_column_browse_results_renderer.contents;
        let sections = content
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut shelf = None;
        let mut album_variants = None;
        for section in sections {
            match section {
                response::music_item::ItemSection::MusicShelfRenderer(sh) => shelf = Some(sh),
                response::music_item::ItemSection::MusicCarouselShelfRenderer(sh) => {
                    album_variants = Some(sh.contents)
                }
                _ => (),
            }
        }
        let shelf = shelf.ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "no sectionListRenderer content",
        )))?;

        let mut subtitle_split = header.subtitle.split(util::DOT_SEPARATOR);

        let (year_txt, artists_p) = match subtitle_split.len() {
            3.. => {
                let year_txt = subtitle_split
                    .swap_remove(2)
                    .0
                    .get(0)
                    .map(|c| c.as_str().to_owned());
                (year_txt, subtitle_split.try_swap_remove(1))
            }
            2 => {
                // The second part may either be the year or the artist
                let p2 = subtitle_split.swap_remove(1);
                let is_year =
                    p2.0.len() == 1 && p2.0[0].as_str().chars().all(|c| c.is_ascii_digit());
                if is_year {
                    (Some(p2.0[0].as_str().to_owned()), None)
                } else {
                    (None, Some(p2))
                }
            }
            _ => (None, None),
        };

        let (artists, by_va) = map_artists(artists_p);
        let album_type_txt = subtitle_split
            .try_swap_remove(0)
            .map(|part| part.to_string())
            .unwrap_or_default();

        let album_type = map_album_type(album_type_txt.as_str(), lang);
        let year = year_txt.and_then(|txt| util::parse_numeric(&txt).ok());

        let (artist_id, playlist_id) = header
            .menu
            .map(|mut menu| {
                (
                    map_artist_id(menu.menu_renderer.items),
                    menu.menu_renderer
                        .top_level_buttons
                        .try_swap_remove(0)
                        .map(|btn| {
                            btn.button_renderer
                                .navigation_endpoint
                                .watch_playlist_endpoint
                                .playlist_id
                        }),
                )
            })
            .unwrap_or_default();
        let artist_id = artist_id.or_else(|| artists.first().and_then(|a| a.id.to_owned()));

        let mut mapper = MusicListMapper::with_album(
            lang,
            artists.clone(),
            by_va,
            AlbumId {
                id: id.to_owned(),
                name: header.title.to_owned(),
            },
        );
        mapper.map_response(shelf.contents);
        let tracks_res = mapper.conv_items();
        let mut warnings = tracks_res.warnings;

        let mut variants_mapper = MusicListMapper::new(lang);
        if let Some(res) = album_variants {
            variants_mapper.map_response(res);
        }
        let mut variants_res = variants_mapper.conv_items();
        warnings.append(&mut variants_res.warnings);

        Ok(MapResult {
            c: MusicAlbum {
                id: id.to_owned(),
                playlist_id,
                name: header.title,
                cover: header.thumbnail.into(),
                artists,
                artist_id,
                description: header.description,
                album_type,
                year,
                by_va,
                tracks: tracks_res.c,
                variants: variants_res.c,
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
    use crate::{model, param::Language};

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_music_playlist(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "music_playlist" / format!("playlist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicPlaylist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::MusicPlaylist> =
            playlist.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_playlist_{name}"), map_res.c, {
            ".last_update" => "[date]"
        });
    }

    #[rstest]
    #[case::one_artist("one_artist", "MPREb_nlBWQROfvjo")]
    #[case::various_artists("various_artists", "MPREb_8QkDeEIawvX")]
    #[case::single("single", "MPREb_bHfHGoy7vuv")]
    #[case::description("description", "MPREb_PiyfuVl6aYd")]
    #[case::unavailable("unavailable", "MPREb_AzuWg8qAVVl")]
    fn map_music_album(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "music_playlist" / format!("album_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicPlaylist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::MusicAlbum> =
            playlist.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_album_{name}"), map_res.c);
    }
}
