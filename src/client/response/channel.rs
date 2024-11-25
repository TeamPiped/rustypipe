use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use super::{
    video_item::YouTubeListRenderer, Alert, AttachmentRun, AvatarViewModel, ChannelBadge,
    ContentRenderer, ContentsRenderer, ContinuationActionWrap, ImageView,
    PageHeaderRendererContent, PhMetadataView, ResponseContext, Thumbnails, TwoColumnBrowseResults,
};
use crate::{
    model::Verification,
    serializer::text::{AttributedText, Text, TextComponent},
};

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
    pub endpoint: Option<ChannelTabEndpoint>,
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
#[allow(clippy::enum_variant_names)]
pub(crate) enum Header {
    C4TabbedHeaderRenderer(HeaderRenderer),
    /// Used for special channels like YouTube Music
    CarouselHeaderRenderer(ContentsRenderer<CarouselHeaderRendererItem>),
    PageHeaderRenderer(ContentRenderer<PageHeaderRendererContent<PageHeaderRendererInner>>),
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageHeaderRendererInner {
    /// Channel title (only used to extract verification badges)
    #[serde_as(as = "DefaultOnError")]
    pub title: Option<PhTitleView>,
    /// Channel avatar
    pub image: PhAvatarView,
    /// Channel metadata (subscribers, video count)
    pub metadata: PhMetadataView,
    #[serde(default)]
    pub banner: PhBannerView,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhTitleView {
    pub dynamic_text_view_model: PhTitleView2,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhTitleView2 {
    pub text: PhTitleView3,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhTitleView3 {
    #[serde_as(as = "VecSkipError<_>")]
    pub attachment_runs: Vec<AttachmentRun>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhAvatarView {
    pub decorated_avatar_view_model: PhAvatarView2,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhAvatarView2 {
    pub avatar: AvatarViewModel,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhBannerView {
    pub image_banner_view_model: ImageView,
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
#[serde(untagged)]
pub(crate) enum ChannelAbout {
    #[serde(rename_all = "camelCase")]
    ReceivedEndpoints {
        #[serde_as(as = "VecSkipError<_>")]
        on_response_received_endpoints: Vec<ContinuationActionWrap<AboutChannelRendererWrap>>,
    },
    Content {
        contents: Option<Contents>,
    },
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

impl From<PhTitleView> for crate::model::Verification {
    fn from(value: PhTitleView) -> Self {
        value
            .dynamic_text_view_model
            .text
            .attachment_runs
            .into_iter()
            .next()
            .map(Verification::from)
            .unwrap_or_default()
    }
}
