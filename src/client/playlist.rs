use std::{borrow::Cow, convert::TryFrom};

use time::OffsetDateTime;

use crate::{
    error::{Error, ExtractionError},
    model::{paginator::Paginator, ChannelId, Playlist, PlaylistVideo},
    timeago,
    util::{self, TryRemove},
};

use super::{response, ClientType, MapResponse, MapResult, QBrowse, QContinuation, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get a YouTube playlist
    pub async fn playlist<S: AsRef<str>>(&self, playlist_id: S) -> Result<Playlist, Error> {
        let playlist_id = playlist_id.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QBrowse {
            context,
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

    /// Get more playlist items using the given continuation token
    pub async fn playlist_continuation<S: AsRef<str>>(
        &self,
        ctoken: S,
    ) -> Result<Paginator<PlaylistVideo>, Error> {
        let ctoken = ctoken.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::PlaylistCont, _, _>(
            ClientType::Desktop,
            "playlist_continuation",
            ctoken,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<Playlist> for response::Playlist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Playlist>, ExtractionError> {
        let (contents, header) = match (self.contents, self.header) {
            (Some(contents), Some(header)) => (contents, header),
            _ => return Err(response::alerts_to_err(self.alerts)),
        };

        let mut tcbr_contents = contents.two_column_browse_results_renderer.contents;

        let video_items = tcbr_contents
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "twoColumnBrowseResultsRenderer empty",
            )))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "sectionListRenderer empty",
            )))?
            .item_section_renderer
            .contents
            .try_swap_remove(0)
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "itemSectionRenderer empty",
            )))?
            .playlist_video_list_renderer
            .contents;

        let (videos, ctoken) = map_playlist_items(video_items.c);

        let (thumbnails, last_update_txt) = match self.sidebar {
            Some(sidebar) => {
                let mut sidebar_items = sidebar.playlist_sidebar_renderer.items;
                let mut primary =
                    sidebar_items
                        .try_swap_remove(0)
                        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                            "no primary sidebar",
                        )))?;

                (
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .thumbnail_renderer
                        .playlist_video_thumbnail_renderer
                        .thumbnail,
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .stats
                        .try_swap_remove(2),
                )
            }
            None => {
                let header_banner = header
                    .playlist_header_renderer
                    .playlist_header_banner
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                        "no thumbnail found",
                    )))?;

                let mut byline = header.playlist_header_renderer.byline;
                let last_update_txt = byline
                    .try_swap_remove(1)
                    .map(|b| b.playlist_byline_renderer.text);

                (
                    header_banner.hero_playlist_thumbnail_renderer.thumbnail,
                    last_update_txt,
                )
            }
        };

        let n_videos = match ctoken {
            Some(_) => util::parse_numeric(&header.playlist_header_renderer.num_videos_text)
                .map_err(|_| ExtractionError::InvalidData(Cow::Borrowed("no video count")))?,
            None => videos.len() as u64,
        };

        let playlist_id = header.playlist_header_renderer.playlist_id;
        if playlist_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {playlist_id}, expected {id}"
            )));
        }

        let name = header.playlist_header_renderer.title;
        let description = header.playlist_header_renderer.description_text;
        let channel = header
            .playlist_header_renderer
            .owner_text
            .and_then(|link| ChannelId::try_from(link).ok());

        let mut warnings = video_items.warnings;
        let last_update = last_update_txt.as_ref().and_then(|txt| {
            timeago::parse_textual_date_or_warn(lang, txt, &mut warnings).map(OffsetDateTime::date)
        });

        Ok(MapResult {
            c: Playlist {
                id: playlist_id,
                name,
                videos: Paginator::new(Some(n_videos), videos, ctoken),
                video_count: n_videos,
                thumbnail: thumbnails.into(),
                description,
                channel,
                last_update,
                last_update_txt,
                visitor_data: self.response_context.visitor_data,
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<PlaylistVideo>> for response::PlaylistCont {
    fn map_response(
        self,
        _id: &str,
        _lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Paginator<PlaylistVideo>>, ExtractionError> {
        let action = self.on_response_received_actions.into_iter().next();

        let ((items, ctoken), warnings) = action
            .map(|action| {
                (
                    map_playlist_items(
                        action.append_continuation_items_action.continuation_items.c,
                    ),
                    action
                        .append_continuation_items_action
                        .continuation_items
                        .warnings,
                )
            })
            .unwrap_or_default();

        Ok(MapResult {
            c: Paginator::new(None, items, ctoken),
            warnings,
        })
    }
}

fn map_playlist_items(
    items: Vec<response::playlist::PlaylistItem>,
) -> (Vec<PlaylistVideo>, Option<String>) {
    let mut ctoken: Option<String> = None;
    let videos = items
        .into_iter()
        .filter_map(|it| match it {
            response::playlist::PlaylistItem::PlaylistVideoRenderer(video) => {
                PlaylistVideo::try_from(video).ok()
            }
            response::playlist::PlaylistItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            response::playlist::PlaylistItem::None => None,
        })
        .collect::<Vec<_>>();
    (videos, ctoken)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::param::Language;

    use super::*;

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_playlist_data(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "playlist" / format!("playlist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_playlist_data_{name}"), map_res.c, {
            ".last_update" => "[date]"
        });
    }

    #[test]
    fn map_playlist_cont() {
        let json_path = path!("testfiles" / "playlist" / "playlist_cont.json");
        let json_file = File::open(json_path).unwrap();

        let playlist: response::PlaylistCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_playlist_cont", map_res.c);
    }
}
