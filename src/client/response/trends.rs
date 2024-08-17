use serde::Deserialize;

use super::{video_item::YouTubeListRendererWrap, Tab, TwoColumnBrowseResults};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Trending {
    pub contents: Contents,
}

type Contents = TwoColumnBrowseResults<Tab<YouTubeListRendererWrap>>;
