pub mod channel;
pub mod player;
pub mod playlist;
pub mod playlist_music;
pub mod search;
pub mod trends;
pub mod url_endpoint;
pub mod video_details;

pub use channel::Channel;
pub use channel::ChannelCont;
pub use player::Player;
pub use playlist::Playlist;
pub use playlist::PlaylistCont;
pub use playlist_music::PlaylistMusic;
pub use search::Search;
pub use search::SearchCont;
pub use trends::Startpage;
pub use trends::StartpageCont;
pub use trends::Trending;
pub use url_endpoint::ResolvedUrl;
pub use video_details::VideoComments;
pub use video_details::VideoDetails;
pub use video_details::VideoRecommendations;

#[cfg(feature = "rss")]
pub mod channel_rss;
#[cfg(feature = "rss")]
pub use channel_rss::ChannelRss;

use chrono::TimeZone;
use serde::Deserialize;
use serde_with::{json::JsonString, serde_as, DefaultOnError, VecSkipError};

use crate::error::ExtractionError;
use crate::model;
use crate::param::Language;
use crate::serializer::MapResult;
use crate::serializer::{
    ignore_any,
    text::{Text, TextComponent},
    VecLogError,
};
use crate::timeago;
use crate::util::MappingError;
use crate::util::{self, TryRemove};

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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoListItem {
    /// Video on channel page
    GridVideoRenderer(GridVideoRenderer),
    /// Video in recommendations
    CompactVideoRenderer(CompactVideoRenderer),
    /// Video in playlist
    PlaylistVideoRenderer(PlaylistVideoRenderer),

    /// Playlist on channel page
    GridPlaylistRenderer(GridPlaylistRenderer),

    /// Video on startpage
    ///
    /// Seems to be currently A/B tested on the channel page,
    /// as of 11.10.2022
    RichItemRenderer { content: RichItem },

    /// Seems to be currently A/B tested on the video details page,
    /// as of 11.10.2022
    ItemSectionRenderer {
        #[serde_as(as = "VecLogError<_>")]
        contents: MapResult<Vec<VideoListItem>>,
    },

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
#[derive(Debug, Deserialize)]
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
    /// Contains video length and Short/Live tag
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
    /// Release date for upcoming videos
    pub upcoming_event_data: Option<UpcomingEventData>,
}

/// Video displayed in recommendations
#[serde_as]
#[derive(Debug, Deserialize)]
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
    /// Contains Short/Live tag
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
}

/// Video displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: Option<TextComponent>,
    pub channel_thumbnail_supported_renderers: Option<ChannelThumbnailSupportedRenderers>,
    #[serde_as(as = "Option<Text>")]
    pub published_time_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
    /// Contains `No views` if the view count is zero
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
    /// Contains Short/Live tag
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
    /// Abbreviated video description (on startpage)
    #[serde_as(as = "Option<Text>")]
    pub description_snippet: Option<String>,
    /// Contains abbreviated video description (on search page)
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub detailed_metadata_snippets: Option<Vec<DetailedMetadataSnippet>>,
    /// Release date for upcoming videos
    pub upcoming_event_data: Option<UpcomingEventData>,
}

/// Playlist displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    /// The first item of this list contains the playlist thumbnail,
    /// subsequent items contain very small thumbnails of the next playlist videos
    pub thumbnails: Vec<Thumbnails>,
    #[serde_as(as = "JsonString")]
    pub video_count: u64,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
    /// First 2 videos
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub videos: Vec<ChildVideoRendererWrap>,
}

/// Video displayed in a playlist
#[serde_as]
#[derive(Debug, Deserialize)]
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

/// Channel displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRenderer {
    pub channel_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    pub thumbnail: Thumbnails,
    /// Abbreviated channel description
    ///
    /// Not present if the channel has no description
    #[serde(default)]
    #[serde_as(as = "Text")]
    pub description_snippet: String,
    /// Not present if the channel has no videos
    #[serde_as(as = "Option<Text>")]
    pub video_count_text: Option<String>,
    #[serde_as(as = "Option<Text>")]
    pub subscriber_count_text: Option<String>,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
}

/// Playlist displayed on a channel page
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPlaylistRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub video_count_short_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum RichItem {
    VideoRenderer(VideoRenderer),
    PlaylistRenderer(GridPlaylistRenderer),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingEventData {
    /// Unixtime in seconds
    #[serde_as(as = "JsonString")]
    pub start_time: i64,
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
pub struct TimeOverlay {
    pub thumbnail_overlay_time_status_renderer: TimeOverlayRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOverlayRenderer {
    /// `29:54`
    ///
    /// Is `LIVE` in case of a livestream and `SHORTS` in case of a short video
    #[serde_as(as = "Text")]
    pub text: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub style: TimeOverlayStyle,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeOverlayStyle {
    #[default]
    Default,
    Live,
    Shorts,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBadge {
    pub metadata_badge_renderer: VideoBadgeRenderer,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelThumbnailSupportedRenderers {
    pub channel_thumbnail_with_link_renderer: ChannelThumbnailWithLinkRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelThumbnailWithLinkRenderer {
    pub thumbnail: Thumbnails,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedMetadataSnippet {
    #[serde_as(as = "Text")]
    pub snippet_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildVideoRendererWrap {
    pub child_video_renderer: ChildVideoRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildVideoRenderer {
    pub video_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
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

pub trait FromWLang<T> {
    fn from_w_lang(from: T, lang: Language) -> Self;
}

pub trait TryFromWLang<T>: Sized {
    fn from_w_lang(from: T, lang: Language) -> Result<Self, util::MappingError>;
}

impl FromWLang<GridVideoRenderer> for model::ChannelVideo {
    fn from_w_lang(video: GridVideoRenderer, lang: Language) -> Self {
        let mut toverlays = video.thumbnail_overlays;
        let is_live = toverlays.is_live();
        let is_short = toverlays.is_short();
        let to = toverlays.try_swap_remove(0);

        Self {
            id: video.video_id,
            title: video.title,
            // Time text is `LIVE` for livestreams, so we ignore parse errors
            length: to.and_then(|to| {
                util::parse_video_length(&to.thumbnail_overlay_time_status_renderer.text)
            }),
            thumbnail: video.thumbnail.into(),
            publish_date: video
                .upcoming_event_data
                .as_ref()
                .map(|upc| {
                    chrono::Local.from_utc_datetime(&chrono::NaiveDateTime::from_timestamp(
                        upc.start_time,
                        0,
                    ))
                })
                .or_else(|| {
                    video
                        .published_time_text
                        .as_ref()
                        .and_then(|txt| timeago::parse_timeago_to_dt(lang, txt))
                }),
            publish_date_txt: video.published_time_text,
            view_count: video
                .view_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
                .unwrap_or_default(),
            is_live,
            is_short,
            is_upcoming: video.upcoming_event_data.is_some(),
        }
    }
}

impl FromWLang<VideoRenderer> for model::ChannelVideo {
    fn from_w_lang(video: VideoRenderer, lang: Language) -> Self {
        let mut toverlays = video.thumbnail_overlays;
        let is_live = toverlays.is_live();
        let is_short = toverlays.is_short();
        let to = toverlays.try_swap_remove(0);

        Self {
            id: video.video_id,
            title: video.title,
            // Time text is `LIVE` for livestreams, so we ignore parse errors
            length: to.and_then(|to| {
                util::parse_video_length(&to.thumbnail_overlay_time_status_renderer.text)
            }),
            thumbnail: video.thumbnail.into(),
            publish_date: video
                .upcoming_event_data
                .as_ref()
                .map(|upc| {
                    chrono::Local.from_utc_datetime(&chrono::NaiveDateTime::from_timestamp(
                        upc.start_time,
                        0,
                    ))
                })
                .or_else(|| {
                    video
                        .published_time_text
                        .as_ref()
                        .and_then(|txt| timeago::parse_timeago_to_dt(lang, txt))
                }),
            publish_date_txt: video.published_time_text,
            view_count: video
                .view_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
                .unwrap_or_default(),
            is_live,
            is_short,
            is_upcoming: video.upcoming_event_data.is_some(),
        }
    }
}

impl From<GridPlaylistRenderer> for model::ChannelPlaylist {
    fn from(playlist: GridPlaylistRenderer) -> Self {
        Self {
            id: playlist.playlist_id,
            name: playlist.title,
            thumbnail: playlist.thumbnail.into(),
            video_count: util::parse_numeric(&playlist.video_count_short_text).ok(),
        }
    }
}

impl TryFromWLang<CompactVideoRenderer> for model::RecommendedVideo {
    fn from_w_lang(
        video: CompactVideoRenderer,
        lang: Language,
    ) -> Result<Self, util::MappingError> {
        let channel = model::ChannelId::try_from(video.channel)?;

        Ok(Self {
            id: video.video_id,
            title: video.title,
            length: video
                .length_text
                .and_then(|txt| util::parse_video_length(&txt)),
            thumbnail: video.thumbnail.into(),
            channel: model::ChannelTag {
                id: channel.id,
                name: channel.name,
                avatar: video.channel_thumbnail.into(),
                verification: video.owner_badges.into(),
                subscriber_count: None,
            },
            publish_date: video
                .published_time_text
                .as_ref()
                .and_then(|txt| timeago::parse_timeago_to_dt(lang, txt)),
            publish_date_txt: video.published_time_text,
            view_count: video
                .view_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
                .unwrap_or_default(),
            is_live: video.badges.is_live(),
            is_short: video.thumbnail_overlays.is_short(),
        })
    }
}

impl TryFromWLang<VideoRenderer> for model::SearchVideo {
    fn from_w_lang(video: VideoRenderer, lang: Language) -> Result<Self, util::MappingError> {
        let channel = model::ChannelId::try_from(
            video
                .channel
                .ok_or_else(|| MappingError("no video channel".into()))?,
        )?;
        let channel_thumbnail = video
            .channel_thumbnail_supported_renderers
            .ok_or_else(|| MappingError("no video channel thumbnail".into()))?;

        Ok(Self {
            id: video.video_id,
            title: video.title,
            length: video
                .length_text
                .and_then(|txt| util::parse_video_length(&txt)),
            thumbnail: video.thumbnail.into(),
            channel: model::ChannelTag {
                id: channel.id,
                name: channel.name,
                avatar: channel_thumbnail
                    .channel_thumbnail_with_link_renderer
                    .thumbnail
                    .into(),
                verification: video.owner_badges.into(),
                subscriber_count: None,
            },
            publish_date: video
                .published_time_text
                .as_ref()
                .and_then(|txt| timeago::parse_timeago_to_dt(lang, txt)),
            publish_date_txt: video.published_time_text,
            view_count: video
                .view_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
                .unwrap_or_default(),
            is_live: video.thumbnail_overlays.is_live(),
            is_short: video.thumbnail_overlays.is_short(),
            short_description: video
                .detailed_metadata_snippets
                .and_then(|mut snippets| snippets.try_swap_remove(0).map(|s| s.snippet_text))
                .or(video.description_snippet)
                .unwrap_or_default(),
        })
    }
}

impl From<PlaylistRenderer> for model::SearchPlaylist {
    fn from(playlist: PlaylistRenderer) -> Self {
        let mut thumbnails = playlist.thumbnails;

        Self {
            id: playlist.playlist_id,
            name: playlist.title,
            thumbnail: thumbnails.try_swap_remove(0).unwrap_or_default().into(),
            video_count: playlist.video_count,
            first_videos: playlist
                .videos
                .into_iter()
                .map(|v| model::SearchPlaylistVideo {
                    id: v.child_video_renderer.video_id,
                    title: v.child_video_renderer.title,
                    length: v
                        .child_video_renderer
                        .length_text
                        .and_then(|txt| util::parse_video_length(&txt)),
                })
                .collect(),
        }
    }
}

impl From<ChannelRenderer> for model::SearchChannel {
    fn from(channel: ChannelRenderer) -> Self {
        Self {
            id: channel.channel_id,
            name: channel.title,
            avatar: channel.thumbnail.into(),
            verification: channel.owner_badges.into(),
            subscriber_count: channel
                .subscriber_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok()),
            video_count: channel
                .video_count_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
                .unwrap_or_default(),
            short_description: channel.description_snippet,
        }
    }
}
