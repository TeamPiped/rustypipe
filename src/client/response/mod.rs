pub mod channel;
pub mod player;
pub mod playlist;
pub mod playlist_music;
pub mod search;
pub mod trends;
pub mod url_endpoint;
pub mod video_details;
pub mod video_item;

pub use channel::Channel;
pub use player::Player;
pub use playlist::Playlist;
pub use playlist::PlaylistCont;
pub use playlist_music::PlaylistMusic;
pub use search::Search;
pub use trends::Startpage;
pub use trends::Trending;
pub use url_endpoint::ResolvedUrl;
pub use video_details::VideoComments;
pub use video_details::VideoDetails;
pub use video_item::YouTubeListItem;
pub use video_item::YouTubeListMapper;

#[cfg(feature = "rss")]
pub mod channel_rss;
#[cfg(feature = "rss")]
pub use channel_rss::ChannelRss;

use serde::Deserialize;
use serde_with::{json::JsonString, serde_as, DefaultOnError, VecSkipError};

use crate::error::ExtractionError;
use crate::serializer::MapResult;
use crate::serializer::{
    text::{Text, TextComponent},
    VecLogError,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRenderer<T> {
    pub content: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentsRenderer<T> {
    #[serde(alias = "tabs")]
    pub contents: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailsWrap {
    #[serde(default)]
    pub thumbnail: Thumbnails,
}

/// List of images in different resolutions.
/// Not only used for thumbnails, but also for avatars and banners.
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationItemRenderer {
    pub continuation_endpoint: ContinuationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationEndpoint {
    pub continuation_command: ContinuationCommand,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationCommand {
    pub token: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwner {
    pub video_owner_renderer: VideoOwnerRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelBadge {
    pub metadata_badge_renderer: ChannelBadgeRenderer,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub alert_renderer: AlertRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRenderer {
    #[serde_as(as = "Text")]
    pub text: String,
}

// CONTINUATION

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    /// Number of search results
    #[serde_as(as = "Option<JsonString>")]
    pub estimated_results: Option<u64>,
    #[serde(
        alias = "onResponseReceivedCommands",
        alias = "onResponseReceivedEndpoints"
    )]
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub on_response_received_actions: Option<Vec<ContinuationActionWrap>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationActionWrap {
    pub append_continuation_items_action: ContinuationAction,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationAction {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseContext {
    pub visitor_data: Option<String>,
}

// YouTube Music

#[serde_as]
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer {
    #[serde(alias = "croppedSquareThumbnailRenderer")]
    pub music_thumbnail_renderer: ThumbnailsWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub video_id: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicContentsRenderer<T> {
    pub contents: Vec<T>,
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub continuations: Option<Vec<MusicContinuation>>,
}

#[derive(Debug, Deserialize)]
pub struct MusicColumn {
    #[serde(
        rename = "musicResponsiveListItemFlexColumnRenderer",
        alias = "musicResponsiveListItemFixedColumnRenderer"
    )]
    pub renderer: MusicColumnRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct MusicColumnRenderer {
    pub text: TextComponent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicContinuation {
    pub next_continuation_data: MusicContinuationData,
}

#[derive(Debug, Deserialize)]
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

pub fn alerts_to_err(alerts: Option<Vec<Alert>>) -> ExtractionError {
    match alerts {
        Some(alerts) => ExtractionError::ContentUnavailable(
            alerts
                .into_iter()
                .map(|a| a.alert_renderer.text)
                .collect::<Vec<_>>()
                .join(" ")
                .into(),
        ),
        None => ExtractionError::ContentUnavailable("content not found".into()),
    }
}
