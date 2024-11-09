use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::serializer::text::Text;

use super::ContentsRenderer;
use super::TextBox;
use super::{
    music_item::{ItemSection, PlaylistPanelRenderer},
    ContentRenderer,
};

/// Response model for YouTube Music track details
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicDetails {
    pub contents: Contents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub single_column_music_watch_next_results_renderer: WatchNextResultsRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchNextResultsRenderer {
    pub tabbed_renderer: TabbedRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabbedRenderer {
    pub watch_next_tabbed_results_renderer: TabbedRendererInner,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabbedRendererInner {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tab {
    pub tab_renderer: TabRenderer,
}

/// Watch next tab
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabRenderer {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub content: Option<TabContent>,
    pub endpoint: Option<TabEndpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabEndpoint {
    pub browse_endpoint: TabBrowseEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabBrowseEndpoint {
    pub browse_id: String,
    pub browse_endpoint_context_supported_configs: TabBrowseEndpointSupportedConfigs,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabBrowseEndpointSupportedConfigs {
    pub browse_endpoint_context_music_config: TabBrowseEndpointMusicConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabBrowseEndpointMusicConfig {
    pub page_type: TabType,
}

#[derive(Debug, Deserialize)]
pub(crate) enum TabType {
    #[serde(rename = "MUSIC_PAGE_TYPE_TRACK_LYRICS")]
    Lyrics,
    #[serde(rename = "MUSIC_PAGE_TYPE_TRACK_RELATED")]
    Related,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContent {
    pub music_queue_renderer: ContentRenderer<PlaylistPanel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistPanel {
    pub playlist_panel_renderer: PlaylistPanelRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicLyrics {
    pub contents: ListOrMessage<LyricsSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ListOrMessage<T> {
    SectionListRenderer(ContentsRenderer<T>),
    MessageRenderer(TextBox),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LyricsSection {
    pub music_description_shelf_renderer: Option<LyricsRenderer>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LyricsRenderer {
    #[serde_as(as = "Text")]
    pub description: String,
    #[serde_as(as = "Text")]
    pub footer: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicRelated {
    pub contents: ListOrMessage<ItemSection>,
}

impl<T> ListOrMessage<T> {
    pub fn into_res(self) -> Result<Vec<T>, String> {
        match self {
            ListOrMessage::SectionListRenderer(c) => Ok(c.contents),
            ListOrMessage::MessageRenderer(msg) => Err(msg.text),
        }
    }
}
