use serde::Deserialize;
use serde_with::{json::JsonString, serde_as};

use super::{video_item::YouTubeListRendererWrap, ResponseContext};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Search {
    #[serde_as(as = "Option<JsonString>")]
    pub estimated_results: Option<u64>,
    pub contents: Contents,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub two_column_search_results_renderer: TwoColumnSearchResultsRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TwoColumnSearchResultsRenderer {
    pub primary_contents: YouTubeListRendererWrap,
}
