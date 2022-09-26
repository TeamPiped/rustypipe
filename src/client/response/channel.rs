use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

use super::ChannelBadge;
use super::Thumbnails;
use super::{ContentRenderer, ContentsRenderer, VideoListItem};
use crate::serializer::ignore_any;
use crate::serializer::{text::Text, MapResult, VecLogError};

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub header: Header,
    pub contents: Contents,
    pub metadata: Metadata,
    pub microformat: Microformat,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_browse_results_renderer: TabsRenderer,
}

/// YouTube channel tab view. Contains multiple tabs
/// (Home, Videos, Playlists, About...). We can ignore unknown tabs.
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<TabRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererWrap {
    pub tab_renderer: ContentRenderer<SectionListRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRendererWrap {
    pub section_list_renderer: SectionListRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    pub contents: Vec<ItemSectionRendererWrap>,
    /// - **Videos**: browse-feedUC2DjFE7Xf11URZqWBigcVOQvideos (...)
    /// - **Playlists**: browse-feedUC2DjFE7Xf11URZqWBigcVOQplaylists104 (...)
    /// - **Info**: None
    pub target_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRendererWrap {
    pub item_section_renderer: ContentsRenderer<ChannelContent>,
}

#[serde_as]
#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChannelContent {
    GridRenderer {
        #[serde_as(as = "VecLogError<_>")]
        items: MapResult<Vec<VideoListItem>>,
    },
    ChannelAboutFullMetadataRenderer(ChannelFullMetadata),
    #[default]
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub c4_tabbed_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    pub channel_id: String,
    /// Channel name
    pub title: String,
    /// Approximate subscriber count (e.g. `880K subscribers`), depends on language.
    ///
    /// `None` if the subscriber count is hidden.
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    pub avatar: Thumbnails,
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub badges: Option<Vec<ChannelBadge>>,
    #[serde(default)]
    pub banner: Thumbnails,
    #[serde(default)]
    pub mobile_banner: Thumbnails,
    /// Fullscreen (16:9) channel banner
    #[serde(default)]
    pub tv_banner: Thumbnails,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub channel_metadata_renderer: ChannelMetadataRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMetadataRenderer {
    pub description: String,
    pub vanity_channel_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Microformat {
    pub microformat_data_renderer: MicroformatDataRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroformatDataRenderer {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelFullMetadata {
    #[serde_as(as = "Text")]
    pub joined_date_text: String,
    #[serde_as(as = "Text")]
    pub view_count_text: String,
    #[serde_as(as = "VecSkipError<_>")]
    pub primary_links: Vec<PrimaryLink>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryLink {
    #[serde_as(as = "Text")]
    pub title: String,
    pub navigation_endpoint: NavigationEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub url_endpoint: UrlEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlEndpoint {
    pub url: String,
}
