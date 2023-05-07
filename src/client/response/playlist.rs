use serde::Deserialize;
use serde_with::{
    json::JsonString, rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError,
};

use crate::serializer::{
    text::{Text, TextComponent},
    MapResult,
};
use crate::util::MappingError;

use super::{
    Alert, ContentsRenderer, ContinuationEndpoint, ResponseContext, SectionList, Tab, Thumbnails,
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistCont {
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_actions: Vec<OnResponseReceivedAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemSection {
    pub item_section_renderer: ContentsRenderer<PlaylistVideoListRenderer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistVideoListRenderer {
    pub playlist_video_list_renderer: PlaylistVideoList,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistVideoList {
    pub contents: MapResult<Vec<PlaylistItem>>,
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlaylistItem {
    /// Video in playlist
    PlaylistVideoRenderer(PlaylistVideoRenderer),
    /// Continauation items are located at the end of a list
    /// and contain the continuation token for progressive loading
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
    /// No video list item (e.g. ad) or unimplemented item
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
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
    #[serde_as(as = "JsonString")]
    pub length_seconds: u32,
}

impl TryFrom<PlaylistVideoRenderer> for crate::model::PlaylistVideo {
    type Error = MappingError;

    fn try_from(video: PlaylistVideoRenderer) -> Result<Self, Self::Error> {
        Ok(Self {
            id: video.video_id,
            name: video.title,
            length: video.length_seconds,
            thumbnail: video.thumbnail.into(),
            channel: crate::model::ChannelId::try_from(video.channel)?,
        })
    }
}

// Continuation

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnResponseReceivedAction {
    pub append_continuation_items_action: AppendAction,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendAction {
    pub continuation_items: MapResult<Vec<PlaylistItem>>,
}
