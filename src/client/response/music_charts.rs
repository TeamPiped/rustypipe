use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, VecSkipError};

use crate::param::Country;

use super::{music_item::MusicCarouselShelf, ContentsRenderer, SectionList, Tab};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCharts {
    pub contents: Contents,
    pub framework_updates: Option<FrameworkUpdates>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub single_column_browse_results_renderer: ContentsRenderer<Tab<SectionList<ItemSection>>>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    MusicCarouselShelfRenderer(Box<MusicCarouselShelf>),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrameworkUpdates {
    pub entity_batch_update: EntityBatchUpdate,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityBatchUpdate {
    #[serde_as(as = "VecSkipError<_>")]
    pub mutations: Vec<CountryOptionMutation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountryOptionMutation {
    pub payload: CountryOptionPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountryOptionPayload {
    pub music_form_boolean_choice: CountryOption,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountryOption {
    pub opaque_token: Country,
}
