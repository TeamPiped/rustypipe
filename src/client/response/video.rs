use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, VecSkipError};

use crate::serializer::text::TextLink;

use super::{ContinuationEndpoint, Icon, Thumbnails, VideoListItem, VideoOwner};

/// Video info response
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub contents: Contents,
    #[serde_as(as = "VecSkipError<_>")]
    pub engagement_panels: Vec<EngagementPanel>,
}

/// Video recommendations response
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRecommendations {
    pub on_response_received_endpoints: Vec<RecommendationsContItem>,
}

/// Video comments response
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoComments {
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_endpoints: Vec<CommentsContItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_watch_next_results: TwoColumnWatchNextResults,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnWatchNextResults {
    pub results: VideoResultsWrap,
    pub secondary_results: RecommendationResultsWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResultsWrap {
    pub results: VideoResults,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResults {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<VideoResultsItem>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoResultsItem {
    #[serde(rename_all = "camelCase")]
    VideoPrimaryInfoRenderer {
        #[serde_as(as = "crate::serializer::text::Text")]
        title: String,
        view_count: ViewCountWrap,
        video_actions: VideoActions,
        #[serde_as(as = "crate::serializer::text::Text")]
        date_text: String,
    },
    #[serde(rename_all = "camelCase")]
    VideoSecondaryInfoRenderer {
        owner: VideoOwner,
        #[serde_as(as = "crate::serializer::text::Text")]
        description: String,
        #[serde_as(deserialize_as = "DefaultOnError")]
        metadata_row_container: Option<MetadataRowContainer>,
    },
    #[serde(rename_all = "camelCase")]
    ItemSectionRenderer {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ItemSection>,
        section_identifier: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCountWrap {
    pub video_view_count_renderer: ViewCount,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCount {
    #[serde_as(as = "crate::serializer::text::Text")]
    pub view_count: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActions {
    pub menu_renderer: VideoActionsMenu,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActionsMenu {
    #[serde_as(as = "VecSkipError<_>")]
    pub top_level_buttons: Vec<ToggleButtonWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButtonWrap {
    pub toggle_button_renderer: ToggleButton,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButton {
    pub default_icon: Icon,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub default_text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainer {
    pub metadata_row_container_renderer: MetadataRowContainerRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainerRenderer {
    pub rows: Vec<MetadataRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRow {
    pub metadata_row_renderer: MetadataRowRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowRenderer {
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde_as(as = "Vec<crate::serializer::text::TextLinks>")]
    pub contents: Vec<Vec<TextLink>>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemSection {
    #[serde(rename_all = "camelCase")]
    CommentsEntryPointHeaderRenderer {
        #[serde_as(as = "crate::serializer::text::Text")]
        header_text: String,
        #[serde_as(as = "crate::serializer::text::Text")]
        comment_count: String,
    },
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResultsWrap {
    pub secondary_results: RecommendationResults,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResults {
    #[serde_as(as = "VecSkipError<_>")]
    pub results: Vec<VideoListItem<RecommendedVideo>>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedVideo {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    #[serde_as(as = "crate::serializer::text::TextLink")]
    pub channel: TextLink,
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub length_text: Option<String>,
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub published_time_text: Option<String>,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub view_count_text: String,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<VideoBadge>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBadge {
    pub metadata_badge_renderer: VideoBadgeRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBadgeRenderer {
    pub style: VideoBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoBadgeStyle {
    BadgeStyleTypeLiveNow,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanel {
    pub engagement_panel_section_list_renderer: EngagementPanelRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelRenderer {
    pub header: EngagementPanelHeader,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelHeader {
    pub engagement_panel_title_header_renderer: EngagementPanelHeaderRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelHeaderRenderer {
    pub menu: EngagementPanelMenu,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelMenu {
    pub sort_filter_sub_menu_renderer: EngagementPanelMenuRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelMenuRenderer {
    pub sub_menu_items: Vec<EngagementPanelMenuItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanelMenuItem {
    pub service_endpoint: ContinuationEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsContItem {
    pub append_continuation_items_action: AppendRecommendations,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecommendations {
    pub continuation_items: Vec<VideoListItem<RecommendedVideo>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsContItem {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: AppendComments,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendComments {
    #[serde_as(as = "VecSkipError<_>")]
    pub continuation_items: Vec<CommentListItem>,
    pub target_id: String,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentListItem {
    #[serde(rename_all = "camelCase")]
    CommentThreadRenderer {
        comment: Comment,
        #[serde(default)]
        replies: Replies,
        #[serde(default)]
        #[serde_as(deserialize_as = "DefaultOnError")]
        rendering_priority: CommentPriority,
    },
    CommentRenderer {
        #[serde(flatten)]
        comment: CommentRenderer,
    },
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },

    // TODO: TMP
    #[serde(rename_all = "camelCase")]
    CommentsHeaderRenderer { count_text: Option<String> },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub comment_renderer: CommentRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRenderer {
    /*
    #[serde_as(as = "crate::serializer::text::Text")]
    pub author_text: String,
    pub author_thumbnail: Thumbnails,
    pub author_endpoint: AuthorEndpoint,
    */
    // There may be comments with missing authors (possibly deleted users?)
    #[serde_as(as = "DefaultOnError<Option<crate::serializer::text::Text>>")]
    pub author_text: Option<String>,
    pub author_thumbnail: Thumbnails,
    #[serde_as(as = "DefaultOnError")]
    pub author_endpoint: Option<AuthorEndpoint>,

    #[serde_as(as = "crate::serializer::text::Text")]
    pub content_text: String,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub published_time_text: String,
    pub comment_id: String,
    pub author_is_channel_owner: bool,
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub vote_count: Option<String>,
    pub author_comment_badge: Option<AuthorCommentBadge>,
    #[serde(default)]
    pub reply_count: u32,
    pub action_buttons: CommentActionButtons,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorEndpoint {
    pub browse_endpoint: BrowseEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommentPriority {
    #[default]
    RenderingPriorityUnknown,
    RenderingPriorityPinnedComment,
}

#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replies {
    pub comment_replies_renderer: RepliesRenderer,
}

#[serde_as]
#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepliesRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<CommentListItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtons {
    pub comment_action_buttons_renderer: CommentActionButtonsRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtonsRenderer {
    pub creator_heart: Option<CreatorHeart>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHeart {
    pub creator_heart_renderer: CreatorHeartRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHeartRenderer {
    pub is_hearted: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCommentBadge {
    pub author_comment_badge_renderer: AuthorCommentBadgeRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCommentBadgeRenderer {
    pub icon: Icon,
}
