use serde::Deserialize;

use super::{video_item::YouTubeListRendererWrap, Tab, TwoColumnBrowseResults};

#[derive(Debug, Deserialize)]
pub(crate) struct History {
    pub contents: TwoColumnBrowseResults<Tab<YouTubeListRendererWrap>>,
}
