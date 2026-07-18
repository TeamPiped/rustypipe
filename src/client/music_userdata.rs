use std::fmt::Debug;

use crate::{
    client::{
        response::{self, music_item::MusicListMapper},
        ClientType, MapJsonResponse, RustyPipeQuery,
    },
    error::{Error, ExtractionError},
    json::{yt_continuation, ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        AlbumItem, ArtistItem, HistoryItem, MusicPlaylist, MusicPlaylistItem, TrackItem,
    },
    request_body::ytbody,
    serializer::MapResult,
};

use super::{pagination::MusicContinuationJson, MapRespCtx, MapRespOptions};

#[derive(Debug)]
struct MusicHistoryJson;

impl RustyPipeQuery {
    /// Get a list of tracks from YouTube Music which the current user recently played
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_history(&self) -> Result<Paginator<HistoryItem<TrackItem>>, Error> {
        let request_body = ytbody!({
            "browseId": "FEmusic_history",
            "params": "oggECgIIAQ%3D%3D",
        });

        self.clone()
            .authenticated()
            .execute_request::<MusicHistoryJson, _, _>(
                ClientType::DesktopMusic,
                "music_history",
                "",
                "browse",
                &request_body,
            )
            .await
    }

    /// Get more YouTube Music history items from the given continuation token
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_history_continuation<S: AsRef<str> + Debug>(
        &self,
        ctoken: S,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<HistoryItem<TrackItem>>, Error> {
        let ctoken = ctoken.as_ref();
        let request_body = ytbody!({
            "continuation": ctoken,
        });

        self.clone()
            .authenticated()
            .execute_request_ctx::<MusicContinuationJson, _, _>(
                ClientType::Desktop,
                "history_continuation",
                ctoken,
                "browse",
                &request_body,
                MapRespOptions {
                    visitor_data,
                    ..Default::default()
                },
            )
            .await
    }

    /// Get a list of YouTube Music artists which the current user subscribed to
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_saved_artists(&self) -> Result<Paginator<ArtistItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgIyEh5GRW11c2ljX2xpYnJhcnlfY29ycHVzX2FydGlzdHMaEGdnTUdLZ1FJQUJBQm9BWUI%3D",
                ContinuationEndpoint::MusicBrowse,
                None,
            )
            .await
    }

    /// Get a list of YouTube Music albums which the current user has added to their collection
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_saved_albums(&self) -> Result<Paginator<AlbumItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgIoEhRGRW11c2ljX2xpa2VkX2FsYnVtcxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D",
                ContinuationEndpoint::MusicBrowse,
                None,
            )
            .await
    }

    /// Get a list of YouTube Music tracks which the current user has added to their collection
    ///
    /// Contains both liked tracks and tracks from saved albums.
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_saved_tracks(&self) -> Result<Paginator<TrackItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgIoEhRGRW11c2ljX2xpa2VkX3ZpZGVvcxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D",
                ContinuationEndpoint::MusicBrowse,
                None,
            )
            .await
    }

    /// Get a list of YouTube Music playlists which the current user has added to their collection
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_saved_playlists(&self) -> Result<Paginator<MusicPlaylistItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgIrEhdGRW11c2ljX2xpa2VkX3BsYXlsaXN0cxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D",
                ContinuationEndpoint::MusicBrowse,
                None,
            )
            .await
    }

    /// Get all liked YouTube Music tracks of the logged-in user
    ///
    /// The difference to [`RustyPipeQuery::music_saved_tracks`] is that this function only returns
    /// tracks that were explicitly liked by the user.
    ///
    /// Requires authentication cookies.
    pub async fn music_liked_tracks(&self) -> Result<MusicPlaylist, Error> {
        self.clone()
            .authenticated()
            .music_playlist("LM")
            .await
            .map_err(crate::util::map_internal_playlist_err)
    }
}

fn yt_music_history_sections<'a>(root: &'a JsonNode<'a>) -> Result<JsonNode<'a>, ExtractionError> {
    root.first_of(&[
        ytq!(
            .contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
                .sectionListRenderer.contents
        ),
        ytq!(
            .contents.singleColumnBrowseResultsRenderer.contents[0].tabRenderer.content
                .sectionListRenderer.contents
        ),
        ytq!(
            .contents.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer
                .contents
        ),
    ])
    .ok_or_else(|| ExtractionError::InvalidData("no music history contents".into()))
}

fn yt_music_history_continuations<'a>(root: &'a JsonNode<'a>) -> Option<JsonNode<'a>> {
    root.first_of(&[
        ytq!(
            .contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
                .sectionListRenderer.continuations
        ),
        ytq!(
            .contents.singleColumnBrowseResultsRenderer.contents[0].tabRenderer.content
                .sectionListRenderer.continuations
        ),
        ytq!(
            .contents.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer
                .continuations
        ),
    ])
}

impl MapJsonResponse<Paginator<HistoryItem<TrackItem>>> for MusicHistoryJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<TrackItem>>>, ExtractionError> {
        json.with_root(|root| {
            let contents = yt_music_history_sections(&root)?;
            let continuations = yt_music_history_continuations(&root);
            let mut map_res = MapResult::default();

            for shelf in contents.items() {
                let Some(shelf) = shelf.query(ytq!(.musicShelfRenderer)) else {
                    continue;
                };
                if let Ok(shelf) = shelf.deserialize::<response::music_item::MusicShelf>() {
                    let mut mapper = MusicListMapper::new(ctx.lang);
                    mapper.map_response(shelf.contents);
                    mapper.conv_history_items(shelf.title, ctx.utc_offset, &mut map_res);
                }
            }

            let ctoken = continuations
                .and_then(|cont| cont.items().into_iter().next())
                .and_then(|cont| yt_continuation(&cont));

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::MusicBrowse,
                    true,
                ),
                warnings: map_res.warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;

    use crate::util::tests::TESTFILES;

    use super::*;

    #[test]
    fn map_history() {
        let json_path = path!(*TESTFILES / "music_userdata" / "music_history.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res = MusicHistoryJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(map_res.c, {
            ".items[].playback_date" => "[date]",
        });
    }
}
