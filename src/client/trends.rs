use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::VideoItem,
    serializer::MapResult,
};

use super::{response, ClientType, MapRespCtx, MapResponse, QBrowseParams, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get the videos from the YouTube trending page
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn trending(&self) -> Result<Vec<VideoItem>, Error> {
        let request_body = QBrowseParams {
            browse_id: "FEtrending",
            params: "4gIOGgxtb3N0X3BvcHVsYXI%3D",
        };

        self.execute_request::<response::Trending, _, _>(
            ClientType::Desktop,
            "trends",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<Vec<VideoItem>> for response::Trending {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<VideoItem>>, ExtractionError> {
        let items = self
            .contents
            .two_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no contents")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: mapper.items,
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
        model::VideoItem,
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::base("videos")]
    #[case::page_header_renderer("20230501_page_header_renderer")]
    fn map_trending(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "trends" / format!("trending_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let trending: response::Trending =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<VideoItem>> =
            trending.map_response(&MapRespCtx::test("")).unwrap();

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
