use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::{AlbumType, ChannelId, MusicAlbum, MusicPlaylist, Paginator, TrackItem},
    serializer::MapResult,
    util::{self, TryRemove},
};

use super::{
    response::{self, music_item::MusicListMapper},
    ClientType, MapResponse, QBrowse, QContinuation, RustyPipeQuery,
};

impl RustyPipeQuery {
    pub async fn music_playlist(&self, playlist_id: &str) -> Result<MusicPlaylist, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: "VL".to_owned() + playlist_id,
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

    pub async fn music_playlist_continuation(
        &self,
        ctoken: &str,
    ) -> Result<Paginator<TrackItem>, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::MusicPlaylistCont, _, _>(
            ClientType::DesktopMusic,
            "music_playlist_continuation",
            ctoken,
            "browse",
            &request_body,
        )
        .await
    }

    pub async fn music_album(&self, album_id: &str) -> Result<MusicAlbum, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: album_id.to_owned(),
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
        _lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicPlaylist>, ExtractionError> {
        // dbg!(&self);

        let header = self.header.music_detail_header_renderer;

        let mut content = self.contents.single_column_browse_results_renderer.contents;
        let mut shelf = content
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .into_iter()
            .find_map(|section| match section {
                response::music_playlist::ItemSection::MusicShelfRenderer(shelf) => Some(shelf),
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
            .any(|c| c.as_str() == "YouTube Music");

        let channel = header
            .subtitle
            .0
            .into_iter()
            .find_map(|c| ChannelId::try_from(c).ok());

        let mut mapper = MusicListMapper::<TrackItem>::new();
        mapper.map_response(shelf.contents);

        let ctoken = shelf
            .continuations
            .try_swap_remove(0)
            .map(|cont| cont.next_continuation_data.continuation);

        let track_count = match ctoken {
            Some(_) => header
                .second_subtitle
                .first()
                .and_then(|txt| util::parse_numeric::<u64>(txt).ok()),
            None => Some(mapper.items.len() as u64),
        };

        Ok(MapResult {
            c: MusicPlaylist {
                id: playlist_id,
                name: header.title,
                thumbnail: header.thumbnail.into(),
                channel,
                description: header.description,
                track_count,
                from_ytm,
                tracks: Paginator::new(track_count, mapper.items, ctoken),
            },
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<Paginator<TrackItem>> for response::MusicPlaylistCont {
    fn map_response(
        self,
        _id: &str,
        _lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<TrackItem>>, ExtractionError> {
        let mut mapper = MusicListMapper::<TrackItem>::new();
        let mut shelf = self.continuation_contents.music_playlist_shelf_continuation;
        mapper.map_response(shelf.contents);

        let ctoken = shelf
            .continuations
            .try_swap_remove(0)
            .map(|cont| cont.next_continuation_data.continuation);

        Ok(MapResult {
            c: Paginator::new(None, mapper.items, ctoken),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<MusicAlbum> for response::MusicPlaylist {
    fn map_response(
        self,
        id: &str,
        _lang: crate::param::Language,
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
        let mut album_versions = None;
        for section in sections {
            match section {
                response::music_playlist::ItemSection::MusicShelfRenderer(sh) => shelf = Some(sh),
                response::music_playlist::ItemSection::MusicCarouselShelfRenderer { contents } => {
                    album_versions = Some(contents)
                }
                response::music_playlist::ItemSection::None => (),
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

        let subtitle_len = header.subtitle.0.len();
        if subtitle_len < 5 {
            return Err(ExtractionError::InvalidData(Cow::Owned(format!(
                "header text is missing elements: {}",
                header.subtitle.to_string()
            ))));
        }

        let mut artists = Vec::new();
        let mut artists_txt = String::new();

        let mut st_parts = header.subtitle.0.into_iter();
        let album_type_txt = st_parts.next().unwrap();
        st_parts.next();

        for _ in 0..subtitle_len - 4 {
            let part = st_parts.next().unwrap();
            artists_txt += part.as_str();

            if let Ok(a) = ChannelId::try_from(part) {
                artists.push(a);
            }
        }

        st_parts.next();
        let year_txt = st_parts.next().unwrap();

        let by_va = artists_txt == "Various Artists";

        // TODO: add support for different languages
        let album_type = match album_type_txt.as_str() {
            "Single" => AlbumType::Single,
            "EP" => AlbumType::Ep,
            _ => AlbumType::Album,
        };
        let year = util::parse_numeric(year_txt.as_str())
            .ok()
            .unwrap_or_default();

        let mut mapper = match by_va {
            true => MusicListMapper::<TrackItem>::new(),
            false => {
                MusicListMapper::<TrackItem>::with_artists(artists.clone(), artists_txt.clone())
            }
        };
        mapper.map_response(shelf.contents);

        Ok(MapResult {
            c: MusicAlbum {
                id: id.to_owned(),
                playlist_id,
                name: header.title,
                cover: header.thumbnail.into(),
                artists,
                artists_txt,
                album_type,
                year,
                by_va,
                tracks: mapper.items,
            },
            warnings: mapper.warnings,
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
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_music_playlist(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/music_playlist/playlist_{}.json", name);
        let json_path = Path::new(&filename);
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

    #[test]
    fn map_music_playlist_cont() {
        let json_path = Path::new("testfiles/music_playlist/playlist_cont.json");
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicPlaylistCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_music_playlist_cont", map_res.c);
    }

    #[rstest]
    #[case::one_artist("one_artist", "MPREb_nlBWQROfvjo")]
    #[case::various_artists("various_artists", "MPREb_8QkDeEIawvX")]
    #[case::single("single", "MPREb_bHfHGoy7vuv")]
    fn map_music_album(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/music_playlist/album_{}.json", name);
        let json_path = Path::new(&filename);
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
        insta::assert_ron_snapshot!(format!("map_music_album_{}", name), map_res.c, {
            ".last_update" => "[date]"
        });
    }
}
