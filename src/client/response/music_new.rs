use serde::Deserialize;

use super::{
    music_item::{Grid, SingleColumnBrowseResult},
    SectionList, Tab,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicNew {
    pub contents: SingleColumnBrowseResult<Tab<SectionList<Grid>>>,
}
