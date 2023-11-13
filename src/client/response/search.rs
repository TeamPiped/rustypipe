use serde::{
    de::{IgnoredAny, Visitor},
    Deserialize,
};
use serde_with::{serde_as, DisplayFromStr};

use super::{video_item::YouTubeListRendererWrap, ResponseContext};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Search {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub estimated_results: Option<u64>,
    pub contents: Contents,
    pub response_context: ResponseContext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub two_column_search_results_renderer: TwoColumnSearchResultsRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TwoColumnSearchResultsRenderer {
    pub primary_contents: YouTubeListRendererWrap,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchSuggestion(IgnoredAny, pub Vec<SearchSuggestionItem>, IgnoredAny);

#[derive(Debug)]
pub(crate) struct SearchSuggestionItem(pub String);

impl<'de> Deserialize<'de> for SearchSuggestionItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ItemVisitor;

        impl<'de> Visitor<'de> for ItemVisitor {
            type Value = SearchSuggestionItem;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("search suggestion item")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                match seq.next_element::<String>()? {
                    Some(s) => {
                        // Ignore the rest of the list
                        while seq.next_element::<IgnoredAny>()?.is_some() {}
                        Ok(SearchSuggestionItem(s))
                    }
                    None => Err(serde::de::Error::invalid_length(0, &"1")),
                }
            }
        }

        deserializer.deserialize_seq(ItemVisitor)
    }
}
