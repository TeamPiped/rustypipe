use std::{borrow::Cow, rc::Rc};

use fancy_regex::Regex;
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{AlbumItem, ArtistId, MusicArtist},
    serializer::MapResult,
    util::{self, TryRemove},
};

use super::{
    response::{self, music_item::MusicListMapper, url_endpoint::PageType},
    ClientType, MapResponse, QBrowse, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowseParams<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
    params: &'a str,
}

impl RustyPipeQuery {
    pub async fn music_artist(
        &self,
        artist_id: &str,
        all_albums: bool,
    ) -> Result<MusicArtist, Error> {
        if all_albums {
            let visitor_data = self.get_ytm_visitor_data().await?;
            let context = self
                .get_context(ClientType::DesktopMusic, true, Some(&visitor_data))
                .await;
            let request_body = QBrowse {
                context,
                browse_id: artist_id,
            };

            let (mut artist, album_page_params) = self
                .execute_request::<response::MusicArtist, _, _>(
                    ClientType::DesktopMusic,
                    "music_artist",
                    artist_id,
                    "browse",
                    &request_body,
                )
                .await?;

            let visitor_data = Rc::new(visitor_data);
            let album_page_results = stream::iter(album_page_params)
                .map(|params| {
                    let visitor_data = visitor_data.clone();
                    async move {
                        self.music_artist_album_page(artist_id, &params, &visitor_data)
                            .await
                    }
                })
                .buffer_unordered(2)
                .collect::<Vec<_>>()
                .await;

            for res in album_page_results {
                let mut res = res?;
                artist.albums.append(&mut res);
            }

            Ok(artist)
        } else {
            let context = self.get_context(ClientType::DesktopMusic, true, None).await;
            let request_body = QBrowse {
                context,
                browse_id: artist_id,
            };

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

    async fn music_artist_album_page(
        &self,
        artist_id: &str,
        params: &str,
        visitor_data: &str,
    ) -> Result<Vec<AlbumItem>, Error> {
        let context = self
            .get_context(ClientType::DesktopMusic, true, Some(visitor_data))
            .await;
        let request_body = QBrowseParams {
            context,
            browse_id: artist_id,
            params,
        };

        self.execute_request::<response::MusicArtistAlbums, _, _>(
            ClientType::DesktopMusic,
            "music_artist_albums",
            artist_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<MusicArtist> for response::MusicArtist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicArtist>, ExtractionError> {
        let mapped = map_artist_page(self, id, lang, false)?;
        Ok(MapResult {
            c: mapped.c.0,
            warnings: mapped.warnings,
        })
    }
}

impl MapResponse<(MusicArtist, Vec<String>)> for response::MusicArtist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<(MusicArtist, Vec<String>)>, ExtractionError> {
        map_artist_page(self, id, lang, true)
    }
}

fn map_artist_page(
    res: response::MusicArtist,
    id: &str,
    lang: crate::param::Language,
    skip_extendables: bool,
) -> Result<MapResult<(MusicArtist, Vec<String>)>, ExtractionError> {
    // dbg!(&self);

    let header = res.header.music_immersive_header_renderer;

    let mut content = res.contents.single_column_browse_results_renderer.contents;
    let sections = content
        .try_swap_remove(0)
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
        .tab_renderer
        .content
        .section_list_renderer
        .contents;

    let mut mapper = MusicListMapper::with_artist(
        lang,
        ArtistId {
            id: Some(id.to_owned()),
            name: header.title.to_owned(),
        },
    );

    let mut tracks_playlist_id = None;
    let mut videos_playlist_id = None;
    let mut album_page_params = Vec::new();

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

                mapper.map_response(shelf.contents);
            }
            response::music_item::ItemSection::MusicCarouselShelfRenderer { header, contents } => {
                let mut extendable_albums = false;
                if let Some(h) = header {
                    if let Some(button) = h
                        .music_carousel_shelf_basic_header_renderer
                        .more_content_button
                    {
                        if let Some(bep) =
                            button.button_renderer.navigation_endpoint.browse_endpoint
                        {
                            if let Some(cfg) = bep.browse_endpoint_context_supported_configs {
                                match cfg.browse_endpoint_context_music_config.page_type {
                                    PageType::Playlist => {
                                        if videos_playlist_id.is_none() {
                                            videos_playlist_id = Some(bep.browse_id);
                                        }
                                    }
                                    PageType::Artist => {
                                        album_page_params.push(bep.params);
                                        extendable_albums = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                if !skip_extendables || !extendable_albums {
                    mapper.map_response(contents);
                }
            }
            response::music_item::ItemSection::None => {}
        }
    }

    let mapped = mapper.group_items();

    static WIKIPEDIA_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"https://[a-z]+\.wikipedia.org/wiki/[^()\s]+").unwrap());
    let wikipedia_url = WIKIPEDIA_REGEX
        .captures(&header.description)
        .ok()
        .flatten()
        .and_then(|c| c.get(0))
        .map(|m| m.as_str().to_owned());

    Ok(MapResult {
        c: (
            MusicArtist {
                id: id.to_owned(),
                name: header.title,
                header_image: header.thumbnail.into(),
                description: header.description,
                wikipedia_url,
                subscriber_count: header.subscription_button.and_then(|btn| {
                    util::parse_large_numstr(
                        &btn.subscribe_button_renderer.subscriber_count_text,
                        lang,
                    )
                }),
                tracks: mapped.c.tracks,
                albums: mapped.c.albums,
                playlists: mapped.c.playlists,
                similar_artists: mapped.c.artists,
                tracks_playlist_id,
                videos_playlist_id,
            },
            album_page_params,
        ),
        warnings: mapped.warnings,
    })
}

impl MapResponse<Vec<AlbumItem>> for response::MusicArtistAlbums {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Vec<AlbumItem>>, ExtractionError> {
        // dbg!(&self);

        let mut content = self.contents.single_column_browse_results_renderer.contents;
        let grids = content
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut mapper = MusicListMapper::with_artist(
            lang,
            ArtistId {
                id: Some(id.to_owned()),
                name: self.header.music_header_renderer.title,
            },
        );

        for grid in grids {
            mapper.map_response(grid.grid_renderer.items);
        }

        let mapped = mapper.group_items();

        Ok(MapResult {
            c: mapped.c.albums,
            warnings: mapped.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::param::Language;

    use super::*;

    #[rstest]
    #[case::default("default", "UClmXPfaYhXOYsNn_QUyheWQ")]
    #[case::no_more_albums("no_more_albums", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::only_singles("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw")]
    #[case::no_artist("no_artist", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::only_more_singles("only_more_singles", "UC0aXrjVxG5pZr99v77wZdPQ")]
    fn map_music_artist(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "music_artist" / format!("artist_{}.json", name));
        let json_file = File::open(json_path).unwrap();

        let mut album_page_paths = Vec::new();
        for i in 1..=2 {
            let json_path =
                path!("testfiles" / "music_artist" / format!("artist_{}_{}.json", name, i));
            if !json_path.exists() {
                break;
            }
            album_page_paths.push(json_path);
        }

        let resp: response::MusicArtist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<(MusicArtist, Vec<String>)> =
            resp.map_response(id, Language::En, None).unwrap();
        let (mut artist, album_page_params) = map_res.c;

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        assert_eq!(album_page_params.len(), album_page_paths.len());

        for json_path in album_page_paths {
            let json_file = File::open(json_path).unwrap();
            let resp: response::MusicArtistAlbums =
                serde_json::from_reader(BufReader::new(json_file)).unwrap();
            let mut map_res: MapResult<Vec<AlbumItem>> =
                resp.map_response(id, Language::En, None).unwrap();

            assert!(
                map_res.warnings.is_empty(),
                "deserialization/mapping warnings: {:?}",
                map_res.warnings
            );
            artist.albums.append(&mut map_res.c);
        }

        insta::assert_ron_snapshot!(format!("map_music_artist_{}", name), artist);
    }

    #[test]
    fn map_music_artist_no_cont() {
        let json_path = path!("testfiles" / "music_artist" / "artist_default.json");
        let json_file = File::open(json_path).unwrap();

        let artist: response::MusicArtist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicArtist> = artist
            .map_response("UClmXPfaYhXOYsNn_QUyheWQ", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(map_res.c);
    }
}
