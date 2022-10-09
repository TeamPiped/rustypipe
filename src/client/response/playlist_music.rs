use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

use crate::serializer::text::Text;

use super::MusicThumbnailRenderer;
use super::{
    ContentRenderer, ContentsRenderer, MusicContentsRenderer, MusicContinuation, MusicItem,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMusic {
    pub contents: Contents,
    pub header: Header,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub single_column_browse_results_renderer: ContentsRenderer<Tab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: ContentRenderer<SectionList>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionList {
    /// Includes a continuation token for fetching recommendations
    pub section_list_renderer: MusicContentsRenderer<ItemSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSection {
    #[serde(alias = "musicPlaylistShelfRenderer")]
    pub music_shelf_renderer: MusicShelf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelf {
    /// Playlist ID (only for playlists)
    pub playlist_id: Option<String>,
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<PlaylistMusicItem>,
    /// Continuation token for fetching more (>100) playlist items
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub continuations: Option<Vec<MusicContinuation>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMusicItem {
    pub music_responsive_list_item_renderer: MusicItem,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub music_detail_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    /// Content type + Channel/Artist + Year.
    /// Missing on artist_tracks view.
    ///
    /// `"Playlist", " • ", <"Best Music">, " • ", "2022"`
    ///
    /// `"Album", " • ", <"Helene Fischer">, " • ", "2021"`
    pub subtitle: Option<Text>,
    /// Playlist description. May contain hashtags which are
    /// displayed as search links on the YouTube website.
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub description: Option<String>,
    /// Playlist thumbnail / album cover.
    /// Missing on artist_tracks view.
    pub thumbnail: Option<MusicThumbnailRenderer>,
    /// Number of tracks + playtime.
    /// Missing on artist_tracks view.
    ///
    /// `"64 songs", " • ", "3 hours, 40 minutes"`
    pub second_subtitle: Option<Text>,
}
