use std::borrow::Cow;
use std::marker::PhantomData;

use crate::{
    client::response::music_item::{map_music_items, music_grid_items, music_grid_node},
    error::{Error, ExtractionError},
    json::{yt_single_column_sections, JsonDoc},
    model::{traits::FromYtItem, AlbumItem, TrackItem},
    param::MusicNewKind,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct MusicNewEndpoint<T>(PhantomData<T>);

/// Result of [`RustyPipeQuery::music_new`]
///
/// Different kinds of new-release feeds return different item types, so the
/// result is wrapped in an enum.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum MusicNewResult {
    /// [`MusicNewKind::Albums`] -> [`Vec<AlbumItem>`]
    Albums(Vec<AlbumItem>),
    /// [`MusicNewKind::Videos`] -> [`Vec<TrackItem>`]
    Videos(Vec<TrackItem>),
}

impl RustyPipeQuery {
    /// Get new releases from YouTube Music
    ///
    /// The kind of items returned is controlled by [`MusicNewKind`]; see
    /// [`MusicNewResult`] for how to extract the concrete list.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_new(&self, kind: MusicNewKind) -> Result<MusicNewResult, Error> {
        let browse_id = match kind {
            MusicNewKind::Albums => "FEmusic_new_releases_albums",
            MusicNewKind::Videos => "FEmusic_new_releases_videos",
        };
        let request_body = ytbody!({
            "browseId": browse_id,
        });

        let res = match kind {
            MusicNewKind::Albums => self
                .execute_request::<MusicNewEndpoint<AlbumItem>, _, _>(
                    ClientType::DesktopMusic,
                    "music_new",
                    "",
                    "browse",
                    &request_body,
                )
                .await
                .map(MusicNewResult::Albums)?,
            MusicNewKind::Videos => self
                .execute_request::<MusicNewEndpoint<TrackItem>, _, _>(
                    ClientType::DesktopMusic,
                    "music_new",
                    "",
                    "browse",
                    &request_body,
                )
                .await
                .map(MusicNewResult::Videos)?,
        };
        Ok(res)
    }
}

impl<T: FromYtItem> MapEndpoint<Vec<T>> for MusicNewEndpoint<T> {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<T>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_single_column_sections(&root)?;
            let grid = sections
                .items()
                .into_iter()
                .next()
                .and_then(|section| music_grid_node(&section))
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?;
            let items = music_grid_items(&grid)
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no grid items")))?;

            Ok(map_music_items(&items, ctx.lang).0)
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
            MusicNewEndpoint::<AlbumItem>::map(&json, &MapRespCtx::test("")).unwrap();

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
            MusicNewEndpoint::<TrackItem>::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_new_videos_{name}"), map_res.c);
    }
}
