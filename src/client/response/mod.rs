pub(crate) mod channel;
pub(crate) mod music_artist;
pub(crate) mod music_item;
pub(crate) mod music_playlist;
pub(crate) mod music_search;
pub(crate) mod player;
pub(crate) mod playlist;
pub(crate) mod search;
pub(crate) mod trends;
pub(crate) mod url_endpoint;
pub(crate) mod video_details;
pub(crate) mod video_item;

pub(crate) use channel::Channel;
pub(crate) use music_artist::MusicArtist;
pub(crate) use music_artist::MusicArtistAlbums;
pub(crate) use music_item::MusicContinuation;
pub(crate) use music_playlist::MusicPlaylist;
pub(crate) use music_search::MusicSearch;
pub(crate) use player::Player;
pub(crate) use playlist::Playlist;
pub(crate) use playlist::PlaylistCont;
pub(crate) use search::Search;
pub(crate) use trends::Startpage;
pub(crate) use trends::Trending;
pub(crate) use url_endpoint::ResolvedUrl;
pub(crate) use video_details::VideoComments;
pub(crate) use video_details::VideoDetails;
pub(crate) use video_item::YouTubeListItem;
pub(crate) use video_item::YouTubeListMapper;

#[cfg(feature = "rss")]
pub(crate) mod channel_rss;
#[cfg(feature = "rss")]
pub(crate) use channel_rss::ChannelRss;

use serde::Deserialize;
use serde_with::{json::JsonString, serde_as, VecSkipError};

use crate::error::ExtractionError;
use crate::serializer::MapResult;
use crate::serializer::{text::Text, VecLogError};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentRenderer<T> {
    pub content: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentsRenderer<T> {
    #[serde(alias = "tabs")]
    pub contents: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tab<T> {
    pub tab_renderer: ContentRenderer<T>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailsWrap {
    #[serde(default)]
    pub thumbnail: Thumbnails,
}

/// List of images in different resolutions.
/// Not only used for thumbnails, but also for avatars and banners.
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Thumbnails {
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationItemRenderer {
    pub continuation_endpoint: ContinuationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationEndpoint {
    pub continuation_command: ContinuationCommand,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationCommand {
    pub token: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Icon {
    pub icon_type: IconType,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum IconType {
    /// Checkmark for verified channels
    Check,
    /// Music note for verified artists
    OfficialArtistBadge,
    /// Like button
    Like,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelBadge {
    pub metadata_badge_renderer: ChannelBadgeRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelBadgeRenderer {
    pub style: ChannelBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ChannelBadgeStyle {
    BadgeStyleTypeVerified,
    BadgeStyleTypeVerifiedArtist,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Alert {
    pub alert_renderer: AlertRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlertRenderer {
    #[serde_as(as = "Text")]
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResponseContext {
    pub visitor_data: Option<String>,
}

// CONTINUATION

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Continuation {
    /// Number of search results
    #[serde_as(as = "Option<JsonString>")]
    pub estimated_results: Option<u64>,
    #[serde(
        alias = "onResponseReceivedCommands",
        alias = "onResponseReceivedEndpoints"
    )]
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub on_response_received_actions: Option<Vec<ContinuationActionWrap>>,
    /// Used for channel video rich grid renderer
    ///
    /// A/B test seen on 19.10.2022
    pub continuation_contents: Option<RichGridContinuationContents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationActionWrap {
    pub append_continuation_items_action: ContinuationAction,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationAction {
    #[serde_as(as = "VecLogError<_>")]
    pub continuation_items: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RichGridContinuationContents {
    pub rich_grid_continuation: RichGridContinuation,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RichGridContinuation {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<YouTubeListItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuationData {
    pub next_continuation_data: MusicContinuationDataInner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuationDataInner {
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

pub(crate) fn alerts_to_err(alerts: Option<Vec<Alert>>) -> ExtractionError {
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
