use std::{borrow::Cow, convert::TryFrom, fmt::Debug};

use time::OffsetDateTime;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        richtext::RichText,
        ChannelId, Playlist, VideoItem,
    },
    serializer::text::{TextComponent, TextComponents},
    util::{self, dictionary, timeago, TryRemove},
};

use super::{response, ClientType, MapRespCtx, MapResponse, MapResult, QBrowse, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get a YouTube playlist
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn playlist<S: AsRef<str> + Debug>(&self, playlist_id: S) -> Result<Playlist, Error> {
        let playlist_id = playlist_id.as_ref();
        let request_body = QBrowse {
            browse_id: &format!("VL{playlist_id}"),
        };

        self.execute_request::<response::Playlist, _, _>(
            ClientType::Desktop,
            "playlist",
            playlist_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<Playlist> for response::Playlist {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<Playlist>, ExtractionError> {
        let (Some(contents), Some(header)) = (self.contents, self.header) else {
            return Err(response::alerts_to_err(ctx.id, self.alerts));
        };

        let video_items = contents
            .two_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "twoColumnBrowseResultsRenderer empty",
            )))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "sectionListRenderer empty",
            )))?
            .item_section_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "itemSectionRenderer empty",
            )))?
            .playlist_video_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
        mapper.map_response(video_items);

        let (description, thumbnails, last_update_txt) = match self.sidebar {
            Some(sidebar) => {
                let sidebar_items = sidebar.playlist_sidebar_renderer.contents;
                let mut primary =
                    sidebar_items
                        .into_iter()
                        .next()
                        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                            "no primary sidebar",
                        )))?;

                (
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .description
                        .filter(|d| !d.0.is_empty()),
                    Some(
                        primary
                            .playlist_sidebar_primary_info_renderer
                            .thumbnail_renderer
                            .playlist_video_thumbnail_renderer
                            .thumbnail,
                    ),
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .stats
                        .try_swap_remove(2),
                )
            }
            None => (None, None, None),
        };

        let (name, playlist_id, channel, n_videos_txt, description2, thumbnails2, last_update_txt2) =
            match header {
                response::playlist::Header::PlaylistHeaderRenderer(header_renderer) => {
                    let mut byline = header_renderer.byline;
                    let last_update_txt = byline
                        .try_swap_remove(1)
                        .map(|b| b.playlist_byline_renderer.text);

                    (
                        header_renderer.title,
                        header_renderer.playlist_id,
                        header_renderer
                            .owner_text
                            .and_then(|link| ChannelId::try_from(link).ok()),
                        header_renderer.num_videos_text,
                        header_renderer
                            .description_text
                            .map(|text| TextComponents(vec![TextComponent::new(text)])),
                        header_renderer
                            .playlist_header_banner
                            .map(|b| b.hero_playlist_thumbnail_renderer.thumbnail),
                        last_update_txt,
                    )
                }
                response::playlist::Header::PageHeaderRenderer(content_renderer) => {
                    let h = content_renderer.content.page_header_view_model;
                    let rows = h.metadata.content_metadata_view_model.metadata_rows;
                    let n_videos_txt = rows
                        .get(1)
                        .and_then(|r| r.metadata_parts.get(1))
                        .map(|p| p.as_str().to_owned())
                        .ok_or(ExtractionError::InvalidData("no video count".into()))?;
                    let mut channel = rows
                        .into_iter()
                        .next()
                        .and_then(|r| r.metadata_parts.into_iter().next())
                        .and_then(|p| match p {
                            response::MetadataPart::Text(_) => None,
                            response::MetadataPart::AvatarStack {
                                avatar_stack_view_model,
                            } => ChannelId::try_from(avatar_stack_view_model.text).ok(),
                        });
                    // remove "by" prefix
                    if let Some(c) = channel.as_mut() {
                        let entry = dictionary::entry(ctx.lang);
                        let n = c.name.strip_prefix(entry.chan_prefix).unwrap_or(&c.name);
                        let n = n.strip_suffix(entry.chan_suffix).unwrap_or(n);
                        c.name = n.trim().to_owned();
                    }

                    let playlist_id = h
                        .actions
                        .flexible_actions_view_model
                        .actions_rows
                        .into_iter()
                        .next()
                        .and_then(|r| r.actions.into_iter().next())
                        .and_then(|a| {
                            a.button_view_model
                                .on_tap
                                .innertube_command
                                .into_playlist_id()
                        })
                        .ok_or(ExtractionError::InvalidData("no playlist id".into()))?;
                    (
                        h.title.dynamic_text_view_model.text,
                        playlist_id,
                        channel,
                        n_videos_txt,
                        h.description.description_preview_view_model.description,
                        h.hero_image.content_preview_image_view_model.image.into(),
                        None,
                    )
                }
            };

        let n_videos = if mapper.ctoken.is_some() {
            util::parse_numeric(&n_videos_txt)
                .map_err(|_| ExtractionError::InvalidData("no video count".into()))?
        } else {
            mapper.items.len() as u64
        };

        if playlist_id != ctx.id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {}, expected {}",
                playlist_id, ctx.id
            )));
        }

        let description = description.or(description2).map(RichText::from);
        let thumbnails = thumbnails
            .or(thumbnails2)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "no thumbnail found",
            )))?;
        let last_update = last_update_txt
            .as_deref()
            .or(last_update_txt2.as_deref())
            .and_then(|txt| {
                timeago::parse_textual_date_or_warn(
                    ctx.lang,
                    ctx.utc_offset,
                    txt,
                    &mut mapper.warnings,
                )
                .map(OffsetDateTime::date)
            });

        Ok(MapResult {
            c: Playlist {
                id: playlist_id,
                name,
                videos: Paginator::new_ext(
                    Some(n_videos),
                    mapper.items,
                    mapper.ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                    ctx.authenticated,
                ),
                video_count: n_videos,
                thumbnail: thumbnails.into(),
                description,
                channel,
                last_update,
                last_update_txt,
                visitor_data: self
                    .response_context
                    .visitor_data
                    .or_else(|| ctx.visitor_data.map(str::to_owned)),
            },
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::util::tests::TESTFILES;

    use super::*;

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    #[case::live("live", "UULVvqRdlKsE5Q8mf8YXbdIJLw")]
    #[case::pageheader("20241011_pageheader", "PLT2w2oBf1TZKyvY_M6JsASs73m-wjLzH5")]
    #[case::cmdexecutor("20250316_cmdexecutor", "PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")]
    fn map_playlist_data(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "playlist" / format!("playlist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response(&MapRespCtx::test(id)).unwrap();

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
