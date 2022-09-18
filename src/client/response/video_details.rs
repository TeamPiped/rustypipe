#![allow(clippy::enum_variant_names)]

use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, VecSkipError};

use crate::serializer::MapResult;
use crate::serializer::{
    ignore_any,
    text::{AccessibilityText, Text, TextLink, TextLinks},
    VecLogError,
};

use super::{ContentsRenderer, ContinuationEndpoint, Icon, Thumbnails, VideoListItem, VideoOwner};

/*
#VIDEO DETAILS
*/

/// Video details response
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    /// Video metadata + recommended videos
    pub contents: Contents,
    #[serde_as(as = "VecLogError<_>")]
    /// Video chapters + comment section
    pub engagement_panels: MapResult<Vec<EngagementPanel>>,
}

/// Video details main object, contains video metadata and recommended videos
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_watch_next_results: TwoColumnWatchNextResults,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnWatchNextResults {
    /// Metadata about the video
    pub results: VideoResultsWrap,
    /// Video recommendations
    pub secondary_results: RecommendationResultsWrap,
}

/// Metadata about the video
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResultsWrap {
    pub results: VideoResults,
}

/// Video metadata items
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResults {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<VideoResultsItem>>,
}

/// Video metadata item
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoResultsItem {
    #[serde(rename_all = "camelCase")]
    VideoPrimaryInfoRenderer {
        #[serde_as(as = "Text")]
        title: String,
        view_count: ViewCount,
        /// Like/Dislike button
        video_actions: VideoActions,
        /// Absolute textual date (e.g. `Dec 29, 2019`)
        #[serde_as(as = "Text")]
        date_text: String,
    },
    #[serde(rename_all = "camelCase")]
    VideoSecondaryInfoRenderer {
        owner: VideoOwner,
        #[serde_as(as = "Text")]
        description: String,
        /// Additional metadata (e.g. Creative Commons License)
        #[serde(default)]
        #[serde_as(deserialize_as = "DefaultOnError")]
        metadata_row_container: Option<MetadataRowContainer>,
    },
    /*
    /// The comment section consists of 2 ItemSectionRenderers:
    ///
    /// 1. sectionIdentifier: "comments-entry-point", contains number of comments
    /// 2. sectionIdentifier: "comment-item-section", contains continuation token
    #[serde(rename_all = "camelCase")]
    ItemSectionRenderer {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ItemSection>,
        section_identifier: String,
    },
    */
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCount {
    pub video_view_count_renderer: ViewCountRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCountRenderer {
    #[serde_as(as = "Text")]
    pub view_count: String,
}

/// Like/Dislike buttons
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActions {
    pub menu_renderer: VideoActionsMenu,
}

/// Like/Dislike buttons
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActionsMenu {
    #[serde_as(as = "VecSkipError<_>")]
    pub top_level_buttons: Vec<ToggleButtonWrap>,
}

/// Like/Dislike button
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButtonWrap {
    pub toggle_button_renderer: ToggleButton,
}

/// Like/Dislike button
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButton {
    /// Icon type: `LIKE` / `DISLIKE`
    pub default_icon: Icon,
    /// Number of likes (`4,010,157 likes`)
    #[serde_as(as = "AccessibilityText")]
    pub default_text: String,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainer {
    pub metadata_row_container_renderer: MetadataRowContainerRenderer,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainerRenderer {
    pub rows: Vec<MetadataRow>,
}

/// Additional video metadata item (Creative Commons License)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRow {
    pub metadata_row_renderer: MetadataRowRenderer,
}

/// Additional video metadata item (Creative Commons License)
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowRenderer {
    // `License`
    // #[serde_as(as = "Text")]
    // pub title: String,
    /// Creative commons license:
    ///
    /// Text (en): `Creative Commons Attribution license (reuse allowed)`
    ///
    /// URL: `https://www.youtube.com/t/creative_commons`
    #[serde_as(as = "Vec<TextLinks>")]
    pub contents: Vec<Vec<TextLink>>,
}

/*
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemSection {
    #[serde(rename_all = "camelCase")]
    CommentsEntryPointHeaderRenderer {
        #[serde_as(as = "Text")]
        comment_count: String,
    },
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
}
*/

/// Video recommendations
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResultsWrap {
    pub secondary_results: RecommendationResults,
}

/// Video recommendations
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResults {
    #[serde_as(as = "VecLogError<_>")]
    pub results: MapResult<Vec<VideoListItem<RecommendedVideo>>>,
}

/// Video recommendation item
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedVideo {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    #[serde_as(as = "TextLink")]
    pub channel: TextLink,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub published_time_text: Option<String>,
    #[serde_as(as = "Text")]
    pub view_count_text: String,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<VideoBadge>,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBadge {
    pub metadata_badge_renderer: VideoBadgeRenderer,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBadgeRenderer {
    pub style: VideoBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoBadgeStyle {
    /// Active livestream
    BadgeStyleTypeLiveNow,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanel {
    pub engagement_panel_section_list_renderer: EngagementPanelRenderer,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "panelIdentifier")]
pub enum EngagementPanelRenderer {
    /// Chapter markers
    EngagementPanelMacroMarkersDescriptionChapters { content: ChapterMarkersContent },
    /// Comment section (contains no comments, but the
    /// continuation tokens for fetching top/latest comments)
    CommentItemSection { header: CommentItemSectionHeader },
    /// Ignored items:
    /// - `engagement-panel-ads`
    /// - `engagement-panel-structured-description`
    ///   (Desctiption already included in `VideoSecondaryInfoRenderer`)
    /// - `engagement-panel-searchable-transcript`
    ///   (basically video subtitles in a different format)
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

/// Chapter markers
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMarkersContent {
    pub macro_markers_list_renderer: ContentsRenderer<MacroMarkersListItem>,
}

/// Chapter marker
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItem {
    pub macro_markers_list_item_renderer: MacroMarkersListItemRenderer,
}

/// Chapter marker
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemRenderer {
    /// Contains chapter start time in seconds
    pub on_tap: MacroMarkersListItemOnTap,
    pub thumbnail: Thumbnails,
    /// Textual time (`1:42`)
    #[serde_as(as = "Text")]
    pub time_description: String,
    /// Chapter title
    #[serde_as(as = "Text")]
    pub title: String,
}

/// Contains chapter start time in seconds
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemOnTap {
    pub watch_endpoint: MacroMarkersListItemWatchEndpoint,
}
/// Contains chapter start time in seconds
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemWatchEndpoint {
    /// Chapter start time in seconds
    pub start_time_seconds: u32,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeader {
    pub engagement_panel_title_header_renderer: CommentItemSectionHeaderRenderer,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderRenderer {
    /// Average comment count (e.g. `81`, `2.2K`, `705K`)
    ///
    /// The accurate count is included in the first comment response.
    #[serde_as(as = "Text")]
    pub contextual_info: String,
    pub menu: CommentItemSectionHeaderMenu,
}

/// Comment section menu
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenu {
    pub sort_filter_sub_menu_renderer: CommentItemSectionHeaderMenuRenderer,
}

/// Comment section menu
///
/// Items:
/// - Top comments
/// - Latest comments
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenuRenderer {
    pub sub_menu_items: Vec<CommentItemSectionHeaderMenuItem>,
}

/// Comment section menu item
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenuItem {
    /// Continuation token for fetching comments
    pub service_endpoint: ContinuationEndpoint,
}

/*
#RECOMMENDATIONS CONTINUATION
*/

/// Video recommendations continuation response
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRecommendations {
    pub on_response_received_endpoints: Vec<RecommendationsContItem>,
}

/// Video recommendations continuation
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsContItem {
    pub append_continuation_items_action: AppendRecommendations,
}

/// Video recommendations continuation
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecommendations {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<VideoListItem<RecommendedVideo>>>,
}

/*
#COMMENTS CONTINUATION
*/

/// Video comments continuation response
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoComments {
    /// - Initial response: 2*reloadContinuationItemsCommand
    ///   - 1*commentsHeaderRenderer: number of comments
    ///   - n*commentThreadRenderer, continuationItemRenderer:
    ///     comments + continuation
    /// - Continuation response: appendContinuationItemsAction
    ///   - n*commentThreadRenderer, continuationItemRenderer:
    ///     comments + continuation
    /// - Comment replies: appendContinuationItemsAction
    ///   - n*commentRenderer, continuationItemRenderer:
    ///     replies + continuation
    #[serde_as(as = "VecLogError<_>")]
    pub on_response_received_endpoints: MapResult<Vec<CommentsContItem>>,
}

/// Video comments continuation
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsContItem {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: AppendComments,
}

/// Video comments continuation action
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendComments {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<CommentListItem>>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentListItem {
    /// Top-level comment
    #[serde(rename_all = "camelCase")]
    CommentThreadRenderer {
        comment: Comment,
        /// Continuation token to fetch replies
        #[serde(default)]
        replies: Replies,
        #[serde(default)]
        #[serde_as(deserialize_as = "DefaultOnError")]
        rendering_priority: CommentPriority,
    },
    /// Reply comment
    CommentRenderer {
        #[serde(flatten)]
        comment: CommentRenderer,
    },
    /// Continuation token to fetch more comments
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
    /// Header of the comment section (contains number of comments)
    #[serde(rename_all = "camelCase")]
    CommentsHeaderRenderer {
        #[serde_as(as = "Text")]
        count_text: Vec<String>
    },
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
    /// Author name
    ///
    /// There may be comments with missing authors (possibly deleted users?)
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Option<Text>>")]
    pub author_text: Option<String>,
    pub author_thumbnail: Thumbnails,
    #[serde(default)]
    /// ID of the author's channel
    #[serde_as(as = "DefaultOnError")]
    pub author_endpoint: Option<AuthorEndpoint>,
    /// Comment text
    #[serde_as(as = "Text")]
    pub content_text: String,
    /// Textual publish date (e.g. `15 minutes ago`, `2 days ago`)
    #[serde_as(as = "Text")]
    pub published_time_text: String,
    pub comment_id: String,
    pub author_is_channel_owner: bool,
    #[serde_as(as = "Option<Text>")]
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
    /// Default rendering priority
    #[default]
    RenderingPriorityUnknown,
    /// Comment pinned by the creator
    RenderingPriorityPinnedComment,
}

/// Does not contain replies directly but a continuation token
/// for fetching them.
#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replies {
    pub comment_replies_renderer: RepliesRenderer,
}

/// Does not contain replies directly but a continuation token
/// for fetching them.
#[serde_as]
#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepliesRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<CommentListItem>,
}

/// These are the buttons for comment interaction. Contains the CreatorHeart.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtons {
    pub comment_action_buttons_renderer: CommentActionButtonsRenderer,
}

/// These are the buttons for comment interaction. Contains the CreatorHeart.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtonsRenderer {
    pub creator_heart: Option<CreatorHeart>,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHeart {
    pub creator_heart_renderer: CreatorHeartRenderer,
}

/// Video creators can endorse comments by marking them with a ❤️.
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

/// YouTube channel badge (verified) of the comment author
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCommentBadgeRenderer {
    /// Verified: `CHECK`
    pub icon: Icon,
}
