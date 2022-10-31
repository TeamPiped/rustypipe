use serde::Deserialize;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::{ignore_any, text::Text};

use super::{music_item::MusicShelf, ContentsRenderer, Tab};

/// Response model for YouTube Music search
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicSearch {
    pub contents: Contents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub tabbed_search_results_renderer: ContentsRenderer<Tab<SectionList>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionList {
    pub section_list_renderer: ContentsRenderer<ItemSection>,
}

#[allow(clippy::enum_variant_names)]
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    MusicShelfRenderer(MusicShelf),
    ItemSectionRenderer {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ShowingResultsFor>,
    },
    #[serde(other, deserialize_with = "ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShowingResultsFor {
    pub showing_results_for_renderer: ShowingResultsForRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShowingResultsForRenderer {
    #[serde_as(as = "Text")]
    pub corrected_query: String,
}
