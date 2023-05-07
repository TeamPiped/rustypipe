use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_with::{
    json::JsonString, rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError,
};
use time::OffsetDateTime;

use super::{url_endpoint::NavigationEndpoint, ChannelBadge, ContinuationEndpoint, Thumbnails};
use crate::{
    model::{
        Channel, ChannelId, ChannelInfo, ChannelItem, ChannelTag, PlaylistItem, VideoItem,
        YouTubeItem,
    },
    param::Language,
    serializer::{
        text::{AccessibilityText, Text, TextComponent},
        MapResult,
    },
    util::{self, timeago, TryRemove},
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum YouTubeListItem {
    #[serde(alias = "gridVideoRenderer", alias = "compactVideoRenderer")]
    VideoRenderer(VideoRenderer),
    ReelItemRenderer(ReelItemRenderer),

    #[serde(alias = "gridPlaylistRenderer")]
    PlaylistRenderer(PlaylistRenderer),

    ChannelRenderer(ChannelRenderer),

    /// Continauation items are located at the end of a list
    /// and contain the continuation token for progressive loading
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },

    /// Corrected search query
    #[serde(rename_all = "camelCase")]
    ShowingResultsForRenderer {
        #[serde_as(as = "Text")]
        corrected_query: String,
    },

    /// Channel metadata (about tab)
    ChannelAboutFullMetadataRenderer(ChannelFullMetadata),

    /// Contains video on startpage
    ///
    /// Seems to be currently A/B tested on the channel page,
    /// as of 11.10.2022
    #[serde(alias = "shelfRenderer")]
    RichItemRenderer {
        content: Box<YouTubeListItem>,
    },

    /// Contains search results
    ///
    /// Seems to be currently A/B tested on the video details page,
    /// as of 11.10.2022
    ///
    /// GridRenderer: contains videos on channel page
    #[serde(alias = "expandedShelfContentsRenderer", alias = "gridRenderer")]
    ItemSectionRenderer {
        #[serde(alias = "items")]
        contents: MapResult<Vec<YouTubeListItem>>,
    },

    /// No video list item (e.g. ad) or unimplemented item
    ///
    /// Unimplemented:
    /// - compactPlaylistRenderer (recommended playlists)
    /// - compactRadioRenderer (recommended mix)
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: Option<TextComponent>,
    pub channel_thumbnail: Option<Thumbnails>,
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
    /// Contains live tag for recommended videos
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<VideoBadge>,
    /// Contains Short/Live tag
    #[serde(default)]
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

/// Short video item
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelItemRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub headline: String,
    /// Contains `No views` if the view count is zero
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    /// video duration
    ///
    /// Example: `the horror maze - 44 seconds - play video`
    ///
    /// Dashes may be `\u2013` (emdash)
    #[serde_as(as = "Option<AccessibilityText>")]
    pub accessibility: Option<String>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub navigation_endpoint: Option<ReelNavigationEndpoint>,
}

/// Playlist displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    pub thumbnail: Option<Thumbnails>,
    /// Used by playlists from search page
    ///
    /// The first item of this list contains the playlist thumbnail,
    /// subsequent items contain very small thumbnails of the next playlist videos
    pub thumbnails: Option<Vec<Thumbnails>>,
    #[serde_as(as = "Option<JsonString>")]
    pub video_count: Option<u64>,
    #[serde_as(as = "Option<Text>")]
    pub video_count_short_text: Option<String>,
    #[serde(rename = "shortBylineText")]
    pub channel: Option<TextComponent>,
    /// Channel verification badge
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub owner_badges: Vec<ChannelBadge>,
}

/// Channel displayed in search results
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelRenderer {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YouTubeListRendererWrap {
    #[serde(alias = "richGridRenderer")]
    pub section_list_renderer: YouTubeListRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YouTubeListRenderer {
    pub contents: MapResult<Vec<YouTubeListItem>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpcomingEventData {
    /// Unixtime in seconds
    #[serde_as(as = "JsonString")]
    pub start_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeOverlay {
    pub thumbnail_overlay_time_status_renderer: TimeOverlayRenderer,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoBadge {
    pub metadata_badge_renderer: VideoBadgeRenderer,
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoBadgeRenderer {
    pub style: VideoBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum VideoBadgeStyle {
    /// Active livestream
    BadgeStyleTypeLiveNow,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeOverlayRenderer {
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
pub(crate) enum TimeOverlayStyle {
    #[default]
    Default,
    Live,
    Shorts,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DetailedMetadataSnippet {
    #[serde_as(as = "Text")]
    pub snippet_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelThumbnailSupportedRenderers {
    pub channel_thumbnail_with_link_renderer: ChannelThumbnailWithLinkRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelThumbnailWithLinkRenderer {
    pub thumbnail: Thumbnails,
}

/// Short video item navigation endpoint (contains upload date)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelNavigationEndpoint {
    pub reel_watch_endpoint: ReelWatchEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelWatchEndpoint {
    pub overlay: ReelPlayerOverlay,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelPlayerOverlay {
    pub reel_player_overlay_renderer: ReelPlayerOverlayRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelPlayerOverlayRenderer {
    pub reel_player_header_supported_renderers: ReelPlayerHeaderRenderers,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelPlayerHeaderRenderers {
    pub reel_player_header_renderer: ReelPlayerHeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReelPlayerHeaderRenderer {
    #[serde_as(as = "Text")]
    pub timestamp_text: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelFullMetadata {
    #[serde_as(as = "Text")]
    pub joined_date_text: String,
    #[serde_as(as = "Option<Text>")]
    pub view_count_text: Option<String>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub primary_links: Vec<PrimaryLink>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrimaryLink {
    #[serde_as(as = "Text")]
    pub title: String,
    pub navigation_endpoint: NavigationEndpoint,
}

trait IsLive {
    fn is_live(&self) -> bool;
}

trait IsShort {
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

/// Result of mapping a list of different YouTube enities
/// (videos, channels, playlists)
#[derive(Debug)]
pub(crate) struct YouTubeListMapper<T> {
    lang: Language,
    channel: Option<ChannelTag>,

    pub items: Vec<T>,
    pub warnings: Vec<String>,
    pub ctoken: Option<String>,
    pub corrected_query: Option<String>,
    pub channel_info: Option<ChannelInfo>,
}

impl<T> YouTubeListMapper<T> {
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            channel: None,
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
            corrected_query: None,
            channel_info: None,
        }
    }

    pub fn with_channel<C>(lang: Language, channel: &Channel<C>, warnings: Vec<String>) -> Self {
        Self {
            lang,
            channel: Some(ChannelTag {
                id: channel.id.to_owned(),
                name: channel.name.to_owned(),
                avatar: Vec::new(),
                verification: channel.verification,
                subscriber_count: channel.subscriber_count,
            }),
            items: Vec::new(),
            warnings,
            ctoken: None,
            corrected_query: None,
            channel_info: None,
        }
    }

    fn map_video(&mut self, video: VideoRenderer) -> VideoItem {
        let mut tn_overlays = video.thumbnail_overlays;
        let length_text = video.length_text.or_else(|| {
            tn_overlays
                .try_swap_remove(0)
                .map(|overlay| overlay.thumbnail_overlay_time_status_renderer.text)
        });

        VideoItem {
            id: video.video_id,
            name: video.title,
            length: length_text.and_then(|txt| util::parse_video_length(&txt)),
            thumbnail: video.thumbnail.into(),
            channel: video
                .channel
                .and_then(|c| {
                    ChannelId::try_from(c).ok().map(|c| ChannelTag {
                        id: c.id,
                        name: c.name,
                        avatar: video
                            .channel_thumbnail_supported_renderers
                            .map(|tn| tn.channel_thumbnail_with_link_renderer.thumbnail)
                            .or(video.channel_thumbnail)
                            .unwrap_or_default()
                            .into(),
                        verification: video.owner_badges.into(),
                        subscriber_count: None,
                    })
                })
                .or_else(|| self.channel.clone()),
            publish_date: video
                .upcoming_event_data
                .as_ref()
                .and_then(|upc| OffsetDateTime::from_unix_timestamp(upc.start_time).ok())
                .or_else(|| {
                    video.published_time_text.as_ref().and_then(|txt| {
                        timeago::parse_timeago_dt_or_warn(self.lang, txt, &mut self.warnings)
                    })
                }),
            publish_date_txt: video.published_time_text,
            view_count: video
                .view_count_text
                .map(|txt| util::parse_numeric(&txt).unwrap_or_default()),
            is_live: tn_overlays.is_live() || video.badges.is_live(),
            is_short: tn_overlays.is_short(),
            is_upcoming: video.upcoming_event_data.is_some(),
            short_description: video
                .detailed_metadata_snippets
                .and_then(|mut snippets| snippets.try_swap_remove(0).map(|s| s.snippet_text))
                .or(video.description_snippet),
        }
    }

    fn map_short_video(&mut self, video: ReelItemRenderer, lang: Language) -> VideoItem {
        static ACCESSIBILITY_SEP_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(" [-\u{2013}] (.+) [-\u{2013}] ").unwrap());

        let pub_date_txt = video.navigation_endpoint.map(|n| {
            n.reel_watch_endpoint
                .overlay
                .reel_player_overlay_renderer
                .reel_player_header_supported_renderers
                .reel_player_header_renderer
                .timestamp_text
        });

        VideoItem {
            id: video.video_id,
            name: video.headline,
            length: video.accessibility.and_then(|acc| {
                ACCESSIBILITY_SEP_REGEX.captures(&acc).and_then(|cap| {
                    cap.get(1).and_then(|c| {
                        timeago::parse_video_duration_or_warn(
                            self.lang,
                            c.as_str(),
                            &mut self.warnings,
                        )
                    })
                })
            }),
            thumbnail: video.thumbnail.into(),
            channel: self.channel.clone(),
            publish_date: pub_date_txt.as_ref().and_then(|txt| {
                timeago::parse_timeago_dt_or_warn(self.lang, txt, &mut self.warnings)
            }),
            publish_date_txt: pub_date_txt,
            view_count: video
                .view_count_text
                .and_then(|txt| util::parse_large_numstr_or_warn(&txt, lang, &mut self.warnings)),
            is_live: false,
            is_short: true,
            is_upcoming: false,
            short_description: None,
        }
    }

    fn map_playlist(&self, playlist: PlaylistRenderer) -> PlaylistItem {
        PlaylistItem {
            id: playlist.playlist_id,
            name: playlist.title,
            thumbnail: playlist
                .thumbnail
                .or_else(|| playlist.thumbnails.and_then(|mut t| t.try_swap_remove(0)))
                .unwrap_or_default()
                .into(),
            channel: playlist
                .channel
                .and_then(|c| {
                    ChannelId::try_from(c).ok().map(|c| ChannelTag {
                        id: c.id,
                        name: c.name,
                        avatar: Vec::new(),
                        verification: playlist.owner_badges.into(),
                        subscriber_count: None,
                    })
                })
                .or_else(|| self.channel.clone()),
            video_count: playlist.video_count.or_else(|| {
                playlist
                    .video_count_short_text
                    .and_then(|txt| util::parse_numeric(&txt).ok())
            }),
        }
    }

    fn map_channel(&mut self, channel: ChannelRenderer) -> ChannelItem {
        // channel handle instead of subscriber count (A/B test 3)
        let (sc_txt, vc_text) = match channel
            .subscriber_count_text
            .as_ref()
            .map(|txt| txt.starts_with('@'))
            .unwrap_or_default()
        {
            true => (channel.video_count_text, None),
            false => (channel.subscriber_count_text, channel.video_count_text),
        };

        ChannelItem {
            id: channel.channel_id,
            name: channel.title,
            avatar: channel.thumbnail.into(),
            verification: channel.owner_badges.into(),
            subscriber_count: sc_txt.and_then(|txt| {
                util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
            }),
            video_count: vc_text.and_then(|txt| {
                util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
            }),
            short_description: channel.description_snippet,
        }
    }
}

impl YouTubeListMapper<YouTubeItem> {
    fn map_item(&mut self, item: YouTubeListItem) {
        match item {
            YouTubeListItem::VideoRenderer(video) => {
                let mapped = YouTubeItem::Video(self.map_video(video));
                self.items.push(mapped);
            }
            YouTubeListItem::ReelItemRenderer(video) => {
                let mapped = self.map_short_video(video, self.lang);
                self.items.push(YouTubeItem::Video(mapped));
            }
            YouTubeListItem::PlaylistRenderer(playlist) => {
                let mapped = YouTubeItem::Playlist(self.map_playlist(playlist));
                self.items.push(mapped);
            }
            YouTubeListItem::ChannelRenderer(channel) => {
                let mapped = YouTubeItem::Channel(self.map_channel(channel));
                self.items.push(mapped);
            }
            YouTubeListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => self.ctoken = Some(continuation_endpoint.continuation_command.token),
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::ChannelAboutFullMetadataRenderer(meta) => {
                self.channel_info = Some(ChannelInfo {
                    create_date: timeago::parse_textual_date_or_warn(
                        self.lang,
                        &meta.joined_date_text,
                        &mut self.warnings,
                    )
                    .map(OffsetDateTime::date),
                    view_count: meta
                        .view_count_text
                        .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut self.warnings)),
                    links: meta
                        .primary_links
                        .into_iter()
                        .filter_map(|l| {
                            l.navigation_endpoint
                                .url_endpoint
                                .map(|url| (l.title, util::sanitize_yt_url(&url.url)))
                        })
                        .collect(),
                })
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents } => {
                self.warnings.append(&mut contents.warnings);
                contents.c.into_iter().for_each(|it| self.map_item(it));
            }
            YouTubeListItem::None => {}
        }
    }

    pub(crate) fn map_response(&mut self, mut res: MapResult<Vec<YouTubeListItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| self.map_item(item));
    }
}

impl YouTubeListMapper<VideoItem> {
    fn map_item(&mut self, item: YouTubeListItem) {
        match item {
            YouTubeListItem::VideoRenderer(video) => {
                let mapped = self.map_video(video);
                self.items.push(mapped);
            }
            YouTubeListItem::ReelItemRenderer(video) => {
                let mapped = self.map_short_video(video, self.lang);
                self.items.push(mapped);
            }
            YouTubeListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => self.ctoken = Some(continuation_endpoint.continuation_command.token),
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents } => {
                self.warnings.append(&mut contents.warnings);
                contents.c.into_iter().for_each(|it| self.map_item(it));
            }
            _ => {}
        }
    }

    pub(crate) fn map_response(&mut self, mut res: MapResult<Vec<YouTubeListItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| self.map_item(item));
    }
}

impl YouTubeListMapper<PlaylistItem> {
    fn map_item(&mut self, item: YouTubeListItem) {
        match item {
            YouTubeListItem::PlaylistRenderer(playlist) => {
                let mapped = self.map_playlist(playlist);
                self.items.push(mapped)
            }
            YouTubeListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => self.ctoken = Some(continuation_endpoint.continuation_command.token),
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents } => {
                self.warnings.append(&mut contents.warnings);
                contents.c.into_iter().for_each(|it| self.map_item(it));
            }
            _ => {}
        }
    }

    pub(crate) fn map_response(&mut self, mut res: MapResult<Vec<YouTubeListItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| self.map_item(item));
    }
}
