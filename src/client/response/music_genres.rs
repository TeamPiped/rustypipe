use serde::Deserialize;
use serde_with::serde_as;

use crate::{json::JsonValue, serializer::text::Text};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigationButtonRenderer {
    #[serde_as(as = "Text")]
    pub button_text: String,
    pub solid: NavigationButtonColor,
    pub click_command: JsonValue,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigationButtonColor {
    #[serde(default)]
    pub left_stripe_color: u32,
}
