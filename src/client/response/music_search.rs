use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, VecSkipError};

use crate::serializer::text::Text;

use super::{
    music_item::{ListMusicItem, MusicCardShelf, MusicShelf},
    ContentsRenderer, SectionList, Tab,
};

/// Response model for YouTube Music search
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicSearch {
    pub contents: Contents,
}

/// Response model for YouTube Music search suggestion
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicSearchSuggestion {
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<SearchSuggestionsSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub tabbed_search_results_renderer: ContentsRenderer<Tab<SectionList<ItemSection>>>,
}

#[allow(clippy::enum_variant_names)]
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    MusicShelfRenderer(MusicShelf),
    MusicCardShelfRenderer(MusicCardShelf),
    ItemSectionRenderer {
        #[serde_as(as = "VecSkipError<_>")]
        contents: Vec<ShowingResultsFor>,
    },
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchSuggestionsSection {
    pub search_suggestions_section_renderer: ContentsRenderer<SearchSuggestionItem>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SearchSuggestionItem {
    SearchSuggestionRenderer {
        #[serde_as(as = "Text")]
        suggestion: String,
    },
    MusicResponsiveListItemRenderer(Box<ListMusicItem>),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}
