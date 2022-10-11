use serde::Deserialize;
use serde_with::json::JsonString;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::ignore_any;
use crate::serializer::{
    text::{Text, TextComponent},
    MapResult, VecLogError,
};

use super::{ChannelBadge, ContentsRenderer, ContinuationEndpoint, Thumbnails, TimeOverlay};

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

/// Video displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    pub channel_thumbnail_supported_renderers: ChannelThumbnailSupportedRenderers,
    #[serde_as(as = "Option<Text>")]
    pub published_time_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
    /// Contains `No views` if the view count is zero
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
    /// Contains Short/Live tag
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub detailed_metadata_snippets: Vec<DetailedMetadataSnippet>,
}

/// Playlist displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    /// The first item of this list contains the playlist thumbnail,
    /// subsequent items contain very small thumbnails of the next playlist videos
    pub thumbnails: Vec<Thumbnails>,
    #[serde_as(as = "JsonString")]
    pub video_count: u64,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
    /// First 2 videos
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub videos: Vec<ChildVideoRendererWrap>,
}

/// Channel displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRenderer {
    pub channel_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    pub thumbnail: Thumbnails,
    /// Abbreviated channel description
    ///
    /// Not present if the channel has no description
    #[serde(default)]
    #[serde_as(as = "Text")]
    pub description_snippet: String,
    /// Not present if the channel has no videos
    #[serde_as(as = "Option<Text>")]
    pub video_count_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelThumbnailSupportedRenderers {
    pub channel_thumbnail_with_link_renderer: ChannelThumbnailWithLinkRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelThumbnailWithLinkRenderer {
    pub thumbnail: Thumbnails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildVideoRendererWrap {
    pub child_video_renderer: ChildVideoRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildVideoRenderer {
    pub video_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedMetadataSnippet {
    #[serde_as(as = "Text")]
    pub snippet_text: String,
}
