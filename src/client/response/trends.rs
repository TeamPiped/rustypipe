use serde::Deserialize;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::{ignore_any, MapResult, VecLogError};

use super::{ContentRenderer, ContentsRenderer, VideoListItem, VideoRenderer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Startpage {
    pub contents: Contents<BrowseResultsStartpage>,
    pub response_context: ResponseContext,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartpageCont {
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_actions: Vec<OnResponseReceivedAction>,
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
    pub contents: MapResult<Vec<VideoListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trending {
    pub contents: Contents<BrowseResultsTrends>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingTabContent {
    pub section_list_renderer: ContentsRenderer<ItemSectionRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRenderer {
    pub item_section_renderer: ContentsRenderer<ShelfRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfRenderer {
    pub shelf_renderer: ContentRenderer<ShelfContents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfContents {
    pub expanded_shelf_contents_renderer: Option<ShelfContentsRenderer>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfContentsRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub items: MapResult<Vec<TrendingListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseContext {
    pub visitor_data: Option<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum TrendingListItem {
    VideoRenderer(VideoRenderer),

    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnResponseReceivedAction {
    pub append_continuation_items_action: AppendAction,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendAction {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<VideoListItem>>,
}
