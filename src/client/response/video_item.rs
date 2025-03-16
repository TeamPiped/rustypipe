use serde::Deserialize;
use serde_with::{
    rust::deserialize_ignore_any, serde_as, DefaultOnError, DisplayFromStr, VecSkipError,
};
use time::OffsetDateTime;

use super::{ChannelBadge, ContentImage, ContinuationItemRenderer, PhMetadataView, Thumbnails};
use crate::{
    model::{Channel, ChannelItem, ChannelTag, PlaylistItem, VideoItem, YouTubeItem},
    param::Language,
    serializer::{
        text::{AttributedText, Text, TextComponent},
        MapResult,
    },
    util::{self, timeago, TryRemove},
};

#[cfg(feature = "userdata")]
use crate::{client::response::SimpleHeaderRenderer, model::HistoryItem};
#[cfg(feature = "userdata")]
use time::UtcOffset;

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum YouTubeListItem {
    #[serde(alias = "gridVideoRenderer", alias = "compactVideoRenderer")]
    VideoRenderer(VideoRenderer),
    ReelItemRenderer(ReelItemRenderer),
    ShortsLockupViewModel(ShortsLockupViewModel),
    PlaylistVideoRenderer(PlaylistVideoRenderer),

    #[serde(alias = "gridPlaylistRenderer")]
    PlaylistRenderer(PlaylistRenderer),

    ChannelRenderer(ChannelRenderer),

    LockupViewModel(LockupViewModel),

    /// Continuation items are located at the end of a list
    /// and contain the continuation token for progressive loading
    ContinuationItemRenderer(ContinuationItemRenderer),

    /// Corrected search query
    #[serde(rename_all = "camelCase")]
    ShowingResultsForRenderer {
        #[serde_as(as = "Text")]
        corrected_query: String,
    },

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
        #[cfg(feature = "userdata")]
        header: Option<ItemSectionHeader>,
        #[serde(alias = "items")]
        contents: MapResult<Vec<YouTubeListItem>>,
    },

    /// Age-restricted channel
    #[serde(rename_all = "camelCase")]
    ChannelAgeGateRenderer {
        channel_title: String,
        #[serde_as(as = "Text")]
        main_text: String,
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
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub navigation_endpoint: Option<ReelNavigationEndpoint>,
}

// New short video item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShortsLockupViewModel {
    /// `shorts-shelf-item-[video_id]`
    pub entity_id: String,
    pub thumbnail: Thumbnails,
    pub overlay_metadata: ShortsOverlayMetadata,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShortsOverlayMetadata {
    /// Title
    #[serde_as(as = "AttributedText")]
    pub primary_text: String,
    /// View count
    #[serde_as(as = "Option<AttributedText>")]
    pub secondary_text: Option<String>,
}

/// Generalized list item, currently only used for channel playlists and YTM items
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockupViewModel {
    pub content_id: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub content_type: LockupContentType,
    pub content_image: ContentImage,
    pub metadata: LockupViewModelMetadata,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum LockupContentType {
    LockupContentTypePlaylist,
    LockupContentTypeVideo,
    #[default]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockupViewModelMetadata {
    pub lockup_metadata_view_model: LockupViewModelMetadataInner,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockupViewModelMetadataInner {
    #[serde_as(as = "AttributedText")]
    pub title: String,
    pub metadata: PhMetadataView,
}

/// Video displayed in a playlist
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistVideoRenderer {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    pub channel: TextComponent,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub length_seconds: Option<u32>,
    /// Regular video: `["29K views", " • ", "13 years ago"]`
    /// Livestream: `["66K", " watching"]`
    /// Upcoming: `["8", " waiting"]`
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Text>")]
    pub video_info: Vec<String>,
    /// Contains Short/Live tag
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlay>,
    /// Release date for upcoming videos
    pub upcoming_event_data: Option<UpcomingEventData>,
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
    #[serde_as(as = "Option<DisplayFromStr>")]
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

#[cfg(feature = "userdata")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemSectionHeader {
    pub item_section_header_renderer: SimpleHeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpcomingEventData {
    /// Unixtime in seconds
    #[serde_as(as = "DisplayFromStr")]
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
        }
    }

    pub fn with_channel<C>(lang: Language, channel: &Channel<C>, warnings: Vec<String>) -> Self {
        Self {
            lang,
            channel: Some(ChannelTag {
                id: channel.id.clone(),
                name: channel.name.clone(),
                avatar: Vec::new(),
                verification: channel.verification,
                subscriber_count: channel.subscriber_count,
            }),
            items: Vec::new(),
            warnings,
            ctoken: None,
            corrected_query: None,
        }
    }

    fn map_video(&mut self, video: VideoRenderer) -> VideoItem {
        let is_live = video.thumbnail_overlays.is_live() || video.badges.is_live();
        let is_short = video.thumbnail_overlays.is_short();

        let length_text = video.length_text.or_else(|| {
            video
                .thumbnail_overlays
                .into_iter()
                .find(|ol| {
                    ol.thumbnail_overlay_time_status_renderer.style == TimeOverlayStyle::Default
                })
                .map(|ol| ol.thumbnail_overlay_time_status_renderer.text)
        });

        VideoItem {
            id: video.video_id,
            name: video.title,
            duration: length_text.and_then(|txt| util::parse_video_length(&txt)),
            thumbnail: video.thumbnail.into(),
            channel: video
                .channel
                .and_then(|c| ChannelTag::try_from(c).ok())
                .map(|mut c| {
                    c.avatar = video
                        .channel_thumbnail_supported_renderers
                        .map(|tn| tn.channel_thumbnail_with_link_renderer.thumbnail)
                        .or(video.channel_thumbnail)
                        .unwrap_or_default()
                        .into();
                    if !c.verification.verified() {
                        c.verification = video.owner_badges.into();
                    }
                    c
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
            is_live,
            is_short,
            is_upcoming: video.upcoming_event_data.is_some(),
            short_description: video
                .detailed_metadata_snippets
                .and_then(|snippets| snippets.into_iter().next().map(|s| s.snippet_text))
                .or(video.description_snippet),
        }
    }

    fn map_short_video(&mut self, video: ReelItemRenderer) -> VideoItem {
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
            duration: None,
            thumbnail: video.thumbnail.into(),
            channel: self.channel.clone(),
            publish_date: pub_date_txt.as_ref().and_then(|txt| {
                timeago::parse_timeago_dt_or_warn(self.lang, txt, &mut self.warnings)
            }),
            publish_date_txt: pub_date_txt,
            view_count: video.view_count_text.and_then(|txt| {
                util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
            }),
            is_live: false,
            is_short: true,
            is_upcoming: false,
            short_description: None,
        }
    }

    fn map_short_video2(&mut self, video: ShortsLockupViewModel) -> Option<VideoItem> {
        if let Some(video_id) = video.entity_id.strip_prefix("shorts-shelf-item-") {
            Some(VideoItem {
                id: video_id.to_owned(),
                name: video.overlay_metadata.primary_text,
                duration: None,
                thumbnail: video.thumbnail.into(),
                channel: self.channel.clone(),
                publish_date: None,
                publish_date_txt: None,
                view_count: video.overlay_metadata.secondary_text.and_then(|txt| {
                    util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
                }),
                is_live: false,
                is_short: true,
                is_upcoming: false,
                short_description: None,
            })
        } else {
            self.warnings
                .push(format!("invalid shorts entityId: {}", video.entity_id));
            None
        }
    }

    fn map_playlist_video(&mut self, video: PlaylistVideoRenderer) -> VideoItem {
        let channel = ChannelTag::try_from(video.channel).ok();
        let mut video_info = video.video_info.into_iter();
        let video_info1 = video_info
            .next()
            .map(|s| match video_info.next().as_deref() {
                None | Some(util::DOT_SEPARATOR) => s,
                Some(s2) => s + s2,
            });
        let video_info2 = video_info.next();

        // RU: "7 лет назад" " • " "210 млн просмотров" (order flipped)
        let (view_count_txt, publish_date_txt) =
            if self.lang == Language::Ru && video_info2.is_some() {
                (video_info2, video_info1)
            } else {
                (video_info1, video_info2)
            };

        let is_live = video.thumbnail_overlays.is_live();

        let publish_date = video
            .upcoming_event_data
            .as_ref()
            .and_then(|upc| OffsetDateTime::from_unix_timestamp(upc.start_time).ok())
            .or_else(|| {
                if is_live {
                    None
                } else {
                    publish_date_txt.as_ref().and_then(|txt| {
                        timeago::parse_timeago_dt_or_warn(self.lang, txt, &mut self.warnings)
                    })
                }
            });

        VideoItem {
            id: video.video_id,
            name: video.title,
            duration: video.length_seconds,
            thumbnail: video.thumbnail.into(),
            channel,
            publish_date,
            publish_date_txt,
            view_count: view_count_txt.and_then(|txt| {
                util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
            }),
            is_live,
            is_short: video.thumbnail_overlays.is_short(),
            is_upcoming: video.upcoming_event_data.is_some(),
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
                .and_then(|c| ChannelTag::try_from(c).ok())
                .map(|mut c| {
                    if !c.verification.verified() {
                        c.verification = playlist.owner_badges.into();
                    }
                    c
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
        let (handle, sc_txt) = if channel
            .subscriber_count_text
            .as_ref()
            .map(|txt| txt.starts_with('@'))
            .unwrap_or_default()
        {
            (channel.subscriber_count_text, channel.video_count_text)
        } else {
            (None, channel.subscriber_count_text)
        };

        ChannelItem {
            id: channel.channel_id,
            name: channel.title,
            handle,
            avatar: channel.thumbnail.into(),
            verification: channel.owner_badges.into(),
            subscriber_count: sc_txt.and_then(|txt| {
                util::parse_large_numstr_or_warn(&txt, self.lang, &mut self.warnings)
            }),
            short_description: channel.description_snippet,
        }
    }

    fn map_lockup(&mut self, lockup: LockupViewModel) -> Option<YouTubeItem> {
        let md = lockup.metadata.lockup_metadata_view_model;
        let tn = lockup.content_image.into_image();
        match lockup.content_type {
            LockupContentType::LockupContentTypePlaylist => {
                Some(YouTubeItem::Playlist(PlaylistItem {
                    id: lockup.content_id,
                    name: md.title,
                    thumbnail: tn.image.into(),
                    channel: self.channel.clone(),
                    video_count: tn
                        .overlays
                        .first()
                        .and_then(|ol| {
                            ol.thumbnail_overlay_badge_view_model
                                .thumbnail_badges
                                .first()
                        })
                        .and_then(|badge| {
                            util::parse_numeric(&badge.thumbnail_badge_view_model.text).ok()
                        }),
                }))
            }
            LockupContentType::LockupContentTypeVideo => {
                let mut mdr = md
                    .metadata
                    .content_metadata_view_model
                    .metadata_rows
                    .into_iter();
                let channel = mdr
                    .next()
                    .and_then(|r| r.metadata_parts.into_iter().next())
                    .and_then(|p| ChannelTag::try_from(p.into_text_component()).ok());
                let (view_count, publish_date_txt) = mdr
                    .next()
                    .map(|metadata_row| {
                        let mut parts = metadata_row.metadata_parts.into_iter();
                        let p1 = parts.next();
                        let p2 = parts.next();
                        (
                            p1.and_then(|p| {
                                util::parse_large_numstr_or_warn(
                                    p.as_str(),
                                    self.lang,
                                    &mut self.warnings,
                                )
                            }),
                            p2.map(|p2| p2.into_text_component().into_string()),
                        )
                    })
                    .unwrap_or_default();

                Some(YouTubeItem::Video(VideoItem {
                    id: lockup.content_id,
                    name: md.title,
                    duration: tn
                        .overlays
                        .first()
                        .and_then(|ol| {
                            ol.thumbnail_overlay_badge_view_model
                                .thumbnail_badges
                                .first()
                        })
                        .and_then(|badge| {
                            util::parse_video_length(&badge.thumbnail_badge_view_model.text)
                        }),
                    thumbnail: tn.image.into(),
                    channel,
                    publish_date: publish_date_txt.as_deref().and_then(|t| {
                        timeago::parse_timeago_dt_or_warn(self.lang, t, &mut self.warnings)
                    }),
                    publish_date_txt,
                    view_count,
                    is_live: false,
                    is_short: false,
                    is_upcoming: false,
                    short_description: None,
                }))
            }
            LockupContentType::Unknown => None,
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
            YouTubeListItem::ShortsLockupViewModel(video) => {
                if let Some(mapped) = self.map_short_video2(video) {
                    self.items.push(YouTubeItem::Video(mapped));
                }
            }
            YouTubeListItem::ReelItemRenderer(video) => {
                let mapped = self.map_short_video(video);
                self.items.push(YouTubeItem::Video(mapped));
            }
            YouTubeListItem::PlaylistVideoRenderer(video) => {
                let mapped = self.map_playlist_video(video);
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
            YouTubeListItem::LockupViewModel(lockup) => {
                if let Some(mapped) = self.map_lockup(lockup) {
                    self.items.push(mapped);
                }
            }
            YouTubeListItem::ContinuationItemRenderer(r) => {
                if self.ctoken.is_none() {
                    self.ctoken = r.continuation_endpoint.into_token();
                }
            }
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents, .. } => {
                self.warnings.append(&mut contents.warnings);
                contents.c.into_iter().for_each(|it| self.map_item(it));
            }
            YouTubeListItem::None | YouTubeListItem::ChannelAgeGateRenderer { .. } => {}
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
                let mapped = self.map_short_video(video);
                self.items.push(mapped);
            }
            YouTubeListItem::ShortsLockupViewModel(video) => {
                if let Some(mapped) = self.map_short_video2(video) {
                    self.items.push(mapped);
                }
            }
            YouTubeListItem::PlaylistVideoRenderer(video) => {
                let mapped = self.map_playlist_video(video);
                self.items.push(mapped);
            }
            YouTubeListItem::LockupViewModel(lockup) => {
                if let Some(YouTubeItem::Video(mapped)) = self.map_lockup(lockup) {
                    self.items.push(mapped);
                }
            }
            YouTubeListItem::ContinuationItemRenderer(r) => {
                if self.ctoken.is_none() {
                    self.ctoken = r.continuation_endpoint.into_token();
                }
            }
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents, .. } => {
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

    #[cfg(feature = "userdata")]
    pub(crate) fn conv_history_items(
        self,
        date_txt: Option<String>,
        utc_offset: UtcOffset,
        res: &mut MapResult<Vec<HistoryItem<VideoItem>>>,
    ) {
        res.warnings.extend(self.warnings);
        res.c.extend(self.items.into_iter().map(|item| HistoryItem {
            item,
            playback_date: date_txt.as_deref().and_then(|s| {
                timeago::parse_textual_date_to_d(self.lang, utc_offset, s, &mut res.warnings)
            }),
            playback_date_txt: date_txt.clone(),
        }));
    }
}

impl YouTubeListMapper<PlaylistItem> {
    fn map_item(&mut self, item: YouTubeListItem) {
        match item {
            YouTubeListItem::PlaylistRenderer(playlist) => {
                let mapped = self.map_playlist(playlist);
                self.items.push(mapped);
            }
            YouTubeListItem::LockupViewModel(lockup) => {
                if let Some(YouTubeItem::Playlist(mapped)) = self.map_lockup(lockup) {
                    self.items.push(mapped);
                }
            }
            YouTubeListItem::ContinuationItemRenderer(r) => {
                if self.ctoken.is_none() {
                    self.ctoken = r.continuation_endpoint.into_token();
                }
            }
            YouTubeListItem::ShowingResultsForRenderer { corrected_query } => {
                self.corrected_query = Some(corrected_query);
            }
            YouTubeListItem::RichItemRenderer { content } => {
                self.map_item(*content);
            }
            YouTubeListItem::ItemSectionRenderer { mut contents, .. } => {
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
