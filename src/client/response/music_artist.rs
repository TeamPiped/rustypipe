use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::serializer::text::Text;

use super::{
    music_item::{Grid, ItemSection, MusicThumbnailRenderer},
    ContentsRenderer, SectionList, Tab,
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

/// Response model for YouTube Music artist album page
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicArtistAlbums {
    pub header: SimpleHeader,
    pub contents: Contents<Grid>,
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
