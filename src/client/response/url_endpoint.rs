use crate::{
    json::{JsonGet, JsonValue},
    model::{TrackType, UrlTarget},
    util,
};

#[derive(Debug)]
pub(crate) struct WatchEndpoint {
    pub video_id: String,
    pub playlist_id: Option<String>,
    pub start_time_seconds: u32,
    pub watch_endpoint_music_supported_configs: WatchEndpointConfig,
}

impl WatchEndpoint {
    fn from_value(v: &JsonValue) -> Option<Self> {
        let video_id = v.get_str("videoId")?;
        let playlist_id = v.get_str("playlistId");
        let start_time_seconds = v.get_u32("startTimeSeconds").unwrap_or(0);
        let watch_endpoint_music_supported_configs = v
            .get("watchEndpointMusicSupportedConfigs")
            .and_then(|w| w.get("watchEndpointMusicConfig"))
            .map(WatchEndpointConfig::from_value)
            .unwrap_or_default();
        Some(Self {
            video_id,
            playlist_id,
            start_time_seconds,
            watch_endpoint_music_supported_configs,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct WatchEndpointConfig {
    pub music_video_type: MusicVideoType,
}

impl WatchEndpointConfig {
    fn from_value(v: &JsonValue) -> Self {
        Self {
            music_video_type: v
                .get_str("musicVideoType")
                .as_deref()
                .and_then(MusicVideoType::from_str)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct BrowseEndpoint {
    pub browse_id: String,
    pub params: String,
    pub browse_endpoint_context_supported_configs: Option<BrowseEndpointConfig>,
}

impl BrowseEndpoint {
    fn from_value(v: &JsonValue) -> Option<Self> {
        let mut browse_id = v.get_str("browseId")?;
        let params = v.get_str("params").unwrap_or_default();
        let browse_endpoint_context_supported_configs = v
            .get("browseEndpointContextSupportedConfigs")
            .map(BrowseEndpointConfig::from_value);
        let page_type = browse_endpoint_context_supported_configs
            .as_ref()
            .map(|c| c.browse_endpoint_context_music_config.page_type);
        // Remove the VL prefix from the playlist id
        if page_type == Some(PageType::Playlist) {
            if let Some(stripped) = browse_id.strip_prefix("VL") {
                browse_id = stripped.to_owned();
            }
        }
        Some(Self {
            browse_id,
            params,
            browse_endpoint_context_supported_configs,
        })
    }
}

impl<'de> serde::Deserialize<'de> for BrowseEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        Self::from_value(&value).ok_or_else(|| serde::de::Error::custom("invalid browse endpoint"))
    }
}

#[derive(Debug)]
pub(crate) struct WatchPlaylistEndpoint {
    pub playlist_id: String,
}

impl WatchPlaylistEndpoint {
    fn from_value(v: &JsonValue) -> Option<Self> {
        Some(Self {
            playlist_id: v.get_str("playlistId")?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct UrlEndpoint {
    pub url: String,
}

impl UrlEndpoint {
    fn from_value(v: &JsonValue) -> Option<Self> {
        Some(Self {
            url: v.get_str("url")?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BrowseNavigationEndpoint {
    pub browse_endpoint: BrowseEndpoint,
    pub command_metadata: Option<CommandMetadata>,
}

impl BrowseNavigationEndpoint {
    fn from_value(v: &JsonValue) -> Option<Self> {
        let browse_endpoint = BrowseEndpoint::from_value(v.get("browseEndpoint")?)?;
        let command_metadata = v.get("commandMetadata").map(CommandMetadata::from_value);
        Some(Self {
            browse_endpoint,
            command_metadata,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BrowseEndpointConfig {
    pub browse_endpoint_context_music_config: BrowseEndpointMusicConfig,
}

impl BrowseEndpointConfig {
    fn from_value(v: &JsonValue) -> Self {
        Self {
            browse_endpoint_context_music_config: v
                .get("browseEndpointContextMusicConfig")
                .map(BrowseEndpointMusicConfig::from_value)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct BrowseEndpointMusicConfig {
    pub page_type: PageType,
}

impl BrowseEndpointMusicConfig {
    fn from_value(v: &JsonValue) -> Self {
        Self {
            page_type: v
                .get_str("pageType")
                .as_deref()
                .and_then(PageType::from_str)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommandMetadata {
    pub web_command_metadata: WebCommandMetadata,
}

impl CommandMetadata {
    fn from_value(v: &JsonValue) -> Self {
        Self {
            web_command_metadata: WebCommandMetadata {
                web_page_type: v
                    .get("webCommandMetadata")
                    .and_then(|w| w.get_str("webPageType"))
                    .as_deref()
                    .and_then(PageType::from_str)
                    .unwrap_or_default(),
            },
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct WebCommandMetadata {
    pub web_page_type: PageType,
}

#[derive(Debug, Clone)]
pub(crate) struct OnTap {
    pub innertube_command: JsonValue,
}

impl<'de> serde::Deserialize<'de> for OnTap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OnTapDe {
            innertube_command: JsonValue,
        }
        let de = OnTapDe::deserialize(deserializer)?;
        Ok(Self {
            innertube_command: de.innertube_command,
        })
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MusicVideoType {
    #[default]
    Video,
    Track,
    Episode,
}

impl MusicVideoType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "MUSIC_VIDEO_TYPE_OMV" | "MUSIC_VIDEO_TYPE_UGC" => Some(Self::Video),
            "MUSIC_VIDEO_TYPE_ATV" => Some(Self::Track),
            "MUSIC_VIDEO_TYPE_PODCAST_EPISODE" => Some(Self::Episode),
            _ => None,
        }
    }

    pub fn is_video(self) -> bool {
        self != Self::Track
    }

    pub fn from_is_video(is_video: bool) -> Self {
        if is_video {
            Self::Video
        } else {
            Self::Track
        }
    }
}

impl From<MusicVideoType> for TrackType {
    fn from(value: MusicVideoType) -> Self {
        match value {
            MusicVideoType::Video => Self::Video,
            MusicVideoType::Track => Self::Track,
            MusicVideoType::Episode => Self::Episode,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageType {
    Artist,
    Album,
    Channel,
    Playlist,
    Podcast,
    Episode,
    #[default]
    Unknown,
}

impl PageType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_AUDIOBOOK_ARTIST" => Some(Self::Artist),
            "MUSIC_PAGE_TYPE_ALBUM" | "MUSIC_PAGE_TYPE_AUDIOBOOK" => Some(Self::Album),
            "WEB_PAGE_TYPE_CHANNEL" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => Some(Self::Channel),
            "MUSIC_PAGE_TYPE_PLAYLIST" | "WEB_PAGE_TYPE_PLAYLIST" => Some(Self::Playlist),
            "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE" => Some(Self::Podcast),
            "MUSIC_PAGE_TYPE_NON_MUSIC_AUDIO_TRACK_PAGE" => Some(Self::Episode),
            _ => None,
        }
    }

    pub(crate) fn to_url_target(self, id: String) -> Option<UrlTarget> {
        match self {
            PageType::Artist | PageType::Channel => Some(UrlTarget::Channel { id }),
            PageType::Album => Some(UrlTarget::Album { id }),
            PageType::Playlist => Some(UrlTarget::Playlist { id }),
            PageType::Podcast => Some(UrlTarget::Playlist {
                id: util::strip_prefix(&id, util::PODCAST_PLAYLIST_PREFIX),
            }),
            PageType::Episode => Some(UrlTarget::Video {
                id: util::strip_prefix(&id, util::PODCAST_EPISODE_PREFIX),
                start_time: 0,
            }),
            PageType::Unknown => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum MusicPageType {
    Artist,
    Album,
    Playlist { is_podcast: bool },
    Track { vtype: MusicVideoType },
    User,
    None,
}

impl From<PageType> for MusicPageType {
    fn from(t: PageType) -> Self {
        match t {
            PageType::Artist => MusicPageType::Artist,
            PageType::Album => MusicPageType::Album,
            PageType::Playlist => MusicPageType::Playlist { is_podcast: false },
            PageType::Podcast => MusicPageType::Playlist { is_podcast: true },
            PageType::Channel => MusicPageType::User,
            PageType::Episode => MusicPageType::Track {
                vtype: MusicVideoType::Episode,
            },
            PageType::Unknown => MusicPageType::None,
        }
    }
}

pub(crate) struct MusicPage {
    pub id: String,
    pub typ: MusicPageType,
}

impl MusicPage {
    /// Create a new MusicPage object, applying the required ID fixes when
    /// mapping a browse link
    pub fn from_browse(mut id: String, typ: PageType) -> Self {
        if typ == PageType::Podcast {
            id = util::strip_prefix(&id, util::PODCAST_PLAYLIST_PREFIX);
        } else if typ == PageType::Episode && id.len() == 15 {
            id = util::strip_prefix(&id, util::PODCAST_EPISODE_PREFIX);
        }

        Self {
            id,
            typ: typ.into(),
        }
    }
}

pub(crate) fn watch_endpoint(endpoint: &JsonValue) -> Option<WatchEndpoint> {
    endpoint
        .get("watchEndpoint")
        .or_else(|| endpoint.get("reelWatchEndpoint"))
        .and_then(WatchEndpoint::from_value)
}

pub(crate) fn browse_endpoint(endpoint: &JsonValue) -> Option<BrowseNavigationEndpoint> {
    BrowseNavigationEndpoint::from_value(endpoint)
}

pub(crate) fn url_endpoint(endpoint: &JsonValue) -> Option<UrlEndpoint> {
    endpoint.get("urlEndpoint").and_then(UrlEndpoint::from_value)
}

pub(crate) fn watch_playlist_endpoint(endpoint: &JsonValue) -> Option<WatchPlaylistEndpoint> {
    endpoint
        .get("watchPlaylistEndpoint")
        .and_then(WatchPlaylistEndpoint::from_value)
}

/// Get the YouTube Music page and id from a browse/watch endpoint.
pub(crate) fn music_page(endpoint: &JsonValue) -> Option<MusicPage> {
    if let Some(watch_endpoint) = watch_endpoint(endpoint) {
        if watch_endpoint
            .playlist_id
            .as_deref()
            .map(|plid| plid.starts_with("RDQM"))
            .unwrap_or_default()
        {
            // Genre radios (e.g. "pop radio") will be skipped.
            return Some(MusicPage {
                id: watch_endpoint.video_id,
                typ: MusicPageType::None,
            });
        }
        return Some(MusicPage {
            id: watch_endpoint.video_id,
            typ: MusicPageType::Track {
                vtype: watch_endpoint
                    .watch_endpoint_music_supported_configs
                    .music_video_type,
            },
        });
    }

    if let Some(browse) = browse_endpoint(endpoint) {
        return browse
            .browse_endpoint
            .browse_endpoint_context_supported_configs
            .map(|config| {
                MusicPage::from_browse(
                    browse.browse_endpoint.browse_id,
                    config.browse_endpoint_context_music_config.page_type,
                )
            });
    }

    if let Some(watch_playlist_endpoint) = watch_playlist_endpoint(endpoint) {
        return Some(MusicPage {
            id: watch_playlist_endpoint.playlist_id,
            typ: MusicPageType::Playlist { is_podcast: false },
        });
    }

    endpoint.get("createPlaylistEndpoint").map(|_| MusicPage {
        id: String::new(),
        typ: MusicPageType::None,
    })
}

/// Get the page type of a browse endpoint.
pub(crate) fn page_type(endpoint: &JsonValue) -> Option<PageType> {
    let browse = browse_endpoint(endpoint)?;
    browse
        .browse_endpoint
        .browse_endpoint_context_supported_configs
        .as_ref()
        .map(|c| c.browse_endpoint_context_music_config.page_type)
        .or_else(|| {
            browse
                .command_metadata
                .as_ref()
                .map(|c| c.web_command_metadata.web_page_type)
        })
}

pub(crate) fn playlist_id(endpoint: &JsonValue) -> Option<String> {
    if let Some(watch_endpoint) = watch_endpoint(endpoint) {
        return watch_endpoint.playlist_id;
    }

    if let Some(browse) = browse_endpoint(endpoint) {
        return Some(browse.browse_endpoint.browse_id).filter(|_| {
            browse
                .browse_endpoint
                .browse_endpoint_context_supported_configs
                .map(|c| c.browse_endpoint_context_music_config.page_type == PageType::Playlist)
                .unwrap_or_default()
                || browse
                    .command_metadata
                    .map(|c| c.web_command_metadata.web_page_type == PageType::Playlist)
                    .unwrap_or_default()
        });
    }

    watch_playlist_endpoint(endpoint).map(|endpoint| endpoint.playlist_id)
}
