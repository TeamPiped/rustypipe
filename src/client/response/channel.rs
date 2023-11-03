use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use super::{
    video_item::YouTubeListRenderer, Alert, ChannelBadge, ContentsRenderer, ContinuationActionWrap,
    ResponseContext, Thumbnails, TwoColumnBrowseResults,
};
use crate::serializer::text::{AttributedText, Text, TextComponent};

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

pub(crate) type Contents = TwoColumnBrowseResults<TabRendererWrap>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabRendererWrap {
    #[serde(alias = "expandableTabRenderer")]
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
    pub section_list_renderer: Option<YouTubeListRenderer>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub rich_grid_renderer: Option<YouTubeListRenderer>,
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
        #[serde_as(as = "Option<Text>")]
        subtitle: Option<String>,
        #[serde(default)]
        avatar: Thumbnails,
    },
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
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
pub(crate) struct ChannelAbout {
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_endpoints: Vec<ContinuationActionWrap<AboutChannelRendererWrap>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AboutChannelRendererWrap {
    pub about_channel_renderer: AboutChannelRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AboutChannelRenderer {
    pub metadata: ChannelMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMetadata {
    pub about_channel_view_model: ChannelMetadataView,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMetadataView {
    pub channel_id: String,
    pub canonical_channel_url: String,
    pub country: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde_as(as = "Option<Text>")]
    pub joined_date_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub video_count_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    #[serde(default)]
    pub links: Vec<ExternalLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalLink {
    pub channel_external_link_view_model: ExternalLinkInner,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalLinkInner {
    #[serde_as(as = "AttributedText")]
    pub title: TextComponent,
    #[serde_as(as = "AttributedText")]
    pub link: TextComponent,
}
