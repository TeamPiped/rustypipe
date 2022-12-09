use std::borrow::Cow;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    model::{traits::FromYtItem, AlbumItem, TrackItem},
};

use super::{response, ClientType, MapResponse, QBrowse, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get the new albums that were released on YouTube Music
    pub async fn music_new_albums(&self) -> Result<Vec<AlbumItem>, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
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
    pub async fn music_new_videos(&self) -> Result<Vec<TrackItem>, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QBrowse {
            context,
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
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<crate::serializer::MapResult<Vec<T>>, ExtractionError> {
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

        let mut mapper = MusicListMapper::new(lang);
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
    use crate::{param::Language, serializer::MapResult};

    #[rstest]
    #[case::default("default")]
    fn map_music_new_albums(#[case] name: &str) {
        let json_path = path!("testfiles" / "music_new" / format!("albums_{}.json", name));
        let json_file = File::open(json_path).unwrap();

        let new_albums: response::MusicNew =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<AlbumItem>> =
            new_albums.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_albums_{}", name), map_res.c);
    }

    #[rstest]
    #[case::default("default")]
    fn map_music_new_videos(#[case] name: &str) {
        let json_path = path!("testfiles" / "music_new" / format!("videos_{}.json", name));
        let json_file = File::open(json_path).unwrap();

        let new_albums: response::MusicNew =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<TrackItem>> =
            new_albums.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_videos_{}", name), map_res.c);
    }
}
