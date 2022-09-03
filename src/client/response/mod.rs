pub mod channel;
pub mod player;
pub mod playlist;
pub mod playlist_music;
pub mod video;

pub use channel::Channel;
pub use player::Player;
pub use playlist::Playlist;
pub use playlist_music::PlaylistMusic;
pub use video::Video;
pub use video::VideoComments;
pub use video::VideoRecommendations;

use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::serializer::text::TextLink;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRenderer<T> {
    pub content: T,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentsRenderer<T> {
    #[serde(alias = "tabs")]
    pub contents: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailsWrap {
    pub thumbnail: Thumbnails,
}

#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoListItem<T> {
    #[serde(alias = "playlistVideoRenderer", alias = "compactVideoRenderer")]
    GridVideoRenderer {
        #[serde(flatten)]
        video: T,
    },
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationEndpoint {
    pub continuation_command: ContinuationCommand,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationCommand {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    pub icon_type: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwner {
    pub video_owner_renderer: VideoOwnerRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwnerRenderer {
    #[serde_as(as = "crate::serializer::text::TextLink")]
    pub title: TextLink,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<UserBadge>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBadge {
    pub metadata_badge_renderer: UserBadgeRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBadgeRenderer {
    pub style: UserBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserBadgeStyle {
    BadgeStyleTypeVerified,
    BadgeStyleTypeVerifiedArtist,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOverlay {
    pub thumbnail_overlay_time_status_renderer: TimeOverlayRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOverlayRenderer {
    #[serde_as(as = "crate::serializer::text::Text")]
    pub text: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub style: TimeOverlayStyle,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeOverlayStyle {
    #[default]
    Default,
    Live,
    Shorts,
}

// YouTube Music

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicItem {
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub playlist_item_data: Option<PlaylistItemData>,
    #[serde_as(as = "VecSkipError<_>")]
    pub flex_columns: Vec<MusicColumn>,
    #[serde_as(as = "VecSkipError<_>")]
    pub fixed_columns: Vec<MusicColumn>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer {
    #[serde(alias = "croppedSquareThumbnailRenderer")]
    pub music_thumbnail_renderer: ThumbnailsWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub video_id: String,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicContentsRenderer<T> {
    pub contents: Vec<T>,
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub continuations: Option<Vec<MusicContinuation>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MusicColumn {
    #[serde(
        rename = "musicResponsiveListItemFlexColumnRenderer",
        alias = "musicResponsiveListItemFixedColumnRenderer"
    )]
    pub renderer: MusicColumnRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct MusicColumnRenderer {
    #[serde_as(as = "crate::serializer::text::TextLinks")]
    pub text: Vec<TextLink>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicContinuation {
    pub next_continuation_data: MusicContinuationData,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicContinuationData {
    pub continuation: String,
}
