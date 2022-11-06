use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::serializer::text::{Text, TextComponents};

use super::music_item::{ItemSection, MusicContentsRenderer, MusicThumbnailRenderer};
use super::{ContentsRenderer, Tab};

/// Response model for YouTube Music playlists and albums
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicPlaylist {
    pub contents: Contents,
    pub header: Header,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub single_column_browse_results_renderer: ContentsRenderer<Tab<SectionList>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionList {
    /// Includes a continuation token for fetching recommendations
    pub section_list_renderer: MusicContentsRenderer<ItemSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Header {
    pub music_detail_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeaderRenderer {
    #[serde_as(as = "Text")]
    pub title: String,
    /// Content type + Channel/Artist + Year.
    /// Missing on artist_tracks view.
    ///
    /// `"Playlist", " • ", <"Best Music">, " • ", "2022"`
    ///
    /// `"Album", " • ", <"Helene Fischer">, " • ", "2021"`
    #[serde(default)]
    pub subtitle: TextComponents,
    /// Playlist/album description. May contain hashtags which are
    /// displayed as search links on the YouTube website.
    #[serde_as(as = "Option<Text>")]
    pub description: Option<String>,
    /// Playlist thumbnail / album cover.
    /// Missing on artist_tracks view.
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
    /// Number of tracks + playtime.
    /// Missing on artist_tracks view.
    ///
    /// `"64 songs", " • ", "3 hours, 40 minutes"`
    #[serde(default)]
    #[serde_as(as = "Text")]
    pub second_subtitle: Vec<String>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub menu: Option<HeaderMenu>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeaderMenu {
    pub menu_renderer: HeaderMenuRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeaderMenuRenderer {
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub top_level_buttons: Vec<TopLevelButton>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopLevelButton {
    pub button_renderer: ButtonRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonRenderer {
    pub navigation_endpoint: PlaylistEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistEndpoint {
    pub watch_playlist_endpoint: PlaylistWatchEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistWatchEndpoint {
    pub playlist_id: String,
}
