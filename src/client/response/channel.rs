use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

use super::{ContentRenderer, ContentsRenderer, Thumbnails, VideoListItem};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub contents: Contents,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub two_column_browse_results_renderer: TabsRenderer,
}

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
    pub section_list_renderer: ContentsRenderer<ItemSectionRendererWrap>,
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
    #[serde_as(as = "VecSkipError<_>")]
    pub items: Vec<VideoListItem<ChannelVideo>>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelVideo {
    pub video_id: String,
    pub thumbnail: Thumbnails,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub title: String,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub published_time_text: String,
    #[serde_as(as = "crate::serializer::text::Text")]
    pub view_count_text: String,
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_overlays: Vec<TimeOverlayWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOverlayWrap {
    pub thumbnail_overlay_time_status_renderer: TimeOverlay,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOverlay {
    #[serde_as(as = "crate::serializer::text::Text")]
    pub text: String,
}
