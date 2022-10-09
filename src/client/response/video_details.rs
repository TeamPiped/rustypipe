#![allow(clippy::enum_variant_names)]

use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, VecSkipError};

use crate::serializer::{
    ignore_any,
    text::{AccessibilityText, AttributedText, Text, TextComponents},
    MapResult, VecLogError,
};

use super::{
    ContinuationEndpoint, ContinuationItemRenderer, Icon, Thumbnails, VideoListItem, VideoOwner,
};

/*
#VIDEO DETAILS
*/

/// Video details response
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    /// Video metadata + recommended videos
    pub contents: Contents,
    /// Video ID
    pub current_video_endpoint: CurrentVideoEndpoint,
    /// Video chapters + comment section
    #[serde_as(as = "VecLogError<_>")]
    pub engagement_panels: MapResult<Vec<EngagementPanel>>,
}

/// Video details main object, contains video metadata and recommended videos
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_watch_next_results: TwoColumnWatchNextResults,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnWatchNextResults {
    /// Metadata about the video
    pub results: VideoResultsWrap,
    /// Video recommendations
    ///
    /// Can be `None` for age-restricted videos
    pub secondary_results: Option<RecommendationResultsWrap>,
}

/// Metadata about the video
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResultsWrap {
    pub results: VideoResults,
}

/// Video metadata items
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResults {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<VideoResultsItem>>,
}

/// Video metadata item
#[serde_as]
#[derive(Debug, Deserialize)]
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
        description: Option<TextComponents>,
        #[serde_as(as = "Option<AttributedText>")]
        attributed_description: Option<TextComponents>,
        /// Additional metadata (e.g. Creative Commons License)
        #[serde(default)]
        #[serde_as(deserialize_as = "DefaultOnError")]
        metadata_row_container: Option<MetadataRowContainer>,
    },
    /// The comment section consists of 2 ItemSectionRenderers:
    ///
    /// 1. sectionIdentifier: "comments-entry-point", contains number of comments
    /// 2. sectionIdentifier: "comment-item-section", contains continuation token
    ItemSectionRenderer(#[serde_as(deserialize_as = "DefaultOnError")] ItemSection),
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCount {
    pub video_view_count_renderer: ViewCountRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCountRenderer {
    /// View count (`232,975,196 views`)
    #[serde_as(as = "Text")]
    pub view_count: String,
    #[serde(default)]
    pub is_live: bool,
}

/// Like/Dislike buttons
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActions {
    pub menu_renderer: VideoActionsMenu,
}

/// Like/Dislike buttons
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActionsMenu {
    #[serde_as(as = "VecSkipError<_>")]
    pub top_level_buttons: Vec<TopLevelButton>,
}

/// The different TopLevelButtons
///
/// YouTube seems to be A/B testing the SegmentedLikeDislikeButtonRenderer
///
/// See: https://github.com/TeamNewPipe/NewPipeExtractor/pull/926
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopLevelButton {
    ToggleButtonRenderer(ToggleButton),
    #[serde(rename_all = "camelCase")]
    SegmentedLikeDislikeButtonRenderer {
        like_button: ToggleButtonWrap,
    },
}

/// Like/Dislike button
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButtonWrap {
    pub toggle_button_renderer: ToggleButton,
}

/// Like/Dislike button
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleButton {
    /// Icon type: `LIKE` / `DISLIKE`
    pub default_icon: Icon,
    /// Number of likes (`like this video along with 4,010,156 other people`)
    ///
    /// Contains no digits (e.g. `I like this`) if likes are hidden by the creator.
    #[serde_as(as = "AccessibilityText")]
    pub accessibility_data: String,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainer {
    pub metadata_row_container_renderer: MetadataRowContainerRenderer,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRowContainerRenderer {
    pub rows: Vec<MetadataRow>,
}

/// Additional video metadata item (Creative Commons License)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRow {
    pub metadata_row_renderer: MetadataRowRenderer,
}

/// Additional video metadata item (Creative Commons License)
#[serde_as]
#[derive(Debug, Deserialize)]
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
    pub contents: Vec<TextComponents>,
}

/// Contains current video ID
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentVideoEndpoint {
    pub watch_endpoint: CurrentVideoWatchEndpoint,
}
/// Contains current video ID
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentVideoWatchEndpoint {
    pub video_id: String,
}

/// The comment section consists of 2 ItemSections:
///
/// 1. CommentsEntryPointHeaderRenderer: contains number of comments
/// 2. ContinuationItemRenderer: contains continuation token
#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "sectionIdentifier")]
pub enum ItemSection {
    CommentsEntryPoint {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ItemSectionCommentCount>,
    },
    CommentItemSection {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ItemSectionComments>,
    },
    #[default]
    None,
}

/// Item section containing comment count
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionCommentCount {
    pub comments_entry_point_header_renderer: CommentsEntryPointHeaderRenderer,
}

/// Renderer of item section containing comment count
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsEntryPointHeaderRenderer {
    #[serde_as(as = "Text")]
    pub comment_count: String,
}

/// Item section containing comments ctoken
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionComments {
    pub continuation_item_renderer: ContinuationItemRenderer,
}

/// Video recommendations
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResultsWrap {
    pub secondary_results: RecommendationResults,
}

/// Video recommendations
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResults {
    /// Can be `None` for age-restricted videos
    #[serde_as(as = "Option<VecLogError<_>>")]
    pub results: Option<MapResult<Vec<VideoListItem>>>,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementPanel {
    pub engagement_panel_section_list_renderer: EngagementPanelRenderer,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "targetId")]
pub enum EngagementPanelRenderer {
    /// Chapter markers
    EngagementPanelMacroMarkersDescriptionChapters { content: ChapterMarkersContent },
    /// Comment section (contains no comments, but the
    /// continuation tokens for fetching top/latest comments)
    EngagementPanelCommentsSection { header: CommentItemSectionHeader },
    /// Ignored items:
    /// - `engagement-panel-ads`
    /// - `engagement-panel-structured-description`
    ///   (Description already included in `VideoSecondaryInfoRenderer`)
    /// - `engagement-panel-searchable-transcript`
    ///   (basically video subtitles in a different format)
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

/// Chapter markers
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMarkersContent {
    pub macro_markers_list_renderer: MacroMarkersListRenderer,
}

/// Chapter markers
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<MacroMarkersListItem>>,
}

/// Chapter marker
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItem {
    pub macro_markers_list_item_renderer: MacroMarkersListItemRenderer,
}

/// Chapter marker
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemRenderer {
    /// Contains chapter start time in seconds
    pub on_tap: MacroMarkersListItemOnTap,
    #[serde(default)]
    pub thumbnail: Thumbnails,
    /// Chapter title
    #[serde_as(as = "Text")]
    pub title: String,
}

/// Contains chapter start time in seconds
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemOnTap {
    pub watch_endpoint: MacroMarkersListItemWatchEndpoint,
}
/// Contains chapter start time in seconds
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroMarkersListItemWatchEndpoint {
    /// Chapter start time in seconds
    pub start_time_seconds: u32,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeader {
    pub engagement_panel_title_header_renderer: CommentItemSectionHeaderRenderer,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderRenderer {
    /// Approximate comment count (e.g. `81`, `2.2K`, `705K`)
    ///
    /// The accurate count is included in the first comment response.
    ///
    /// Is `None` if there are no comments.
    #[serde_as(as = "Option<Text>")]
    pub contextual_info: Option<String>,
    pub menu: CommentItemSectionHeaderMenu,
}

/// Comment section menu
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenu {
    pub sort_filter_sub_menu_renderer: CommentItemSectionHeaderMenuRenderer,
}

/// Comment section menu
///
/// Items:
/// - Top comments
/// - Latest comments
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenuRenderer {
    pub sub_menu_items: Vec<CommentItemSectionHeaderMenuItem>,
}

/// Comment section menu item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItemSectionHeaderMenuItem {
    /// Continuation token for fetching comments
    pub service_endpoint: ContinuationEndpoint,
}

/*
#RECOMMENDATIONS CONTINUATION
*/

/// Video recommendations continuation response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRecommendations {
    pub on_response_received_endpoints: Vec<RecommendationsContItem>,
}

/// Video recommendations continuation
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsContItem {
    pub append_continuation_items_action: AppendRecommendations,
}

/// Video recommendations continuation
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecommendations {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<VideoListItem>>,
}

/*
#COMMENTS CONTINUATION
*/

/// Video comments continuation response
#[serde_as]
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsContItem {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: AppendComments,
}

/// Video comments continuation action
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendComments {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<CommentListItem>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
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
    CommentRenderer(CommentRenderer),
    /// Continuation token to fetch more comments
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
    /// Header of the comment section (contains number of comments)
    #[serde(rename_all = "camelCase")]
    CommentsHeaderRenderer {
        /// `4,238,993 Comments`
        #[serde_as(as = "Option<Text>")]
        count_text: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub comment_renderer: CommentRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRenderer {
    /// Author name
    ///
    /// There may be comments with missing authors (possibly deleted users?)
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Option<Text>>")]
    pub author_text: Option<String>,
    #[serde(default)]
    pub author_thumbnail: Thumbnails,
    #[serde(default)]
    /// ID of the author's channel
    #[serde_as(as = "DefaultOnError")]
    pub author_endpoint: Option<AuthorEndpoint>,
    /// Comment text
    pub content_text: TextComponents,
    /// Textual publish date (e.g. `15 minutes ago`, `2 days ago`)
    #[serde_as(as = "Text")]
    pub published_time_text: String,
    pub comment_id: String,
    pub author_is_channel_owner: bool,
    // #[serde_as(as = "Option<Text>")]
    // pub vote_count: Option<String>,
    pub author_comment_badge: Option<AuthorCommentBadge>,
    #[serde(default)]
    pub reply_count: u64,
    /// Buttons for comment interaction (Like/Dislike/Reply)
    pub action_buttons: CommentActionButtons,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorEndpoint {
    pub browse_endpoint: BrowseEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Replies {
    pub comment_replies_renderer: RepliesRenderer,
}

/// Does not contain replies directly but a continuation token
/// for fetching them.
#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepliesRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<CommentListItem>,
}

/// These are the buttons for comment interaction (Like/Dislike/Reply).
/// Contains the CreatorHeart.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtons {
    pub comment_action_buttons_renderer: CommentActionButtonsRenderer,
}

/// These are the buttons for comment interaction (Like/Dislike/Reply).
/// Contains the CreatorHeart.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentActionButtonsRenderer {
    pub like_button: ToggleButtonWrap,
    pub creator_heart: Option<CreatorHeart>,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHeart {
    pub creator_heart_renderer: CreatorHeartRenderer,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHeartRenderer {
    pub is_hearted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCommentBadge {
    pub author_comment_badge_renderer: AuthorCommentBadgeRenderer,
}

/// YouTube channel badge (verified) of the comment author
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCommentBadgeRenderer {
    /// Verified: `CHECK`
    ///
    /// Artist: `OFFICIAL_ARTIST_BADGE`
    pub icon: Icon,
}
