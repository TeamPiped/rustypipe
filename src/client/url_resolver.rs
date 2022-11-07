use std::borrow::Cow;

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
struct QResolveUrl<'a> {
    context: YTContext<'a>,
    url: String,
}

impl RustyPipeQuery {
    pub async fn resolve_url(self, url: &str, resolve_albums: bool) -> Result<UrlTarget, Error> {
        let (url, params) = util::url_to_params(url)?;

        let mut is_shortlink = url.domain().and_then(|d| match d {
            "youtu.be" => Some(true),
            "youtube.com" => Some(false),
            _ => None,
        });
        let mut path_split = url
            .path_segments()
            .ok_or(Error::Other(Cow::Borrowed("invalid url: empty path")))?;

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
                    .ok_or(Error::Other(Cow::Borrowed("invalid url: no video id")))?
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
                    .ok_or(Error::Other(Cow::Borrowed("invalid url: no playlist id")))?
                    .to_string();

                // YouTube Music album has to be resolved by the YTM API
                if resolve_albums && id.starts_with(util::PLAYLIST_ID_ALBUM_PREFIX) {
                    self._navigation_resolve_url(
                        &format!("/playlist?list={}", id),
                        ClientType::DesktopMusic,
                    )
                    .await
                } else {
                    Ok(UrlTarget::Playlist { id })
                }
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
                        self._navigation_resolve_url(url.path(), ClientType::Desktop)
                            .await
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
                            match self
                                ._navigation_resolve_url(url.path(), ClientType::Desktop)
                                .await
                            {
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

    pub async fn resolve_string(
        self,
        string: &str,
        resolve_albums: bool,
    ) -> Result<UrlTarget, Error> {
        // URL with protocol
        if string.starts_with("http://") || string.starts_with("https://") {
            self.resolve_url(string, resolve_albums).await
        }
        // URL without protocol
        else if string.contains('/') && string.contains('.') {
            self.resolve_url(&format!("https://{}", string), resolve_albums)
                .await
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
            if resolve_albums && string.starts_with(util::PLAYLIST_ID_ALBUM_PREFIX) {
                self._navigation_resolve_url(
                    &format!("/playlist?list={}", string),
                    ClientType::DesktopMusic,
                )
                .await
            } else {
                Ok(UrlTarget::Playlist {
                    id: string.to_owned(),
                })
            }
        } else if util::ALBUM_ID_REGEX.is_match(string).unwrap_or_default() {
            Ok(UrlTarget::Album {
                id: string.to_owned(),
            })
        }
        // Channel name only
        else if util::VANITY_PATH_REGEX.is_match(string).unwrap_or_default() {
            self._navigation_resolve_url(
                &format!("/{}", string.trim_start_matches('/')),
                ClientType::Desktop,
            )
            .await
        } else {
            Err(Error::Other("invalid input string".into()))
        }
    }

    async fn _navigation_resolve_url(
        &self,
        url_path: &str,
        ctype: ClientType,
    ) -> Result<UrlTarget, Error> {
        let context = self.get_context(ctype, true, None).await;
        let request_body = QResolveUrl {
            context,
            url: format!(
                "https://{}.youtube.com{}",
                match ctype {
                    ClientType::DesktopMusic => "music",
                    _ => "www",
                },
                url_path
            ),
        };

        self.execute_request::<response::ResolvedUrl, _, _>(
            ctype,
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
        let browse_endpoint = self
            .endpoint
            .browse_endpoint
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("No browse ID")))?;

        let page_type = self
            .endpoint
            .command_metadata
            .map(|c| c.web_command_metadata.web_page_type)
            .or_else(|| {
                browse_endpoint
                    .browse_endpoint_context_supported_configs
                    .map(|c| c.browse_endpoint_context_music_config.page_type)
            })
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("No page type")))?;

        Ok(MapResult {
            c: page_type.to_url_target(browse_endpoint.browse_id),
            warnings: Vec::new(),
        })
    }
}
