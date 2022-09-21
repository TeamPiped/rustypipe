use anyhow::{bail, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{model::ChannelVideos, serializer::MapResult};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel {
    context: YTContext,
    browse_id: String,
    params: Params,
}

#[derive(Debug, Serialize)]
enum Params {
    #[serde(rename = "EgZ2aWRlb3PyBgQKAjoA")]
    VideosLatest,
    #[serde(rename = "EgZ2aWRlb3MYAiAAMAE%3D")]
    VideosOldest,
    #[serde(rename = "EgZ2aWRlb3MYASAAMAE%3D")]
    VideosPopular,
    #[serde(rename = "EglwbGF5bGlzdHMgAQ%3D%3D")]
    Playlists,
    #[serde(rename = "EgVhYm91dPIGBAoCEgA%3D")]
    Info,
}

impl RustyPipeQuery {
    pub async fn channel_videos(&self, channel_id: &str) -> Result<ChannelVideos> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id.to_owned(),
            params: Params::VideosLatest,
        };

        self.execute_request::<response::Channel, _, _>(
            ClientType::Desktop,
            "channel_videos",
            channel_id,
            Method::POST,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<ChannelVideos> for response::Channel {
    fn map_response(
        self,
        id: &str,
        _lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<ChannelVideos>> {
        let warnings = Vec::new();
        // dbg!(&self);
        let header = self.header.c4_tabbed_header_renderer;

        if header.channel_id != id {
            bail!(
                "got wrong channel id {}, expected {}",
                header.channel_id,
                id
            );
        }

        Ok(MapResult {
            c: ChannelVideos {
                id: header.channel_id,
                name: header.title,
                subscriber_count_txt: header.subscriber_count_text,
            },
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {}
