use std::fmt::Debug;

use crate::{
    client::{
        response::{self, music_item::extend_music_history_items_value},
        ClientType, MapEndpoint, RustyPipeQuery,
    },
    error::{Error, ExtractionError},
    json::{yt_continuation, ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        AlbumItem, ArtistItem, HistoryItem, MusicPlaylist, MusicPlaylistItem, TrackItem,
    },
    param::MusicSavedKind,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{pagination::MusicContinuationMarker, MapRespCtx, MapRespOptions};

/// Result of [`RustyPipeQuery::music_saved`]
///
/// Different kinds of saved items return different `Paginator<T>` types, so the
/// result is wrapped in an enum. Use the [`From`] impls (or the
/// [`FromYtItem`](crate::model::traits::FromYtItem) helper) to extract the
/// concrete paginator you need.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum MusicSavedResult {
    /// [`MusicSavedKind::Artists`] -> [`Paginator<ArtistItem>`]
    Artists(Paginator<ArtistItem>),
    /// [`MusicSavedKind::Albums`] -> [`Paginator<AlbumItem>`]
    Albums(Paginator<AlbumItem>),
    /// [`MusicSavedKind::Tracks`] -> [`Paginator<TrackItem>`]
    Tracks(Paginator<TrackItem>),
    /// [`MusicSavedKind::Playlists`] -> [`Paginator<MusicPlaylistItem>`]
    Playlists(Paginator<MusicPlaylistItem>),
}

#[derive(Debug)]
struct MusicHistoryEndpoint;

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
            .execute_request::<MusicHistoryEndpoint, _, _>(
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
            .execute_request_ctx::<MusicContinuationMarker, _, _>(
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

    /// Get items from the user's YouTube Music library ("saved X" feed)
    ///
    /// Requires authentication cookies. The kind of items returned is
    /// determined by [`MusicSavedKind`]; see [`MusicSavedResult`] for how to
    /// extract the concrete paginator.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_saved(&self, kind: MusicSavedKind) -> Result<MusicSavedResult, Error> {
        let ctoken = match kind {
            MusicSavedKind::Artists => {
                "4qmFsgIyEh5GRW11c2ljX2xpYnJhcnlfY29ycHVzX2FydGlzdHMaEGdnTUdLZ1FJQUJBQm9BWUI%3D"
            }
            MusicSavedKind::Albums => {
                "4qmFsgIoEhRGRW11c2ljX2xpa2VkX2FsYnVtcxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D"
            }
            MusicSavedKind::Tracks => {
                "4qmFsgIoEhRGRW11c2ljX2xpa2VkX3ZpZGVvcxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D"
            }
            MusicSavedKind::Playlists => {
                "4qmFsgIrEhdGRW11c2ljX2xpa2VkX3BsYXlsaXN0cxoQZ2dNR0tnUUlBQkFCb0FZQg%3D%3D"
            }
        };

        let q = self.clone().authenticated();
        let res = match kind {
            MusicSavedKind::Artists => {
                q.continuation::<ArtistItem, _>(ctoken, ContinuationEndpoint::MusicBrowse, None)
                    .await
                    .map(MusicSavedResult::Artists)?
            }
            MusicSavedKind::Albums => {
                q.continuation::<AlbumItem, _>(ctoken, ContinuationEndpoint::MusicBrowse, None)
                    .await
                    .map(MusicSavedResult::Albums)?
            }
            MusicSavedKind::Tracks => {
                q.continuation::<TrackItem, _>(ctoken, ContinuationEndpoint::MusicBrowse, None)
                    .await
                    .map(MusicSavedResult::Tracks)?
            }
            MusicSavedKind::Playlists => {
                q.continuation::<MusicPlaylistItem, _>(
                    ctoken,
                    ContinuationEndpoint::MusicBrowse,
                    None,
                )
                .await
                .map(MusicSavedResult::Playlists)?
            }
        };
        Ok(res)
    }

    /// Get all liked YouTube Music tracks of the logged-in user
    ///
    /// The difference to [`RustyPipeQuery::music_saved`] (with
    /// [`MusicSavedKind::Tracks`]) is that this function only returns tracks that
    /// were explicitly liked by the user.
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
    root.query(ytq!(
        .contents.singleColumnBrowseResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content
            .sectionListRenderer.contents
        || .contents.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer
            .contents
    ))
    .ok_or_else(|| ExtractionError::InvalidData("no music history contents".into()))
}

fn yt_music_history_continuations<'a>(root: &'a JsonNode<'a>) -> Option<JsonNode<'a>> {
    root.query(ytq!(
        .contents.singleColumnBrowseResultsRenderer.(.tabs[0] || .contents[0]).tabRenderer.content
            .sectionListRenderer.continuations
        || .contents.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer
            .continuations
    ))
}

impl MapEndpoint<Paginator<HistoryItem<TrackItem>>> for MusicHistoryEndpoint {
    fn map(
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
                    extend_music_history_items_value(
                        shelf.contents,
                        ctx.lang,
                        shelf.title,
                        ctx.utc_offset,
                        &mut map_res,
                    );
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
        let map_res = MusicHistoryEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

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
