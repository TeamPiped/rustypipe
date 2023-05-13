use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::serializer::text::{Text, TextComponent};

use super::{
    video_item::YouTubeListRenderer, Alert, ContentsRenderer, ResponseContext, SectionList, Tab,
    ThumbnailsWrap, TwoColumnBrowseResults,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Playlist {
    pub contents: Option<TwoColumnBrowseResults<Tab<SectionList<ItemSection>>>>,
    pub header: Option<Header>,
    pub sidebar: Option<Sidebar>,
    #[serde_as(as = "Option<DefaultOnError>")]
    pub alerts: Option<Vec<Alert>>,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemSection {
    pub item_section_renderer: ContentsRenderer<PlaylistVideoListRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistVideoListRenderer {
    pub playlist_video_list_renderer: YouTubeListRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Header {
    pub playlist_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeaderRenderer {
    pub playlist_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Option<Text>>")]
    pub description_text: Option<String>,
    #[serde_as(as = "Text")]
    pub num_videos_text: String,
    pub owner_text: Option<TextComponent>,

    // Alternative layout
    pub playlist_header_banner: Option<PlaylistHeaderBanner>,
    #[serde(default)]
    pub byline: Vec<Byline>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistHeaderBanner {
    pub hero_playlist_thumbnail_renderer: ThumbnailsWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Byline {
    pub playlist_byline_renderer: BylineRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BylineRenderer {
    #[serde_as(as = "Text")]
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sidebar {
    pub playlist_sidebar_renderer: ContentsRenderer<SidebarItemPrimary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarItemPrimary {
    pub playlist_sidebar_primary_info_renderer: SidebarPrimaryInfoRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SidebarPrimaryInfoRenderer {
    pub thumbnail_renderer: PlaylistThumbnailRenderer,
    /// - `"495", " videos"`
    /// - `"3,310,996 views"`
    /// - `"Last updated on ", "Aug 7, 2022"`
    #[serde_as(as = "Vec<Text>")]
    pub stats: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistThumbnailRenderer {
    // the alternative field name is used by YTM playlists
    #[serde(alias = "playlistCustomThumbnailRenderer")]
    pub playlist_video_thumbnail_renderer: ThumbnailsWrap,
}
