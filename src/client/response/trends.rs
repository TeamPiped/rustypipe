use serde::Deserialize;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::{MapResult, VecLogError};

use super::{ContentRenderer, YouTubeListItem};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Startpage {
    pub contents: Contents<BrowseResultsStartpage>,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents<T> {
    pub two_column_browse_results_renderer: T,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResultsStartpage {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<Tab<StartpageTabContent>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResultsTrends {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<Tab<TrendingTabContent>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab<T> {
    pub tab_renderer: ContentRenderer<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartpageTabContent {
    pub rich_grid_renderer: RichGridRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichGridRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trending {
    pub contents: Contents<BrowseResultsTrends>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingTabContent {
    pub section_list_renderer: SectionListRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseContext {
    pub visitor_data: Option<String>,
}
