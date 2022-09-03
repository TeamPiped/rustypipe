use anyhow::Result;
use reqwest::Method;
use serde::Serialize;

use super::{response, ClientType, ContextYT, RustyTube};

#[derive(Clone, Debug, Serialize)]
struct QVideo {
    context: ContextYT,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
struct QVideoCont {
    context: ContextYT,
    continuation: String,
}

impl RustyTube {
    pub async fn get_video_response(&self, video_id: &str) -> Result<response::Video> {
        let client = self.get_ytclient(ClientType::Desktop);
        let context = client.get_context(true).await;
        let request_body = QVideo {
            context,
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        };

        let resp = client
            .request_builder(Method::POST, "next")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(resp.json::<response::Video>().await?)
    }

    pub async fn get_comments_response(&self, ctoken: &str) -> Result<response::VideoComments> {
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

    pub async fn get_recommendations_response(
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t_get_video_response() {
        let rt = RustyTube::new();
        // rt.get_video("ZeerrnuLi5E").await.unwrap();
        dbg!(rt.get_video_response("iQfSvIgIs_M").await.unwrap());
    }

    #[tokio::test]
    async fn t_get_comments_response() {
        let rt = RustyTube::new();
        // rt.get_comments("Eg0SC2lRZlN2SWdJc19NGAYyJSIRIgtpUWZTdklnSXNfTTAAeAJCEGNvbW1lbnRzLXNlY3Rpb24%3D").await.unwrap();
        dbg!(rt.get_comments_response("Eg0SC2lRZlN2SWdJc19NGAYychpFEhRVZ2lnVGJVTEZ6Qk5FWGdDb0FFQyICCAAqGFVDWFgwUldPSUJqdDRvM3ppSHUtNmE1QTILaVFmU3ZJZ0lzX01AAUgKQiljb21tZW50LXJlcGxpZXMtaXRlbS1VZ2lnVGJVTEZ6Qk5FWGdDb0FFQw%3D%3D").await.unwrap());
    }

    #[tokio::test]
    async fn t_get_recommendations_response() {
        let rt = RustyTube::new();
        dbg!(rt.get_recommendations_response("CBQSExILaVFmU3ZJZ0lzX03AAQHIAQEYACqkBjJzNkw2d3pVQkFyUkJBb0Q4ajRBQ2c3Q1Bnc0lvWXlRejhLZnRZUGNBUW9EOGo0QUNnN0NQZ3NJeElEX2w0YjFtNnUtQVFvRDhqNEFDZzNDUGdvSXg5Ykx3WUNKenFwX0NnUHlQZ0FLRGNJLUNnaW83T2pqZzVPTHZEOEtBX0ktQUFvTndqNEtDTE9venZmQThybVhXd29EOGo0QUNnM0NQZ29JdzZETV9vSFk0cHRCQ2dQeVBnQUtEc0ktQ3dqbW9QbURpcHVPel80QkNnUHlQZ0FLRGNJLUNnalY4THpEazlfOTRCWUtBX0ktQUFvT3dqNExDTXVZNU9YZzE3ejV2d0VLQV9JLUFBb053ajRLQ1A3eHZiSGswTnVuYWdvRDhqNEFDZzdDUGdzSXFQYVU5ZGp2Ml96S0FRb0Q4ajRBQ2c3Q1Bnc0lfSW1acUtQOTlfQ09BUW9EOGo0QUNnM0NQZ29JeGRtNzlZS3prcUFqQ2dQeVBnQUtEY0ktQ2dpZ3FJMkg0UENRX2s0S0FfSS1BQW9Pd2o0TENQV0V5NV9ZeDhERl9nRUtBX0ktQUFvT3dqNExDTzJid3VuV3BPX3ppd0VLQV9JLUFBb2gwajRlQ2h4U1JFTk5WVU5ZV0RCU1YwOUpRbXAwTkc4emVtbElkUzAyWVRWQkNnUHlQZ0FLRGNJLUNnaXpqcXZwcDh5MWwwMEtBX0ktQUFvTndqNEtDTFhWbl83dHhfWDJOUW9EOGo0QUNnN0NQZ3NJNWR5ZWc1NjZyUGUwQVJJVUFBSUVCZ2dLREE0UUVoUVdHQm9jSGlBaUpDWWFCQWdBRUFFYUJBZ0NFQU1hQkFnRUVBVWFCQWdHRUFjYUJBZ0lFQWthQkFnS0VBc2FCQWdNRUEwYUJBZ09FQThhQkFnUUVCRWFCQWdTRUJNYUJBZ1VFQlVhQkFnV0VCY2FCQWdZRUJrYUJBZ2FFQnNhQkFnY0VCMGFCQWdlRUI4YUJBZ2dFQ0VhQkFnaUVDTWFCQWdrRUNVYUJBZ21FQ2NxRkFBQ0JBWUlDZ3dPRUJJVUZoZ2FIQjRnSWlRbWoPd2F0Y2gtbmV4dC1mZWVk").await.unwrap());
    }
}
