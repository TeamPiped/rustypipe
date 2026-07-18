use crate::{
    error::{Error, ExtractionError},
    json::{yt_two_column_list_items, JsonDoc},
    model::VideoItem,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{response, ClientType, MapJsonResponse, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct TrendingJson;

impl RustyPipeQuery {
    /// Get the videos from the YouTube trending page
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn trending(&self) -> Result<Vec<VideoItem>, Error> {
        let request_body = ytbody!({
            "browseId": "FEtrending",
            "params": "4gIOGgxtb3N0X3BvcHVsYXI%3D",
        });

        self.execute_request::<TrendingJson, _, _>(
            ClientType::Desktop,
            "trends",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapJsonResponse<Vec<VideoItem>> for TrendingJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<VideoItem>>, ExtractionError> {
        json.with_root(|root| {
            let items = yt_two_column_list_items(&root)?;
            let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
            mapper.map_response_node(&items);
            Ok(MapResult {
                c: mapper.items,
                warnings: mapper.warnings,
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
    use crate::{model::VideoItem, util::tests::TESTFILES};

    #[rstest]
    #[case::base("videos")]
    #[case::page_header_renderer("20230501_page_header_renderer")]
    fn map_trending(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "trends" / format!("trending_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Vec<VideoItem>> =
            TrendingJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_trending_{name}"), map_res.c, {
            "[].publish_date" => "[date]",
        });
    }
}
