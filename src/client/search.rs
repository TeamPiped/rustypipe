use std::fmt::Debug;

use crate::{
    error::{Error, ExtractionError},
    json::{yt_estimated_results, yt_search_primary_items, JsonDoc},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        SearchResult,
    },
    param::search_filter::SearchFilter,
    request_body::ytbody,
};

use super::{response, ClientType, MapEndpoint, MapRespCtx, MapResult, RustyPipeQuery};

#[derive(Debug)]
struct SearchEndpoint;

impl RustyPipeQuery {
    /// Search YouTube
    ///
    /// Pass `filter = None` for an unfiltered search, or `filter = Some(&sf)`
    /// to constrain results to e.g. a single item type, upload date, or
    /// duration.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn search<T: FromYtItem, S: AsRef<str> + Debug>(
        &self,
        query: S,
        filter: Option<&SearchFilter>,
    ) -> Result<SearchResult<T>, Error> {
        let query = query.as_ref();
        let params = filter.map_or_else(|| "8AEB".to_owned(), |f| f.encode());
        let request_body = ytbody!({
            "query": query,
            "params": params,
        });

        self.execute_request::<SearchEndpoint, _, _>(
            ClientType::Desktop,
            "search",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Get YouTube search suggestions
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn search_suggestion<S: AsRef<str> + Debug>(
        &self,
        query: S,
    ) -> Result<Vec<String>, Error> {
        let url = url::Url::parse_with_params(
            "https://suggestqueries-clients6.youtube.com/complete/search?client=youtube&xhr=t",
            &[
                ("hl", self.opts.lang.to_string()),
                ("gl", self.opts.country.to_string()),
                ("q", query.as_ref().to_owned()),
            ],
        )
        .map_err(|_| Error::Other("could not build url".into()))?;

        let response = self
            .client
            .http_request_txt(&self.client.inner.http.get(url).build()?)
            .await?;

        JsonDoc::new(response)
            .with_root(|root| {
                Ok(root
                    .items()
                    .get(1)
                    .cloned()
                    .map(|arr| {
                        arr.items()
                            .into_iter()
                            .filter_map(|item| item.items().into_iter().next())
                            .filter_map(|value| value.as_str())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default())
            })
            .map_err(Error::from)
    }
}

impl<T: FromYtItem> MapEndpoint<SearchResult<T>> for SearchEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<SearchResult<T>>, ExtractionError> {
        json.with_root(|root| {
            let items = yt_search_primary_items(&root)?;
            let (mapped, ctoken, corrected_query) =
                response::video_item::map_youtube_items(&items, ctx.lang);

            Ok(MapResult {
                c: SearchResult {
                    items: Paginator::new_ext(
                        yt_estimated_results(&root),
                        mapped.c.into_iter().filter_map(T::from_yt_item).collect(),
                        ctoken,
                        ctx.visitor_data.map(str::to_owned),
                        ContinuationEndpoint::Search,
                        false,
                    ),
                    corrected_query,
                    visitor_data: ctx.visitor_data(&root),
                },
                warnings: mapped.warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{
        model::{SearchResult, YouTubeItem},
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::default("default")]
    #[case::playlists("playlists")]
    #[case::empty("empty")]
    #[case::ab3_channel_handles("20221121_AB3_channel_handles")]
    fn t_map_search(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "search" / format!("{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<SearchResult<YouTubeItem>> =
            SearchEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_search_{name}"), map_res.c, {
            ".items.items.*.publish_date" => "[date]",
        });
    }
}
