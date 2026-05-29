#![allow(clippy::enum_variant_names)]

use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use crate::serializer::{
    text::{AccessibilityText, AttributedText, Text, TextComponent, TextComponents},
    MapResult,
};

use super::{
    url_endpoint::{BrowseEndpoint, BrowseEndpointWrap, OnTap},
    ContinuationEndpoint, ContinuationItemRenderer, Icon, MusicContinuationData, Thumbnails,
};
use super::{
    ChannelBadge, ContentsRendererLogged, FrameworkUpdates, ImageView, ResponseContext,
    YouTubeListItem,
};

/*
#VIDEO DETAILS
*/

/// Video details response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoDetails {
    /// Video metadata + recommended videos
    pub contents: Option<Contents>,
    /// Video ID
    pub current_video_endpoint: Option<CurrentVideoEndpoint>,
    /// Video chapters + comment section
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoResults {
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
        #[serde_as(as = "Option<Text>")]
        date_text: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    VideoSecondaryInfoRenderer {
        owner: VideoOwner,
        #[serde(default)]
        #[serde_as(as = "DefaultOnError<Option<AttributedText>>")]
        description: Option<TextComponents>,
        #[serde(default)]
        #[serde_as(as = "DefaultOnError<Option<AttributedText>>")]
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
    #[serde(rename_all = "camelCase")]
    SegmentedLikeDislikeButtonViewModel {
        like_button_view_model: LikeButtonViewModelWrap,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LikeButtonViewModelWrap {
    pub like_button_view_model: LikeButtonViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LikeButtonViewModel {
    pub toggle_button_view_model: ToggleButtonViewModelWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToggleButtonViewModelWrap {
    pub toggle_button_view_model: ToggleButtonViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToggleButtonViewModel {
    pub default_button_view_model: ButtonViewModelWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonViewModelWrap {
    pub button_view_model: ButtonViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonViewModel {
    pub accessibility_text: String,
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
    #[serde(default)]
    pub title: Option<TextComponent>,
    #[serde(default)]
    pub attributed_title: Option<OwnerAttributedTitle>,
    #[serde(default)]
    pub navigation_endpoint: Option<OwnerNavigationEndpoint>,
    #[serde(default)]
    pub thumbnail: Thumbnails,
    #[serde(default)]
    pub avatar_stack: Option<AvatarStackWrap>,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
}

/// Channel title for videos with multiple collaborators
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerAttributedTitle {
    #[allow(dead_code)]
    pub content: String,
    #[serde(default)]
    pub command_runs: Vec<OwnerAttributedTitleCommandRun>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerAttributedTitleCommandRun {
    #[serde(default)]
    pub on_tap: Option<OwnerAttributedTitleOnTap>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerAttributedTitleOnTap {
    pub innertube_command: OwnerAttributedTitleInnertubeCommand,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerAttributedTitleInnertubeCommand {
    #[serde(default)]
    pub show_dialog_command: Option<CollaboratorsDialogCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerNavigationEndpoint {
    #[serde(default)]
    #[allow(dead_code)]
    pub browse_endpoint: Option<BrowseEndpoint>,
    #[serde(default)]
    pub show_dialog_command: Option<CollaboratorsDialogCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsDialogCommand {
    pub panel_loading_strategy: CollaboratorsPanelLoadingStrategy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsPanelLoadingStrategy {
    pub inline_content: CollaboratorsInlineContent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsInlineContent {
    pub dialog_view_model: CollaboratorsDialogViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsDialogViewModel {
    pub custom_content: CollaboratorsCustomContent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsCustomContent {
    pub list_view_model: CollaboratorsListViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsListViewModel {
    #[serde(default)]
    pub list_items: Vec<CollaboratorsListItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsListItem {
    pub list_item_view_model: CollaboratorsListItemViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsListItemViewModel {
    pub title: CollaboratorsListItemTitle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsListItemTitle {
    pub content: String,
    #[serde(default)]
    pub command_runs: Vec<CollaboratorsListItemCommandRun>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollaboratorsListItemCommandRun {
    pub on_tap: OnTap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AvatarStackWrap {
    pub avatar_stack_view_model: AvatarStackViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AvatarStackViewModel {
    #[serde(default)]
    pub avatars: Vec<super::AvatarViewModel>,
}

impl VideoOwnerRenderer {
    pub(crate) fn collaborators_dialog(&self) -> Option<&CollaboratorsDialogCommand> {
        self.navigation_endpoint
            .as_ref()
            .and_then(|ep| ep.show_dialog_command.as_ref())
            .or_else(|| {
                self.attributed_title.as_ref().and_then(|title| {
                    title.command_runs.iter().find_map(|run| {
                        run.on_tap.as_ref().and_then(|tap| {
                            tap.innertube_command
                                .show_dialog_command
                                .as_ref()
                        })
                    })
                })
            })
    }

    pub(crate) fn collaborator_channels(&self) -> Vec<(String, String)> {
        let Some(dialog) = self.collaborators_dialog() else {
            return Vec::new();
        };
        dialog
            .panel_loading_strategy
            .inline_content
            .dialog_view_model
            .custom_content
            .list_view_model
            .list_items
            .iter()
            .filter_map(|item| {
                let title = &item.list_item_view_model.title;
                let browse_id = title.command_runs.iter().find_map(|run| match &run.on_tap.innertube_command {
                    super::url_endpoint::NavigationEndpoint::Browse { browse_endpoint, .. } => {
                        Some(browse_endpoint.browse_id.clone())
                    }
                    _ => None,
                })?;
                Some((browse_id, title.content.clone()))
            })
            .collect()
    }

    pub(crate) fn thumbnail_or_avatar_stack(&self) -> Thumbnails {
        if !self.thumbnail.thumbnails.is_empty() {
            return self.thumbnail.clone();
        }
        self.avatar_stack
            .as_ref()
            .map(|stack| {
                stack
                    .avatar_stack_view_model
                    .avatars
                    .first()
                    .map(|a| a.avatar_view_model.image.clone())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }
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
    pub results: Option<MapResult<Vec<YouTubeListItem>>>,
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub continuations: Option<Vec<MusicContinuationData>>,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngagementPanel {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub engagement_panel_section_list_renderer: Option<EngagementPanelRenderer>,
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
    pub macro_markers_list_renderer: ContentsRendererLogged<MacroMarkersListItem>,
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
    pub on_response_received_endpoints: MapResult<Vec<CommentsContItem>>,
    pub framework_updates: Option<FrameworkUpdates<Payload>>,
}

/// Video comments continuation
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentsContItem {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: AppendComments,
}

/// Video comments continuation action
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendComments {
    pub continuation_items: MapResult<Vec<CommentListItem>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CommentListItem {
    /// Top-level comment
    CommentThreadRenderer(CommentThreadRenderer),
    /// Reply comment
    CommentRenderer(CommentRenderer),
    /// Reply comment (A/B #14)
    CommentViewModel(CommentViewModel),
    /// Continuation token to fetch more comments
    ContinuationItemRenderer(ContinuationItemVariants),
    /// Header of the comment section (contains number of comments)
    #[serde(rename_all = "camelCase")]
    CommentsHeaderRenderer {
        /// `4,238,993 Comments`
        #[serde_as(as = "Option<Text>")]
        count_text: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ContinuationItemVariants {
    #[serde(rename_all = "camelCase")]
    Ep {
        continuation_endpoint: ContinuationEndpoint,
    },
    Btn {
        button: ContinuationButton,
    },
}

impl ContinuationItemVariants {
    pub fn into_token(self) -> Option<String> {
        match self {
            ContinuationItemVariants::Ep {
                continuation_endpoint,
            } => continuation_endpoint,
            ContinuationItemVariants::Btn { button } => button.button_renderer.command,
        }
        .into_token()
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentThreadRenderer {
    /// Missing on the FrameworkUpdate data model (A/B #14)
    pub comment: Option<Comment>,
    pub comment_view_model: Option<CommentViewModelWrap>,
    /// Continuation token to fetch replies
    #[serde(default)]
    pub replies: Replies,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub rendering_priority: CommentPriority,
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

#[derive(Default, Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CommentPriority {
    /// Default rendering priority
    #[default]
    RenderingPriorityUnknown,
    /// Comment pinned by the creator
    RenderingPriorityPinnedComment,
}

impl From<CommentPriority> for bool {
    fn from(value: CommentPriority) -> Self {
        matches!(value, CommentPriority::RenderingPriorityPinnedComment)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentViewModelWrap {
    pub comment_view_model: CommentViewModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentViewModel {
    pub comment_id: String,
    pub comment_key: String,
    pub comment_surface_key: String,
    pub toolbar_state_key: String,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Payload {
    CommentEntityPayload(CommentEntityPayload),
    CommentSurfaceEntityPayload(CommentSurfaceEntityPayload),
    #[serde(rename_all = "camelCase")]
    EngagementToolbarStateEntityPayload {
        heart_state: HeartState,
    },
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentEntityPayload {
    pub properties: CommentProperties,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub author: Option<CommentAuthor>,
    pub toolbar: CommentToolbar,
    #[serde(default)]
    pub avatar: ImageView,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentSurfaceEntityPayload {
    pub voice_reply_container_view_model: Option<VoiceReplyContainer>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentProperties {
    #[serde_as(as = "AttributedText")]
    pub content: TextComponents,
    pub published_time: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentAuthor {
    pub channel_id: String,
    pub display_name: String,
    #[serde(default)]
    pub is_verified: bool,
    #[serde(default)]
    pub is_artist: bool,
    #[serde(default)]
    pub is_creator: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentToolbar {
    pub like_count_notliked: String,
    pub reply_count: String,
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum HeartState {
    ToolbarHeartStateUnhearted,
    ToolbarHeartStateHearted,
}

impl From<HeartState> for bool {
    fn from(value: HeartState) -> Self {
        match value {
            HeartState::ToolbarHeartStateUnhearted => false,
            HeartState::ToolbarHeartStateHearted => true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationButton {
    pub button_renderer: ContinuationButtonRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationButtonRenderer {
    pub command: ContinuationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceReplyContainer {
    pub voice_reply_container_view_model: VoiceReplyContainer2,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceReplyContainer2 {
    #[serde_as(as = "AttributedText")]
    pub transcript_text: TextComponents,
}
