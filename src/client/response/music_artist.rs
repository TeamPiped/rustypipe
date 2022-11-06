use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::serializer::{ignore_any, text::Text, MapResult, VecLogError};

use super::{
    music_item::{MusicResponseItem, MusicShelf, MusicThumbnailRenderer},
    url_endpoint::NavigationEndpoint,
    ContentsRenderer, Tab,
};

/// Response model for YouTube Music artists
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicArtist {
    pub contents: Contents<ItemSection>,
    pub header: Header,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents<T> {
    pub single_column_browse_results_renderer: ContentsRenderer<Tab<SectionList<T>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionList<T> {
    pub section_list_renderer: ContentsRenderer<T>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    MusicShelfRenderer(MusicShelf),
    MusicCarouselShelfRenderer {
        #[serde(default)]
        #[serde_as(as = "DefaultOnError")]
        header: Option<MusicCarouselShelfHeader>,
        #[serde_as(as = "VecLogError<_>")]
        contents: MapResult<Vec<MusicResponseItem>>,
    },
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Header {
    #[serde(alias = "musicVisualHeaderRenderer")]
    pub music_immersive_header_renderer: MusicHeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicHeaderRenderer {
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub subscription_button: Option<SubscriptionButton>,
    #[serde(default)]
    #[serde_as(as = "Text")]
    pub description: String,
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubscriptionButton {
    pub subscribe_button_renderer: SubscriptionButtonRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubscriptionButtonRenderer {
    #[serde_as(as = "Text")]
    pub subscriber_count_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeader {
    pub music_carousel_shelf_basic_header_renderer: MusicCarouselShelfHeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeaderRenderer {
    pub more_content_button: MoreContentButton,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoreContentButton {
    pub button_renderer: ButtonRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonRenderer {
    pub navigation_endpoint: NavigationEndpoint,
}

/// Response model for YouTube Music artist album page
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicArtistAlbums {
    pub header: SimpleHeader,
    pub contents: Contents<Grid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Grid {
    pub grid_renderer: GridRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub items: MapResult<Vec<MusicResponseItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimpleHeader {
    pub music_header_renderer: SimpleHeaderRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimpleHeaderRenderer {
    #[serde_as(as = "Text")]
    pub title: String,
}
