use anyhow::{anyhow, bail, Context, Result};
use reqwest::Method;
use serde::{Serialize};

use super::{response, BaseRequest, RustyTube, YTClient};
use crate::util;

// REQUEST

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlayer {
    #[serde(flatten)]
    base: BaseRequest,
    /// Website playback context
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_context: Option<QPlaybackContext>,
    /// Content playback nonce (16 random chars)
    cpn: String,
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
    pub async fn fetch_player(&self, video_id: &str) -> Result<response::Player> {
        let sts = self.desktop_client.deobf.get_sts().await?;

        let request_body = QPlayer {
            base: self.desktop_client.get_base_request_body(false).await,
            playback_context: Some(QPlaybackContext {
                content_playback_context: QContentPlaybackContext {
                    signature_timestamp: sts,
                    referer: format!("https://www.youtube.com/watch?v={}", video_id),
                },
            }),
            cpn: util::generate_content_playback_nonce(),
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        };

        let resp = self
            .desktop_client
            .request_builder(Method::POST, "player")
            .await
            .json(&request_body)
            .send()
            .await?;

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
        let stream = rt.fetch_player("ZeerrnuLi5E").await.unwrap();

        dbg!(stream);
    }
}
