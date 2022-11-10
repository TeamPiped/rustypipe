use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, VecSkipError};

use super::url_endpoint::NavigationEndpoint;
use super::{Alert, ChannelBadge, ContentsRenderer, ResponseContext, Thumbnails, YouTubeListItem};
use crate::serializer::ignore_any;
use crate::serializer::{text::Text, MapResult, VecLogError};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Channel {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub header: Option<Header>,
    pub contents: Option<Contents>,
    pub metadata: Option<Metadata>,
    pub microformat: Option<Microformat>,
    #[serde_as(as = "Option<DefaultOnError>")]
    pub alerts: Option<Vec<Alert>>,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub two_column_browse_results_renderer: TabsRenderer,
}

/// YouTube channel tab view. Contains multiple tabs
/// (Home, Videos, Playlists, About...). We can ignore unknown tabs.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<TabRendererWrap>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabRendererWrap {
    pub tab_renderer: TabRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabRenderer {
    #[serde(default)]
    pub content: TabContent,
    pub endpoint: ChannelTabEndpoint,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContent {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub section_list_renderer: Option<ContentsRenderer<ItemSectionRendererWrap>>,
    /// Seems to be currently A/B tested, as of 11.10.2022
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub rich_grid_renderer: Option<RichGridRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelTabEndpoint {
    pub command_metadata: ChannelTabCommandMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelTabCommandMetadata {
    pub web_command_metadata: ChannelTabWebCommandMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelTabWebCommandMetadata {
    pub url: String,
}

/// Seems to be currently A/B tested, as of 11.10.2022
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RichGridRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemSectionRendererWrap {
    pub item_section_renderer: ContentsRenderer<ChannelContent>,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChannelContent {
    GridRenderer {
        #[serde_as(as = "VecLogError<_>")]
        items: MapResult<Vec<YouTubeListItem>>,
    },
    ChannelAboutFullMetadataRenderer(ChannelFullMetadata),
    #[default]
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Header {
    C4TabbedHeaderRenderer(HeaderRenderer),
    /// Used for special channels like YouTube Music
    CarouselHeaderRenderer(ContentsRenderer<CarouselHeaderRendererItem>),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeaderRenderer {
    /// Approximate subscriber count (e.g. `880K subscribers`), depends on language.
    ///
    /// `None` if the subscriber count is hidden.
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    pub avatar: Thumbnails,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
    #[serde(default)]
    pub banner: Thumbnails,
    #[serde(default)]
    pub mobile_banner: Thumbnails,
    /// Fullscreen (16:9) channel banner
    #[serde(default)]
    pub tv_banner: Thumbnails,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CarouselHeaderRendererItem {
    #[serde(rename_all = "camelCase")]
    TopicChannelDetailsRenderer {
        #[serde_as(as = "Option<Text>")]
        subscriber_count_text: Option<String>,
        #[serde(default)]
        avatar: Thumbnails,
    },
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metadata {
    pub channel_metadata_renderer: ChannelMetadataRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMetadataRenderer {
    pub title: String,
    /// Channel ID
    pub external_id: String,
    pub description: String,
    pub vanity_channel_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Microformat {
    pub microformat_data_renderer: MicroformatDataRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicroformatDataRenderer {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelFullMetadata {
    #[serde_as(as = "Text")]
    pub joined_date_text: String,
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub primary_links: Vec<PrimaryLink>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrimaryLink {
    #[serde_as(as = "Text")]
    pub title: String,
    pub navigation_endpoint: NavigationEndpoint,
}
