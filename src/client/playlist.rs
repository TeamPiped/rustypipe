use std::convert::TryFrom;

use serde::Serialize;

use crate::{
    deobfuscate::Deobfuscator,
    error::{Error, ExtractionError},
    model::{ChannelId, Paginator, Playlist, PlaylistVideo},
    param::Language,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response, ClientType, MapResponse, MapResult, QContinuation, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylist {
    context: YTContext,
    browse_id: String,
}

impl RustyPipeQuery {
    pub async fn playlist(self, playlist_id: &str) -> Result<Playlist, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QPlaylist {
            context,
            browse_id: "VL".to_owned() + playlist_id,
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

    pub async fn playlist_continuation(
        self,
        ctoken: &str,
    ) -> Result<Paginator<PlaylistVideo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
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
        lang: Language,
        _deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<Playlist>, ExtractionError> {
        // TODO: think about a deserializer that deserializes only first list item
        let mut tcbr_contents = self.contents.two_column_browse_results_renderer.contents;
        let video_items = some_or_bail!(
            some_or_bail!(
                some_or_bail!(
                    tcbr_contents.try_swap_remove(0),
                    Err(ExtractionError::InvalidData(
                        "twoColumnBrowseResultsRenderer empty".into()
                    ))
                )
                .tab_renderer
                .content
                .section_list_renderer
                .contents
                .try_swap_remove(0),
                Err(ExtractionError::InvalidData(
                    "sectionListRenderer empty".into()
                ))
            )
            .item_section_renderer
            .contents
            .try_swap_remove(0),
            Err(ExtractionError::InvalidData(
                "itemSectionRenderer empty".into()
            ))
        )
        .playlist_video_list_renderer
        .contents;

        let (videos, ctoken) = map_playlist_items(video_items.c);

        let (thumbnails, last_update_txt) = match self.sidebar {
            Some(sidebar) => {
                let mut sidebar_items = sidebar.playlist_sidebar_renderer.items;
                let mut primary = some_or_bail!(
                    sidebar_items.try_swap_remove(0),
                    Err(ExtractionError::InvalidData("no primary sidebar".into()))
                );

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
                let header_banner = some_or_bail!(
                    self.header.playlist_header_renderer.playlist_header_banner,
                    Err(ExtractionError::InvalidData("no thumbnail found".into()))
                );

                let mut byline = self.header.playlist_header_renderer.byline;
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
            Some(_) => {
                ok_or_bail!(
                    util::parse_numeric(&self.header.playlist_header_renderer.num_videos_text),
                    Err(ExtractionError::InvalidData("no video count".into()))
                )
            }
            None => videos.len() as u64,
        };

        let playlist_id = self.header.playlist_header_renderer.playlist_id;
        if playlist_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {}, expected {}",
                playlist_id, id
            )));
        }

        let name = self.header.playlist_header_renderer.title;
        let description = self.header.playlist_header_renderer.description_text;
        let channel = self
            .header
            .playlist_header_renderer
            .owner_text
            .and_then(|link| ChannelId::try_from(link).ok());

        let mut warnings = video_items.warnings;
        let last_update = last_update_txt
            .as_ref()
            .and_then(|txt| timeago::parse_textual_date_or_warn(lang, txt, &mut warnings));

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
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<PlaylistVideo>> for response::PlaylistCont {
    fn map_response(
        self,
        _id: &str,
        _lang: Language,
        _deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<Paginator<PlaylistVideo>>, ExtractionError> {
        let mut actions = self.on_response_received_actions;
        let action = some_or_bail!(
            actions.try_swap_remove(0),
            Err(ExtractionError::InvalidData(
                "no continuation action".into()
            ))
        );

        let (items, ctoken) =
            map_playlist_items(action.append_continuation_items_action.continuation_items.c);

        Ok(MapResult {
            c: Paginator::new(None, items, ctoken),
            warnings: action
                .append_continuation_items_action
                .continuation_items
                .warnings,
        })
    }
}

fn map_playlist_items(items: Vec<response::VideoListItem>) -> (Vec<PlaylistVideo>, Option<String>) {
    let mut ctoken: Option<String> = None;
    let videos = items
        .into_iter()
        .filter_map(|it| match it {
            response::VideoListItem::PlaylistVideoRenderer(video) => {
                match ChannelId::try_from(video.channel) {
                    Ok(channel) => Some(PlaylistVideo {
                        id: video.video_id,
                        title: video.title,
                        length: video.length_seconds,
                        thumbnail: video.thumbnail.into(),
                        channel,
                    }),
                    Err(_) => None,
                }
            }
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (videos, ctoken)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn map_playlist_data(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/playlist/playlist_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_playlist_data_{}", name), map_res.c, {
            ".last_update" => "[date]"
        });
    }
}
