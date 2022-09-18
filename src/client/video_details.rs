use anyhow::Result;
use reqwest::Method;
use serde::Serialize;

use crate::{model::VideoDetails, serializer::MapResult};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Clone, Debug, Serialize)]
struct QVideo {
    context: YTContext,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
struct QVideoCont {
    context: YTContext,
    continuation: String,
}

impl RustyPipeQuery {
    pub async fn video_details(self, video_id: &str) -> Result<VideoDetails> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QVideo {
            context,
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        };

        self.execute_request::<response::VideoDetails, _, _>(
            ClientType::Desktop,
            "video_details",
            video_id,
            Method::POST,
            "next",
            &request_body,
        )
        .await
    }

    /*
    async fn get_comments_response(&self, ctoken: &str) -> Result<response::VideoComments> {
        let client = self.get_ytclient(ClientType::Desktop);
        let context = client.get_context(true).await;
        let request_body = QVideoCont {
            context,
            continuation: ctoken.to_owned(),
        };

        let resp = client
            .request_builder(Method::POST, "next")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(resp.json::<response::VideoComments>().await?)
    }

    async fn get_recommendations_response(
        &self,
        ctoken: &str,
    ) -> Result<response::VideoRecommendations> {
        let client = self.get_ytclient(ClientType::Desktop);
        let context = client.get_context(true).await;
        let request_body = QVideoCont {
            context,
            continuation: ctoken.to_owned(),
        };

        let resp = client
            .request_builder(Method::POST, "next")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(resp.json::<response::VideoRecommendations>().await?)
    }
    */
}

impl MapResponse<VideoDetails> for response::VideoDetails {
    fn map_response(
        self,
        id: &str,
        lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<VideoDetails>> {
        Ok(MapResult {
            c: VideoDetails {
                id: id.to_owned(),
                title: "".to_owned(),
                description: "".to_owned(),
            },
            warnings: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
