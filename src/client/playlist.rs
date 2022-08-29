// REQUEST

use anyhow::{anyhow, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{
    model::{Channel, Playlist, Thumbnail, Video},
    serializer::text::{PageType, Text, TextLink},
};

use super::{response, ClientType, ContextYT, RustyTube};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylist {
    context: ContextYT,
    browse_id: String,
}

#[derive(Clone, Debug)]
pub struct TmpEntry {
    pub title: String,
    pub video_id: String,
}

impl RustyTube {
    pub async fn get_playlist(&self, playlist_id: &str) -> Result<Playlist> {
        let client = self.get_ytclient(ClientType::Desktop);
        let context = client.get_context(true).await;

        let request_body = QPlaylist {
            context,
            browse_id: "VL".to_owned() + playlist_id,
        };

        let resp = client
            .request_builder(Method::POST, "browse")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        let playlist_response = resp.json::<response::Playlist>().await?;

        map_playlist(&playlist_response)
    }
}

fn map_playlist(response: &response::Playlist) -> Result<Playlist> {
    let video_items = &some_or_bail!(
        some_or_bail!(
            some_or_bail!(
                response
                    .contents
                    .two_column_browse_results_renderer
                    .contents
                    .get(0),
                Err(anyhow!("twoColumnBrowseResultsRenderer empty"))
            )
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .get(0),
            Err(anyhow!("sectionListRenderer empty"))
        )
        .item_section_renderer
        .contents
        .get(0),
        Err(anyhow!("itemSectionRenderer empty"))
    )
    .playlist_video_list_renderer
    .contents;

    let mut ctoken: Option<String> = None;
    let videos = video_items
        .iter()
        .filter_map(|it| match it {
            response::playlist::PlaylistVideoItem::PlaylistVideoRenderer { video } => {
                match &video.channel {
                    TextLink::Browse {
                        text,
                        page_type,
                        browse_id,
                    } => match page_type {
                        PageType::Channel => Some(Video {
                            id: video.video_id.to_owned(),
                            title: video.title.to_owned(),
                            length: video.length_seconds,
                            thumbnails: video
                                .thumbnail
                                .thumbnails
                                .iter()
                                .map(|t| Thumbnail {
                                    url: t.url.to_owned(),
                                    width: t.width,
                                    height: t.height,
                                })
                                .collect(),
                            channel: Channel {
                                id: browse_id.to_string(),
                                name: text.to_owned(),
                            },
                        }),
                        _ => None,
                    },
                    _ => None,
                }
            }
            response::playlist::PlaylistVideoItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token.to_owned());
                None
            }
        })
        .collect::<Vec<_>>();

    let thumbnail_renderer = some_or_bail!(
        response
            .sidebar
            .playlist_sidebar_renderer
            .items
            .iter()
            .find_map(|s| match s {
                response::playlist::SidebarRendererItem::PlaylistSidebarPrimaryInfoRenderer {
                    thumbnail_renderer,
                } => Some(thumbnail_renderer),
                _ => None,
            }),
        Err(anyhow!("no primary sidebar"))
    );

    let video_owner_wrap = response
        .sidebar
        .playlist_sidebar_renderer
        .items
        .iter()
        .find_map(|s| match s {
            response::playlist::SidebarRendererItem::PlaylistSidebarSecondaryInfoRenderer {
                video_owner,
            } => Some(video_owner),
            _ => None,
        });

    let n_videos = match ctoken {
        Some(_) => {
            some_or_bail!(
                match &response.header.playlist_header_renderer.num_videos_text {
                    Text::Multiple { runs } =>
                        if runs.len() == 2 && runs[1] == " videos" {
                            runs[0].parse().ok()
                        } else {
                            None
                        },
                    _ => None,
                },
                Err(anyhow!("no video count"))
            )
        }
        None => videos.len() as u32,
    };

    let thumbnails = thumbnail_renderer
        .playlist_video_thumbnail_renderer
        .thumbnail
        .thumbnails
        .iter()
        .map(|t| Thumbnail {
            url: t.url.to_owned(),
            width: t.width,
            height: t.height,
        })
        .collect::<Vec<_>>();

    let name = response.header.playlist_header_renderer.title.to_owned();
    let description = response
        .header
        .playlist_header_renderer
        .description_text
        .to_owned();

    let channel = match video_owner_wrap {
        Some(o) => match &o.video_owner_renderer.title {
            TextLink::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                PageType::Channel => Some(Channel {
                    id: browse_id.to_owned(),
                    name: text.to_owned(),
                }),
                _ => None,
            },
            _ => None,
        },
        None => None,
    };

    Ok(Playlist {
        videos,
        n_videos,
        ctoken,
        name,
        thumbnails,
        description,
        channel,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn download_testfiles() {
        let tf_dir = Path::new("testfiles/playlist");

        let rt = RustyTube::new();

        for (name, id) in [
            ("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"),
            ("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ"),
            ("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
        ] {
            let mut json_path = tf_dir.to_path_buf();
            json_path.push(format!("playlist_{}.json", name));
            if json_path.exists() {
                continue;
            }

            let client = rt.get_ytclient(ClientType::Desktop);
            let context = client.get_context(false).await;

            let request_body = QPlaylist {
                context,
                browse_id: "VL".to_owned() + id,
            };

            let resp = client
                .request_builder(Method::POST, "browse")
                .await
                .json(&request_body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();

            let mut file = std::fs::File::create(json_path).unwrap();
            let mut content = std::io::Cursor::new(resp.bytes().await.unwrap());
            std::io::copy(&mut content, &mut file).unwrap();
        }
    }

    #[rstest]
    #[case::long(
        "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ",
        "Die schönsten deutschen Lieder | Beliebteste Lieder | Beste Deutsche Musik 2022",
        true,
        None,
        Some(Channel {
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
        Some(Channel {
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
        #[case] channel: Option<Channel>,
    ) {
        let rt = RustyTube::new();
        let playlist = rt.get_playlist(id).await.unwrap();

        assert_eq!(playlist.name, name);
        assert!(!playlist.videos.is_empty());
        assert_eq!(playlist.ctoken.is_some(), is_long);
        assert!(playlist.n_videos > 10);
        assert_eq!(playlist.n_videos > 100, is_long);
        assert_eq!(playlist.description, description);
        assert_eq!(playlist.channel, channel);
        assert!(!playlist.thumbnails.is_empty());
    }

    #[rstest]
    #[case::long("long")]
    #[case::short("short")]
    #[case::nomusic("nomusic")]
    fn t_map_player_data(#[case] name: &str) {
        let filename = format!("testfiles/playlist/playlist_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let playlist_data = map_playlist(&playlist).unwrap();
        insta::assert_yaml_snapshot!(format!("map_playlist_data_{}", name), playlist_data);
    }
}
