use std::fmt::Debug;

use crate::{
    client::{ClientType, MapEndpoint, MapRespCtx, MapRespOptions, RustyPipeQuery},
    error::{Error, ExtractionError},
    json::{yt_two_column_list_items, ytq, JsonDoc, JsonValue},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelItem, HistoryItem, Playlist, PlaylistItem, VideoItem,
    },
    param::UserPlaylistKind,
    request_body::ytbody,
    serializer::MapResult,
};

use super::pagination::ContinuationMarker;

#[derive(Debug)]
struct HistoryEndpoint;

impl RustyPipeQuery {
    /// Get a list of videos from YouTube which the current user recently played
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn history(&self) -> Result<Paginator<HistoryItem<VideoItem>>, Error> {
        let request_body = ytbody!({
            "browseId": "FEhistory",
        });

        self.clone()
            .authenticated()
            .execute_request::<HistoryEndpoint, _, _>(
                ClientType::Desktop,
                "history",
                "",
                "browse",
                &request_body,
            )
            .await
    }

    /// Get more YouTube history items from the given continuation token
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn history_continuation<S: AsRef<str> + Debug>(
        &self,
        ctoken: S,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<HistoryItem<VideoItem>>, Error> {
        let ctoken = ctoken.as_ref();
        let request_body = ytbody!({
            "continuation": ctoken,
        });

        self.clone()
            .authenticated()
            .execute_request_ctx::<ContinuationMarker, _, _>(
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

    /// Search the YouTube playback history of the current user
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn history_search<S: AsRef<str> + Debug>(
        &self,
        query: S,
    ) -> Result<Paginator<HistoryItem<VideoItem>>, Error> {
        let query = query.as_ref();
        let request_body = ytbody!({
            "browseId": "FEhistory",
            "query": query,
        });

        self.clone()
            .authenticated()
            .execute_request::<HistoryEndpoint, _, _>(
                ClientType::Desktop,
                "history_search",
                query,
                "browse",
                &request_body,
            )
            .await
    }

    /// Get a list of channels the current user subscribed to from YouTube
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn subscriptions(&self) -> Result<Paginator<ChannelItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgIqEgpGRWNoYW5uZWxzGgRrQUlDmgIVYnJvd3NlLWZlZWRGRWNoYW5uZWxz",
                ContinuationEndpoint::Browse,
                None,
            )
            .await
    }

    /// Get the YouTube subscription feed of the current user
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn subscription_feed(&self) -> Result<Paginator<VideoItem>, Error> {
        let request_body = ytbody!({
            "browseId": "FEsubscriptions",
        });

        self.clone()
            .authenticated()
            .execute_request::<HistoryEndpoint, _, _>(
                ClientType::Desktop,
                "subscription_feed",
                "",
                "browse",
                &request_body,
            )
            .await
    }

    /// Get a list of YouTube playlists the current user added to their library
    ///
    /// Requires authentication cookies.
    pub async fn saved_playlists(&self) -> Result<Paginator<PlaylistItem>, Error> {
        self.clone()
            .authenticated()
            .continuation(
                "4qmFsgJFEhZGRXBsYXlsaXN0X2FnZ3JlZ2F0aW9uGgRxQUlDmgIkNjc5MjVhZTYtMDAwMC0yYzQyLWFjMjItM2MyODZkNDI1MTQy",
                ContinuationEndpoint::Browse,
                None,
            )
            .await
    }

    /// Get a built-in user playlist (Liked videos, Watch later)
    ///
    /// Requires authentication cookies.
    ///
    /// For liked music tracks, use
    /// [`RustyPipeQuery::music_liked_tracks`](super::RustyPipeQuery::music_liked_tracks),
    /// which returns a [`crate::model::MusicPlaylist`] (different type).
    pub async fn user_playlist(
        &self,
        kind: UserPlaylistKind,
    ) -> Result<Playlist, Error> {
        let browse_id = match kind {
            UserPlaylistKind::LikedVideos => "LL",
            UserPlaylistKind::WatchLater => "WL",
            UserPlaylistKind::MusicLikedTracks => "LM",
        };
        self.clone()
            .authenticated()
            .playlist(browse_id)
            .await
            .map_err(crate::util::map_internal_playlist_err)
    }
}

impl MapEndpoint<Paginator<HistoryItem<VideoItem>>> for HistoryEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<VideoItem>>>, ExtractionError> {
        json.with_root(|root| {
            let items = yt_two_column_list_items(&root)?;

            let mut map_res = MapResult {
                warnings: Vec::new(),
                ..Default::default()
            };
            let mut ctoken = None;

            for item in items.items() {
                if let Some(contents) = item.query(ytq!(
                    .itemSectionRenderer.contents || .expandedShelfContentsRenderer.items
                )) {
                    let date_txt = item.text_at(ytq!(
                        .itemSectionRenderer.header.itemSectionHeaderRenderer.title
                    ));
                    super::response::video_item::extend_video_history_items(
                        &contents,
                        ctx.lang,
                        date_txt,
                        ctx.utc_offset,
                        &mut map_res,
                    );
                } else if ctoken.is_none() {
                    ctoken = item
                        .query(ytq!(.continuationItemRenderer.continuationEndpoint))
                        .and_then(|node| node.deserialize::<JsonValue>().ok())
                        .and_then(|endpoint| crate::json::yt_continuation_value(&endpoint));
                }
            }

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                    true,
                ),
                warnings: map_res.warnings,
            })
        })
    }
}

impl MapEndpoint<Paginator<VideoItem>> for HistoryEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<VideoItem>>, ExtractionError> {
        json.with_root(|root| {
            let items = yt_two_column_list_items(&root)?;
            let (mapped, ctoken, _) =
                super::response::video_item::map_video_items(&items, ctx.lang);

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    mapped.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                    true,
                ),
                warnings: mapped.warnings,
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
        let json_path = path!(*TESTFILES / "userdata" / "history.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<HistoryItem<VideoItem>>> =
            HistoryEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(map_res.c, {
            ".items[].playback_date" => "[date]",
        });
    }

    #[test]
    fn map_subscription_feed() {
        let json_path = path!(*TESTFILES / "userdata" / "subscription_feed.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<VideoItem>> =
            HistoryEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }
}
