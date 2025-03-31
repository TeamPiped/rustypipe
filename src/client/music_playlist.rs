use std::{borrow::Cow, fmt::Debug};

use crate::{
    client::response::url_endpoint::NavigationEndpoint,
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        richtext::RichText,
        AlbumId, ChannelId, MusicAlbum, MusicPlaylist, TrackItem, TrackType,
    },
    serializer::{text::TextComponents, MapResult},
    util::{self, dictionary, TryRemove, DOT_SEPARATOR},
};

use self::response::url_endpoint::MusicPageType;

use super::{
    response::{
        self,
        music_item::{map_album_type, map_artist_id, map_artists, MusicListMapper},
    },
    ClientType, MapRespCtx, MapResponse, QBrowse, RustyPipeQuery,
};

impl RustyPipeQuery {
    /// Get a playlist from YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_playlist<S: AsRef<str> + Debug>(
        &self,
        playlist_id: S,
    ) -> Result<MusicPlaylist, Error> {
        let playlist_id = playlist_id.as_ref();
        let request_body = QBrowse {
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
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_album<S: AsRef<str> + Debug>(
        &self,
        album_id: S,
    ) -> Result<MusicAlbum, Error> {
        let album_id = album_id.as_ref();
        let request_body = QBrowse {
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

        // In rare cases, albums may have track numbers =0 (example: MPREb_RM0QfZ0eSKL)
        // They should be replaced with the track number derived from the previous track.
        let mut n_prev = 0;
        for track in &mut album.tracks {
            let tn = track.track_nr.unwrap_or_default();
            if tn == 0 {
                n_prev += 1;
                track.track_nr = Some(n_prev);
            } else {
                n_prev = tn;
            }
        }

        // YouTube Music is replacing album tracks with their respective music videos. To get the original
        // tracks, we have to fetch the album as a playlist and replace the offending track ids.
        if let Some(playlist_id) = &album.playlist_id {
            // Get a list of music videos in the album
            let to_replace = album
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(i, track)| {
                    if track.track_type.is_video() {
                        Some((i, track.name.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let last_tn = album
                .tracks
                .last()
                .and_then(|t| t.track_nr)
                .unwrap_or_default();
            if !to_replace.is_empty() || last_tn < album.track_count {
                tracing::debug!(
                    "fetching album playlist ({} tracks, {} to replace)",
                    album.track_count,
                    to_replace.len()
                );
                let mut playlist = self.music_playlist(playlist_id).await?;
                playlist
                    .tracks
                    .extend_limit(&self, album.track_count.into())
                    .await?;

                for (i, title) in to_replace {
                    let found_track = playlist.tracks.items.iter().find_map(|track| {
                        if track.name == title && track.track_type.is_track() {
                            Some((track.id.clone(), track.duration))
                        } else {
                            None
                        }
                    });
                    if let Some((track_id, duration)) = found_track {
                        album.tracks[i].id = track_id;
                        if let Some(duration) = duration {
                            album.tracks[i].duration = Some(duration);
                        }
                        album.tracks[i].track_type = TrackType::Track;
                    }
                }

                // Extend the list of album tracks with the ones from the playlist if the playlist returned more tracks
                // This is the case for albums with more than 200 tracks (e.g. audiobooks)
                if album.tracks.len() < playlist.tracks.items.len() {
                    let mut tn = last_tn;
                    for mut t in playlist.tracks.items.into_iter().skip(album.tracks.len()) {
                        tn += 1;
                        t.album = album.tracks.first().and_then(|t| t.album.clone());
                        t.track_nr = Some(tn);
                        album.tracks.push(t);
                    }
                }
            }
        }
        Ok(album)
    }
}

impl MapResponse<MusicPlaylist> for response::MusicPlaylist {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
        let contents = match self.contents {
            Some(c) => c,
            None => {
                if self.microformat.microformat_data_renderer.noindex {
                    return Err(ExtractionError::NotFound {
                        id: ctx.id.to_owned(),
                        msg: "no contents".into(),
                    });
                } else {
                    return Err(ExtractionError::InvalidData("no contents".into()));
                }
            }
        };

        let (header, music_contents) = match contents {
            response::music_playlist::Contents::SingleColumnBrowseResultsRenderer(c) => (
                self.header,
                c.contents
                    .into_iter()
                    .next()
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
                    .tab_renderer
                    .content
                    .section_list_renderer,
            ),
            response::music_playlist::Contents::TwoColumnBrowseResultsRenderer {
                secondary_contents,
                tabs,
            } => (
                tabs.into_iter()
                    .next()
                    .and_then(|t| {
                        t.tab_renderer
                            .content
                            .section_list_renderer
                            .contents
                            .into_iter()
                            .next()
                    })
                    .or(self.header),
                secondary_contents.section_list_renderer,
            ),
        };
        let shelf = music_contents
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
            if playlist_id != ctx.id {
                return Err(ExtractionError::WrongResult(format!(
                    "got wrong playlist id {}, expected {}",
                    playlist_id, ctx.id
                )));
            }
        }

        let mut mapper = MusicListMapper::new(ctx.lang);
        mapper.map_response(shelf.contents);

        let ctoken = mapper.ctoken.clone().or_else(|| {
            shelf
                .continuations
                .into_iter()
                .next()
                .map(|cont| cont.next_continuation_data.continuation)
        });
        let map_res = mapper.conv_items();

        let track_count = if ctoken.is_some() {
            header.as_ref().and_then(|h| {
                let parts = h
                    .music_detail_header_renderer
                    .second_subtitle
                    .split(|p| p == DOT_SEPARATOR)
                    .collect::<Vec<_>>();
                parts
                    .get(usize::from(parts.len() > 2))
                    .and_then(|txt| util::parse_numeric::<u64>(&txt[0]).ok())
            })
        } else {
            Some(map_res.c.len() as u64)
        };

        let related_ctoken = music_contents
            .continuations
            .into_iter()
            .next()
            .map(|c| c.next_continuation_data.continuation);

        let (from_ytm, channel, name, thumbnail, description) = match header {
            Some(header) => {
                let h = header.music_detail_header_renderer;

                let (from_ytm, channel) = match h.facepile {
                    Some(facepile) => {
                        let from_ytm = facepile.avatar_stack_view_model.text.starts_with("YouTube");
                        let channel = facepile
                            .avatar_stack_view_model
                            .renderer_context
                            .command_context
                            .and_then(|c| {
                                c.on_tap
                                    .innertube_command
                                    .music_page()
                                    .filter(|p| p.typ == MusicPageType::User)
                                    .map(|p| p.id)
                            })
                            .map(|id| ChannelId {
                                id,
                                name: facepile.avatar_stack_view_model.text,
                            });

                        (from_ytm && channel.is_none(), channel)
                    }
                    None => {
                        let st = match h.strapline_text_one {
                            Some(s) => s,
                            None => h.subtitle,
                        };

                        let from_ytm = st.0.iter().any(util::is_ytm);
                        let channel = st.0.into_iter().find_map(|c| ChannelId::try_from(c).ok());
                        (from_ytm, channel)
                    }
                };

                (
                    from_ytm,
                    channel,
                    h.title,
                    h.thumbnail.into(),
                    h.description.map(TextComponents::from),
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
                id: ctx.id.to_owned(),
                name,
                thumbnail,
                channel,
                description: description.map(RichText::from),
                track_count,
                from_ytm,
                tracks: Paginator::new_ext(
                    track_count,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::MusicBrowse,
                    ctx.authenticated,
                ),
                related_playlists: Paginator::new_ext(
                    None,
                    Vec::new(),
                    related_ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::MusicBrowse,
                    ctx.authenticated,
                ),
            },
            warnings: map_res.warnings,
        })
    }
}

impl MapResponse<MusicAlbum> for response::MusicPlaylist {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<MusicAlbum>, ExtractionError> {
        let contents = match self.contents {
            Some(c) => c,
            None => {
                if self.microformat.microformat_data_renderer.noindex {
                    return Err(ExtractionError::NotFound {
                        id: ctx.id.to_owned(),
                        msg: "no contents".into(),
                    });
                } else {
                    return Err(ExtractionError::InvalidData("no contents".into()));
                }
            }
        };

        let (header, sections) = match contents {
            response::music_playlist::Contents::SingleColumnBrowseResultsRenderer(c) => (
                self.header,
                c.contents
                    .into_iter()
                    .next()
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
                    .tab_renderer
                    .content
                    .section_list_renderer
                    .contents,
            ),
            response::music_playlist::Contents::TwoColumnBrowseResultsRenderer {
                secondary_contents,
                tabs,
            } => (
                tabs.into_iter()
                    .next()
                    .and_then(|t| {
                        t.tab_renderer
                            .content
                            .section_list_renderer
                            .contents
                            .into_iter()
                            .next()
                    })
                    .or(self.header),
                secondary_contents.section_list_renderer.contents,
            ),
        };
        let header = header
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no header")))?
            .music_detail_header_renderer;

        let mut shelf = None;
        let mut album_variants = None;
        for section in sections {
            match section {
                response::music_item::ItemSection::MusicShelfRenderer(sh) => shelf = Some(sh),
                response::music_item::ItemSection::MusicCarouselShelfRenderer(sh) => {
                    if sh
                        .header
                        .map(|h| {
                            h.music_carousel_shelf_basic_header_renderer
                                .title
                                .first_str()
                                == dictionary::entry(ctx.lang).album_versions_title
                        })
                        .unwrap_or_default()
                    {
                        album_variants = Some(sh.contents);
                    }
                }
                _ => (),
            }
        }
        let shelf = shelf.ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "no sectionListRenderer content",
        )))?;

        let mut subtitle_split = header.subtitle.split(util::DOT_SEPARATOR);

        let (year_txt, artists_p) = match header.strapline_text_one {
            // New (2column) album layout
            Some(sl) => {
                let year_txt = subtitle_split
                    .try_swap_remove(1)
                    .and_then(|t| t.0.first().map(|c| c.as_str().to_owned()));
                (year_txt, Some(sl))
            }
            // Old album layout
            None => match subtitle_split.len() {
                3.. => {
                    let year_txt = subtitle_split
                        .swap_remove(2)
                        .0
                        .first()
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
            },
        };

        let (artists, by_va) = map_artists(artists_p);
        let album_type_txt = subtitle_split
            .into_iter()
            .next()
            .map(|part| part.to_string())
            .unwrap_or_default();

        let album_type = map_album_type(album_type_txt.as_str(), ctx.lang);
        let year = year_txt.and_then(|txt| util::parse_numeric(&txt).ok());

        fn map_playlist_id(ep: &NavigationEndpoint) -> Option<String> {
            if let NavigationEndpoint::WatchPlaylist {
                watch_playlist_endpoint,
            } = ep
            {
                Some(watch_playlist_endpoint.playlist_id.to_owned())
            } else {
                None
            }
        }

        let playlist_id = self
            .microformat
            .microformat_data_renderer
            .url_canonical
            .and_then(|x| {
                x.strip_prefix("https://music.youtube.com/playlist?list=")
                    .map(str::to_owned)
            });
        let (playlist_id, artist_id) = header
            .menu
            .or_else(|| header.buttons.into_iter().next())
            .map(|menu| {
                (
                    playlist_id.or_else(|| {
                        menu.menu_renderer
                            .top_level_buttons
                            .iter()
                            .find_map(|btn| {
                                map_playlist_id(&btn.button_renderer.navigation_endpoint)
                            })
                            .or_else(|| {
                                menu.menu_renderer.items.iter().find_map(|itm| {
                                    map_playlist_id(
                                        &itm.menu_navigation_item_renderer.navigation_endpoint,
                                    )
                                })
                            })
                    }),
                    map_artist_id(menu.menu_renderer.items),
                )
            })
            .unwrap_or_default();
        let artist_id = artist_id.or_else(|| artists.first().and_then(|a| a.id.clone()));

        let second_subtitle_parts = header
            .second_subtitle
            .split(|p| p == DOT_SEPARATOR)
            .collect::<Vec<_>>();
        let track_count = second_subtitle_parts
            .get(usize::from(second_subtitle_parts.len() > 2))
            .and_then(|txt| util::parse_numeric::<u16>(&txt[0]).ok());

        let mut mapper = MusicListMapper::with_album(
            ctx.lang,
            artists.clone(),
            by_va,
            AlbumId {
                id: ctx.id.to_owned(),
                name: header.title.clone(),
            },
        );
        mapper.map_response(shelf.contents);
        let tracks_res = mapper.conv_items();
        let mut warnings = tracks_res.warnings;

        let mut variants_mapper = MusicListMapper::new(ctx.lang);
        if let Some(res) = album_variants {
            variants_mapper.map_response(res);
        }
        let mut variants_res = variants_mapper.conv_items();
        warnings.append(&mut variants_res.warnings);

        Ok(MapResult {
            c: MusicAlbum {
                id: ctx.id.to_owned(),
                playlist_id,
                name: header.title,
                cover: header.thumbnail.into(),
                artists,
                artist_id,
                description: header
                    .description
                    .map(|t| RichText::from(TextComponents::from(t))),
                album_type,
                year,
                by_va,
                track_count: track_count.unwrap_or(tracks_res.c.len() as u16),
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
    use crate::{model, util::tests::TESTFILES};

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    #[case::two_columns("20240228_twoColumns", "RDCLAK5uy_kb7EBi6y3GrtJri4_ZH56Ms786DFEimbM")]
    #[case::n_album("20240228_album", "OLAK5uy_kdSWBZ-9AZDkYkuy0QCc3p0KO9DEHVNH0")]
    #[case::facepile("20241125_facepile", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_music_playlist(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_playlist" / format!("playlist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicPlaylist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::MusicPlaylist> =
            playlist.map_response(&MapRespCtx::test(id)).unwrap();

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
    #[case::two_columns("20240228_twoColumns", "MPREb_bHfHGoy7vuv")]
    #[case::recommends("20250225_recommends", "MPREb_u1I69lSAe5v")]
    fn map_music_album(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_playlist" / format!("album_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicPlaylist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::MusicAlbum> =
            playlist.map_response(&MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_album_{name}"), map_res.c);
    }
}
