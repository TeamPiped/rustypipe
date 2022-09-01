use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{json::JsonString, DefaultOnError, VecSkipError};

use crate::serializer::text::{Text, TextLink};

use super::{ContentRenderer, ContentsRenderer, Thumbnails, ThumbnailsWrap, VideoListItem};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub contents: Contents,
    pub header: Header,
    pub sidebar: Sidebar,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistCont {
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_actions: Vec<OnResponseReceivedAction>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
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
    pub item_section_renderer: ContentsRenderer<PlaylistVideoListRenderer>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoListRenderer {
    pub playlist_video_list_renderer: PlaylistVideoList,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoList {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<VideoListItem<PlaylistVideo>>,
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
    pub playlist_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    pub playlist_id: String,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<Option<crate::serializer::text::Text>>")]
    pub description_text: Option<String>,
    /// `"495", " videos"`
    pub num_videos_text: Text,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub playlist_sidebar_renderer: SidebarRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub items: Vec<SidebarRendererItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidebarRendererItem {
    #[serde(rename_all = "camelCase")]
    PlaylistSidebarPrimaryInfoRenderer {
        thumbnail_renderer: PlaylistThumbnailRenderer,
        // - `"495", " videos"`
        // - `"3,310,996 views"`
        // - `"Last updated on ", "Aug 7, 2022"`
        // stats: Vec<Text>,
    },
    #[serde(rename_all = "camelCase")]
    PlaylistSidebarSecondaryInfoRenderer { video_owner: VideoOwnerWrap },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistThumbnailRenderer {
    // the alternative field name is used by YTM playlists
    #[serde(alias = "playlistCustomThumbnailRenderer")]
    pub playlist_video_thumbnail_renderer: ThumbnailsWrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwnerWrap {
    pub video_owner_renderer: VideoOwner,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOwner {
    #[serde_as(as = "crate::serializer::text::TextLink")]
    pub title: TextLink,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnResponseReceivedAction {
    pub append_continuation_items_action: AppendAction,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendAction {
    #[serde_as(as = "VecSkipError<_>")]
    pub continuation_items: Vec<VideoListItem<PlaylistVideo>>,
    pub target_id: String,
}
