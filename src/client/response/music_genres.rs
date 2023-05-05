use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as};

use crate::serializer::text::Text;

use super::{
    music_item::{ItemSection, SimpleHeader, SingleColumnBrowseResult},
    url_endpoint::BrowseEndpointWrap,
    ContentsRendererLogged, SectionList, Tab,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicGenres {
    pub contents: SingleColumnBrowseResult<Tab<SectionList<Grid>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Grid {
    pub grid_renderer: ContentsRendererLogged<NavigationButton>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NavigationButton {
    #[serde(rename_all = "camelCase")]
    MusicNavigationButtonRenderer(NavigationButtonRenderer),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigationButtonRenderer {
    #[serde_as(as = "Text")]
    pub button_text: String,
    pub solid: NavigationButtonColor,
    pub click_command: BrowseEndpointWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigationButtonColor {
    pub left_stripe_color: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicGenre {
    pub contents: SingleColumnBrowseResult<Tab<SectionList<ItemSection>>>,
    pub header: SimpleHeader,
}
