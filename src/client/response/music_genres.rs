use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as};

use crate::serializer::text::Text;

use super::url_endpoint::BrowseEndpointWrap;

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
