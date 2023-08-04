use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::{model::UrlTarget, util};

/// navigation/resolve_url response model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedUrl {
    pub endpoint: NavigationEndpoint,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum NavigationEndpoint {
    #[serde(rename_all = "camelCase")]
    Watch {
        #[serde(alias = "reelWatchEndpoint")]
        watch_endpoint: WatchEndpoint,
    },
    #[serde(rename_all = "camelCase")]
    Browse {
        browse_endpoint: BrowseEndpoint,
        #[serde(default)]
        #[serde_as(deserialize_as = "DefaultOnError")]
        command_metadata: Option<CommandMetadata>,
    },
    #[serde(rename_all = "camelCase")]
    Url { url_endpoint: UrlEndpoint },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchEndpoint {
    pub video_id: String,
    pub playlist_id: Option<String>,
    #[serde(default)]
    pub start_time_seconds: u32,
    #[serde(default)]
    pub watch_endpoint_music_supported_configs: WatchEndpointConfigWrap,
}

#[derive(Debug)]
pub(crate) struct BrowseEndpoint {
    pub browse_id: String,
    pub params: String,
    pub browse_endpoint_context_supported_configs: Option<BrowseEndpointConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowseEndpointWrap {
    pub browse_endpoint: BrowseEndpoint,
}

impl<'de> Deserialize<'de> for BrowseEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BEp {
            pub browse_id: String,
            #[serde(default)]
            pub params: String,
            pub browse_endpoint_context_supported_configs: Option<BrowseEndpointConfig>,
        }

        let bep = BEp::deserialize(deserializer)?;

        // Remove the VL prefix from the playlist id
        #[allow(clippy::map_unwrap_or)]
        let browse_id = bep
            .browse_endpoint_context_supported_configs
            .as_ref()
            .and_then(
                |cfg| match cfg.browse_endpoint_context_music_config.page_type {
                    PageType::Playlist => bep.browse_id.strip_prefix("VL"),
                    _ => None,
                },
            )
            .map(str::to_owned)
            .unwrap_or(bep.browse_id);

        Ok(Self {
            browse_id,
            params: bep.params,
            browse_endpoint_context_supported_configs: bep
                .browse_endpoint_context_supported_configs,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlEndpoint {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowseEndpointConfig {
    pub browse_endpoint_context_music_config: BrowseEndpointMusicConfig,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowseEndpointMusicConfig {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub page_type: PageType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandMetadata {
    pub web_command_metadata: WebCommandMetadata,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCommandMetadata {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub web_page_type: PageType,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchEndpointConfigWrap {
    pub watch_endpoint_music_config: WatchEndpointConfig,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchEndpointConfig {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub music_video_type: MusicVideoType,
}

#[derive(Default, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(crate) enum MusicVideoType {
    #[default]
    #[serde(rename = "MUSIC_VIDEO_TYPE_OMV")]
    Video,
    #[serde(rename = "MUSIC_VIDEO_TYPE_ATV")]
    Track,
}

#[derive(Default, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(crate) enum PageType {
    #[serde(
        rename = "MUSIC_PAGE_TYPE_ARTIST",
        alias = "MUSIC_PAGE_TYPE_AUDIOBOOK_ARTIST"
    )]
    Artist,
    #[serde(rename = "MUSIC_PAGE_TYPE_ALBUM", alias = "MUSIC_PAGE_TYPE_AUDIOBOOK")]
    Album,
    #[serde(
        rename = "WEB_PAGE_TYPE_CHANNEL",
        alias = "MUSIC_PAGE_TYPE_USER_CHANNEL"
    )]
    Channel,
    #[serde(rename = "MUSIC_PAGE_TYPE_PLAYLIST", alias = "WEB_PAGE_TYPE_PLAYLIST")]
    Playlist,
    #[default]
    Unknown,
}

impl PageType {
    pub(crate) fn to_url_target(self, id: String) -> Option<UrlTarget> {
        match self {
            PageType::Artist | PageType::Channel => Some(UrlTarget::Channel { id }),
            PageType::Album => Some(UrlTarget::Album { id }),
            PageType::Playlist => Some(UrlTarget::Playlist { id }),
            PageType::Unknown => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum MusicPageType {
    Artist,
    Album,
    Playlist,
    Track { is_video: bool },
    Unknown,
    None,
}

impl From<PageType> for MusicPageType {
    fn from(t: PageType) -> Self {
        match t {
            PageType::Artist => MusicPageType::Artist,
            PageType::Album => MusicPageType::Album,
            PageType::Playlist => MusicPageType::Playlist,
            PageType::Channel => MusicPageType::None,
            PageType::Unknown => MusicPageType::Unknown,
        }
    }
}

impl NavigationEndpoint {
    /// Get the YouTube Music page and id from a browse/watch endpoint
    pub(crate) fn music_page(self) -> Option<(MusicPageType, String)> {
        match self {
            NavigationEndpoint::Watch { watch_endpoint } => {
                if watch_endpoint
                    .playlist_id
                    .map(|plid| plid.starts_with("RDQM"))
                    .unwrap_or_default()
                {
                    // Genre radios (e.g. "pop radio") will be skipped
                    Some((MusicPageType::None, watch_endpoint.video_id))
                } else {
                    Some((
                        MusicPageType::Track {
                            is_video: watch_endpoint
                                .watch_endpoint_music_supported_configs
                                .watch_endpoint_music_config
                                .music_video_type
                                == MusicVideoType::Video,
                        },
                        watch_endpoint.video_id,
                    ))
                }
            }
            NavigationEndpoint::Browse {
                browse_endpoint, ..
            } => browse_endpoint
                .browse_endpoint_context_supported_configs
                .map(|config| {
                    (
                        config.browse_endpoint_context_music_config.page_type.into(),
                        browse_endpoint.browse_id,
                    )
                }),
            NavigationEndpoint::Url { .. } => None,
        }
    }

    /// Get the page type of a browse endpoint
    pub(crate) fn page_type(&self) -> Option<PageType> {
        if let NavigationEndpoint::Browse {
            browse_endpoint,
            command_metadata,
        } = self
        {
            browse_endpoint
                .browse_endpoint_context_supported_configs
                .as_ref()
                .map(|c| c.browse_endpoint_context_music_config.page_type)
                .or_else(|| {
                    command_metadata
                        .as_ref()
                        .map(|c| c.web_command_metadata.web_page_type)
                })
        } else {
            None
        }
    }

    /// Get the sanitized URL from a url endpoint
    pub(crate) fn url(&self) -> Option<String> {
        match self {
            NavigationEndpoint::Url { url_endpoint } => {
                Some(util::sanitize_yt_url(&url_endpoint.url))
            }
            _ => None,
        }
    }
}
