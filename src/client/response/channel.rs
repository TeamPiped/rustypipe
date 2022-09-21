use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

use super::ChannelBadge;
use super::Thumbnails;
use super::{ContentRenderer, ContentsRenderer, VideoListItem};
use crate::serializer::{text::Text, MapResult, VecLogError};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub header: Header,
    pub contents: Contents,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_browse_results_renderer: TabsRenderer,
}

/// YouTube channel tab view. Contains multiple tabs
/// (Home, Videos, Playlists, About...). We can ignore unknown tabs.
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<TabRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererWrap {
    pub tab_renderer: ContentRenderer<SectionListRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRendererWrap {
    pub section_list_renderer: SectionListRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    pub contents: Vec<ItemSectionRendererWrap>,
    pub target_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRendererWrap {
    pub item_section_renderer: ContentsRenderer<GridRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRendererWrap {
    pub grid_renderer: GridRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub items: MapResult<Vec<VideoListItem>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub c4_tabbed_header_renderer: HeaderRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    pub channel_id: String,
    /// Channel name
    pub title: String,
    /// Approximate subscriber count (e.g. `880K subscribers`), depends on language
    #[serde_as(as = "Text")]
    pub subscriber_count_text: String,
    pub avatar: Thumbnails,
    #[serde_as(as = "VecSkipError<_>")]
    pub badges: Vec<ChannelBadge>,
    pub banner: Thumbnails,
    pub mobile_banner: Thumbnails,
    /// Fullscreen (16:9) channel banner
    pub tv_banner: Thumbnails,
}
