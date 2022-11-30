use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::{AlbumId, ChannelId, MusicAlbum, MusicPlaylist, Paginator},
    serializer::MapResult,
    util::{self, TryRemove},
};

use super::{
    response::{
        self,
        music_item::{map_album_type, map_artists, MusicListMapper},
    },
    ClientType, MapResponse, QBrowse, RustyPipeQuery,
};

impl RustyPipeQuery {
    pub async fn music_playlist(&self, playlist_id: &str) -> Result<MusicPlaylist, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: &format!("VL{}", playlist_id),
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

    pub async fn music_album(&self, album_id: &str) -> Result<MusicAlbum, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: album_id,
        };

        self.execute_request::<response::MusicPlaylist, _, _>(
            ClientType::DesktopMusic,
            "music_album",
            album_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<MusicPlaylist> for response::MusicPlaylist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
        // dbg!(&self);

        let header = self.header.music_detail_header_renderer;

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

        let playlist_id = shelf
            .playlist_id
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "no playlist id",
            )))?;

        if playlist_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {}, expected {}",
                playlist_id, id
            )));
        }

        let from_ytm = header
            .subtitle
            .0
            .iter()
            .any(|c| c.as_str() == util::YT_MUSIC_NAME);

        let channel = header
            .subtitle
            .0
            .into_iter()
            .find_map(|c| ChannelId::try_from(c).ok());

        let mut mapper = MusicListMapper::new(lang);
        mapper.map_response(shelf.contents);
        let map_res = mapper.conv_items();

        let ctoken = shelf
            .continuations
            .try_swap_remove(0)
            .map(|cont| cont.next_continuation_data.continuation);

        let track_count = match ctoken {
            Some(_) => header
                .second_subtitle
                .first()
                .and_then(|txt| util::parse_numeric::<u64>(txt).ok()),
            None => Some(map_res.c.len() as u64),
        };

        let related_ctoken = music_contents
            .continuations
            .try_swap_remove(0)
            .map(|c| c.next_continuation_data.continuation);

        Ok(MapResult {
            c: MusicPlaylist {
                id: playlist_id,
                name: header.title,
                thumbnail: header.thumbnail.into(),
                channel,
                description: header.description,
                track_count,
                from_ytm,
                tracks: Paginator::new_ext(
                    track_count,
                    map_res.c,
                    ctoken,
                    None,
                    crate::param::ContinuationEndpoint::MusicBrowse,
                ),
                related_playlists: Paginator::new_ext(
                    None,
                    Vec::new(),
                    related_ctoken,
                    None,
                    crate::param::ContinuationEndpoint::MusicBrowse,
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
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicAlbum>, ExtractionError> {
        // dbg!(&self);

        let header = self.header.music_detail_header_renderer;

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
                response::music_item::ItemSection::MusicCarouselShelfRenderer {
                    contents, ..
                } => album_variants = Some(contents),
                response::music_item::ItemSection::None => (),
            }
        }
        let shelf = shelf.ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "no sectionListRenderer content",
        )))?;

        let playlist_id = header.menu.and_then(|mut menu| {
            menu.menu_renderer
                .top_level_buttons
                .try_swap_remove(0)
                .map(|btn| {
                    btn.button_renderer
                        .navigation_endpoint
                        .watch_playlist_endpoint
                        .playlist_id
                })
        });

        let mut subtitle_split = header.subtitle.split(util::DOT_SEPARATOR);
        let year_txt = subtitle_split.try_swap_remove(2).map(|cmp| cmp.to_string());

        let artists_p = subtitle_split.try_swap_remove(1);
        let (artists, by_va) = map_artists(artists_p);
        let album_type_txt = subtitle_split
            .try_swap_remove(0)
            .map(|part| part.to_string())
            .unwrap_or_default();

        let album_type = map_album_type(album_type_txt.as_str(), lang);
        let year = year_txt.and_then(|txt| util::parse_numeric(&txt).ok());

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
        let json_path = path!("testfiles" / "music_playlist" / format!("playlist_{}.json", name));
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
        insta::assert_ron_snapshot!(format!("map_music_playlist_{}", name), map_res.c, {
            ".last_update" => "[date]"
        });
    }

    #[rstest]
    #[case::one_artist("one_artist", "MPREb_nlBWQROfvjo")]
    #[case::various_artists("various_artists", "MPREb_8QkDeEIawvX")]
    #[case::single("single", "MPREb_bHfHGoy7vuv")]
    #[case::description("description", "MPREb_PiyfuVl6aYd")]
    fn map_music_album(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "music_playlist" / format!("album_{}.json", name));
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
        insta::assert_ron_snapshot!(format!("map_music_album_{}", name), map_res.c);
    }
}
