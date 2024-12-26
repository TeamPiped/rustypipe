use std::fmt::Debug;

use serde::Serialize;

use crate::{
    client::{response, ClientType, MapRespCtx, MapResponse, QBrowse, RustyPipeQuery},
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelItem, VideoItem,
    },
    serializer::MapResult,
};

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
    pub async fn history(&self) -> Result<Paginator<VideoItem>, Error> {
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

    /// Search the YouTube playback history of the current user
    ///
    /// Requires authentication cookies.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn history_search<S: AsRef<str> + Debug>(
        &self,
        query: S,
    ) -> Result<Paginator<VideoItem>, Error> {
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
                None,
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
    use rstest::rstest;

    use crate::util::tests::TESTFILES;

    use super::*;

    #[rstest]
    #[case::history("history")]
    #[case::subscription_feed("subscription_feed")]
    fn map_history(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "history" / format!("{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let history: response::History =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = history.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }
}
