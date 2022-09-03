use anyhow::Result;
use reqwest::Method;
use serde::Serialize;

use super::{response, ClientType, ContextYT, RustyTube};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel {
    context: ContextYT,
    browse_id: String,
    params: String,
}

impl RustyTube {
    async fn get_channel_response(&self, channel_id: &str) -> Result<response::Channel> {
        let client = self.get_ytclient(ClientType::Desktop);
        let context = client.get_context(true).await;

        let request_body = QChannel {
            context,
            browse_id: channel_id.to_owned(),
            params: "EgZ2aWRlb3PyBgQKAjoA".to_owned(),
        };

        let resp = client
            .request_builder(Method::POST, "browse")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(resp.json::<response::Channel>().await?)
    }
}

#[cfg(test)]
mod tests {

}
