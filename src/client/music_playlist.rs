use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::{ChannelId, MusicPlaylist, Paginator, TrackItem},
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
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "no sectionListRenderer content",
            )))?
            .music_shelf_renderer;

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

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;
    use crate::param::Language;

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
        let map_res = playlist.map_response(id, Language::En, None).unwrap();

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
}
