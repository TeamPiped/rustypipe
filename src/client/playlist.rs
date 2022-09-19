use anyhow::{anyhow, bail, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{
    deobfuscate::Deobfuscator,
    model::{ChannelId, Language, Paginator, Playlist, PlaylistVideo},
    serializer::text::{PageType, TextLink},
    timeago,
    util::{self, TryRemove},
};

use super::{response, ClientType, MapResponse, MapResult, RustyPipeQuery, YTContext};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylist {
    context: YTContext,
    browse_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylistCont {
    context: YTContext,
    continuation: String,
}

impl RustyPipeQuery {
    pub async fn playlist(self, playlist_id: &str) -> Result<Playlist> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QPlaylist {
            context,
            browse_id: "VL".to_owned() + playlist_id,
        };

        self.execute_request::<response::Playlist, _, _>(
            ClientType::Desktop,
            "playlist",
            playlist_id,
            Method::POST,
            "browse",
            &request_body,
        )
        .await
    }

    pub async fn get_playlist_continuation(self, ctoken: &str) -> Result<Paginator<PlaylistVideo>> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QPlaylistCont {
            context,
            continuation: ctoken.to_owned(),
        };

        self.execute_request::<response::PlaylistCont, _, _>(
            ClientType::Desktop,
            "get_playlist_continuation",
            ctoken,
            Method::POST,
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
    ) -> Result<MapResult<Playlist>> {
        // TODO: think about a deserializer that deserializes only first list item
        let mut tcbr_contents = self.contents.two_column_browse_results_renderer.contents;
        let video_items = some_or_bail!(
            some_or_bail!(
                some_or_bail!(
                    tcbr_contents.try_swap_remove(0),
                    Err(anyhow!("twoColumnBrowseResultsRenderer empty"))
                )
                .tab_renderer
                .content
                .section_list_renderer
                .contents
                .try_swap_remove(0),
                Err(anyhow!("sectionListRenderer empty"))
            )
            .item_section_renderer
            .contents
            .try_swap_remove(0),
            Err(anyhow!("itemSectionRenderer empty"))
        )
        .playlist_video_list_renderer
        .contents;

        let (videos, ctoken) = map_playlist_items(video_items.c);

        let (thumbnails, last_update_txt) = match self.sidebar {
            Some(sidebar) => {
                let mut sidebar_items = sidebar.playlist_sidebar_renderer.items;
                let mut primary = some_or_bail!(
                    sidebar_items.try_swap_remove(0),
                    Err(anyhow!("no primary sidebar"))
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
                    Err(anyhow!("no thumbnail found"))
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
                    Err(anyhow!("no video count"))
                )
            }
            None => videos.len() as u32,
        };

        let playlist_id = self.header.playlist_header_renderer.playlist_id;
        if playlist_id != id {
            bail!("got wrong playlist id {}, expected {}", playlist_id, id);
        }

        let name = self.header.playlist_header_renderer.title;
        let description = self.header.playlist_header_renderer.description_text;

        let channel = match self.header.playlist_header_renderer.owner_text {
            Some(TextLink::Browse {
                text,
                page_type: PageType::Channel,
                browse_id,
            }) => Some(ChannelId {
                id: browse_id,
                name: text,
            }),
            _ => None,
        };

        let mut warnings = video_items.warnings;
        let last_update = match &last_update_txt {
            Some(textual_date) => {
                let parsed = timeago::parse_textual_date_to_dt(lang, textual_date);
                if parsed.is_none() {
                    warnings.push(format!("could not parse textual date `{}`", textual_date));
                }
                parsed
            }
            None => None,
        };

        Ok(MapResult {
            c: Playlist {
                id: playlist_id,
                name,
                videos: Paginator {
                    items: videos,
                    ctoken,
                },
                n_videos,
                thumbnails: thumbnails.into(),
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
    ) -> Result<MapResult<Paginator<PlaylistVideo>>> {
        let mut actions = self.on_response_received_actions;
        let action = some_or_bail!(
            actions.try_swap_remove(0),
            Err(anyhow!("no continuation action"))
        );

        let (items, ctoken) =
            map_playlist_items(action.append_continuation_items_action.continuation_items.c);

        Ok(MapResult {
            c: Paginator { items, ctoken },
            warnings: action
                .append_continuation_items_action
                .continuation_items
                .warnings,
        })
    }
}

fn map_playlist_items(
    items: Vec<response::VideoListItem<response::playlist::PlaylistVideo>>,
) -> (Vec<PlaylistVideo>, Option<String>) {
    let mut ctoken: Option<String> = None;
    let videos = items
        .into_iter()
        .filter_map(|it| match it {
            response::VideoListItem::GridVideoRenderer { video } => match video.channel {
                TextLink::Browse {
                    text,
                    page_type: PageType::Channel,
                    browse_id,
                } => Some(PlaylistVideo {
                    id: video.video_id,
                    title: video.title,
                    length: video.length_seconds,
                    thumbnails: video.thumbnail.into(),
                    channel: ChannelId {
                        id: browse_id,
                        name: text,
                    },
                }),
                _ => None,
            },
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            response::VideoListItem::None => None,
        })
        .collect::<Vec<_>>();
    (videos, ctoken)
}

impl Paginator<PlaylistVideo> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.get_playlist_continuation(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => {
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => {
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use crate::client::RustyPipe;

    use super::*;

    #[rstest]
    #[case::long(
        "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ",
        "Die schönsten deutschen Lieder | Beliebteste Lieder | Beste Deutsche Musik 2022",
        true,
        None,
        Some(ChannelId {
            id: "UCIekuFeMaV78xYfvpmoCnPg".to_owned(),
            name: "Best Music".to_owned(),
        })
    )]
    #[case::short(
        "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk",
        "Easy Pop",
        false,
        None,
        None
    )]
    #[case::nomusic(
        "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe",
        "Minecraft SHINE",
        false,
        Some("SHINE - Survival Hardcore in New Environment: Auf einem Server machen sich tapfere Spieler auf, mystische Welten zu erkunden, magische Technologien zu erforschen und vorallem zu überleben...".to_owned()),
        Some(ChannelId {
            id: "UCQM0bS4_04-Y4JuYrgmnpZQ".to_owned(),
            name: "Chaosflo44".to_owned(),
        })
    )]
    #[test_log::test(tokio::test)]
    async fn t_get_playlist(
        #[case] id: &str,
        #[case] name: &str,
        #[case] is_long: bool,
        #[case] description: Option<String>,
        #[case] channel: Option<ChannelId>,
    ) {
        let rp = RustyPipe::builder().strict().build();
        let playlist = rp.query().playlist(id).await.unwrap();

        assert_eq!(playlist.id, id);
        assert_eq!(playlist.name, name);
        assert!(!playlist.videos.is_empty());
        assert_eq!(!playlist.videos.is_exhausted(), is_long);
        assert!(playlist.n_videos > 10);
        assert_eq!(playlist.n_videos > 100, is_long);
        assert_eq!(playlist.description, description);
        if channel.is_some() {
            assert_eq!(playlist.channel, channel);
        }
        assert!(!playlist.thumbnails.is_empty());
    }

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    fn t_map_playlist_data(#[case] name: &str, #[case] id: &str) {
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
        insta::assert_yaml_snapshot!(format!("map_playlist_data_{}", name), map_res.c, {
            ".last_update" => "[date]"
        });
    }

    #[test_log::test(tokio::test)]
    async fn t_playlist_cont() {
        let rp = RustyPipe::builder().strict().build();
        let mut playlist = rp
            .query()
            .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
            .await
            .unwrap();

        playlist
            .videos
            .extend_pages(rp.query(), usize::MAX)
            .await
            .unwrap();
        assert!(playlist.videos.items.len() > 100);
    }

    #[test_log::test(tokio::test)]
    async fn t_playlist_cont2() {
        let rp = RustyPipe::builder().strict().build();
        let mut playlist = rp
            .query()
            .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
            .await
            .unwrap();

        playlist.videos.extend_limit(rp.query(), 101).await.unwrap();
        assert!(playlist.videos.items.len() > 100);
    }
}
