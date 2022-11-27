use serde::Deserialize;

use super::{music_item::Grid, ContentsRenderer, SectionList, Tab};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicNew {
    pub contents: Contents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub single_column_browse_results_renderer: ContentsRenderer<Tab<SectionList<Grid>>>,
}
