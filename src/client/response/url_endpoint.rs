use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::model::UrlTarget;

/// navigation/resolve_url response model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedUrl {
    pub endpoint: NavigationEndpoint,
}

#[serde_as]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigationEndpoint {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub watch_endpoint: Option<WatchEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub browse_endpoint: Option<BrowseEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub url_endpoint: Option<UrlEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub command_metadata: Option<CommandMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchEndpoint {
    pub video_id: String,
    #[serde(default)]
    pub start_time_seconds: u32,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowseEndpointMusicConfig {
    pub page_type: PageType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandMetadata {
    pub web_command_metadata: WebCommandMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCommandMetadata {
    pub web_page_type: PageType,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
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
}

impl PageType {
    pub(crate) fn to_url_target(self, id: String) -> UrlTarget {
        match self {
            PageType::Artist => UrlTarget::Channel { id },
            PageType::Album => UrlTarget::Album { id },
            PageType::Channel => UrlTarget::Channel { id },
            PageType::Playlist => UrlTarget::Playlist { id },
        }
    }
}

impl NavigationEndpoint {
    pub(crate) fn music_page(self) -> Option<(PageType, String)> {
        match self.browse_endpoint {
            Some(browse) => match browse.browse_endpoint_context_supported_configs {
                Some(config) => Some((
                    config.browse_endpoint_context_music_config.page_type,
                    browse.browse_id,
                )),
                None => None,
            },
            None => None,
        }
    }
}
