pub mod channel;
pub mod player;
pub mod playlist;
pub mod playlist_music;
pub mod video_details;

pub use channel::Channel;
pub use channel::ChannelCont;
pub use player::Player;
pub use playlist::Playlist;
pub use playlist::PlaylistCont;
pub use playlist_music::PlaylistMusic;
pub use video_details::VideoComments;
pub use video_details::VideoDetails;
pub use video_details::VideoRecommendations;

#[cfg(feature = "rss")]
pub mod channel_rss;
#[cfg(feature = "rss")]
pub use channel_rss::ChannelRss;

use serde::Deserialize;
use serde_with::{json::JsonString, serde_as, DefaultOnError, VecSkipError};

use crate::serializer::{
    ignore_any,
    text::{Text, TextComponent},
};

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
    #[serde(default)]
    pub thumbnail: Thumbnails,
}

/// List of images in different resolutions.
/// Not only used for thumbnails, but also for avatars and banners.
#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoListItem {
    GridVideoRenderer(GridVideoRenderer),
    CompactVideoRenderer(CompactVideoRenderer),
    PlaylistVideoRenderer(PlaylistVideoRenderer),

    GridPlaylistRenderer(GridPlaylistRenderer),

    /// Continauation items are located at the end of a list
    /// and contain the continuation token for progressive loading
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
    /// No video list item (e.g. ad) or unimplemented item
    ///
    /// Unimplemented:
    /// - compactPlaylistRenderer (recommended playlists)
    /// - compactRadioRenderer (recommended mix)
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

/// Video displayed on a channel page
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridVideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde_as(as = "Option<Text>")]
    pub published_time_text: Option<String>,
    /// Contains `No views` if the view count is zero
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    /// Contains video length
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
    /// Release date for upcoming videos
    pub upcoming_event_data: Option<UpcomingEventData>,
}

/// Video displayed in recommendations
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactVideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    pub channel_thumbnail: Thumbnails,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
    /// (e.g. `11 months ago`)
    #[serde_as(as = "Option<Text>")]
    pub published_time_text: Option<String>,
    /// Contains `No views` if the view count is zero
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    /// Badges are displayed on the video thumbnail and
    /// show certain video properties (e.g. active livestream)
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<VideoBadge>,
}

/// Video displayed in a playlist
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    #[serde_as(as = "JsonString")]
    pub length_seconds: u32,
}

/// Playlist displayed on a channel page
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPlaylistRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub video_count_short_text: String,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingEventData {
    /// Unixtime in seconds
    #[serde_as(as = "JsonString")]
    pub start_time: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationItemRenderer {
    pub continuation_endpoint: ContinuationEndpoint,
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

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    pub icon_type: IconType,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IconType {
    /// Checkmark for verified channels
    Check,
    /// Music note for verified artists
    OfficialArtistBadge,
    /// Like button
    Like,
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
    pub title: TextComponent,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelBadge {
    pub metadata_badge_renderer: ChannelBadgeRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelBadgeRenderer {
    pub style: ChannelBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChannelBadgeStyle {
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
    /// `29:54`
    ///
    /// Is `LIVE` in case of a livestream
    #[serde_as(as = "Text")]
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
    pub text: TextComponent,
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

/*
#MAPPING
*/

impl From<Thumbnail> for crate::model::Thumbnail {
    fn from(tn: Thumbnail) -> Self {
        crate::model::Thumbnail {
            url: tn.url,
            width: tn.width,
            height: tn.height,
        }
    }
}

impl From<Thumbnails> for Vec<crate::model::Thumbnail> {
    fn from(ts: Thumbnails) -> Self {
        ts.thumbnails
            .into_iter()
            .map(|t| crate::model::Thumbnail {
                url: t.url,
                width: t.width,
                height: t.height,
            })
            .collect()
    }
}

impl From<Vec<ChannelBadge>> for crate::model::Verification {
    fn from(badges: Vec<ChannelBadge>) -> Self {
        badges.get(0).map_or(crate::model::Verification::None, |b| {
            match b.metadata_badge_renderer.style {
                ChannelBadgeStyle::BadgeStyleTypeVerified => Self::Verified,
                ChannelBadgeStyle::BadgeStyleTypeVerifiedArtist => Self::Artist,
            }
        })
    }
}

impl From<Icon> for crate::model::Verification {
    fn from(icon: Icon) -> Self {
        match icon.icon_type {
            IconType::Check => Self::Verified,
            IconType::OfficialArtistBadge => Self::Artist,
            _ => Self::None,
        }
    }
}

pub trait IsLive {
    fn is_live(&self) -> bool;
}

pub trait IsShort {
    fn is_short(&self) -> bool;
}

impl IsLive for Vec<VideoBadge> {
    fn is_live(&self) -> bool {
        self.iter().any(|badge| {
            badge.metadata_badge_renderer.style == VideoBadgeStyle::BadgeStyleTypeLiveNow
        })
    }
}

impl IsLive for Vec<TimeOverlay> {
    fn is_live(&self) -> bool {
        self.iter().any(|overlay| {
            overlay.thumbnail_overlay_time_status_renderer.style == TimeOverlayStyle::Live
        })
    }
}

impl IsShort for Vec<TimeOverlay> {
    fn is_short(&self) -> bool {
        self.iter().any(|overlay| {
            overlay.thumbnail_overlay_time_status_renderer.style == TimeOverlayStyle::Shorts
        })
    }
}
