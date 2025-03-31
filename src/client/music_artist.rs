use std::borrow::Cow;

use once_cell::sync::Lazy;
use regex::Regex;
use tracing::debug;

use crate::{
    client::{
        response::{music_item::map_album_type, url_endpoint::NavigationEndpoint},
        MapRespOptions, QContinuation,
    },
    error::{Error, ExtractionError},
    model::{
        paginator::Paginator, traits::FromYtItem, AlbumItem, AlbumType, ArtistId, MusicArtist,
        MusicItem,
    },
    param::{AlbumFilter, AlbumOrder},
    serializer::MapResult,
    util::{self, ProtoBuilder},
};

use super::{
    response::{self, music_item::MusicListMapper, url_endpoint::PageType},
    ClientType, MapRespCtx, MapResponse, QBrowse, QBrowseParams, RustyPipeQuery,
};

impl RustyPipeQuery {
    /// Get a YouTube Music artist page
    ///
    /// Set `all_albums` to [`true`] if you want to fetch the albums behind the *More* buttons, too.
    pub async fn music_artist<S: AsRef<str>>(
        &self,
        artist_id: S,
        all_albums: bool,
    ) -> Result<MusicArtist, Error> {
        let artist_id = artist_id.as_ref();
        let res = self._music_artist(artist_id, all_albums).await;

        if let Err(Error::Extraction(ExtractionError::Redirect(id))) = res {
            debug!("music artist {} redirects to {}", artist_id, &id);
            self._music_artist(&id, all_albums).await
        } else {
            res
        }
    }

    async fn _music_artist(&self, artist_id: &str, all_albums: bool) -> Result<MusicArtist, Error> {
        let request_body = QBrowse {
            browse_id: artist_id,
        };

        if all_albums {
            let (mut artist, can_fetch_more) = self
                .execute_request::<response::MusicArtist, _, _>(
                    ClientType::DesktopMusic,
                    "music_artist",
                    artist_id,
                    "browse",
                    &request_body,
                )
                .await?;

            if can_fetch_more {
                artist.albums = self
                    .music_artist_albums(artist_id, None, Some(AlbumOrder::Recency))
                    .await?;
            }

            Ok(artist)
        } else {
            self.execute_request::<response::MusicArtist, _, _>(
                ClientType::DesktopMusic,
                "music_artist",
                artist_id,
                "browse",
                &request_body,
            )
            .await
        }
    }

    /// Get a list of all albums of a YouTube Music artist
    pub async fn music_artist_albums(
        &self,
        artist_id: &str,
        filter: Option<AlbumFilter>,
        order: Option<AlbumOrder>,
    ) -> Result<Vec<AlbumItem>, Error> {
        let request_body = QBrowseParams {
            browse_id: &format!("{}{}", util::ARTIST_DISCOGRAPHY_PREFIX, artist_id),
            params: &albums_param(filter, order),
        };

        let first_page = self
            .execute_request::<response::MusicArtistAlbums, _, _>(
                ClientType::DesktopMusic,
                "music_artist_albums",
                artist_id,
                "browse",
                &request_body,
            )
            .await?;

        let mut albums = first_page.albums;
        let mut ctoken = first_page.ctoken;

        while let Some(tkn) = &ctoken {
            let request_body = QContinuation { continuation: tkn };
            let resp: Paginator<MusicItem> = self
                .execute_request_ctx::<response::MusicContinuation, Paginator<MusicItem>, _>(
                    ClientType::DesktopMusic,
                    "music_artist_albums_cont",
                    artist_id,
                    "browse",
                    &request_body,
                    MapRespOptions {
                        artist: Some(first_page.artist.clone()),
                        visitor_data: first_page.visitor_data.as_deref(),
                        ..Default::default()
                    },
                )
                .await?;
            if resp.items.is_empty() {
                tracing::warn!("artist albums [{artist_id}] empty continuation");
            }
            ctoken = resp.ctoken;
            albums.extend(resp.items.into_iter().filter_map(AlbumItem::from_ytm_item));
        }
        Ok(albums)
    }
}

impl MapResponse<MusicArtist> for response::MusicArtist {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<MusicArtist>, ExtractionError> {
        let mapped = map_artist_page(self, ctx, false)?;
        Ok(MapResult {
            c: mapped.c.0,
            warnings: mapped.warnings,
        })
    }
}

impl MapResponse<(MusicArtist, bool)> for response::MusicArtist {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<(MusicArtist, bool)>, ExtractionError> {
        map_artist_page(self, ctx, true)
    }
}

fn map_artist_page(
    res: response::MusicArtist,
    ctx: &MapRespCtx<'_>,
    skip_extendables: bool,
) -> Result<MapResult<(MusicArtist, bool)>, ExtractionError> {
    let contents = match res.contents {
        Some(c) => c,
        None => {
            if res.microformat.microformat_data_renderer.noindex {
                return Err(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: "no contents".into(),
                });
            } else {
                return Err(ExtractionError::InvalidData("no contents".into()));
            }
        }
    };

    let header = res
        .header
        .ok_or(ExtractionError::InvalidData("no header".into()))?
        .music_immersive_header_renderer;

    if let Some(share) = header.share_endpoint {
        let pb = share.share_entity_endpoint.serialized_share_entity;

        let share_channel_id = urlencoding::decode(&pb)
            .ok()
            .and_then(|pb| util::b64_decode(pb.as_bytes()).ok())
            .and_then(|pb| util::string_from_pb(pb, 3));

        if let Some(share_channel_id) = share_channel_id {
            if share_channel_id != ctx.id {
                return Err(ExtractionError::Redirect(share_channel_id));
            }
        }
    }

    let sections = contents
        .single_column_browse_results_renderer
        .contents
        .into_iter()
        .next()
        .map(|c| c.tab_renderer.content.section_list_renderer.contents)
        .unwrap_or_default();

    let mut mapper = MusicListMapper::with_artist(
        ctx.lang,
        ArtistId {
            id: Some(ctx.id.to_owned()),
            name: header.title.clone(),
        },
    );

    let mut tracks_playlist_id = None;
    let mut videos_playlist_id = None;
    let mut can_fetch_more = false;

    for section in sections {
        match section {
            response::music_item::ItemSection::MusicShelfRenderer(shelf) => {
                if tracks_playlist_id.is_none() {
                    if let Some(ep) = shelf.bottom_endpoint {
                        if let Some(cfg) =
                            ep.browse_endpoint.browse_endpoint_context_supported_configs
                        {
                            if cfg.browse_endpoint_context_music_config.page_type
                                == PageType::Playlist
                            {
                                tracks_playlist_id = Some(ep.browse_endpoint.browse_id);
                            }
                        }
                    }
                }
                mapper.album_type = AlbumType::Single;
                mapper.map_response(shelf.contents);
            }
            response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf) => {
                let mut extendable_albums = false;
                mapper.album_type = AlbumType::Single;
                if let Some(h) = shelf.header {
                    if let Some(button) = h
                        .music_carousel_shelf_basic_header_renderer
                        .more_content_button
                    {
                        if let NavigationEndpoint::Browse {
                            browse_endpoint, ..
                        } = button.button_renderer.navigation_endpoint
                        {
                            // Music videos
                            if browse_endpoint
                                .browse_endpoint_context_supported_configs
                                .map(|cfg| {
                                    cfg.browse_endpoint_context_music_config.page_type
                                        == PageType::Playlist
                                })
                                .unwrap_or_default()
                            {
                                if videos_playlist_id.is_none() {
                                    videos_playlist_id = Some(browse_endpoint.browse_id);
                                }
                            } else if browse_endpoint
                                .browse_id
                                .starts_with(util::ARTIST_DISCOGRAPHY_PREFIX)
                            {
                                can_fetch_more = true;
                                extendable_albums = true;
                            } else {
                                // Peek at the first item to determine type
                                if let Some(response::music_item::MusicResponseItem::MusicTwoRowItemRenderer(item)) = shelf.contents.c.first() {
                                    if let Some(PageType::Album) = item.navigation_endpoint.page_type() {
                                        can_fetch_more = true;
                                        extendable_albums = true;
                                    }
                                }
                            }
                        }
                    }
                    mapper.album_type = map_album_type(
                        h.music_carousel_shelf_basic_header_renderer
                            .title
                            .first_str(),
                        ctx.lang,
                    );
                }

                if !skip_extendables || !extendable_albums {
                    mapper.map_response(shelf.contents);
                }
            }
            _ => {}
        }
    }

    let mut mapped = mapper.group_items();

    static WIKIPEDIA_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\(?https://[a-z\d-]+\.wikipedia.org/wiki/[^\s]+").unwrap());
    let wikipedia_url = header.description.as_deref().and_then(|h| {
        WIKIPEDIA_REGEX.captures(h).and_then(|c| c.get(0)).map(|m| {
            let m = m.as_str();
            match m.strip_prefix('(') {
                Some(m) => match m.strip_suffix(')') {
                    Some(m) => m.to_owned(),
                    None => m.to_owned(),
                },
                None => m.to_owned(),
            }
        })
    });

    let radio_id = header.start_radio_button.and_then(|b| {
        if let NavigationEndpoint::Watch { watch_endpoint } = b.button_renderer.navigation_endpoint
        {
            watch_endpoint.playlist_id
        } else {
            None
        }
    });

    Ok(MapResult {
        c: (
            MusicArtist {
                id: ctx.id.to_owned(),
                name: header.title,
                header_image: header.thumbnail.into(),
                description: header.description,
                wikipedia_url,
                subscriber_count: header.subscription_button.and_then(|btn| {
                    util::parse_large_numstr_or_warn(
                        &btn.subscribe_button_renderer.subscriber_count_text,
                        ctx.lang,
                        &mut mapped.warnings,
                    )
                }),
                tracks: mapped.c.tracks,
                albums: mapped.c.albums,
                playlists: mapped.c.playlists,
                similar_artists: mapped.c.artists,
                tracks_playlist_id,
                videos_playlist_id,
                radio_id,
            },
            can_fetch_more,
        ),
        warnings: mapped.warnings,
    })
}

#[derive(Debug)]
struct FirstAlbumPage {
    albums: Vec<AlbumItem>,
    ctoken: Option<String>,
    artist: ArtistId,
    visitor_data: Option<String>,
}

impl MapResponse<FirstAlbumPage> for response::MusicArtistAlbums {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<FirstAlbumPage>, ExtractionError> {
        let Some(header) = self.header else {
            return Err(ExtractionError::NotFound {
                id: ctx.id.into(),
                msg: "no header".into(),
            });
        };

        let grids = self
            .contents
            .single_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let artist_id = ArtistId {
            id: Some(ctx.id.to_owned()),
            name: header.music_header_renderer.title,
        };
        let mut mapper = MusicListMapper::with_artist(ctx.lang, artist_id.clone());
        let mut ctoken = None;
        for grid in grids {
            mapper.map_response(grid.grid_renderer.items);
            if ctoken.is_none() {
                ctoken = grid
                    .grid_renderer
                    .continuations
                    .into_iter()
                    .next()
                    .map(|g| g.next_continuation_data.continuation);
            }
        }

        let mapped = mapper.group_items();

        Ok(MapResult {
            c: FirstAlbumPage {
                albums: mapped.c.albums,
                ctoken,
                artist: artist_id,
                visitor_data: ctx.visitor_data.map(str::to_owned),
            },
            warnings: mapped.warnings,
        })
    }
}

fn albums_param(filter: Option<AlbumFilter>, order: Option<AlbumOrder>) -> String {
    let mut pb_filter = ProtoBuilder::new();
    if let Some(filter) = filter {
        pb_filter.varint(1, filter as u64);
    }
    if let Some(order) = order {
        pb_filter.varint(2, order as u64);
    }
    pb_filter.bytes(3, &[1, 2]);

    let mut pb_48 = ProtoBuilder::new();
    pb_48.embedded(15, pb_filter);

    let mut pb_3 = ProtoBuilder::new();
    pb_3.embedded(48, pb_48);
    pb_3.to_base64()
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::util::tests::TESTFILES;

    use super::*;

    #[rstest]
    #[case::default("default", "UClmXPfaYhXOYsNn_QUyheWQ")]
    #[case::only_singles("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw")]
    #[case::no_artist("no_artist", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::only_more_singles("only_more_singles", "UC0aXrjVxG5pZr99v77wZdPQ")]
    #[case::grouped_albums("20250113_grouped_albums", "UCOR4_bSVIXPsGa4BbCSt60Q")]
    fn map_music_artist(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_artist" / format!("artist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let mut album_page_path = None;
        let json_path = path!(*TESTFILES / "music_artist" / format!("artist_{name}_1.json"));
        if json_path.exists() {
            album_page_path = Some(json_path);
        }

        let resp: response::MusicArtist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<(MusicArtist, bool)> =
            resp.map_response(&MapRespCtx::test(id)).unwrap();
        let (mut artist, can_fetch_more) = map_res.c;

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        assert_eq!(can_fetch_more, album_page_path.is_some());

        // Album overview
        if let Some(album_page_path) = album_page_path {
            let json_file = File::open(album_page_path).unwrap();
            let resp: response::MusicArtistAlbums =
                serde_json::from_reader(BufReader::new(json_file)).unwrap();
            let map_res: MapResult<FirstAlbumPage> =
                resp.map_response(&MapRespCtx::test(id)).unwrap();

            assert!(
                map_res.warnings.is_empty(),
                "deserialization/mapping warnings: {:?}",
                map_res.warnings
            );
            artist.albums = map_res.c.albums;

            // Album overview continuation
            for i in 2..10 {
                let cont_path =
                    path!(*TESTFILES / "music_artist" / format!("artist_{name}_{i}.json"));
                if !cont_path.is_file() {
                    break;
                }
                let json_file = File::open(cont_path).unwrap();
                let resp: response::MusicContinuation =
                    serde_json::from_reader(BufReader::new(json_file)).unwrap();
                let map_res: MapResult<Paginator<MusicItem>> =
                    resp.map_response(&MapRespCtx::test(id)).unwrap();
                assert!(!map_res.c.items.is_empty());
                artist.albums.extend(
                    map_res
                        .c
                        .items
                        .into_iter()
                        .filter_map(AlbumItem::from_ytm_item),
                );
            }
        }

        insta::assert_ron_snapshot!(format!("map_music_artist_{name}"), artist);
    }

    #[test]
    fn map_music_artist_no_cont() {
        let json_path = path!(*TESTFILES / "music_artist" / "artist_default.json");
        let json_file = File::open(json_path).unwrap();

        let artist: response::MusicArtist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicArtist> = artist
            .map_response(&MapRespCtx::test("UClmXPfaYhXOYsNn_QUyheWQ"))
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(map_res.c);
    }

    #[test]
    fn map_music_artist_secondary_channel() {
        let json_path = path!(*TESTFILES / "music_artist" / "artist_secondary_channel.json");
        let json_file = File::open(json_path).unwrap();

        let artist: response::MusicArtist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let res: Result<MapResult<MusicArtist>, ExtractionError> =
            artist.map_response(&MapRespCtx::test("UCLkAepWjdylmXSltofFvsYQ"));
        let e = res.unwrap_err();

        match e {
            ExtractionError::Redirect(id) => {
                assert_eq!(id, "UCOR4_bSVIXPsGa4BbCSt60Q");
            }
            _ => panic!("error: {e}"),
        }
    }
}
