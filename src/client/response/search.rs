use serde::Deserialize;
use serde_with::json::JsonString;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::ignore_any;
use crate::serializer::{text::Text, MapResult, VecLogError};

use super::{
    ChannelRenderer, ContentsRenderer, ContinuationEndpoint, PlaylistRenderer, VideoRenderer,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    #[serde_as(as = "Option<JsonString>")]
    pub estimated_results: Option<u64>,
    pub contents: Contents,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCont {
    #[serde_as(as = "Option<JsonString>")]
    pub estimated_results: Option<u64>,
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_commands: Vec<SearchContCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContCommand {
    pub append_continuation_items_action: SearchContAction,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContAction {
    pub continuation_items: Vec<SectionListItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_search_results_renderer: TwoColumnSearchResultsRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnSearchResultsRenderer {
    pub primary_contents: PrimaryContents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryContents {
    pub section_list_renderer: ContentsRenderer<SectionListItem>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionListItem {
    #[serde(rename_all = "camelCase")]
    ItemSectionRenderer {
        #[serde_as(as = "VecLogError<_>")]
        contents: MapResult<Vec<SearchItem>>,
    },
    /// Continuation token to fetch more search results
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchItem {
    /// Video in search results
    VideoRenderer(VideoRenderer),
    /// Playlist in search results
    PlaylistRenderer(PlaylistRenderer),
    /// Channel displayed in search results
    ChannelRenderer(ChannelRenderer),

    /// Corrected search query
    #[serde(rename_all = "camelCase")]
    ShowingResultsForRenderer {
        #[serde_as(as = "Text")]
        corrected_query: String,
    },
    /// No search result item (e.g. ad) or unimplemented item
    ///
    /// Unimplemented:
    /// - shelfRenderer (e.g. Latest from channel, For you)
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}
