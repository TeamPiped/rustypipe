use std::borrow::Cow;

use crate::{
    error::ExtractionError,
    json::{yt_first_tab, yt_thumbnails, ytq, JsonNode},
    serializer::text::TextComponents,
    util::TryRemove,
};

pub(crate) struct SidebarInfo {
    pub description: Option<TextComponents>,
    pub thumbnails: Option<Vec<crate::model::Thumbnail>>,
    pub last_update_txt: Option<String>,
}

pub(crate) fn video_list_node<'a>(root: &JsonNode<'a>) -> Result<JsonNode<'a>, ExtractionError> {
    let browse = root.require(
        ytq!(.contents.twoColumnBrowseResultsRenderer),
        "two column browse results",
    )?;
    let tab = yt_first_tab(&browse).ok_or({
        ExtractionError::InvalidData(Cow::Borrowed("twoColumnBrowseResultsRenderer empty"))
    })?;
    let sections = tab.require(
        ytq!(.tabRenderer.content.sectionListRenderer.contents),
        "section list renderer",
    )?;
    let section = sections
        .items()
        .into_iter()
        .next()
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "sectionListRenderer empty",
        )))?;
    let item_section =
        section.require(ytq!(.itemSectionRenderer.contents), "item section renderer")?;
    let item = item_section
        .items()
        .into_iter()
        .next()
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "itemSectionRenderer empty",
        )))?;
    item.query(ytq!(.(.playlistVideoListRenderer || .richGridRenderer).contents))
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "playlist video list empty",
        )))
}

pub(crate) fn sidebar_info(root: &JsonNode<'_>) -> Result<SidebarInfo, ExtractionError> {
    let Some(sidebar) = root.query(ytq!(.sidebar)) else {
        return Ok(SidebarInfo {
            description: None,
            thumbnails: None,
            last_update_txt: None,
        });
    };

    let sidebar_items = sidebar
        .require(
            ytq!(.playlistSidebarRenderer.items),
            "playlist sidebar items",
        )?
        .items();
    let primary = sidebar_items
        .into_iter()
        .next()
        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
            "no primary sidebar",
        )))?;
    let info = primary.require(
        ytq!(.playlistSidebarPrimaryInfoRenderer),
        "playlist sidebar primary info",
    )?;

    Ok(SidebarInfo {
        description: info
            .query(ytq!(.description))
            .and_then(|node| node.deserialize::<TextComponents>().ok())
            .filter(|d| !d.0.is_empty()),
        thumbnails: info
            .query(ytq!(
                .thumbnailRenderer.(.playlistVideoThumbnailRenderer || .playlistCustomThumbnailRenderer).thumbnail
            ))
            .map(|node| yt_thumbnails(&node)),
        last_update_txt: info
            .query(ytq!(.stats))
            .map(|stats| {
                stats
                    .items()
                    .into_iter()
                    .filter_map(|item| item.text())
                    .collect::<Vec<_>>()
            })
            .and_then(|mut stats| stats.try_swap_remove(2)),
    })
}
