use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, VecSkipError};

use super::Thumbnails;
use super::{Alert, ChannelBadge};
use super::{ContentRenderer, ContentsRenderer, VideoListItem};
use crate::serializer::ignore_any;
use crate::serializer::{text::Text, MapResult, VecLogError};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub header: Option<Header>,
    pub contents: Option<Contents>,
    pub metadata: Option<Metadata>,
    pub microformat: Option<Microformat>,
    #[serde_as(as = "Option<DefaultOnError>")]
    pub alerts: Option<Vec<Alert>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCont {
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_actions: Vec<OnResponseReceivedAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_browse_results_renderer: TabsRenderer,
}

/// YouTube channel tab view. Contains multiple tabs
/// (Home, Videos, Playlists, About...). We can ignore unknown tabs.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<TabRendererWrap>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererWrap {
    pub tab_renderer: ContentRenderer<TabContent>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabContent {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub section_list_renderer: Option<SectionListRenderer>,
    /// Seems to be currently A/B tested, as of 11.10.2022
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub rich_grid_renderer: Option<RichGridRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    pub contents: Vec<ItemSectionRendererWrap>,
    /// - **Videos**: browse-feedUC2DjFE7Xf11URZqWBigcVOQvideos (...)
    /// - **Playlists**: browse-feedUC2DjFE7Xf11URZqWBigcVOQplaylists104 (...)
    /// - **Info**: None
    pub target_id: Option<String>,
}

/// Seems to be currently A/B tested, as of 11.10.2022
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichGridRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<VideoListItem>>,
    /// - **Videos**: browse-feedUC2DjFE7Xf11URZqWBigcVOQvideos (...)
    /// - **Playlists**: browse-feedUC2DjFE7Xf11URZqWBigcVOQplaylists104 (...)
    /// - **Info**: None
    pub target_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRendererWrap {
    pub item_section_renderer: ContentsRenderer<ChannelContent>,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Header {
    C4TabbedHeaderRenderer(HeaderRenderer),
    /// Used for special channels like YouTube Music
    CarouselHeaderRenderer(ContentsRenderer<CarouselHeaderRendererItem>),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CarouselHeaderRendererItem {
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
pub struct Metadata {
    pub channel_metadata_renderer: ChannelMetadataRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMetadataRenderer {
    pub title: String,
    /// Channel ID
    pub external_id: String,
    pub description: String,
    pub vanity_channel_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Microformat {
    pub microformat_data_renderer: MicroformatDataRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroformatDataRenderer {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelFullMetadata {
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
pub struct PrimaryLink {
    #[serde_as(as = "Text")]
    pub title: String,
    pub navigation_endpoint: NavigationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub url_endpoint: UrlEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlEndpoint {
    pub url: String,
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
