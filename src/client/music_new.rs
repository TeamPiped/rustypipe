use std::borrow::Cow;

use crate::{
    client::response::music_item::MusicListMapper,
    error::{Error, ExtractionError},
    json::{JsonDoc, yt_single_column_sections, ytq},
    model::{traits::FromYtItem, AlbumItem, TrackItem},
    request_body::ytbody,
    serializer::MapResult,
};

use super::{ClientType, MapJsonResponse, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct MusicNewJson;

impl RustyPipeQuery {
    /// Get the new albums that were released on YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_new_albums(&self) -> Result<Vec<AlbumItem>, Error> {
        let request_body = ytbody!({
            "browseId": "FEmusic_new_releases_albums",
        });

        self.execute_request::<MusicNewJson, _, _>(
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
        let request_body = ytbody!({
            "browseId": "FEmusic_new_releases_videos",
        });

        self.execute_request::<MusicNewJson, _, _>(
            ClientType::DesktopMusic,
            "music_new_videos",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

impl<T: FromYtItem> MapJsonResponse<Vec<T>> for MusicNewJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<T>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_single_column_sections(&root)?;
            let grid = sections
                .items()
                .into_iter()
                .next()
                .and_then(|section| section.query(ytq!(.gridRenderer)))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?;
            let items = grid
                .query(ytq!(.items))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no grid items")))?;

            let mut mapper = MusicListMapper::new(ctx.lang);
            mapper.map_response_node(&items);
            Ok(mapper.conv_items())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{serializer::MapResult, util::tests::TESTFILES};

    #[rstest]
    #[case::default("default")]
    fn map_music_new_albums(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "music_new" / format!("albums_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Vec<AlbumItem>> =
            MusicNewJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Vec<TrackItem>> =
            MusicNewJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_videos_{name}"), map_res.c);
    }
}
