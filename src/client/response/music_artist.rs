use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::serializer::text::Text;

use super::{
    music_item::{
        Button, Grid, ItemSection, MusicMicroformat, MusicThumbnailRenderer, SimpleHeader,
        SingleColumnBrowseResult,
    },
    SectionList, Tab,
};

/// Response model for YouTube Music artists
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicArtist {
    pub contents: Option<SingleColumnBrowseResult<Tab<SectionList<ItemSection>>>>,
    pub header: Option<Header>,
    #[serde(default)]
    pub microformat: MusicMicroformat,
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
    #[serde_as(as = "Option<Text>")]
    pub description: Option<String>,
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub share_endpoint: Option<ShareEndpoint>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub start_radio_button: Option<Button>,
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
pub(crate) struct ShareEndpoint {
    pub share_entity_endpoint: ShareEntityEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareEntityEndpoint {
    pub serialized_share_entity: String,
}

/// Response model for YouTube Music artist album page
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicArtistAlbums {
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub header: Option<SimpleHeader>,
    pub contents: SingleColumnBrowseResult<Tab<SectionList<Grid>>>,
}
