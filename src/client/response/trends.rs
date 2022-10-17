use serde::Deserialize;
use serde_with::{serde_as, VecSkipError};

use super::{video_item::YouTubeListRendererWrap, ContentRenderer, ResponseContext};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Startpage {
    pub contents: Contents,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trending {
    pub contents: Contents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_browse_results_renderer: BrowseResults,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResults {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<Tab<YouTubeListRendererWrap>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab<T> {
    pub tab_renderer: ContentRenderer<T>,
}
