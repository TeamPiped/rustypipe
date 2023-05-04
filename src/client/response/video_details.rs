#![allow(clippy::enum_variant_names)]

use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use crate::serializer::text::TextComponent;
use crate::serializer::{
    text::{AccessibilityText, AttributedText, Text, TextComponents},
    MapResult, VecLogError,
};

use super::{
    url_endpoint::BrowseEndpointWrap, ContinuationEndpoint, ContinuationItemRenderer, Icon,
    MusicContinuationData, Thumbnails,
};
use super::{ChannelBadge, ResponseContext, YouTubeListItem};

/*
#VIDEO DETAILS
*/

/// Video details response
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoDetails {
    /// Video metadata + recommended videos
    pub contents: Option<Contents>,
    /// Video ID
    pub current_video_endpoint: Option<CurrentVideoEndpoint>,
    /// Video chapters + comment section
    #[serde_as(as = "VecLogError<_>")]
    pub engagement_panels: MapResult<Vec<EngagementPanel>>,
    pub response_context: ResponseContext,
}

/// Video details main object, contains video metadata and recommended videos
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub two_column_watch_next_results: TwoColumnWatchNextResults,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TwoColumnWatchNextResults {
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
pub(crate) struct VideoResultsWrap {
    pub results: VideoResults,
}

/// Video metadata items
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoResults {
    #[serde_as(as = "Option<VecLogError<_>>")]
    pub contents: Option<MapResult<Vec<VideoResultsItem>>>,
}

/// Video metadata item
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum VideoResultsItem {
    #[serde(rename_all = "camelCase")]
    VideoPrimaryInfoRenderer {
        #[serde_as(as = "Text")]
        title: String,
        view_count: Option<ViewCount>,
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
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewCount {
    pub video_view_count_renderer: ViewCountRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewCountRenderer {
    /// View count (`232,975,196 views`)
    #[serde_as(as = "Text")]
    pub view_count: String,
    #[serde(default)]
    pub is_live: bool,
}

/// Like/Dislike buttons
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoActions {
    pub menu_renderer: VideoActionsMenu,
}

/// Like/Dislike buttons
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoActionsMenu {
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
pub(crate) enum TopLevelButton {
    ToggleButtonRenderer(ToggleButton),
    #[serde(rename_all = "camelCase")]
    SegmentedLikeDislikeButtonRenderer {
        like_button: ToggleButtonWrap,
    },
}

/// Like/Dislike button
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToggleButtonWrap {
    pub toggle_button_renderer: ToggleButton,
}

/// Like/Dislike button
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToggleButton {
    /// Icon type: `LIKE` / `DISLIKE`
    pub default_icon: Icon,
    /// Number of likes (`like this video along with 4,010,156 other people`)
    ///
    /// Contains no digits (e.g. `I like this`) if likes are hidden by the creator.
    #[serde_as(as = "AccessibilityText")]
    pub accessibility_data: String,
}

/// Video channel information
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoOwner {
    pub video_owner_renderer: VideoOwnerRenderer,
}

/// Video channel information
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoOwnerRenderer {
    pub title: TextComponent,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataRowContainer {
    pub metadata_row_container_renderer: MetadataRowContainerRenderer,
}

/// Shows additional video metadata. Its only known use is for
/// the Creative Commonse License.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataRowContainerRenderer {
    pub rows: Vec<MetadataRow>,
}

/// Additional video metadata item (Creative Commons License)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataRow {
    pub metadata_row_renderer: MetadataRowRenderer,
}

/// Additional video metadata item (Creative Commons License)
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataRowRenderer {
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
pub(crate) struct CurrentVideoEndpoint {
    pub watch_endpoint: CurrentVideoWatchEndpoint,
}
/// Contains current video ID
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentVideoWatchEndpoint {
    pub video_id: String,
}

/// The comment section consists of 2 ItemSections:
///
/// 1. CommentsEntryPointHeaderRenderer: contains number of comments
/// 2. ContinuationItemRenderer: contains continuation token
#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "sectionIdentifier")]
pub(crate) enum ItemSection {
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
pub(crate) struct ItemSectionCommentCount {
    pub comments_entry_point_header_renderer: CommentsEntryPointHeaderRenderer,
}

/// Renderer of item section containing comment count
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentsEntryPointHeaderRenderer {
    #[serde_as(as = "Text")]
    pub comment_count: String,
}

/// Item section containing comments ctoken
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemSectionComments {
    pub continuation_item_renderer: ContinuationItemRenderer,
}

/// Video recommendations
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecommendationResultsWrap {
    pub secondary_results: RecommendationResults,
}

/// Video recommendations
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecommendationResults {
    /// Can be `None` for age-restricted videos
    #[serde_as(as = "Option<VecLogError<_>>")]
    pub results: Option<MapResult<Vec<YouTubeListItem>>>,
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub continuations: Option<Vec<MusicContinuationData>>,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngagementPanel {
    pub engagement_panel_section_list_renderer: EngagementPanelRenderer,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "targetId")]
pub(crate) enum EngagementPanelRenderer {
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
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

/// Chapter markers
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterMarkersContent {
    pub macro_markers_list_renderer: MacroMarkersListRenderer,
}

/// Chapter markers
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MacroMarkersListRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<MacroMarkersListItem>>,
}

/// Chapter marker
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MacroMarkersListItem {
    pub macro_markers_list_item_renderer: MacroMarkersListItemRenderer,
}

/// Chapter marker
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MacroMarkersListItemRenderer {
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
pub(crate) struct MacroMarkersListItemOnTap {
    pub watch_endpoint: MacroMarkersListItemWatchEndpoint,
}
/// Contains chapter start time in seconds
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MacroMarkersListItemWatchEndpoint {
    /// Chapter start time in seconds
    pub start_time_seconds: u32,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentItemSectionHeader {
    pub engagement_panel_title_header_renderer: CommentItemSectionHeaderRenderer,
}

/// Comment section header
/// (contains continuation tokens for fetching top/latest comments)
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentItemSectionHeaderRenderer {
    pub menu: CommentItemSectionHeaderMenu,
}

/// Comment section menu
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentItemSectionHeaderMenu {
    pub sort_filter_sub_menu_renderer: CommentItemSectionHeaderMenuRenderer,
}

/// Comment section menu
///
/// Items:
/// - Top comments
/// - Latest comments
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentItemSectionHeaderMenuRenderer {
    pub sub_menu_items: Vec<CommentItemSectionHeaderMenuItem>,
}

/// Comment section menu item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentItemSectionHeaderMenuItem {
    /// Continuation token for fetching comments
    pub service_endpoint: ContinuationEndpoint,
}

/*
#COMMENTS CONTINUATION
*/

/// Video comments continuation response
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoComments {
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
pub(crate) struct CommentsContItem {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: AppendComments,
}

/// Video comments continuation action
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendComments {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<CommentListItem>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CommentListItem {
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
pub(crate) struct Comment {
    pub comment_renderer: CommentRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentRenderer {
    /// Author name
    ///
    /// There may be comments with missing authors (possibly deleted users?)
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Option<Text>>")]
    pub author_text: Option<String>,
    #[serde(default)]
    pub author_thumbnail: Thumbnails,
    /// ID of the author's channel
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub author_endpoint: Option<BrowseEndpointWrap>,
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
    #[serde_as(as = "Option<Text>")]
    pub vote_count: Option<String>,
    /// Buttons for comment interaction (Like/Dislike/Reply)
    pub action_buttons: CommentActionButtons,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CommentPriority {
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
pub(crate) struct Replies {
    pub comment_replies_renderer: RepliesRenderer,
}

/// Does not contain replies directly but a continuation token
/// for fetching them.
#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepliesRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<CommentListItem>,
}

/// These are the buttons for comment interaction (Like/Dislike/Reply).
/// Contains the CreatorHeart.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentActionButtons {
    pub comment_action_buttons_renderer: CommentActionButtonsRenderer,
}

/// These are the buttons for comment interaction (Like/Dislike/Reply).
/// Contains the CreatorHeart.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentActionButtonsRenderer {
    pub creator_heart: Option<CreatorHeart>,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatorHeart {
    pub creator_heart_renderer: CreatorHeartRenderer,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatorHeartRenderer {
    pub is_hearted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorCommentBadge {
    pub author_comment_badge_renderer: AuthorCommentBadgeRenderer,
}

/// YouTube channel badge (verified) of the comment author
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorCommentBadgeRenderer {
    /// Verified: `CHECK`
    ///
    /// Artist: `OFFICIAL_ARTIST_BADGE`
    pub icon: Icon,
}
