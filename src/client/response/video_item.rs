use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DefaultOnError, DisplayFromStr, VecSkipError};
use time::OffsetDateTime;

use super::{ChannelBadge, ContentImage, PhMetadataView, Thumbnails};
use crate::{
    json::{yt_continuation_value, ytq, JsonNode, JsonValue},
    model::{Channel, ChannelItem, ChannelTag, PlaylistItem, VideoItem, YouTubeItem},
    param::Language,
    serializer::text::{AttributedText, Text, TextComponent},
    util::{self, timeago, TryRemove},
    yt_string_enum,
    FromYtNode,
};

use crate::serializer::MapResult;

fn continuation_token(endpoint: &JsonValue) -> Option<String> {
    yt_continuation_value(endpoint)
}

fn deserialize_at<T: serde::de::DeserializeOwned>(
    node: &JsonNode<'_>,
    queries: &[crate::json::Query],
) -> Option<T> {
    node.first_of(queries).and_then(|node| node.deserialize().ok())
}

#[cfg(feature = "userdata")]
use crate::model::HistoryItem;

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
#[derive(Debug, FromYtNode)]
pub(crate) struct ShortsLockupViewModel {
    /// `shorts-shelf-item-[video_id]`
    pub entity_id: String,
    pub thumbnail: Thumbnails,
    pub overlay_metadata: ShortsOverlayMetadata,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ShortsOverlayMetadata {
    /// Title
    #[ytq_attributed_text]
    pub primary_text: String,
    /// View count
    #[ytq_attributed_text]
    pub secondary_text: Option<String>,
}

yt_string_enum! {
    #[allow(clippy::enum_variant_names)]
    pub(crate) enum LockupContentType {
        LockupContentTypePlaylist = "LOCKUP_CONTENT_TYPE_PLAYLIST",
        LockupContentTypeVideo = "LOCKUP_CONTENT_TYPE_VIDEO",
        Unknown = "",
    }
    default: LockupContentType::Unknown
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

#[derive(Debug)]
pub(crate) struct LockupViewModelMetadata {
    pub lockup_metadata_view_model: LockupViewModelMetadataInner,
}

impl<'de> Deserialize<'de> for LockupViewModelMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let raw = crate::json::value_to_json_string(
            value
                .get("lockupMetadataViewModel")
                .ok_or_else(|| serde::de::Error::missing_field("lockupMetadataViewModel"))?,
        );
        let inner: LockupViewModelMetadataInner = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("lockup metadata: {e}")))?;
        Ok(Self {
            lockup_metadata_view_model: inner,
        })
    }
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

#[derive(Debug)]
pub(crate) struct UpcomingEventData {
    /// Unixtime in seconds
    pub start_time: i64,
}

impl<'de> Deserialize<'de> for UpcomingEventData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        Ok(Self {
            start_time: value
                .get("startTime")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| serde::de::Error::missing_field("startTime"))?,
        })
    }
}

/// Badges are displayed on the video thumbnail and
/// show certain video properties (e.g. active livestream)
#[derive(Debug, FromYtNode)]
pub(crate) struct VideoBadge {
    #[ytq(.metadataBadgeRenderer.style)]
    pub style: VideoBadgeStyle,
}

yt_string_enum! {
    pub(crate) enum VideoBadgeStyle {
        /// Active livestream
        BadgeStyleTypeLiveNow = "BADGE_STYLE_TYPE_LIVE_NOW",
    }
    default: VideoBadgeStyle::BadgeStyleTypeLiveNow
}

#[serde_as]
#[derive(Debug)]
pub(crate) struct TimeOverlay {
    /// `29:54`
    ///
    /// Is `LIVE` in case of a livestream and `SHORTS` in case of a short video
    pub text: String,
    pub style: TimeOverlayStyle,
}

impl<'de> Deserialize<'de> for TimeOverlay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[serde_as]
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            #[serde_as(as = "Text")]
            text: String,
            #[serde(default)]
            #[serde_as(deserialize_as = "DefaultOnError")]
            style: TimeOverlayStyle,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wrap {
            thumbnail_overlay_time_status_renderer: Inner,
        }

        let wrap = Wrap::deserialize(deserializer)?;
        Ok(Self {
            text: wrap.thumbnail_overlay_time_status_renderer.text,
            style: wrap.thumbnail_overlay_time_status_renderer.style,
        })
    }
}

yt_string_enum! {
    pub(crate) enum TimeOverlayStyle {
        Default = "",
        Live = "LIVE",
        Shorts = "SHORTS",
    }
    default: TimeOverlayStyle::Default,
    fallback_to_default
}

#[derive(Debug, FromYtNode)]
pub(crate) struct DetailedMetadataSnippet {
    #[ytq_text]
    pub snippet_text: String,
}

#[derive(Debug, FromYtNode)]
pub(crate) struct ChannelThumbnailSupportedRenderers {
    #[ytq(.channelThumbnailWithLinkRenderer.thumbnail)]
    pub thumbnail: Thumbnails,
}

/// Short video item navigation endpoint (contains upload date)
#[derive(Debug, FromYtNode)]
pub(crate) struct ReelNavigationEndpoint {
    #[ytq(.reelWatchEndpoint.overlay.reelPlayerOverlayRenderer.reelPlayerHeaderSupportedRenderers.reelPlayerHeaderRenderer.timestampText)]
    #[ytq_text]
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
        self.iter()
            .any(|badge| badge.style == VideoBadgeStyle::BadgeStyleTypeLiveNow)
    }
}

impl IsLive for Vec<TimeOverlay> {
    fn is_live(&self) -> bool {
        self.iter()
            .any(|overlay| overlay.style == TimeOverlayStyle::Live)
    }
}

impl IsShort for Vec<TimeOverlay> {
    fn is_short(&self) -> bool {
        self.iter()
            .any(|overlay| overlay.style == TimeOverlayStyle::Shorts)
    }
}

#[derive(Clone)]
struct YoutubeMapCtx {
    lang: Language,
    channel: Option<ChannelTag>,
}

fn channel_tag_from_channel<C>(channel: &Channel<C>) -> ChannelTag {
    ChannelTag {
        id: channel.id.clone(),
        name: channel.name.clone(),
        avatar: Vec::new(),
        verification: channel.verification,
        subscriber_count: channel.subscriber_count,
    }
}

fn parse_video(video: VideoRenderer, ctx: &YoutubeMapCtx, warnings: &mut Vec<String>) -> VideoItem {
    let is_live = video.thumbnail_overlays.is_live() || video.badges.is_live();
    let is_short = video.thumbnail_overlays.is_short();

    let length_text = video.length_text.or_else(|| {
        video
            .thumbnail_overlays
            .into_iter()
            .find(|ol| ol.style == TimeOverlayStyle::Default)
            .map(|ol| ol.text)
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
                    .map(|tn| tn.thumbnail)
                    .or(video.channel_thumbnail)
                    .unwrap_or_default()
                    .into();
                if !c.verification.verified() {
                    c.verification = video.owner_badges.into();
                }
                c
            })
            .or_else(|| ctx.channel.clone()),
        publish_date: video
            .upcoming_event_data
            .as_ref()
            .and_then(|upc| OffsetDateTime::from_unix_timestamp(upc.start_time).ok())
            .or_else(|| {
                video
                    .published_time_text
                    .as_ref()
                    .and_then(|txt| timeago::parse_timeago_dt_or_warn(ctx.lang, txt, warnings))
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

fn parse_short_video(
    video: ReelItemRenderer,
    ctx: &YoutubeMapCtx,
    warnings: &mut Vec<String>,
) -> VideoItem {
    let pub_date_txt = video.navigation_endpoint.map(|n| n.timestamp_text);

    VideoItem {
        id: video.video_id,
        name: video.headline,
        duration: None,
        thumbnail: video.thumbnail.into(),
        channel: ctx.channel.clone(),
        publish_date: pub_date_txt
            .as_ref()
            .and_then(|txt| timeago::parse_timeago_dt_or_warn(ctx.lang, txt, warnings)),
        publish_date_txt: pub_date_txt,
        view_count: video
            .view_count_text
            .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, warnings)),
        is_live: false,
        is_short: true,
        is_upcoming: false,
        short_description: None,
    }
}

fn parse_short_video2(
    video: ShortsLockupViewModel,
    ctx: &YoutubeMapCtx,
    warnings: &mut Vec<String>,
) -> Option<VideoItem> {
    if let Some(video_id) = video.entity_id.strip_prefix("shorts-shelf-item-") {
        Some(VideoItem {
            id: video_id.to_owned(),
            name: video.overlay_metadata.primary_text,
            duration: None,
            thumbnail: video.thumbnail.into(),
            channel: ctx.channel.clone(),
            publish_date: None,
            publish_date_txt: None,
            view_count: video
                .overlay_metadata
                .secondary_text
                .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, warnings)),
            is_live: false,
            is_short: true,
            is_upcoming: false,
            short_description: None,
        })
    } else {
        warnings.push(format!("invalid shorts entityId: {}", video.entity_id));
        None
    }
}

fn parse_playlist_video(
    video: PlaylistVideoRenderer,
    ctx: &YoutubeMapCtx,
    warnings: &mut Vec<String>,
) -> VideoItem {
    let channel = ChannelTag::try_from(video.channel).ok();
    let mut video_info = video.video_info.into_iter();
    let video_info1 = video_info
        .next()
        .map(|s| match video_info.next().as_deref() {
            None | Some(util::DOT_SEPARATOR) => s,
            Some(s2) => s + s2,
        });
    let video_info2 = video_info.next();

    let (view_count_txt, publish_date_txt) = if ctx.lang == Language::Ru && video_info2.is_some() {
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
                publish_date_txt
                    .as_ref()
                    .and_then(|txt| timeago::parse_timeago_dt_or_warn(ctx.lang, txt, warnings))
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
        view_count: view_count_txt
            .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, warnings)),
        is_live,
        is_short: video.thumbnail_overlays.is_short(),
        is_upcoming: video.upcoming_event_data.is_some(),
        short_description: None,
    }
}

fn parse_playlist(playlist: PlaylistRenderer, ctx: &YoutubeMapCtx) -> PlaylistItem {
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
            .or_else(|| ctx.channel.clone()),
        video_count: playlist.video_count.or_else(|| {
            playlist
                .video_count_short_text
                .and_then(|txt| util::parse_numeric(&txt).ok())
        }),
    }
}

fn parse_channel(
    channel: ChannelRenderer,
    ctx: &YoutubeMapCtx,
    warnings: &mut Vec<String>,
) -> ChannelItem {
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
        subscriber_count: sc_txt
            .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, warnings)),
        short_description: channel.description_snippet,
    }
}

fn parse_lockup(
    lockup: LockupViewModel,
    ctx: &YoutubeMapCtx,
    warnings: &mut Vec<String>,
) -> Option<YouTubeItem> {
    let md = lockup.metadata.lockup_metadata_view_model;
    let tn = lockup.content_image.into_image();
    match lockup.content_type {
        LockupContentType::LockupContentTypePlaylist => Some(YouTubeItem::Playlist(PlaylistItem {
            id: lockup.content_id,
            name: md.title,
            thumbnail: tn.image.into(),
            channel: ctx.channel.clone(),
            video_count: tn
                .overlays
                .first()
                .and_then(|ol| {
                    ol.thumbnail_overlay_badge_view_model
                        .thumbnail_badges
                        .first()
                })
                .and_then(|badge| util::parse_numeric(&badge.thumbnail_badge_view_model.text).ok()),
        })),
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
                            util::parse_large_numstr_or_warn(p.as_str(), ctx.lang, warnings)
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
                publish_date: publish_date_txt
                    .as_deref()
                    .and_then(|t| timeago::parse_timeago_dt_or_warn(ctx.lang, t, warnings)),
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

struct YouTubeListState<T> {
    items: Vec<T>,
    warnings: Vec<String>,
    ctoken: Option<String>,
    corrected_query: Option<String>,
}

impl<T> Default for YouTubeListState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
            corrected_query: None,
        }
    }
}

fn collect_youtube_items(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<YouTubeItem>,
) {
    for item in node.items() {
        collect_youtube_item(&item, ctx, state);
    }
}

fn collect_youtube_item(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<YouTubeItem>,
) {
    if let Some(video) = deserialize_at::<VideoRenderer>(
        node,
        &[ytq!(
            .videoRenderer || .gridVideoRenderer || .compactVideoRenderer
        )],
    ) {
        state.items.push(YouTubeItem::Video(parse_video(
            video,
            ctx,
            &mut state.warnings,
        )));
    } else if let Some(video) =
        deserialize_at::<ShortsLockupViewModel>(node, &[ytq!(.shortsLockupViewModel)])
    {
        if let Some(mapped) = parse_short_video2(video, ctx, &mut state.warnings) {
            state.items.push(YouTubeItem::Video(mapped));
        }
    } else if let Some(video) = deserialize_at::<ReelItemRenderer>(node, &[ytq!(.reelItemRenderer)])
    {
        state.items.push(YouTubeItem::Video(parse_short_video(
            video,
            ctx,
            &mut state.warnings,
        )));
    } else if let Some(video) =
        deserialize_at::<PlaylistVideoRenderer>(node, &[ytq!(.playlistVideoRenderer)])
    {
        state.items.push(YouTubeItem::Video(parse_playlist_video(
            video,
            ctx,
            &mut state.warnings,
        )));
    } else if let Some(playlist) = deserialize_at::<PlaylistRenderer>(
        node,
        &[ytq!(.playlistRenderer || .gridPlaylistRenderer)],
    ) {
        state
            .items
            .push(YouTubeItem::Playlist(parse_playlist(playlist, ctx)));
    } else if let Some(channel) = deserialize_at::<ChannelRenderer>(node, &[ytq!(.channelRenderer)])
    {
        state.items.push(YouTubeItem::Channel(parse_channel(
            channel,
            ctx,
            &mut state.warnings,
        )));
    } else if let Some(lockup) = deserialize_at::<LockupViewModel>(node, &[ytq!(.lockupViewModel)])
    {
        if let Some(mapped) = parse_lockup(lockup, ctx, &mut state.warnings) {
            state.items.push(mapped);
        }
    } else if let Some(endpoint) = node.query(ytq!(.continuationItemRenderer.continuationEndpoint))
    {
        if state.ctoken.is_none() {
            if let Ok(endpoint) = endpoint.deserialize::<JsonValue>() {
                state.ctoken = continuation_token(&endpoint);
            }
        }
    } else if let Some(corrected_query) =
        node.text_at(ytq!(.showingResultsForRenderer.correctedQuery))
    {
        state.corrected_query = Some(corrected_query);
    } else if let Some(content) = node
        .query(ytq!(.(.richItemRenderer || .shelfRenderer).content))
    {
        collect_youtube_item(&content, ctx, state);
    } else if let Some(contents) = node.query(ytq!(
        (.itemSectionRenderer || .gridRenderer).contents
            || (.expandedShelfContentsRenderer || .gridRenderer).items
    )) {
        collect_youtube_items(&contents, ctx, state);
    }
}

fn collect_video_items(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<VideoItem>,
) {
    for item in node.items() {
        collect_video_item(&item, ctx, state);
    }
}

fn collect_video_item(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<VideoItem>,
) {
    if let Some(video) = deserialize_at::<VideoRenderer>(
        node,
        &[ytq!(
            .videoRenderer || .gridVideoRenderer || .compactVideoRenderer
        )],
    ) {
        state
            .items
            .push(parse_video(video, ctx, &mut state.warnings));
    } else if let Some(video) = deserialize_at::<ReelItemRenderer>(node, &[ytq!(.reelItemRenderer)])
    {
        state
            .items
            .push(parse_short_video(video, ctx, &mut state.warnings));
    } else if let Some(video) =
        deserialize_at::<ShortsLockupViewModel>(node, &[ytq!(.shortsLockupViewModel)])
    {
        if let Some(mapped) = parse_short_video2(video, ctx, &mut state.warnings) {
            state.items.push(mapped);
        }
    } else if let Some(video) =
        deserialize_at::<PlaylistVideoRenderer>(node, &[ytq!(.playlistVideoRenderer)])
    {
        state
            .items
            .push(parse_playlist_video(video, ctx, &mut state.warnings));
    } else if let Some(lockup) = deserialize_at::<LockupViewModel>(node, &[ytq!(.lockupViewModel)])
    {
        if let Some(YouTubeItem::Video(mapped)) = parse_lockup(lockup, ctx, &mut state.warnings) {
            state.items.push(mapped);
        }
    } else if let Some(endpoint) = node.query(ytq!(.continuationItemRenderer.continuationEndpoint))
    {
        if state.ctoken.is_none() {
            if let Ok(endpoint) = endpoint.deserialize::<JsonValue>() {
                state.ctoken = continuation_token(&endpoint);
            }
        }
    } else if let Some(corrected_query) =
        node.text_at(ytq!(.showingResultsForRenderer.correctedQuery))
    {
        state.corrected_query = Some(corrected_query);
    } else if let Some(content) = node
        .query(ytq!(.(.richItemRenderer || .shelfRenderer).content))
    {
        collect_video_item(&content, ctx, state);
    } else if let Some(contents) = node.query(ytq!(
        (.itemSectionRenderer || .gridRenderer).contents
            || (.expandedShelfContentsRenderer || .gridRenderer).items
    )) {
        collect_video_items(&contents, ctx, state);
    }
}

fn collect_playlist_items(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<PlaylistItem>,
) {
    for item in node.items() {
        collect_playlist_item(&item, ctx, state);
    }
}

fn collect_playlist_item(
    node: &JsonNode<'_>,
    ctx: &YoutubeMapCtx,
    state: &mut YouTubeListState<PlaylistItem>,
) {
    if let Some(playlist) = deserialize_at::<PlaylistRenderer>(
        node,
        &[ytq!(.playlistRenderer || .gridPlaylistRenderer)],
    ) {
        state.items.push(parse_playlist(playlist, ctx));
    } else if let Some(lockup) = deserialize_at::<LockupViewModel>(node, &[ytq!(.lockupViewModel)])
    {
        if let Some(YouTubeItem::Playlist(mapped)) = parse_lockup(lockup, ctx, &mut state.warnings)
        {
            state.items.push(mapped);
        }
    } else if let Some(endpoint) = node.query(ytq!(.continuationItemRenderer.continuationEndpoint))
    {
        if state.ctoken.is_none() {
            if let Ok(endpoint) = endpoint.deserialize::<JsonValue>() {
                state.ctoken = continuation_token(&endpoint);
            }
        }
    } else if let Some(corrected_query) =
        node.text_at(ytq!(.showingResultsForRenderer.correctedQuery))
    {
        state.corrected_query = Some(corrected_query);
    } else if let Some(content) = node
        .query(ytq!(.(.richItemRenderer || .shelfRenderer).content))
    {
        collect_playlist_item(&content, ctx, state);
    } else if let Some(contents) = node.query(ytq!(
        (.itemSectionRenderer || .gridRenderer).contents
            || (.expandedShelfContentsRenderer || .gridRenderer).items
    )) {
        collect_playlist_items(&contents, ctx, state);
    }
}

fn map_result<T>(
    state: YouTubeListState<T>,
) -> (MapResult<Vec<T>>, Option<String>, Option<String>) {
    (
        MapResult {
            c: state.items,
            warnings: state.warnings,
        },
        state.ctoken,
        state.corrected_query,
    )
}

pub(crate) fn map_youtube_items(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<YouTubeItem>>, Option<String>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: None,
    };
    let mut state = YouTubeListState::default();
    collect_youtube_items(node, &ctx, &mut state);
    map_result(state)
}

pub(crate) fn map_youtube_item(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<YouTubeItem>>, Option<String>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: None,
    };
    let mut state = YouTubeListState::default();
    collect_youtube_item(node, &ctx, &mut state);
    map_result(state)
}

pub(crate) fn map_video_items(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<VideoItem>>, Option<String>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: None,
    };
    let mut state = YouTubeListState::default();
    collect_video_items(node, &ctx, &mut state);
    map_result(state)
}

pub(crate) fn map_video_item(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<VideoItem>>, Option<String>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: None,
    };
    let mut state = YouTubeListState::default();
    collect_video_item(node, &ctx, &mut state);
    map_result(state)
}

pub(crate) fn map_channel_video_items<C>(
    node: Option<&JsonNode<'_>>,
    lang: Language,
    channel: &Channel<C>,
    warnings: Vec<String>,
) -> (MapResult<Vec<VideoItem>>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: Some(channel_tag_from_channel(channel)),
    };
    let mut state = YouTubeListState {
        warnings,
        ..Default::default()
    };
    if let Some(node) = node {
        collect_video_items(node, &ctx, &mut state);
    }
    let (mapped, ctoken, _) = map_result(state);
    (mapped, ctoken)
}

pub(crate) fn map_channel_playlist_items<C>(
    node: Option<&JsonNode<'_>>,
    lang: Language,
    channel: &Channel<C>,
    warnings: Vec<String>,
) -> (MapResult<Vec<PlaylistItem>>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: Some(channel_tag_from_channel(channel)),
    };
    let mut state = YouTubeListState {
        warnings,
        ..Default::default()
    };
    if let Some(node) = node {
        collect_playlist_items(node, &ctx, &mut state);
    }
    let (mapped, ctoken, _) = map_result(state);
    (mapped, ctoken)
}

#[allow(dead_code)]
pub(crate) fn map_playlist_items(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<PlaylistItem>>, Option<String>, Option<String>) {
    let ctx = YoutubeMapCtx {
        lang,
        channel: None,
    };
    let mut state = YouTubeListState::default();
    collect_playlist_items(node, &ctx, &mut state);
    map_result(state)
}

#[cfg(feature = "userdata")]
pub(crate) fn extend_video_history_items(
    node: &JsonNode<'_>,
    lang: Language,
    date_txt: Option<String>,
    utc_offset: time::UtcOffset,
    res: &mut MapResult<Vec<HistoryItem<VideoItem>>>,
) {
    let (mut mapped, _, _) = map_video_items(node, lang);
    res.warnings.append(&mut mapped.warnings);
    res.c.extend(mapped.c.into_iter().map(|item| HistoryItem {
        item,
        playback_date:
            date_txt.as_deref().and_then(|s| {
                timeago::parse_textual_date_to_d(lang, utc_offset, s, &mut res.warnings)
            }),
        playback_date_txt: date_txt.clone(),
    }));
}
