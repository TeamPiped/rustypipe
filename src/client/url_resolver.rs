use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::UrlTarget,
    param::Language,
    serializer::MapResult,
    util,
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QResolveUrl {
    context: YTContext,
    url: String,
}

impl RustyPipeQuery {
    pub async fn resolve_url(self, url: &str) -> Result<UrlTarget, Error> {
        let (url, params) = util::url_to_params(url)?;

        let mut is_shortlink = url.domain().and_then(|d| match d {
            "youtu.be" => Some(true),
            "youtube.com" => Some(false),
            _ => None,
        });
        let mut path_split = url
            .path_segments()
            .ok_or_else(|| Error::Other("invalid url: empty path".into()))?;

        let get_start_time = || {
            params
                .get("t")
                .and_then(|t| t.parse::<u32>().ok())
                .unwrap_or_default()
        };

        let target = match path_split.next() {
            Some("watch") => {
                let id = params
                    .get("v")
                    .ok_or_else(|| Error::Other("invalid url: no video id".into()))?
                    .to_string();

                Ok(UrlTarget::Video {
                    id,
                    start_time: get_start_time(),
                })
            }
            Some("channel") => match path_split.next() {
                Some(id) => Ok(UrlTarget::Channel { id: id.to_owned() }),
                None => Err(Error::Other("invalid url: no channel id".into())),
            },
            Some("playlist") => {
                let id = params
                    .get("list")
                    .ok_or_else(|| Error::Other("invalid url: no playlist id".into()))?
                    .to_string();

                Ok(UrlTarget::Playlist { id })
            }
            // Channel vanity URL or youtu.be shortlink
            Some(mut id) => {
                if id == "c" || id == "user" {
                    id = path_split.next().unwrap_or(id);
                    is_shortlink = Some(false);
                }

                if id.is_empty() || id == "user" {
                    return Err(Error::Other(
                        "invalid url: no channel name / video id".into(),
                    ));
                }

                match is_shortlink {
                    Some(true) => {
                        // youtu.be shortlink (e.g. youtu.be/gHzuabZUd6c)
                        Ok(UrlTarget::Video {
                            id: id.to_owned(),
                            start_time: get_start_time(),
                        })
                    }
                    Some(false) => {
                        // Vanity URL (e.g. youtube.com/LinusTechTips) has to be resolved by the Innertube API
                        self._navigation_resolve_url(url.path()).await
                    }
                    None => {
                        // We dont have the original YT domain, so this can be both
                        // If there is a timestamp parameter, it has to be a video
                        // First check the innertube API if this is a channel vanity url
                        // If no channel is found and the identifier has the video ID format, assume it is a video
                        if !params.contains_key("t")
                            && util::VANITY_PATH_REGEX
                                .is_match(url.path())
                                .unwrap_or_default()
                        {
                            match self._navigation_resolve_url(url.path()).await {
                                Ok(target) => Ok(target),
                                Err(Error::Extraction(ExtractionError::ContentUnavailable(e))) => {
                                    match util::VIDEO_ID_REGEX.is_match(id).unwrap_or_default() {
                                        true => Ok(UrlTarget::Video {
                                            id: id.to_owned(),
                                            start_time: get_start_time(),
                                        }),
                                        false => Err(Error::Extraction(
                                            ExtractionError::ContentUnavailable(e),
                                        )),
                                    }
                                }
                                Err(e) => Err(e),
                            }
                        } else if util::VIDEO_ID_REGEX.is_match(id).unwrap_or_default() {
                            Ok(UrlTarget::Video {
                                id: id.to_owned(),
                                start_time: get_start_time(),
                            })
                        } else {
                            Err(Error::Other("invalid video / channel id".into()))
                        }
                    }
                }
            }
            None => Err(Error::Other("invalid url: empty path".into())),
        }?;

        target.validate()?;
        Ok(target)
    }

    pub async fn resolve_string(self, string: &str) -> Result<UrlTarget, Error> {
        // URL with protocol
        if string.starts_with("http://") || string.starts_with("https://") {
            self.resolve_url(string).await
        }
        // URL without protocol
        else if string.contains('/') && string.contains('.') {
            self.resolve_url(&format!("https://{}", string)).await
        }
        // ID only
        else if util::VIDEO_ID_REGEX.is_match(string).unwrap_or_default() {
            Ok(UrlTarget::Video {
                id: string.to_owned(),
                start_time: 0,
            })
        } else if util::CHANNEL_ID_REGEX.is_match(string).unwrap_or_default() {
            Ok(UrlTarget::Channel {
                id: string.to_owned(),
            })
        } else if util::PLAYLIST_ID_REGEX.is_match(string).unwrap_or_default() {
            Ok(UrlTarget::Playlist {
                id: string.to_owned(),
            })
        }
        // Channel name only
        else if util::VANITY_PATH_REGEX.is_match(string).unwrap_or_default() {
            self._navigation_resolve_url(&format!("/{}", string.trim_start_matches('/')))
                .await
        } else {
            Err(Error::Other("invalid input string".into()))
        }
    }

    async fn _navigation_resolve_url(&self, url_path: &str) -> Result<UrlTarget, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QResolveUrl {
            context,
            url: format!("https://www.youtube.com{}", url_path),
        };

        self.execute_request::<response::ResolvedUrl, _, _>(
            ClientType::Desktop,
            "channel_id",
            &request_body.url,
            "navigation/resolve_url",
            &request_body,
        )
        .await
    }
}

impl MapResponse<UrlTarget> for response::ResolvedUrl {
    fn map_response(
        self,
        _id: &str,
        _lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<UrlTarget>, ExtractionError> {
        let page_type = self
            .endpoint
            .command_metadata
            .ok_or_else(|| ExtractionError::InvalidData("No command metadata".into()))?
            .web_command_metadata
            .web_page_type;

        let id = self
            .endpoint
            .browse_endpoint
            .ok_or_else(|| ExtractionError::InvalidData("No browse ID".into()))?
            .browse_id;

        Ok(MapResult {
            c: page_type.to_url_target(id),
            warnings: Vec::new(),
        })
    }
}
