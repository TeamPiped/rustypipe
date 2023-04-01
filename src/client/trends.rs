use std::borrow::Cow;

use crate::{
    error::{Error, ExtractionError},
    model::{paginator::Paginator, VideoItem},
    param::Language,
    serializer::MapResult,
    util::TryRemove,
};

use super::{response, ClientType, MapResponse, QBrowse, QBrowseParams, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get the videos from the YouTube startpage
    pub async fn startpage(&self) -> Result<Paginator<VideoItem>, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QBrowse {
            context,
            browse_id: "FEwhat_to_watch",
        };

        self.execute_request::<response::Startpage, _, _>(
            ClientType::Desktop,
            "startpage",
            "",
            "browse",
            &request_body,
        )
        .await
    }

    /// Get the videos from the YouTube trending page
    pub async fn trending(&self) -> Result<Vec<VideoItem>, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QBrowseParams {
            context,
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

impl MapResponse<Paginator<VideoItem>> for response::Startpage {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Paginator<VideoItem>>, ExtractionError> {
        let mut contents = self.contents.two_column_browse_results_renderer.tabs;
        let grid = contents
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no contents")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        Ok(map_startpage_videos(
            grid,
            lang,
            self.response_context.visitor_data,
        ))
    }
}

impl MapResponse<Vec<VideoItem>> for response::Trending {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Vec<VideoItem>>, ExtractionError> {
        let mut contents = self.contents.two_column_browse_results_renderer.tabs;
        let items = contents
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no contents")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: mapper.items,
            warnings: mapper.warnings,
        })
    }
}

fn map_startpage_videos(
    videos: MapResult<Vec<response::YouTubeListItem>>,
    lang: Language,
    visitor_data: Option<String>,
) -> MapResult<Paginator<VideoItem>> {
    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
    mapper.map_response(videos);

    MapResult {
        c: Paginator::new_ext(
            None,
            mapper.items,
            mapper.ctoken,
            visitor_data,
            crate::model::paginator::ContinuationEndpoint::Browse,
        ),
        warnings: mapper.warnings,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;

    use crate::{
        client::{response, MapResponse},
        model::{paginator::Paginator, VideoItem},
        param::Language,
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    #[test]
    fn map_startpage() {
        let json_path = path!(*TESTFILES / "trends" / "startpage.json");
        let json_file = File::open(json_path).unwrap();

        let startpage: response::Startpage =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<VideoItem>> =
            startpage.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_startpage", map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn map_trending() {
        let json_path = path!(*TESTFILES / "trends" / "trending.json");
        let json_file = File::open(json_path).unwrap();

        let startpage: response::Trending =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<VideoItem>> =
            startpage.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_trending", map_res.c, {
            "[].publish_date" => "[date]",
        });
    }
}
