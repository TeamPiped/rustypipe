use serde::Deserialize;
use serde_with::serde_as;

use crate::{
    error::{ExtractionError, UnavailabilityReason},
    json::{yt_two_column_list_items_from_browse, ytq, JsonNode},
    serializer::text::{AttributedText, Text, TextComponent},
    FromYtNode,
};

#[derive(Debug, FromYtNode)]
pub(crate) struct ChannelMetadataRenderer {
    pub title: String,
    /// Channel ID
    pub external_id: String,
    pub description: String,
    pub vanity_channel_url: Option<String>,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct MicroformatDataRenderer {
    #[ytq_default]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AboutChannelRenderer {
    pub metadata: ChannelMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMetadata {
    pub about_channel_view_model: ChannelMetadataView,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ChannelMetadataView {
    pub channel_id: String,
    pub canonical_channel_url: String,
    pub country: Option<String>,
    #[ytq_default]
    pub description: String,
    #[ytq_text]
    pub joined_date_text: Option<String>,
    #[ytq_text]
    pub subscriber_count_text: Option<String>,
    #[ytq_text]
    pub video_count_text: Option<String>,
    #[ytq_text]
    pub view_count_text: Option<String>,
    #[ytq_default]
    pub links: Vec<ExternalLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalLink {
    pub channel_external_link_view_model: ExternalLinkInner,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalLinkInner {
    #[serde_as(as = "AttributedText")]
    pub title: TextComponent,
    #[serde_as(as = "AttributedText")]
    pub link: TextComponent,
}

pub(crate) struct MappedChannelContent<'a> {
    pub list_node: Option<JsonNode<'a>>,
    pub has_shorts: bool,
    pub has_live: bool,
}

fn tab_renderer<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab.query(ytq!(($root || .expandableTabRenderer).tabRenderer))
}

fn tab_endpoint_url(tab: &JsonNode<'_>) -> Option<String> {
    tab_renderer(tab).and_then(|tr| {
        tr.query(ytq!(.endpoint.commandMetadata.webCommandMetadata.url))
            .and_then(|url| url.as_str())
    })
}

pub(crate) fn map_channel_content<'a>(
    id: &str,
    root: &JsonNode<'a>,
    alerts_to_err: impl FnOnce() -> ExtractionError,
) -> Result<MappedChannelContent<'a>, ExtractionError> {
    let browse = root
        .query(ytq!(.contents.twoColumnBrowseResultsRenderer))
        .ok_or_else(alerts_to_err)?;
    let tabs = browse
        .query(ytq!(.tabs || .contents))
        .map(|node| node.items())
        .unwrap_or_default();

    let mut has_shorts = false;
    let mut has_live = false;
    let mut featured_tab = false;

    for tab in &tabs {
        if let Some(url) = tab_endpoint_url(tab) {
            let selected = tab_renderer(tab)
                .and_then(|tr| tr.query(ytq!(.selected)))
                .and_then(|node| node.as_bool())
                .unwrap_or(false);
            if selected && url.ends_with("/featured") {
                if tab_renderer(tab)
                    .and_then(|tr| tr.query(ytq!(.content.(.sectionListRenderer || .richGridRenderer))))
                    .is_some()
                {
                    featured_tab = true;
                }
            } else if url.ends_with("/shorts") {
                has_shorts = true;
            } else if url.ends_with("/streams") {
                has_live = true;
            }
        } else if let Some(sl) =
            tab_renderer(tab).and_then(|tr| tr.query(ytq!(.content.sectionListRenderer.contents)))
        {
            if let Some(first) = sl.items().first() {
                if let Some(msg) = first
                    .query(ytq!(.channelAgeGateRenderer))
                    .and_then(|node| node.deserialize::<ChannelAgeGateRenderer>().ok())
                    .map(|renderer| format!("{}: {}", renderer.channel_title, renderer.main_text))
                {
                    return Err(ExtractionError::Unavailable {
                        reason: UnavailabilityReason::AgeRestricted,
                        msg,
                    });
                }
            }

            #[serde_as]
            #[derive(Debug, Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ChannelAgeGateRenderer {
                channel_title: String,
                #[serde_as(as = "Text")]
                main_text: String,
            }
        }
    }

    let list_node = if featured_tab {
        None
    } else {
        Some(
            yt_two_column_list_items_from_browse(&browse).ok_or_else(|| {
                ExtractionError::NotFound {
                    id: id.to_owned(),
                    msg: "no tabs".into(),
                }
            })?,
        )
    };

    Ok(MappedChannelContent {
        list_node,
        has_shorts,
        has_live,
    })
}
