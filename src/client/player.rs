use anyhow::{anyhow, bail, Context, Result};
use reqwest::Method;
use serde::Serialize;

use super::{response, ContextYT, ClientType, RustyTube, YTClient};
use crate::util;

// REQUEST

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlayer {
    context: ContextYT,
    /// Website playback context
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_context: Option<QPlaybackContext>,
    /// Content playback nonce (mobile only, 16 random chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    cpn: Option<String>,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaybackContext {
    content_playback_context: QContentPlaybackContext,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContentPlaybackContext {
    /// Signature timestamp extracted from player.js
    signature_timestamp: String,
    /// Referer URL from website
    referer: String,
}

impl RustyTube {
    pub async fn fetch_player(
        &self,
        video_id: &str,
        client_type: ClientType,
    ) -> Result<response::Player> {
        let client = self.get_ytclient(client_type);
        let context = client.get_context(false).await;

        let request_body = if client_type.is_web() {
            QPlayer {
                context,
                playback_context: Some(QPlaybackContext {
                    content_playback_context: QContentPlaybackContext {
                        signature_timestamp: self.desktop_client.deobf.get_sts().await?,
                        referer: format!("https://www.youtube.com/watch?v={}", video_id),
                    },
                }),
                cpn: None,
                video_id: video_id.to_owned(),
                content_check_ok: true,
                racy_check_ok: true,
            }
        } else {
            QPlayer {
                context,
                playback_context: None,
                cpn: Some(util::generate_content_playback_nonce()),
                video_id: video_id.to_owned(),
                content_check_ok: true,
                racy_check_ok: true,
            }
        };

        let resp = self
            .desktop_client
            .request_builder(Method::POST, "player")
            .await
            .json(&request_body)
            .send()
            .await?;

        // println!("{}", resp.text().await?);
        // todo!();

        Ok(resp.json::<response::Player>().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test(tokio::test)]
    async fn t_fetch_stream() {
        let rt = RustyTube::new();
        let stream = rt.fetch_player("ZeerrnuLi5E", ClientType::Desktop).await.unwrap();

        dbg!(stream);
    }
}
