use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{json::JsonString, DefaultOnError, VecSkipError};

use crate::serializer::text::TextLink;

use super::{MusicItem, Thumbnails};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub contents: Contents,
    pub header: Header,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    #[serde(alias = "singleColumnBrowseResultsRenderer")]
    pub two_column_browse_results_renderer: ContentsRenderer<Tab>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: ContentRenderer<SectionList>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionList {
    pub section_list_renderer: ContentsRenderer<ItemSection>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSection {
    pub item_section_renderer: Option<ContentsRenderer<PlaylistVideoList>>,
    pub music_playlist_shelf_renderer: Option<ContentsRenderer<PlaylistMusicItem>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoList {
    pub playlist_video_list_renderer: ContentsRenderer<PlaylistVideoItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoItem {
    pub playlist_video_renderer: PlaylistVideo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMusicItem {
    pub music_responsive_list_item_renderer: MusicItem,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideo {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde(rename = "shortBylineText")]
    #[serde_as(as = "crate::serializer::text::TextLink")]
    pub channel: TextLink,
    #[serde_as(as = "JsonString")]
    pub length_seconds: u32,
    pub is_playable: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    #[serde(alias = "musicDetailHeaderRenderer")]
    pub playlist_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    pub playlist_id: Option<String>,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde_as(as = "Option<crate::serializer::text::Text>")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRenderer<T> {
    pub content: T,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentsRenderer<T> {
    #[serde(alias = "tabs")]
    pub contents: Vec<T>,
}
