use serde::Deserialize;
use serde_with::serde_as;

use crate::{json::JsonValue, serializer::text::AttributedText, FromYtNode};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AvatarStackViewModel {
    // #[serde(default)]
    // pub avatars: Vec<AvatarViewModel>,
    #[serde_as(as = "AttributedText")]
    pub text: String,
    pub renderer_context: AvatarStackRendererContext,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AvatarStackRendererContext {
    pub command_context: Option<JsonValue>,
}
