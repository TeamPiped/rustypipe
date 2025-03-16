use std::fmt::Debug;

use serde::Serialize;

use crate::{
    client::{response, ClientType, MapRespCtx, MapResponse, QBrowse, RustyPipeQuery},
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelItem, HistoryItem, Playlist, PlaylistItem, VideoItem,
    },
    serializer::MapResult,
};

use self::response::YouTubeListMapper;

use super::{MapRespOptions, QContinuation};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QHistorySearch<'a> {
    browse_id: &'a str,
    query: &'a str,
}

impl RustyPipeQuery {
    /// Get a list of videos from YouTube which the current user recently played
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn history(&self) -> Result<Paginator<HistoryItem<VideoItem>>, Error> {
        let request_body = QBrowse {
            browse_id: "FEhistory",
        };

        self.clone()
            .authenticated()
            .execute_request::<response::History, _, _>(
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
        let request_body = QContinuation {
            continuation: ctoken,
        };

        self.clone()
            .authenticated()
            .execute_request_ctx::<response::Continuation, _, _>(
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
        let request_body = QHistorySearch {
            browse_id: "FEhistory",
            query,
        };

        self.clone()
            .authenticated()
            .execute_request::<response::History, _, _>(
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
        let request_body = QBrowse {
            browse_id: "FEsubscriptions",
        };

        self.clone()
            .authenticated()
            .execute_request::<response::History, _, _>(
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

    /// Get all liked videos of the logged-in user
    ///
    /// Requires authentication cookies.
    pub async fn liked_videos(&self) -> Result<Playlist, Error> {
        self.clone()
            .authenticated()
            .playlist("LL")
            .await
            .map_err(crate::util::map_internal_playlist_err)
    }

    /// Get the "Watch later" playlist of the logged-in user
    ///
    /// Requires authentication cookies.
    pub async fn watch_later(&self) -> Result<Playlist, Error> {
        self.clone()
            .authenticated()
            .playlist("WL")
            .await
            .map_err(crate::util::map_internal_playlist_err)
    }
}

impl MapResponse<Paginator<HistoryItem<VideoItem>>> for response::History {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<VideoItem>>>, ExtractionError> {
        let items = self
            .contents
            .two_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(
                "twoColumnBrowseResultsRenderer empty".into(),
            ))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut map_res = MapResult {
            warnings: items.warnings,
            ..Default::default()
        };
        let mut ctoken = None;
        for item in items.c {
            match item {
                response::YouTubeListItem::ItemSectionRenderer { header, contents } => {
                    let mut mapper = YouTubeListMapper::<VideoItem>::new(ctx.lang);
                    mapper.map_response(contents);
                    mapper.conv_history_items(
                        header.map(|h| h.item_section_header_renderer.title),
                        ctx.utc_offset,
                        &mut map_res,
                    );
                }
                response::YouTubeListItem::ContinuationItemRenderer(ep) => {
                    if ctoken.is_none() {
                        ctoken = ep.continuation_endpoint.into_token();
                    }
                }
                _ => {}
            }
        }

        Ok(MapResult {
            c: Paginator::new_ext(
                None,
                map_res.c,
                ctoken,
                ctx.visitor_data.map(str::to_owned),
                crate::model::paginator::ContinuationEndpoint::Browse,
                true,
            ),
            warnings: map_res.warnings,
        })
    }
}

impl MapResponse<Paginator<VideoItem>> for response::History {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<VideoItem>>, ExtractionError> {
        let items = self
            .contents
            .two_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(
                "twoColumnBrowseResultsRenderer empty".into(),
            ))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: Paginator::new_ext(
                None,
                mapper.items,
                mapper.ctoken,
                ctx.visitor_data.map(str::to_owned),
                crate::model::paginator::ContinuationEndpoint::Browse,
                true,
            ),
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;

    use crate::util::tests::TESTFILES;

    use super::*;

    #[test]
    fn map_history() {
        let json_path = path!(*TESTFILES / "userdata" / "history.json");
        let json_file = File::open(json_path).unwrap();

        let history: response::History =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<HistoryItem<VideoItem>>> =
            history.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let history: response::History =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<VideoItem>> =
            history.map_response(&MapRespCtx::test("")).unwrap();

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
