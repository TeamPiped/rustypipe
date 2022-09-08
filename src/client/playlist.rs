use anyhow::{anyhow, Context, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{
    model::{Channel, Language, Playlist, Thumbnail, Video},
    serializer::text::{PageType, TextLink},
    timeago, util,
};

use super::{response, ClientType, ContextYT, RustyTube};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylist {
    context: ContextYT,
    browse_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaylistCont {
    context: ContextYT,
    continuation: String,
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

        let resp_body = resp.text().await?;
        let playlist_response =
            serde_json::from_str::<response::Playlist>(&resp_body).context(resp_body)?;

        map_playlist(&playlist_response, self.localization.language)
    }

    pub async fn get_playlist_cont(&self, playlist: &mut Playlist) -> Result<()> {
        match &playlist.ctoken {
            Some(ctoken) => {
                let client = self.get_ytclient(ClientType::Desktop);
                let context = client.get_context(true).await;

                let request_body = QPlaylistCont {
                    context,
                    continuation: ctoken.to_owned(),
                };

                let resp = client
                    .request_builder(Method::POST, "browse")
                    .await
                    .json(&request_body)
                    .send()
                    .await?
                    .error_for_status()?;

                let cont_response = resp.json::<response::playlist::PlaylistCont>().await?;

                let action = some_or_bail!(
                    cont_response
                        .on_response_received_actions
                        .iter()
                        .find(|a| a.append_continuation_items_action.target_id == playlist.id),
                    Err(anyhow!("no continuation action"))
                );

                let (mut videos, ctoken) =
                    map_playlist_items(&action.append_continuation_items_action.continuation_items);

                playlist.videos.append(&mut videos);
                playlist.ctoken = ctoken;

                if playlist.ctoken.is_none() {
                    playlist.n_videos = playlist.videos.len() as u32;
                }

                Ok(())
            }
            None => Err(anyhow!("no ctoken")),
        }
    }
}

fn map_playlist(response: &response::Playlist, lang: Language) -> Result<Playlist> {
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

    let (videos, ctoken) = map_playlist_items(video_items);

    let (thumbnails, last_update_txt) = match &response.sidebar {
        Some(sidebar) => {
            let primary = some_or_bail!(
                sidebar.playlist_sidebar_renderer.items.get(0),
                Err(anyhow!("no primary sidebar"))
            );

            (
                &primary
                    .playlist_sidebar_primary_info_renderer
                    .thumbnail_renderer
                    .playlist_video_thumbnail_renderer
                    .thumbnail
                    .thumbnails,
                primary
                    .playlist_sidebar_primary_info_renderer
                    .stats
                    .get(2)
                    .map(|t| t.to_owned()),
            )
        }
        None => {
            let header_banner = some_or_bail!(
                &response
                    .header
                    .playlist_header_renderer
                    .playlist_header_banner,
                Err(anyhow!("no thumbnail found"))
            );

            let last_update_txt = response
                .header
                .playlist_header_renderer
                .byline
                .get(1)
                .map(|b| b.playlist_byline_renderer.text.to_owned());

            (
                &header_banner
                    .hero_playlist_thumbnail_renderer
                    .thumbnail
                    .thumbnails,
                last_update_txt,
            )
        }
    };

    let thumbnails = thumbnails
        .iter()
        .map(|t| Thumbnail {
            url: t.url.to_owned(),
            width: t.width,
            height: t.height,
        })
        .collect::<Vec<_>>();

    let n_videos = match ctoken {
        Some(_) => {
            ok_or_bail!(
                util::parse_numeric(&response.header.playlist_header_renderer.num_videos_text),
                Err(anyhow!("no video count"))
            )
        }
        None => videos.len() as u32,
    };

    let id = response
        .header
        .playlist_header_renderer
        .playlist_id
        .to_owned();
    let name = response.header.playlist_header_renderer.title.to_owned();
    let description = response
        .header
        .playlist_header_renderer
        .description_text
        .to_owned();

    let channel = match &response.header.playlist_header_renderer.owner_text {
        Some(owner_text) => match owner_text {
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
        id,
        name,
        videos,
        n_videos,
        ctoken,
        thumbnails,
        description,
        channel,
        last_update: match &last_update_txt {
            Some(textual_date) => timeago::parse_textual_date_to_dt(lang, textual_date),
            None => None,
        },
        last_update_txt,
    })
}

fn map_playlist_items(
    items: &Vec<response::VideoListItem<response::playlist::PlaylistVideo>>,
) -> (Vec<Video>, Option<String>) {
    let mut ctoken: Option<String> = None;
    let videos = items
        .iter()
        .filter_map(|it| match it {
            response::VideoListItem::GridVideoRenderer { video } => match &video.channel {
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
            },
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token.to_owned());
                None
            }
        })
        .collect::<Vec<_>>();
    (videos, ctoken)
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

        assert_eq!(playlist.id, id);
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
    fn t_map_playlist_data(#[case] name: &str) {
        let filename = format!("testfiles/playlist/playlist_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let playlist_data = map_playlist(&playlist, Language::En).unwrap();
        insta::assert_yaml_snapshot!(format!("map_playlist_data_{}", name), playlist_data, {
            ".last_update" => "[date]"
        });
    }

    #[test_log::test(tokio::test)]
    async fn t_playlist_cont() {
        let rt = RustyTube::new();
        let mut playlist = rt
            .get_playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
            .await
            .unwrap();

        while playlist.ctoken.is_some() {
            rt.get_playlist_cont(&mut playlist).await.unwrap();
        }

        assert!(playlist.videos.len() > 100);
    }
}
