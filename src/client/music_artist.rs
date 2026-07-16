use std::borrow::Cow;

use once_cell::sync::Lazy;
use regex::Regex;
use tracing::debug;

use crate::{
    client::{
        response::{music_item::map_album_type, url_endpoint},
        MapRespOptions,
    },
    error::{Error, ExtractionError},
    json::{ytq, JsonDoc, JsonNode, JsonValue},
    model::{
        paginator::Paginator, traits::FromYtItem, AlbumItem, AlbumType, ArtistId, MusicArtist,
        MusicItem,
    },
    param::{AlbumFilter, AlbumOrder, MusicArtistAlbums},
    request_body::ytbody,
    serializer::MapResult,
    util::{self, ProtoBuilder},
};

use super::{
    pagination::MusicContinuationMarker,
    response::{
        music_item::{
            map_grouped_music_items_values, music_carousel_from_value, music_shelf_from_value,
            GridRenderer, MusicMicroformat,
        },
        url_endpoint::PageType,
        SimpleHeaderRenderer,
    },
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicArtistEndpoint;

#[derive(Debug)]
struct MusicArtistAlbumsEndpoint;

impl RustyPipeQuery {
    /// Get a YouTube Music artist page
    ///
    /// Set `albums` to [`MusicArtistAlbums::Include`] if you want to fetch
    /// the albums behind the *More* buttons, too.
    pub async fn music_artist<S: AsRef<str>>(
        &self,
        artist_id: S,
        albums: MusicArtistAlbums,
    ) -> Result<MusicArtist, Error> {
        let all_albums = matches!(albums, MusicArtistAlbums::Include);
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
        let request_body = ytbody!({
            "browseId": artist_id,
        });

        if all_albums {
            let (mut artist, can_fetch_more) = self
                .execute_request::<MusicArtistEndpoint, _, _>(
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
            self.execute_request::<MusicArtistEndpoint, _, _>(
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
        let request_body = ytbody!({
            "browseId": format!("{}{}", util::ARTIST_DISCOGRAPHY_PREFIX, artist_id),
            "params": albums_param(filter, order),
        });

        let first_page = self
            .execute_request::<MusicArtistAlbumsEndpoint, _, _>(
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
            let request_body = ytbody!({
                "continuation": tkn,
            });
            let resp: Paginator<MusicItem> = self
                .execute_request_ctx::<MusicContinuationMarker, Paginator<MusicItem>, _>(
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

struct MusicArtistFields<'a> {
    sections: Option<JsonNode<'a>>,
    header: Option<JsonNode<'a>>,
    microformat: MusicMicroformat,
}

fn deserialize_music_artist_fields<'a>(
    root: &JsonNode<'a>,
) -> Result<MusicArtistFields<'a>, ExtractionError> {
    Ok(MusicArtistFields {
        sections: root.query(ytq!(
            .contents.singleColumnBrowseResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content.sectionListRenderer.contents
        )),
        header: root.query(ytq!(.header)).and_then(|node| {
            node.query(ytq!(.musicImmersiveHeaderRenderer || .musicVisualHeaderRenderer))
        }),
        microformat: root
            .query(ytq!(.microformat))
            .map(|node| node.deserialize())
            .transpose()?
            .unwrap_or_default(),
    })
}

impl MapEndpoint<MusicArtist> for MusicArtistEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicArtist>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_music_artist_fields(&root)?;
            let mapped = map_artist_page(fields, ctx, false)?;
            Ok(MapResult {
                c: mapped.c.0,
                warnings: mapped.warnings,
            })
        })
    }
}

impl MapEndpoint<(MusicArtist, bool)> for MusicArtistEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<(MusicArtist, bool)>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_music_artist_fields(&root)?;
            map_artist_page(fields, ctx, true)
        })
    }
}

fn map_artist_page(
    fields: MusicArtistFields<'_>,
    ctx: &MapRespCtx<'_>,
    skip_extendables: bool,
) -> Result<MapResult<(MusicArtist, bool)>, ExtractionError> {
    let sections = match fields.sections {
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

    let header = fields
        .header
        .ok_or(ExtractionError::InvalidData("no header".into()))?;
    let title = header
        .text_at(ytq!(.title))
        .ok_or(ExtractionError::InvalidData("no artist title".into()))?;
    let description = header.text_at(ytq!(.description));

    if let Some(pb) = header
        .query(ytq!(.shareEndpoint.shareEntityEndpoint.serializedShareEntity))
        .and_then(|node| node.as_str())
    {
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

    let mut tracks_playlist_id = None;
    let mut videos_playlist_id = None;
    let mut can_fetch_more = false;
    let artist = ArtistId {
        id: Some(ctx.id.to_owned()),
        name: title.clone(),
    };
    let mut grouped_values = Vec::new();

    for section in sections.items() {
        let section_value = || section.deserialize::<JsonValue>().ok();
        if let Some(shelf) = section_value().and_then(|section| music_shelf_from_value(&section)) {
            if tracks_playlist_id.is_none() {
                if let Some(ep) = shelf.bottom_endpoint {
                    if let Some(ep) = url_endpoint::browse_endpoint(&ep) {
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
            }
            grouped_values.push((shelf.contents, AlbumType::Single));
        } else if let Some(shelf) =
            section_value().and_then(|section| music_carousel_from_value(&section))
        {
            let mut extendable_albums = false;
            let mut album_type = AlbumType::Single;
            if let Some(h) = shelf.header {
                if let Some(button) = h.more_content_button {
                    if let Some(ep) = button
                        .get("buttonRenderer")
                        .and_then(|button| button.get("navigationEndpoint"))
                        .and_then(url_endpoint::browse_endpoint)
                    {
                        let browse_endpoint = ep.browse_endpoint;
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
                            if let Some(item) = shelf
                                .contents
                                .as_array()
                                .and_then(|items| items.first())
                                .and_then(|item| item.get("musicTwoRowItemRenderer"))
                                .and_then(|item| item.get("navigationEndpoint"))
                            {
                                if let Some(PageType::Album) = url_endpoint::page_type(item) {
                                    can_fetch_more = true;
                                    extendable_albums = true;
                                }
                            }
                        }
                    }
                }
                album_type = map_album_type(h.title.first_str(), ctx.lang);
            }

            if !skip_extendables || !extendable_albums {
                grouped_values.push((shelf.contents, album_type));
            }
        }
    }

    let mut mapped = map_grouped_music_items_values(grouped_values, ctx.lang, Some(artist)).0;

    static WIKIPEDIA_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\(?https://[a-z\d-]+\.wikipedia.org/wiki/[^\s]+").unwrap());
    let wikipedia_url = description.as_deref().and_then(|h| {
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

    let radio_id = header
        .query(ytq!(.startRadioButton))
        .and_then(|node| node.query(ytq!(.buttonRenderer.navigationEndpoint)))
        .and_then(|node| node.deserialize::<JsonValue>().ok())
        .and_then(|endpoint| {
            url_endpoint::watch_endpoint(&endpoint)
                .and_then(|watch_endpoint| watch_endpoint.playlist_id)
        });
    let subscriber_count = header
        .text_at(ytq!(
            .subscriptionButton.subscribeButtonRenderer.subscriberCountText
        ))
        .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut mapped.warnings));
    let header_image = header.query_thumbnails(ytq!(
        .thumbnail.(.musicThumbnailRenderer || .croppedSquareThumbnailRenderer).thumbnail
    ));

    Ok(MapResult {
        c: (
            MusicArtist {
                id: ctx.id.to_owned(),
                name: title,
                header_image,
                description,
                wikipedia_url,
                subscriber_count,
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

struct FirstAlbumPage {
    albums: Vec<AlbumItem>,
    ctoken: Option<String>,
    artist: ArtistId,
    visitor_data: Option<String>,
}

struct MusicArtistAlbumsFields<'a> {
    header: Option<SimpleHeaderRenderer>,
    grids: JsonNode<'a>,
}

fn deserialize_music_artist_albums_fields<'a>(
    root: &JsonNode<'a>,
) -> Result<MusicArtistAlbumsFields<'a>, ExtractionError> {
    Ok(MusicArtistAlbumsFields {
        header: root
            .query(ytq!(.header))
            .and_then(|node| node.query(ytq!(.musicHeaderRenderer)))
            .map(|node| node.deserialize())
            .transpose()?,
        grids: root
            .query(ytq!(
                .contents.singleColumnBrowseResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content.sectionListRenderer.contents
            ))
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?,
    })
}

impl MapEndpoint<FirstAlbumPage> for MusicArtistAlbumsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<FirstAlbumPage>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_music_artist_albums_fields(&root)?;
            map_first_album_page(fields, ctx)
        })
    }
}

fn map_first_album_page(
    fields: MusicArtistAlbumsFields<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<FirstAlbumPage>, ExtractionError> {
    let Some(header) = fields.header else {
        return Err(ExtractionError::NotFound {
            id: ctx.id.into(),
            msg: "no header".into(),
        });
    };

    let artist_id = ArtistId {
        id: Some(ctx.id.to_owned()),
        name: header.title,
    };
    let mut ctoken = None;
    let mut grouped_values = Vec::new();
    for grid_node in fields.grids.items() {
        let Some(grid) = grid_node
            .query(ytq!(.gridRenderer))
            .and_then(|node| node.deserialize::<GridRenderer>().ok())
        else {
            continue;
        };
        grouped_values.push((grid.items, AlbumType::Single));
        if ctoken.is_none() {
            ctoken = grid
                .continuations
                .into_iter()
                .next()
                .map(|g| g.next_continuation_data.continuation);
        }
    }

    let mapped =
        map_grouped_music_items_values(grouped_values, ctx.lang, Some(artist_id.clone())).0;

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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());

        let mut album_page_path = None;
        let json_path = path!(*TESTFILES / "music_artist" / format!("artist_{name}_1.json"));
        if json_path.exists() {
            album_page_path = Some(json_path);
        }

        let map_res: MapResult<(MusicArtist, bool)> =
            MusicArtistEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();
        let (mut artist, can_fetch_more) = map_res.c;

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        assert_eq!(can_fetch_more, album_page_path.is_some());

        // Album overview
        if let Some(album_page_path) = album_page_path {
            let json = JsonDoc::new(std::fs::read_to_string(album_page_path).unwrap());
            let map_res: MapResult<FirstAlbumPage> =
                MusicArtistAlbumsEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

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
                let json = JsonDoc::new(std::fs::read_to_string(cont_path).unwrap());
                let map_res: MapResult<Paginator<MusicItem>> =
                    MusicContinuationMarker::map(&json, &MapRespCtx::test(id)).unwrap();
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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());

        let map_res: MapResult<MusicArtist> = MusicArtistEndpoint::map(
            &json,
            &MapRespCtx::test("UClmXPfaYhXOYsNn_QUyheWQ"),
        )
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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());

        let res: Result<MapResult<MusicArtist>, ExtractionError> =
            MusicArtistEndpoint::map(
                &json,
                &MapRespCtx::test("UCLkAepWjdylmXSltofFvsYQ"),
            );
        let e = res.unwrap_err();

        match e {
            ExtractionError::Redirect(id) => {
                assert_eq!(id, "UCOR4_bSVIXPsGa4BbCSt60Q");
            }
            _ => panic!("error: {e}"),
        }
    }
}
