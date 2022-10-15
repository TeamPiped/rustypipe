use crate::{
    error::{Error, ExtractionError},
    model::{Paginator, SearchVideo},
    param::Language,
    serializer::MapResult,
    util::TryRemove,
};

use super::{
    response::{self, TryFromWLang},
    ClientType, MapResponse, QBrowse, QContinuation, RustyPipeQuery,
};

impl RustyPipeQuery {
    pub async fn startpage(self) -> Result<Paginator<SearchVideo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QBrowse {
            context,
            browse_id: "FEwhat_to_watch".to_owned(),
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

    pub async fn startpage_continuation(
        self,
        ctoken: &str,
        visitor_data: &str,
    ) -> Result<Paginator<SearchVideo>, Error> {
        let mut context = self.get_context(ClientType::Desktop, true).await;
        context.client.visitor_data = Some(visitor_data.to_owned());
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::StartpageCont, _, _>(
            ClientType::Desktop,
            "startpage_continuation",
            ctoken,
            "browse",
            &request_body,
        )
        .await
        .map(|res| Paginator {
            visitor_data: Some(visitor_data.to_owned()),
            ..res
        })
    }

    pub async fn trending(self) -> Result<Vec<SearchVideo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QBrowse {
            context,
            browse_id: "FEtrending".to_owned(),
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

impl MapResponse<Paginator<SearchVideo>> for response::Startpage {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<SearchVideo>>, ExtractionError> {
        let mut contents = self.contents.two_column_browse_results_renderer.tabs;
        let grid = contents
            .try_swap_remove(0)
            .ok_or_else(|| ExtractionError::InvalidData("no contents".into()))?
            .tab_renderer
            .content
            .rich_grid_renderer
            .contents;

        Ok(map_startpage_videos(
            grid,
            lang,
            self.response_context.visitor_data,
        ))
    }
}

impl MapResponse<Paginator<SearchVideo>> for response::StartpageCont {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<SearchVideo>>, ExtractionError> {
        let mut received_actions = self.on_response_received_actions;
        let items = received_actions
            .try_swap_remove(0)
            .ok_or_else(|| ExtractionError::InvalidData("no contents".into()))?
            .append_continuation_items_action
            .continuation_items;

        Ok(map_startpage_videos(items, lang, None))
    }
}

impl MapResponse<Vec<SearchVideo>> for response::Trending {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Vec<SearchVideo>>, ExtractionError> {
        let mut contents = self.contents.two_column_browse_results_renderer.tabs;
        let sections = contents
            .try_swap_remove(0)
            .ok_or_else(|| ExtractionError::InvalidData("no contents".into()))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut items = Vec::new();
        let mut warnings = Vec::new();

        for mut section in sections {
            let shelf = section
                .item_section_renderer
                .contents
                .try_swap_remove(0)
                .and_then(|shelf| {
                    shelf
                        .shelf_renderer
                        .content
                        .expanded_shelf_contents_renderer
                });

            if let Some(mut shelf) = shelf {
                warnings.append(&mut shelf.items.warnings);

                for item in shelf.items.c {
                    if let response::trends::TrendingListItem::VideoRenderer(video) = item {
                        match SearchVideo::from_w_lang(video, lang) {
                            Ok(video) => {
                                items.push(video);
                            }
                            Err(e) => {
                                warnings.push(e.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(MapResult { c: items, warnings })
    }
}

fn map_startpage_videos(
    videos: MapResult<Vec<response::VideoListItem>>,
    lang: Language,
    visitor_data: Option<String>,
) -> MapResult<Paginator<SearchVideo>> {
    let mut warnings = videos.warnings;
    let mut ctoken = None;
    let items = videos
        .c
        .into_iter()
        .filter_map(|item| match item {
            response::VideoListItem::RichItemRenderer {
                content: response::RichItem::VideoRenderer(video),
            } => match SearchVideo::from_w_lang(video, lang) {
                Ok(video) => Some(video),
                Err(e) => {
                    warnings.push(e.to_string());
                    None
                }
            },
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            _ => None,
        })
        .collect();

    MapResult {
        c: Paginator::new_with_vdata(None, items, ctoken, visitor_data),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use crate::{
        client::{response, MapResponse},
        model::{Paginator, SearchVideo},
        param::Language,
        serializer::MapResult,
    };

    #[test]
    fn map_startpage() {
        let filename = "testfiles/trends/startpage.json";
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let startpage: response::Startpage =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<SearchVideo>> =
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
    fn map_startpage_cont() {
        let filename = "testfiles/trends/startpage_cont.json";
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let startpage: response::StartpageCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<SearchVideo>> =
            startpage.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_startpage_cont", map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn map_trending() {
        let filename = "testfiles/trends/trending.json";
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let startpage: response::Trending =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<SearchVideo>> =
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
