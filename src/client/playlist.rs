use std::{borrow::Cow, convert::TryFrom, fmt::Debug};

use time::OffsetDateTime;

use crate::{
    error::{Error, ExtractionError},
    json::{yt_thumbnails, ytq, JsonDoc, JsonNode, JsonValue},
    ytq_attributed_text,
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelId, Playlist,
    },
    request_body::ytbody,
    serializer::text::{TextComponent, TextComponents},
    util::{self, dictionary, timeago, TryRemove},
};

use super::{
    response::{self, url_endpoint},
    ClientType, MapEndpoint, MapRespCtx, MapResult, RustyPipeQuery,
};

#[derive(Debug)]
struct PlaylistEndpoint;

impl RustyPipeQuery {
    /// Get a YouTube playlist
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn playlist<S: AsRef<str> + Debug>(&self, playlist_id: S) -> Result<Playlist, Error> {
        let playlist_id = playlist_id.as_ref();
        let request_body = ytbody!({
            "browseId": format!("VL{playlist_id}"),
        });

        self.execute_request::<PlaylistEndpoint, _, _>(
            ClientType::Desktop,
            "playlist",
            playlist_id,
            "browse",
            &request_body,
        )
        .await
    }
}

fn json_alerts_to_err(id: &str, root: &JsonNode<'_>) -> ExtractionError {
    let alerts = root.query(ytq!(.alerts)).map(|node| {
        let (alerts, _) = node.deserialize_items_lossy::<response::Alert>();
        alerts
    });
    response::alerts_to_err(id, alerts)
}

struct PlaylistHeaderParts {
    name: String,
    playlist_id: String,
    channel: Option<ChannelId>,
    n_videos_txt: String,
    description: Option<TextComponents>,
    thumbnails: Option<Vec<crate::model::Thumbnail>>,
    last_update_txt: Option<String>,
}

fn metadata_part_text(part: &JsonNode<'_>) -> Option<String> {
    part.text_at(ytq!(($root || .avatarStack.avatarStackViewModel).text))
}

fn metadata_part_channel(part: &JsonNode<'_>) -> Option<ChannelId> {
    let avatar_text = part.query(ytq!(.avatarStack.avatarStackViewModel.text))?;
    let name = avatar_text.text()?;
    let id = avatar_text
        .query(ytq!(.commandRuns))
        .and_then(|runs| {
            runs.items().into_iter().find_map(|run| {
                run.query(ytq!(.onTap.innertubeCommand))
                    .and_then(|node| node.deserialize::<JsonValue>().ok())
                    .and_then(|endpoint| url_endpoint::browse_endpoint(&endpoint))
                    .map(|endpoint| endpoint.browse_endpoint.browse_id)
            })
        })
        .or_else(|| {
            avatar_text
                .query(ytq!(.rendererContext.commandContext.onTap.innertubeCommand))
                .and_then(|node| node.deserialize::<JsonValue>().ok())
                .and_then(|endpoint| url_endpoint::browse_endpoint(&endpoint))
                .map(|endpoint| endpoint.browse_endpoint.browse_id)
        })?;
    Some(ChannelId { id, name })
}

fn map_playlist_header(
    header: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<PlaylistHeaderParts, ExtractionError> {
    let page_header = header.query(ytq!(.pageHeaderRenderer.content.pageHeaderViewModel));
    let metadata_rows = page_header
        .as_ref()
        .and_then(|header| header.query(ytq!(.metadata.contentMetadataViewModel.metadataRows)))
        .map(|rows| rows.items())
        .unwrap_or_default();

    let legacy_header = header.query(ytq!(.playlistHeaderRenderer));
    let legacy_header = legacy_header.as_ref();

    let n_videos_txt = legacy_header
        .and_then(|header| header.text_at(ytq!(.numVideosText)))
        .or_else(|| {
            metadata_rows
                .get(1)
                .and_then(|row| row.query(ytq!(.metadataParts)))
                .and_then(|parts| parts.items().get(1).cloned())
                .and_then(|part| metadata_part_text(&part))
        })
        .ok_or(ExtractionError::InvalidData("no video count".into()))?;

    let mut channel = legacy_header
        .and_then(|header| header.query(ytq!(.ownerText)))
        .and_then(|node| node.deserialize::<TextComponent>().ok())
        .and_then(|link| ChannelId::try_from(link).ok())
        .or_else(|| {
            metadata_rows
                .first()
                .and_then(|row| row.query(ytq!(.metadataParts)))
                .and_then(|parts| parts.items().into_iter().next())
                .and_then(|part| metadata_part_channel(&part))
        });

    // remove "by" prefix
    if let Some(c) = channel.as_mut() {
        let entry = dictionary::entry(ctx.lang);
        let n = c.name.strip_prefix(entry.chan_prefix).unwrap_or(&c.name);
        let n = n.strip_suffix(entry.chan_suffix).unwrap_or(n);
        c.name = n.trim().to_owned();
    }

    let playlist_id = header
        .query(ytq!(.playlistHeaderRenderer.playlistId))
        .and_then(|node| node.as_str())
        .or_else(|| {
            page_header
                .as_ref()
                .and_then(|header| {
                    header.query(ytq!(
                        .actions.flexibleActionsViewModel.actionsRows[0].actions[0]
                            .buttonViewModel.onTap.innertubeCommand
                    ))
                })
                .and_then(|node| node.deserialize::<JsonValue>().ok())
                .and_then(|endpoint| url_endpoint::playlist_id(&endpoint))
        })
        .ok_or(ExtractionError::InvalidData("no playlist id".into()))?;

    let mut byline = legacy_header
        .and_then(|header| header.query(ytq!(.byline)))
        .map(|node| {
            node.items()
                .into_iter()
                .filter_map(|item| item.query(ytq!(.playlistBylineRenderer.text))?.text())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(PlaylistHeaderParts {
        name: header
            .text_at(ytq!(
                .playlistHeaderRenderer.title
                    || .pageHeaderRenderer.content.pageHeaderViewModel.title
                        .dynamicTextViewModel.text
            ))
            .ok_or(ExtractionError::InvalidData("no playlist title".into()))?,
        playlist_id,
        channel,
        n_videos_txt,
        description: header
            .text_at(ytq!(.playlistHeaderRenderer.descriptionText))
            .map(|text| TextComponents(vec![TextComponent::new(text)]))
            .or_else(|| {
                ytq_attributed_text!(
                    header,
                    .pageHeaderRenderer.content.pageHeaderViewModel.description
                        .descriptionPreviewViewModel.description
                )
            }),
        thumbnails: header
            .query(ytq!(
                .playlistHeaderRenderer.playlistHeaderBanner.heroPlaylistThumbnailRenderer.thumbnail
                    || .pageHeaderRenderer.content.pageHeaderViewModel.heroImage
                        .contentPreviewImageViewModel.image
            ))
            .map(|node| yt_thumbnails(&node)),
        last_update_txt: byline.try_swap_remove(1),
    })
}

impl MapEndpoint<Playlist> for PlaylistEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Playlist>, ExtractionError> {
        json.with_root(|root| {
            let contents = root.query(ytq!(.contents.twoColumnBrowseResultsRenderer));
            let header = root.query(ytq!(.header));
            if contents.is_none() || header.is_none() {
                return Err(json_alerts_to_err(ctx.id, &root));
            }

            let video_items = response::playlist::video_list_node(&root)?;
            let (mut mapped, ctoken, _) =
                response::video_item::map_video_items(&video_items, ctx.lang);

            let sidebar = response::playlist::sidebar_info(&root)?;
            let description = sidebar.description;
            let thumbnails = sidebar.thumbnails;
            let last_update_txt = sidebar.last_update_txt;

            let header = header.ok_or(ExtractionError::InvalidData(Cow::Borrowed("no header")))?;

            let header_parts = map_playlist_header(&header, ctx)?;

            let n_videos = if ctoken.is_some() {
                util::parse_numeric(&header_parts.n_videos_txt)
                    .map_err(|_| ExtractionError::InvalidData("no video count".into()))?
            } else {
                mapped.c.len() as u64
            };

            if header_parts.playlist_id != ctx.id {
                return Err(crate::client::check_id_matches(
                    &header_parts.playlist_id,
                    ctx.id,
                    "playlist",
                ));
            }

            let description = description.or(header_parts.description);
            let thumbnails =
                thumbnails
                    .or(header_parts.thumbnails)
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                        "no thumbnail found",
                    )))?;
            let last_update = last_update_txt
                .as_deref()
                .or(header_parts.last_update_txt.as_deref())
                .and_then(|txt| {
                    timeago::parse_textual_date_or_warn(
                        ctx.lang,
                        ctx.utc_offset,
                        txt,
                        &mut mapped.warnings,
                    )
                    .map(OffsetDateTime::date)
                });

            Ok(MapResult {
                c: Playlist {
                    id: header_parts.playlist_id,
                    name: header_parts.name,
                    videos: Paginator::new_ext(
                        Some(n_videos),
                        mapped.c,
                        ctoken,
                        ctx.visitor_data.map(str::to_owned),
                        ContinuationEndpoint::Browse,
                        ctx.authenticated,
                    ),
                    video_count: n_videos,
                    thumbnail: thumbnails,
                    description: description.map(Into::into),
                    channel: header_parts.channel,
                    last_update,
                    last_update_txt,
                    visitor_data: ctx.visitor_data(&root),
                },
                warnings: mapped.warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use crate::util::tests::TESTFILES;

    use super::{MapEndpoint, *};

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    #[case::live("live", "UULVvqRdlKsE5Q8mf8YXbdIJLw")]
    #[case::pageheader("20241011_pageheader", "PLT2w2oBf1TZKyvY_M6JsASs73m-wjLzH5")]
    #[case::cmdexecutor("20250316_cmdexecutor", "PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")]
    fn map_playlist_data(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "playlist" / format!("playlist_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Playlist> =
            PlaylistEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_playlist_data_{name}"), map_res.c, {
            ".last_update" => "[date]",
            ".videos.items[].publish_date" => "[date]",
        });
    }
}
