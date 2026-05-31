use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::serializer::text::{AttributedText, Text, TextComponent, TextComponents};

use super::{
    url_endpoint::OnTapWrap, ContentRenderer, ImageView,
    PageHeaderRendererContent, PhMetadataView, TextBox, ThumbnailsWrap,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Header {
    PlaylistHeaderRenderer(HeaderRenderer),
    PageHeaderRenderer(ContentRenderer<PageHeaderRendererContent<PageHeaderRendererInner>>),
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
    pub playlist_byline_renderer: TextBox,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageHeaderRendererInner {
    pub title: PhTitleView,
    pub metadata: PhMetadataView,
    pub actions: PhActions,
    pub description: PhDescription,
    pub hero_image: PhHeroImage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhDescription {
    pub description_preview_view_model: PhDescription2,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhDescription2 {
    #[serde_as(as = "Option<AttributedText>")]
    pub description: Option<TextComponents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhHeroImage {
    pub content_preview_image_view_model: ImageView,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhTitleView {
    pub dynamic_text_view_model: PhTitleInner,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhTitleInner {
    #[serde_as(as = "AttributedText")]
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhActions {
    pub flexible_actions_view_model: PhActions2,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhActions2 {
    pub actions_rows: Vec<ActionsRow>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionsRow {
    #[serde_as(as = "VecSkipError<_>")]
    pub actions: Vec<ButtonAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonAction {
    pub button_view_model: OnTapWrap,
}
