use std::fmt::Debug;

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        SearchResult, YouTubeItem,
    },
    param::search_filter::SearchFilter,
};

use super::{response, ClientType, MapRespCtx, MapResponse, MapResult, RustyPipeQuery};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearch<'a> {
    query: &'a str,
    params: &'a str,
}

impl RustyPipeQuery {
    /// Search YouTube
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn search<T: FromYtItem, S: AsRef<str> + Debug>(
        &self,
        query: S,
    ) -> Result<SearchResult<T>, Error> {
        let query = query.as_ref();
        let request_body = QSearch {
            query,
            params: "8AEB",
        };

        self.execute_request::<response::Search, _, _>(
            ClientType::Desktop,
            "search",
            query,
            "search",
            &request_body,
        )
        .await
    }

    /// Search YouTube using the given [`SearchFilter`]
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn search_filter<T: FromYtItem, S: AsRef<str> + Debug>(
        &self,
        query: S,
        filter: &SearchFilter,
    ) -> Result<SearchResult<T>, Error> {
        let query = query.as_ref();
        let request_body = QSearch {
            query,
            params: &filter.encode(),
        };

        self.execute_request::<response::Search, _, _>(
            ClientType::Desktop,
            "search_filter",
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

        let parsed = serde_json::from_str::<response::SearchSuggestion>(&response)
            .map_err(|e| Error::Extraction(ExtractionError::InvalidData(e.to_string().into())))?;

        Ok(parsed.1.into_iter().map(|item| item.0).collect())
    }
}

impl<T: FromYtItem> MapResponse<SearchResult<T>> for response::Search {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<SearchResult<T>>, ExtractionError> {
        let items = self
            .contents
            .two_column_search_results_renderer
            .primary_contents
            .section_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(ctx.lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: SearchResult {
                items: Paginator::new_ext(
                    self.estimated_results,
                    mapper
                        .items
                        .into_iter()
                        .filter_map(T::from_yt_item)
                        .collect(),
                    mapper.ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Search,
                    false,
                ),
                corrected_query: mapper.corrected_query,
                visitor_data: self
                    .response_context
                    .visitor_data
                    .or_else(|| ctx.visitor_data.map(str::to_owned)),
            },
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapRespCtx, MapResponse},
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
        let json_file = File::open(json_path).unwrap();

        let search: response::Search = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<SearchResult<YouTubeItem>> =
            search.map_response(&MapRespCtx::test("")).unwrap();

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
