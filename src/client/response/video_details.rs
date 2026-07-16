#![allow(clippy::enum_variant_names)]

use serde::{Deserialize, Deserializer};
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use crate::{
    json::{value_from_json_value, yt_continuation_value, JsonNode, JsonValue},
    serializer::text::{AccessibilityText, AttributedText, Text, TextComponent, TextComponents},
    yt_string_enum, FromYtNode, ytq,
};

use super::{url_endpoint::BrowseEndpoint, Icon, Thumbnails};

pub(crate) fn continuation_token(endpoint: &JsonValue) -> Option<String> {
    yt_continuation_value(endpoint)
}
use super::{ChannelBadge, ImageView};
use crate::error::ExtractionError;

/*
#VIDEO DETAILS
*/

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub(crate) struct AttributedDescription {
    #[serde_as(as = "DefaultOnError<AttributedText>")]
    pub text: TextComponents,
}

impl AttributedDescription {
    pub(crate) fn deserialize_node(node: &JsonNode<'_>) -> Result<TextComponents, ExtractionError> {
        Ok(node.deserialize::<Self>()?.text)
    }
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ViewCountRenderer {
    /// View count (`232,975,196 views`)
    #[ytq_text]
    pub view_count: String,
    #[ytq_default]
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
    pub top_level_buttons: Vec<JsonValue>,
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
    pub avatar_stack: Option<JsonValue>,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
}

/// Channel title for videos with multiple collaborators
#[derive(Debug, FromYtNode)]
pub(crate) struct OwnerAttributedTitle {
    #[allow(dead_code)]
    #[ytq_default]
    pub content: String,
    #[ytq_lossy]
    pub command_runs: Vec<OwnerAttributedTitleCommandRun>,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct OwnerAttributedTitleCommandRun {
    pub on_tap: Option<OwnerAttributedTitleOnTap>,
}

#[derive(Debug)]
pub(crate) struct OwnerAttributedTitleOnTap {
    pub innertube_command: OwnerAttributedTitleInnertubeCommand,
}

impl<'de> Deserialize<'de> for OwnerAttributedTitleOnTap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let raw = crate::json::value_to_json_string(
            value
                .get("innertubeCommand")
                .ok_or_else(|| serde::de::Error::missing_field("innertubeCommand"))?,
        );
        let inner: OwnerAttributedTitleInnertubeCommand = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("innertube command: {e}")))?;
        Ok(Self {
            innertube_command: inner,
        })
    }
}

#[derive(Debug)]
pub(crate) struct OwnerAttributedTitleInnertubeCommand {
    pub show_dialog_command: Option<JsonValue>,
}

impl<'de> Deserialize<'de> for OwnerAttributedTitleInnertubeCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        Ok(Self {
            show_dialog_command: value.get("showDialogCommand").cloned(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct OwnerNavigationEndpoint {
    #[allow(dead_code)]
    pub browse_endpoint: Option<BrowseEndpoint>,
    pub show_dialog_command: Option<JsonValue>,
}

impl<'de> Deserialize<'de> for OwnerNavigationEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let browse_endpoint = value
            .get("browseEndpoint")
            .map(|v| {
                let raw = crate::json::value_to_json_string(v);
                flexon::from_str::<BrowseEndpoint>(&raw)
            })
            .transpose()
            .ok()
            .flatten();
        Ok(Self {
            browse_endpoint,
            show_dialog_command: value.get("showDialogCommand").cloned(),
        })
    }
}

impl VideoOwnerRenderer {
    pub(crate) fn collaborators_dialog(&self) -> Option<&JsonValue> {
        self.navigation_endpoint
            .as_ref()
            .and_then(|ep| ep.show_dialog_command.as_ref())
            .or_else(|| {
                self.attributed_title.as_ref().and_then(|title| {
                    title.command_runs.iter().find_map(|run| {
                        run.on_tap
                            .as_ref()
                            .and_then(|tap| tap.innertube_command.show_dialog_command.as_ref())
                    })
                })
            })
    }

    pub(crate) fn collaborator_channels(&self) -> Vec<(String, String)> {
        let Some(dialog) = self.collaborators_dialog() else {
            return Vec::new();
        };
        dialog
            .get("panelLoadingStrategy")
            .and_then(|v| v.get("inlineContent"))
            .and_then(|v| v.get("dialogViewModel"))
            .and_then(|v| v.get("customContent"))
            .and_then(|v| v.get("listViewModel"))
            .and_then(|v| v.get("listItems"))
            .and_then(|v| v.as_array())
            .into_iter()
            .flat_map(|items| items.iter())
            .filter_map(|item| {
                let title = item.get("listItemViewModel")?.get("title")?;
                let name = title.get("content")?.as_str()?.to_owned();
                let browse_id = title
                    .get("commandRuns")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flat_map(|items| items.iter())
                    .filter_map(|run| {
                        run.get("onTap")
                            .and_then(|v| v.get("innertubeCommand"))
                            .and_then(super::url_endpoint::browse_endpoint)
                            .map(|ep| ep.browse_endpoint.browse_id)
                    })
                    .next()?;
                Some((browse_id, name))
            })
            .collect()
    }

    pub(crate) fn thumbnail_or_avatar_stack(&self) -> Thumbnails {
        if !self.thumbnail.thumbnails.is_empty() {
            return self.thumbnail.clone();
        }
        self.avatar_stack
            .as_ref()
            .and_then(|stack| {
                stack
                    .get("avatarStackViewModel")
                    .and_then(|v| v.get("avatars"))
                    .and_then(|v| v.as_array())
                    .and_then(|avatars| avatars.first())
                    .and_then(|avatar| avatar.get("avatarViewModel"))
                    .and_then(|avatar| avatar.get("image"))
                    .cloned()
                    .and_then(|value| value_from_json_value::<Thumbnails>(&value))
            })
            .unwrap_or_default()
    }
}

/// Contains current video ID
#[derive(Debug, FromYtNode)]
pub(crate) struct CurrentVideoEndpoint {
    pub watch_endpoint: CurrentVideoWatchEndpoint,
}

/// Contains current video ID
#[derive(Debug, Default, FromYtNode)]
pub(crate) struct CurrentVideoWatchEndpoint {
    pub video_id: String,
}

/// The engagement panels are displayed below the video and contain chapter markers
/// and the comment section.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngagementPanel {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub engagement_panel_section_list_renderer: Option<JsonValue>,
}

/*
#COMMENTS CONTINUATION
*/

/// Video comments continuation
#[derive(Debug)]
pub(crate) struct CommentsContItem {
    pub append_continuation_items_action: AppendComments,
}

impl<'de> Deserialize<'de> for CommentsContItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let inner_value = value
            .get("appendContinuationItemsAction")
            .or_else(|| value.get("reloadContinuationItemsCommand"))
            .ok_or_else(|| {
                serde::de::Error::missing_field("appendContinuationItemsAction")
            })?;
        let raw = crate::json::value_to_json_string(inner_value);
        let append: AppendComments = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("append comments: {e}")))?;
        Ok(Self {
            append_continuation_items_action: append,
        })
    }
}

/// Video comments continuation action
#[derive(Debug)]
pub(crate) struct AppendComments {
    pub continuation_items: Vec<JsonValue>,
}

impl<'de> Deserialize<'de> for AppendComments {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let items = value
            .get("continuationItems")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        Ok(Self {
            continuation_items: items,
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentThreadRenderer {
    /// Missing on the FrameworkUpdate data model (A/B #14)
    pub comment: Option<JsonValue>,
    pub comment_view_model: Option<JsonValue>,
    /// Continuation token to fetch replies
    #[serde(default = "crate::json::json_null")]
    pub replies: JsonValue,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub rendering_priority: CommentPriority,
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
    pub author_endpoint: Option<JsonValue>,
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

yt_string_enum! {
    pub(crate) enum CommentPriority {
        /// Default rendering priority
        RenderingPriorityUnknown = "",
        /// Comment pinned by the creator
        RenderingPriorityPinnedComment = "RENDERING_PRIORITY_PINNED_COMMENT",
    }
    default: CommentPriority::RenderingPriorityUnknown,
    fallback_to_default
}

impl From<CommentPriority> for bool {
    fn from(value: CommentPriority) -> Self {
        matches!(value, CommentPriority::RenderingPriorityPinnedComment)
    }
}

#[derive(Debug, FromYtNode)]
pub(crate) struct CommentViewModel {
    pub comment_id: String,
    pub comment_key: String,
    pub comment_surface_key: String,
    pub toolbar_state_key: String,
}

/// These are the buttons for comment interaction (Like/Dislike/Reply).
/// Contains the CreatorHeart.
#[derive(Debug, FromYtNode)]
pub(crate) struct CommentActionButtons {
    #[ytq(.commentActionButtonsRenderer.creatorHeart)]
    pub creator_heart: Option<CreatorHeart>,
}

/// Video creators can endorse comments by marking them with a ❤️.
#[derive(Debug, FromYtNode)]
pub(crate) struct CreatorHeart {
    #[ytq(.creatorHeartRenderer.isHearted)]
    pub is_hearted: bool,
}

#[derive(Debug)]
pub(crate) struct AuthorCommentBadge {
    /// Verified: `CHECK`
    ///
    /// Artist: `OFFICIAL_ARTIST_BADGE`
    pub icon: Icon,
}

impl<'de> Deserialize<'de> for AuthorCommentBadge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            icon: Icon,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wrap {
            author_comment_badge_renderer: Inner,
        }

        let wrap = Wrap::deserialize(deserializer)?;
        Ok(Self {
            icon: wrap.author_comment_badge_renderer.icon,
        })
    }
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

#[derive(Debug, FromYtNode)]
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
