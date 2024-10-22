use std::borrow::Cow;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    model::{traits::FromYtItem, AlbumItem, TrackItem},
    serializer::MapResult,
};

use super::{response, ClientType, MapRespCtx, MapResponse, QBrowse, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get the new albums that were released on YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_new_albums(&self) -> Result<Vec<AlbumItem>, Error> {
        let request_body = QBrowse {
            browse_id: "FEmusic_new_releases_albums",
        };

        self.execute_request::<response::MusicNew, _, _>(
            ClientType::DesktopMusic,
            "music_new_albums",
            "",
            "browse",
            &request_body,
        )
        .await
    }

    /// Get the new music videos that were released on YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_new_videos(&self) -> Result<Vec<TrackItem>, Error> {
        let request_body = QBrowse {
            browse_id: "FEmusic_new_releases_videos",
        };

        self.execute_request::<response::MusicNew, _, _>(
            ClientType::DesktopMusic,
            "music_new_videos",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

impl<T: FromYtItem> MapResponse<Vec<T>> for response::MusicNew {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<Vec<T>>, ExtractionError> {
        let items = self
            .contents
            .single_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .grid_renderer
            .items;

        let mut mapper = MusicListMapper::new(ctx.lang);
        mapper.map_response(items);

        Ok(mapper.conv_items())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{serializer::MapResult, util::tests::TESTFILES};

    #[rstest]
    #[case::default("default")]
    fn map_music_new_albums(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_new" / format!("albums_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let new_albums: response::MusicNew =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<AlbumItem>> =
            new_albums.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_albums_{name}"), map_res.c);
    }

    #[rstest]
    #[case::default("default")]
    #[case::default("w_podcasts")]
    fn map_music_new_videos(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_new" / format!("videos_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let new_videos: response::MusicNew =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<TrackItem>> =
            new_videos.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_videos_{name}"), map_res.c);
    }
}
