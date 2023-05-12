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
    /// Resolve the given YouTube URL and return its associated URL target.
    ///
    /// Note that the hostname of the URL is not checked, so this function also accepts URLs
    /// from alternative YouTube frontends like Piped or Invidious.
    ///
    /// The `resolve_albums` flag enables resolving YTM album URLs (e.g.
    /// `OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE`) to their short album ids (`MPREb_GyH43gCvdM5`).
    ///
    /// # Examples
    /// ```
    /// # use rustypipe::client::RustyPipe;
    /// # use rustypipe::model::UrlTarget;
    /// # let rp = RustyPipe::new();
    /// # tokio_test::block_on(async {
    /// // Channel
    /// assert_eq!(
    ///     rp.query().resolve_url("https://www.youtube.com/LinusTechTips", true).await.unwrap(),
    ///     UrlTarget::Channel {id: "UCXuqSBlHAE6Xw-yeJA0Tunw".to_owned()}
    /// );
    /// // Video
    /// assert_eq!(
    ///     rp.query().resolve_url("https://youtu.be/dQw4w9WgXcQ", true).await.unwrap(),
    ///     UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0}
    /// );
    /// // Album
    /// // You can choose whether album URLs should be resolved to their album id or returned as playlists
    /// assert_eq!(
    ///     rp.query().resolve_url("https://music.youtube.com/playlist?list=OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE", true).await.unwrap(),
    ///     UrlTarget::Album {id: "MPREb_GyH43gCvdM5".to_owned()}
    /// );
    /// assert_eq!(
    ///     rp.query().resolve_url("https://music.youtube.com/playlist?list=OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE", false).await.unwrap(),
    ///     UrlTarget::Playlist {id: "OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE".to_owned()}
    /// );
    /// # });
    /// ```
    pub async fn resolve_url<S: AsRef<str>>(
        self,
        url: S,
        resolve_albums: bool,
    ) -> Result<UrlTarget, Error> {
        let (url, params) = util::url_to_params(url.as_ref())?;

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
                        &format!("/playlist?list={id}"),
                        ClientType::DesktopMusic,
                    )
                    .await
                } else {
                    Ok(UrlTarget::Playlist { id })
                }
            }
            // Album or channel
            Some("browse") => match path_split.next() {
                Some(id) => {
                    if util::CHANNEL_ID_REGEX.is_match(id) {
                        Ok(UrlTarget::Channel { id: id.to_owned() })
                    } else if util::ALBUM_ID_REGEX.is_match(id) {
                        Ok(UrlTarget::Album { id: id.to_owned() })
                    } else {
                        Err(Error::Other("invalid url: no browse id".into()))
                    }
                }
                None => Err(Error::Other("invalid url: invalid browse id".into())),
            },
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
                        if !params.contains_key("t") && util::VANITY_PATH_REGEX.is_match(url.path())
                        {
                            match self
                                ._navigation_resolve_url(url.path(), ClientType::Desktop)
                                .await
                            {
                                Ok(target) => Ok(target),
                                Err(e) => {
                                    if matches!(
                                        e,
                                        Error::Extraction(ExtractionError::NotFound { .. })
                                    ) {
                                        match util::VIDEO_ID_REGEX.is_match(id) {
                                            true => Ok(UrlTarget::Video {
                                                id: id.to_owned(),
                                                start_time: get_start_time(),
                                            }),
                                            false => Err(e),
                                        }
                                    } else {
                                        Err(e)
                                    }
                                }
                            }
                        } else if util::VIDEO_ID_REGEX.is_match(id) {
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

    /// Resolve an input string and return a YouTube URL target
    ///
    /// Accepted input strings include YouTube URLs (see [`RustyPipeQuery::resolve_url`]),
    /// Video/Channel/Playlist/Album IDs and channel handles / vanity IDs.
    ///
    /// The `resolve_albums` flag enables resolving YTM album URLs and IDs (e.g.
    /// `OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE`) to their short album id (`MPREb_GyH43gCvdM5`).
    ///
    /// # Examples
    /// ```
    /// # use rustypipe::client::RustyPipe;
    /// # use rustypipe::model::UrlTarget;
    /// # let rp = RustyPipe::new();
    /// # tokio_test::block_on(async {
    /// // Channel
    /// assert_eq!(
    ///     rp.query().resolve_string("LinusTechTips", true).await.unwrap(),
    ///     UrlTarget::Channel {id: "UCXuqSBlHAE6Xw-yeJA0Tunw".to_owned()}
    /// );
    /// // Playlist
    /// assert_eq!(
    ///     rp.query().resolve_string("PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI", true).await.unwrap(),
    ///     UrlTarget::Playlist {id: "PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI".to_owned()}
    /// );
    /// # });
    /// ```
    pub async fn resolve_string<S: AsRef<str>>(
        self,
        s: S,
        resolve_albums: bool,
    ) -> Result<UrlTarget, Error> {
        let s = s.as_ref();

        // URL with protocol
        if s.starts_with("http://") || s.starts_with("https://") {
            self.resolve_url(s, resolve_albums).await
        }
        // URL without protocol
        else if s.contains('/') && s.contains('.') {
            self.resolve_url(&format!("https://{s}"), resolve_albums)
                .await
        }
        // ID only
        else if util::VIDEO_ID_REGEX.is_match(s) {
            Ok(UrlTarget::Video {
                id: s.to_owned(),
                start_time: 0,
            })
        } else if util::CHANNEL_ID_REGEX.is_match(s) {
            Ok(UrlTarget::Channel { id: s.to_owned() })
        } else if util::PLAYLIST_ID_REGEX.is_match(s) {
            if resolve_albums && s.starts_with(util::PLAYLIST_ID_ALBUM_PREFIX) {
                self._navigation_resolve_url(
                    &format!("/playlist?list={s}"),
                    ClientType::DesktopMusic,
                )
                .await
            } else {
                Ok(UrlTarget::Playlist { id: s.to_owned() })
            }
        } else if util::ALBUM_ID_REGEX.is_match(s) {
            Ok(UrlTarget::Album { id: s.to_owned() })
        }
        // Channel name only
        else if util::VANITY_PATH_REGEX.is_match(s) {
            self._navigation_resolve_url(
                &format!("/{}", s.trim_start_matches('/')),
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
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<UrlTarget>, ExtractionError> {
        let browse_endpoint = self
            .endpoint
            .browse_endpoint
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("No browse ID")))?;

        let target = self
            .endpoint
            .command_metadata
            .map(|c| c.web_command_metadata.web_page_type)
            .or_else(|| {
                browse_endpoint
                    .browse_endpoint_context_supported_configs
                    .map(|c| c.browse_endpoint_context_music_config.page_type)
            })
            .and_then(|pt| pt.to_url_target(browse_endpoint.browse_id))
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("No page type")))?;

        Ok(MapResult {
            c: target,
            warnings: Vec::new(),
        })
    }
}
