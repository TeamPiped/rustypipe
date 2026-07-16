use std::{borrow::Cow, fmt::Debug};

use crate::{
    client::response::url_endpoint,
    error::{Error, ExtractionError},
    json::{ytq, JsonDoc, JsonNode, JsonValue},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        AlbumId, ChannelId, MusicAlbum, MusicPlaylist, TrackItem, TrackType,
    },
    request_body::ytbody,
    serializer::MapResult,
    util::{self, dictionary, TryRemove, DOT_SEPARATOR},
};

use self::response::url_endpoint::MusicPageType;

use super::{
    response::{
        self,
        music_item::{
            map_album_track_items, map_album_type, map_artist_id, map_artists,
            map_music_items_value, music_carousel_from_value, music_shelf_from_value,
            MusicMicroformat,
        },
        music_playlist::AvatarStackViewModel,
    },
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicPlaylistEndpoint;

impl RustyPipeQuery {
    /// Get a playlist from YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_playlist<S: AsRef<str> + Debug>(
        &self,
        playlist_id: S,
    ) -> Result<MusicPlaylist, Error> {
        let playlist_id = playlist_id.as_ref();
        let request_body = ytbody!({
            "browseId": format!("VL{playlist_id}"),
        });

        self.execute_request::<MusicPlaylistEndpoint, _, _>(
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
        let request_body = ytbody!({
            "browseId": album_id,
        });

        let mut album = self
            .execute_request::<MusicPlaylistEndpoint, MusicAlbum, _>(
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
                    if track.track_type.is_video() && !track.unavailable {
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
                            Some((track.id.clone(), track.duration, track.unavailable))
                        } else {
                            None
                        }
                    });
                    if let Some((track_id, duration, unavailable)) = found_track {
                        album.tracks[i].id = track_id;
                        if let Some(duration) = duration {
                            album.tracks[i].duration = Some(duration);
                        }
                        album.tracks[i].track_type = TrackType::Track;
                        album.tracks[i].unavailable = unavailable;
                    }
                }

                // Extend the list of album tracks with the ones from the playlist if the playlist returned more tracks
                // This is the case for albums with more than 200 tracks (e.g. audiobooks)
                // Note: in some cases the playlist may contain a loop of repeating tracks. If a track was found in the playlist
                // that already exists in the album, stop.
                if album.tracks.len() < playlist.tracks.items.len() {
                    let mut tn = last_tn;
                    for mut t in playlist.tracks.items.into_iter().skip(album.tracks.len()) {
                        if album.tracks.iter().any(|at| at.id == t.id) {
                            break;
                        }
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

struct MusicBrowseFields<'a> {
    contents: Option<JsonNode<'a>>,
    header: Option<JsonNode<'a>>,
    microformat: MusicMicroformat,
}

fn music_root_header_node<'a>(root: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    root.query(ytq!(.header))
        .and_then(|header| super::response::music_item::music_header_node(&header))
}

fn music_two_column_header_node<'a>(root: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    root.query(ytq!(
        .contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
            .sectionListRenderer.contents[0]
    ))
    .and_then(|header| super::response::music_item::music_header_node(&header))
}

fn deserialize_music_browse_fields<'a>(
    root: &JsonNode<'a>,
) -> Result<MusicBrowseFields<'a>, ExtractionError> {
    Ok(MusicBrowseFields {
        contents: root.query(ytq!(.contents)),
        header: music_root_header_node(root),
        microformat: root
            .query(ytq!(.microformat))
            .map(|node| node.deserialize())
            .transpose()?
            .unwrap_or_default(),
    })
}

fn music_browse_sections<'a>(
    contents: &JsonNode<'a>,
    root: &JsonNode<'a>,
    header: Option<JsonNode<'a>>,
) -> Result<(Option<JsonNode<'a>>, JsonNode<'a>), ExtractionError> {
    if let Some(section_list) = contents.query(ytq!(
        .singleColumnBrowseResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content.sectionListRenderer
    )) {
        return Ok((header, section_list));
    }

    if let Some(section_list) =
        contents.query(ytq!(.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer))
    {
        return Ok((music_two_column_header_node(root).or(header), section_list));
    }

    Err(ExtractionError::InvalidData(Cow::Borrowed("no content")))
}

fn map_music_playlist_fields(
    fields: MusicBrowseFields<'_>,
    root: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
    let contents = match fields.contents {
        Some(c) => c,
        None => {
            if fields.microformat.microformat_data_renderer.noindex {
                return Err(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: "no contents".into(),
                });
            } else {
                return Err(ExtractionError::InvalidData("no contents".into()));
            }
        }
    };

    let (header, music_contents) = music_browse_sections(&contents, root, fields.header)?;
    let shelf = music_contents
        .query(ytq!(.contents))
        .into_iter()
        .flat_map(|contents| contents.items())
        .filter_map(|section| {
            section
                .deserialize::<JsonValue>()
                .ok()
                .and_then(|section| music_shelf_from_value(&section))
        })
        .next()
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "no sectionListRenderer content",
        )))?;

    if let Some(playlist_id) = shelf.playlist_id {
        if playlist_id != ctx.id {
            return Err(crate::client::check_id_matches(
                &playlist_id,
                ctx.id,
                "playlist",
            ));
        }
    }

    let (map_res, mapped_ctoken) = map_music_items_value(shelf.contents, ctx.lang);
    let ctoken = mapped_ctoken.or_else(|| {
        shelf
            .continuations
            .into_iter()
            .next()
            .map(|cont| cont.next_continuation_data.continuation)
    });
    let track_count = if ctoken.is_some() {
        header.as_ref().and_then(|h| {
            let second_subtitle =
                super::response::music_item::music_header_second_subtitle(h);
            let parts = second_subtitle
                .split(|p| p == DOT_SEPARATOR)
                .collect::<Vec<_>>();
            parts
                .get(usize::from(parts.len() > 2))
                .and_then(|txt| txt.first())
                .and_then(|txt| util::parse_numeric::<u64>(txt).ok())
        })
    } else {
        Some(map_res.c.len() as u64)
    };

    let related_ctoken = music_contents
        .query(ytq!(.continuations[0].nextContinuationData.continuation))
        .and_then(|node| node.as_str());

    let (from_ytm, channel, name, thumbnail, description) = match header {
        Some(header) => {
            let facepile = header.query(ytq!(.facepile)).and_then(|node| {
                node.query(ytq!(.avatarStackViewModel))
                    .and_then(|node| node.deserialize::<AvatarStackViewModel>().ok())
                    .or_else(|| node.deserialize::<AvatarStackViewModel>().ok())
            });
            let (from_ytm, channel) = match facepile {
                Some(facepile) => {
                    let from_ytm = facepile.text.starts_with("YouTube");
                    let channel = facepile
                        .renderer_context
                        .command_context
                        .and_then(|c| {
                            c.get("onTap")
                                .and_then(|on_tap| on_tap.get("innertubeCommand"))
                                .and_then(url_endpoint::music_page)
                                .filter(|p| p.typ == MusicPageType::User)
                                .map(|p| p.id)
                        })
                        .map(|id| ChannelId {
                            id,
                            name: facepile.text,
                        });
                    (from_ytm && channel.is_none(), channel)
                }
                None => {
                    let st = super::response::music_item::music_header_components(
                        &header,
                        ytq!(.straplineTextOne),
                    )
                    .or_else(|| {
                        super::response::music_item::music_header_components(
                            &header,
                            ytq!(.subtitle),
                        )
                    })
                    .unwrap_or_default();

                    let from_ytm = st.0.iter().any(util::is_ytm);
                    let channel = st.0.into_iter().find_map(|c| ChannelId::try_from(c).ok());
                    (from_ytm, channel)
                }
            };
            (
                from_ytm,
                channel,
                super::response::music_item::music_header_text(&header, ytq!(.title)).ok_or(
                    ExtractionError::InvalidData(Cow::Borrowed("no music playlist title")),
                )?,
                super::response::music_item::music_header_thumbnail(&header),
                super::response::music_item::music_header_description(&header),
            )
        }
        None => {
            // Album playlists fetched via the playlist method dont include a header
            let (album, cover) = map_res
                .c
                .iter()
                .find_map(|t: &TrackItem| t.album.as_ref().map(|a| (a.clone(), t.cover.clone())))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "playlist without header or album items",
                )))?;

            if !map_res.c.iter().all(|t| {
                t.unavailable
                    || t.album
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
            description: description.map(Into::into),
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

impl MapEndpoint<MusicPlaylist> for MusicPlaylistEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_music_browse_fields(&root)?;
            map_music_playlist_fields(fields, &root, ctx)
        })
    }
}

fn map_music_album_fields(
    fields: MusicBrowseFields<'_>,
    root: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<MusicAlbum>, ExtractionError> {
    let contents = match fields.contents {
        Some(c) => c,
        None => {
            if fields.microformat.microformat_data_renderer.noindex {
                return Err(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: "no contents".into(),
                });
            } else {
                return Err(ExtractionError::InvalidData("no contents".into()));
            }
        }
    };

    let (header, music_contents) = music_browse_sections(&contents, root, fields.header)?;
    let sections = music_contents
        .query(ytq!(.contents))
        .map(|node| node.items())
        .unwrap_or_default();
    let header = header.ok_or(ExtractionError::InvalidData(Cow::Borrowed("no header")))?;

    let mut shelf = None;
    let mut album_variants = None;
    for section in sections {
        let section_value = || section.deserialize::<JsonValue>().ok();
        if let Some(sh) = section_value().and_then(|section| music_shelf_from_value(&section)) {
            shelf = Some(sh);
        } else if let Some(sh) =
            section_value().and_then(|section| music_carousel_from_value(&section))
        {
            let is_album_versions = sh
                .header
                .as_ref()
                .map(|h| h.title.first_str() == dictionary::entry(ctx.lang).album_versions_title)
                .unwrap_or_default();
            if is_album_versions {
                album_variants = Some(sh.contents);
            }
        }
    }
    let shelf = shelf.ok_or(ExtractionError::InvalidData(Cow::Borrowed(
        "no sectionListRenderer content",
    )))?;

    let subtitle = super::response::music_item::music_header_components(&header, ytq!(.subtitle))
        .unwrap_or_default();
    let strapline_text_one =
        super::response::music_item::music_header_components(&header, ytq!(.straplineTextOne));
    let mut subtitle_split = subtitle.split(util::DOT_SEPARATOR);

    let (year_txt, artists_p) = match strapline_text_one {
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

    fn map_playlist_id(ep: &JsonValue) -> Option<String> {
        url_endpoint::watch_playlist_endpoint(ep).map(|endpoint| endpoint.playlist_id)
    }

    let playlist_id = fields
        .microformat
        .microformat_data_renderer
        .url_canonical
        .and_then(|x| {
            x.strip_prefix("https://music.youtube.com/playlist?list=")
                .map(str::to_owned)
        });
    let (playlist_id, artist_id) = super::response::music_item::music_header_menu(&header)
        .map(|menu| {
            (
                playlist_id.or_else(|| {
                    menu.get("menuRenderer")
                        .and_then(|renderer| renderer.get("topLevelButtons"))
                        .and_then(|buttons| buttons.as_array())
                        .into_iter()
                        .flat_map(|items| items.iter())
                        .filter_map(|button| {
                            button
                                .get("buttonRenderer")
                                .and_then(|button| button.get("navigationEndpoint"))
                        })
                        .find_map(map_playlist_id)
                        .or_else(|| {
                            menu.get("menuRenderer")
                                .and_then(|renderer| renderer.get("items"))
                                .and_then(|items| items.as_array())
                                .into_iter()
                                .flat_map(|items| items.iter())
                                .filter_map(|item| {
                                    item.get("menuNavigationItemRenderer")
                                        .and_then(|item| item.get("navigationEndpoint"))
                                })
                                .find_map(map_playlist_id)
                        })
                }),
                map_artist_id(&menu),
            )
        })
        .unwrap_or_default();
    let artist_id = artist_id.or_else(|| artists.first().and_then(|a| a.id.clone()));

    let second_subtitle =
        super::response::music_item::music_header_second_subtitle(&header);
    let second_subtitle_parts = second_subtitle
        .split(|p| p == DOT_SEPARATOR)
        .collect::<Vec<_>>();
    let track_count = second_subtitle_parts
        .get(usize::from(second_subtitle_parts.len() > 2))
        .and_then(|txt| txt.first())
        .and_then(|txt| util::parse_numeric::<u16>(txt).ok());

    let album_title = super::response::music_item::music_header_text(&header, ytq!(.title))
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no album title")))?;
    let (tracks_res, _) = map_album_track_items(
        shelf.contents,
        ctx.lang,
        artists.clone(),
        by_va,
        AlbumId {
            id: ctx.id.to_owned(),
            name: album_title.clone(),
        },
    );
    let mut warnings = tracks_res.warnings;

    let mut variants_res = album_variants
        .map(|res| map_music_items_value(res, ctx.lang).0)
        .unwrap_or_default();
    warnings.append(&mut variants_res.warnings);

    Ok(MapResult {
        c: MusicAlbum {
            id: ctx.id.to_owned(),
            playlist_id,
            name: album_title,
            cover: super::response::music_item::music_header_thumbnail(&header),
            artists,
            artist_id,
            description: super::response::music_item::music_header_description(&header)
                .map(Into::into),
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

impl MapEndpoint<MusicAlbum> for MusicPlaylistEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicAlbum>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_music_browse_fields(&root)?;
            map_music_album_fields(fields, &root, ctx)
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
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    #[case::two_columns("20240228_twoColumns", "RDCLAK5uy_kb7EBi6y3GrtJri4_ZH56Ms786DFEimbM")]
    #[case::n_album("20240228_album", "OLAK5uy_kdSWBZ-9AZDkYkuy0QCc3p0KO9DEHVNH0")]
    #[case::facepile("20241125_facepile", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_music_playlist(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_playlist" / format!("playlist_{name}.json"));
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<model::MusicPlaylist> =
            MusicPlaylistEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<model::MusicAlbum> =
            MusicPlaylistEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_album_{name}"), map_res.c);
    }
}
