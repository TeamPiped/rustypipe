// REQUEST

use anyhow::{anyhow, bail, Result};
use reqwest::Method;
use serde::Serialize;

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
    pub async fn get_playlist(
        &self,
        playlist_id: &str,
        client_type: ClientType,
    ) -> Result<Vec<TmpEntry>> {
        // let client = self.desktop_client.clone();
        let client = self.get_ytclient(client_type);
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

        Ok(map_playlist_tmp(playlist_response))
    }
}

fn map_playlist_tmp(response: response::Playlist) -> Vec<TmpEntry> {
    let content = &response
        .contents
        .two_column_browse_results_renderer
        .contents[0]
        .tab_renderer
        .content
        .section_list_renderer
        .contents[0];

    match &content.item_section_renderer {
        Some(items) => items.contents[0]
            .playlist_video_list_renderer
            .contents
            .iter()
            .map(|it| TmpEntry {
                title: it.playlist_video_renderer.title.to_owned(),
                video_id: it.playlist_video_renderer.video_id.to_owned(),
            })
            .collect(),
        None => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::client::ClientType;

    use super::*;

    #[allow(dead_code)]
    // #[test_log::test(tokio::test)]
    async fn download_testfiles() {
        let tf_dir = Path::new("testfiles/playlist");
        let playlist_id = "RDCLAK5uy_mHW5bcduhjB-PkTePAe6EoRMj1xNT8gzY";

        let rt = RustyTube::new();

        for client_type in [ClientType::Desktop, ClientType::DesktopMusic] {
            let client = rt.get_ytclient(client_type);
            let context = client.get_context(false).await;

            let request_body = QPlaylist {
                context,
                browse_id: "VL".to_owned() + playlist_id,
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

            let mut json_path = tf_dir.to_path_buf();
            json_path.push(format!("{:?}_playlist.json", client_type).to_lowercase());

            let mut file = std::fs::File::create(json_path).unwrap();
            let mut content = std::io::Cursor::new(resp.bytes().await.unwrap());
            std::io::copy(&mut content, &mut file).unwrap();
        }
    }

    #[test_log::test(tokio::test)]
    async fn t_get_playlist() {
        let rt = RustyTube::new();
        let playlist = rt
            .get_playlist(
                "RDCLAK5uy_mHW5bcduhjB-PkTePAe6EoRMj1xNT8gzY",
                ClientType::Desktop,
            )
            .await
            .unwrap();

        dbg!(playlist);
    }

    #[test_log::test(tokio::test)]
    async fn t_get_playlist_music() {
        let rt = RustyTube::new();
        let playlist = rt
            .get_playlist(
                "RDCLAK5uy_mHW5bcduhjB-PkTePAe6EoRMj1xNT8gzY",
                ClientType::Desktop,
            )
            .await
            .unwrap();

        dbg!(playlist);
    }
}
