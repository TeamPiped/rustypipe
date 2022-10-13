use crate::{
    error::{Error, ExtractionError},
    model::{Paginator, SearchVideo},
    serializer::MapResult,
    util::TryRemove,
};

use super::{
    response::{self, TryFromWLang},
    ClientType, MapResponse, QBrowse, RustyPipeQuery,
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

        let mut warnings = grid.warnings;
        let mut ctoken = None;
        let items = grid
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

        Ok(MapResult {
            c: Paginator::new(None, items, ctoken),
            warnings,
        })
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
