use serde::{de::IgnoredAny, Serialize};

use crate::{
    deobfuscate::Deobfuscator,
    error::{Error, ExtractionError},
    model::{Paginator, SearchResult, YouTubeItem},
    param::{search_filter::SearchFilter, Language},
};

use super::{response, ClientType, MapResponse, MapResult, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearch<'a> {
    context: YTContext,
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<String>,
}

impl RustyPipeQuery {
    pub async fn search(self, query: &str) -> Result<SearchResult, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QSearch {
            context,
            query,
            params: None,
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

    pub async fn search_filter(
        self,
        query: &str,
        filter: &SearchFilter,
    ) -> Result<SearchResult, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(filter.encode()),
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

    pub async fn search_suggestion(self, query: &str) -> Result<Vec<String>, Error> {
        let url = url::Url::parse_with_params("https://suggestqueries-clients6.youtube.com/complete/search?client=youtube&gs_rn=64&gs_ri=youtube&ds=yt&cp=1&gs_id=4&xhr=t&xssi=t",
            &[("hl", self.opts.lang.to_string()), ("gl", self.opts.country.to_string()), ("q", query.to_string())]
        ).map_err(|_| Error::Other("could not build url".into()))?;

        let response = self
            .client
            .http_request_txt(self.client.inner.http.get(url).build()?)
            .await?;

        let trimmed = response.get(5..).ok_or_else(|| {
            Error::Extraction(ExtractionError::InvalidData(
                "could not get string slice".into(),
            ))
        })?;

        let parsed = serde_json::from_str::<(
            IgnoredAny,
            Vec<(String, IgnoredAny, IgnoredAny)>,
            IgnoredAny,
        )>(trimmed)
        .map_err(|e| Error::Extraction(ExtractionError::InvalidData(e.to_string().into())))?;

        Ok(parsed.1.into_iter().map(|item| item.0).collect())
    }
}

impl MapResponse<SearchResult> for response::Search {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<SearchResult>, ExtractionError> {
        let items = self
            .contents
            .two_column_search_results_renderer
            .primary_contents
            .section_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: SearchResult {
                items: Paginator::new(self.estimated_results, mapper.items, mapper.ctoken),
                corrected_query: mapper.corrected_query,
            },
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use crate::{
        client::{response, MapResponse},
        model::SearchResult,
        param::Language,
        serializer::MapResult,
    };

    use rstest::rstest;

    #[rstest]
    #[case::default("default")]
    #[case::playlists("playlists")]
    #[case::playlists("empty")]
    fn t_map_search(#[case] name: &str) {
        let filename = format!("testfiles/search/{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let search: response::Search = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<SearchResult> = search.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_search_{}", name), map_res.c, {
            ".items.items.*.publish_date" => "[date]",
        });
    }
}
