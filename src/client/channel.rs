use std::fmt::Debug;

use time::OffsetDateTime;
use url::Url;

use crate::{
    client::response::YouTubeListItem,
    error::{Error, ExtractionError},
    json::{
        yt_response_visitor_data, yt_two_column_list_items_from_browse, ytq, JsonDoc, JsonNode,
    },
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        Channel, ChannelInfo, PlaylistItem, Verification, VideoItem,
    },
    param::{ChannelOrder, ChannelVideoTab, Language},
    request_body::ytbody,
    serializer::{text::TextComponent, MapResult},
    util::{self, timeago, ProtoBuilder},
};

enum ChannelTab {
    Videos,
    Shorts,
    Live,
    Playlists,
    Search,
}

use super::{response, ClientType, MapJsonResponse, MapRespCtx, MapRespOptions, RustyPipeQuery};

#[derive(Debug)]
struct ChannelJson;
#[derive(Debug)]
struct ChannelAboutJson;

impl From<ChannelVideoTab> for ChannelTab {
    fn from(value: ChannelVideoTab) -> Self {
        match value {
            ChannelVideoTab::Videos => Self::Videos,
            ChannelVideoTab::Shorts => Self::Shorts,
            ChannelVideoTab::Live => Self::Live,
        }
    }
}

impl ChannelTab {
    fn params(self) -> &'static str {
        match self {
            Self::Videos => "EgZ2aWRlb3PyBgQKAjoA",
            Self::Shorts => "EgZzaG9ydHPyBgUKA5oBAA%3D%3D",
            Self::Live => "EgdzdHJlYW1z8gYECgJ6AA%3D%3D",
            Self::Playlists => "EglwbGF5bGlzdHMgAQ%3D%3D",
            Self::Search => "EgZzZWFyY2jyBgQKAloA",
        }
    }
}

impl RustyPipeQuery {
    async fn _channel_videos<S: AsRef<str>>(
        &self,
        channel_id: S,
        params: ChannelTab,
        query: Option<&str>,
        operation: &str,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        let channel_id = channel_id.as_ref();
        let request_body = ytbody!({
            "browseId": channel_id,
            "params": params.params(),
            ? "query": query,
        });

        self.execute_request::<ChannelJson, _, _>(
            ClientType::Desktop,
            operation,
            channel_id.as_ref(),
            "browse",
            &request_body,
        )
        .await
    }

    /// Get the videos from a YouTube channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_videos<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, ChannelTab::Videos, None, "channel_videos")
            .await
    }

    /// Get a ordered list of videos from a YouTube channel
    ///
    /// This function does not return channel metadata.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_videos_order<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
        order: ChannelOrder,
    ) -> Result<Paginator<VideoItem>, Error> {
        self.channel_videos_tab_order(channel_id, ChannelVideoTab::Videos, order)
            .await
    }

    /// Get the videos of the given tab (Shorts, Livestreams) from a YouTube channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_videos_tab<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
        tab: ChannelVideoTab,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, tab.into(), None, "channel_videos")
            .await
    }

    /// Get a ordered list of videos from the given tab (Shorts, Livestreams) of a YouTube channel
    ///
    /// This function does not return channel metadata.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_videos_tab_order<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
        tab: ChannelVideoTab,
        order: ChannelOrder,
    ) -> Result<Paginator<VideoItem>, Error> {
        self.continuation(
            order_ctoken(channel_id.as_ref(), tab, order, &random_target()),
            ContinuationEndpoint::Browse,
            None,
        )
        .await
    }

    /// Search the videos of a channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_search<S: AsRef<str> + Debug, S2: AsRef<str> + Debug>(
        &self,
        channel_id: S,
        query: S2,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(
            channel_id,
            ChannelTab::Search,
            Some(query.as_ref()),
            "channel_search",
        )
        .await
    }

    /// Get the playlists of a channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_playlists<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<PlaylistItem>>, Error> {
        let channel_id = channel_id.as_ref();
        let request_body = ytbody!({
            "browseId": channel_id,
            "params": ChannelTab::Playlists.params(),
        });

        self.execute_request::<ChannelJson, _, _>(
            ClientType::Desktop,
            "channel_playlists",
            channel_id,
            "browse",
            &request_body,
        )
        .await
    }

    /// Get additional metadata from the *About* tab of a channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_info<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
    ) -> Result<ChannelInfo, Error> {
        let channel_id = channel_id.as_ref();
        let request_body = ytbody!({
            "continuation": channel_info_ctoken(channel_id, &random_target()),
        });

        self.execute_request_ctx::<ChannelAboutJson, _, _>(
            ClientType::Desktop,
            "channel_info",
            channel_id,
            "browse",
            &request_body,
            MapRespOptions {
                unlocalized: true,
                ..Default::default()
            },
        )
        .await
    }
}

fn json_alerts_to_err(id: &str, root: &JsonNode<'_>) -> ExtractionError {
    let alerts = root.query(ytq!(.alerts)).map(|node| {
        let (alerts, _) = node.deserialize_items_lossy::<response::Alert>();
        alerts
    });
    response::alerts_to_err(id, alerts)
}

fn tab_renderer<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab.query(ytq!(.tabRenderer))
        .or_else(|| tab.query(ytq!(.expandableTabRenderer.tabRenderer)))
}

fn tab_endpoint_url(tab: &JsonNode<'_>) -> Option<String> {
    tab_renderer(tab).and_then(|tr| {
        tr.query(ytq!(.endpoint.commandMetadata.webCommandMetadata.url))
            .and_then(|url| url.as_str())
    })
}

struct MappedChannelContent<'a> {
    list_node: Option<JsonNode<'a>>,
    has_shorts: bool,
    has_live: bool,
}

fn map_channel_content<'a>(
    id: &str,
    root: &JsonNode<'a>,
) -> Result<MappedChannelContent<'a>, ExtractionError> {
    let browse = root
        .query(ytq!(.contents.twoColumnBrowseResultsRenderer))
        .ok_or_else(|| json_alerts_to_err(id, root))?;
    let tabs = browse
        .first_of(&[ytq!(.tabs), ytq!(.contents)])
        .map(|node| node.items())
        .unwrap_or_default();

    let mut has_shorts = false;
    let mut has_live = false;
    let mut featured_tab = false;

    for tab in &tabs {
        if let Some(url) = tab_endpoint_url(tab) {
            let selected = tab_renderer(tab)
                .and_then(|tr| tr.query(ytq!(.selected)))
                .and_then(|node| node.as_bool())
                .unwrap_or(false);
            if selected && url.ends_with("/featured") {
                if tab_renderer(tab)
                    .and_then(|tr| {
                        tr.query(ytq!(.content.sectionListRenderer))
                            .or_else(|| tr.query(ytq!(.content.richGridRenderer)))
                    })
                    .is_some()
                {
                    featured_tab = true;
                }
            } else if url.ends_with("/shorts") {
                has_shorts = true;
            } else if url.ends_with("/streams") {
                has_live = true;
            }
        } else if let Some(sl) =
            tab_renderer(tab).and_then(|tr| tr.query(ytq!(.content.sectionListRenderer.contents)))
        {
            if let Some(first) = sl.items().first() {
                if let Ok(YouTubeListItem::ChannelAgeGateRenderer {
                    channel_title,
                    main_text,
                }) = first.deserialize()
                {
                    return Err(ExtractionError::Unavailable {
                        reason: crate::error::UnavailabilityReason::AgeRestricted,
                        msg: format!("{channel_title}: {main_text}"),
                    });
                }
            }
        }
    }

    let list_node = if featured_tab {
        None
    } else {
        Some(
            yt_two_column_list_items_from_browse(&browse).ok_or_else(|| {
                ExtractionError::NotFound {
                    id: id.to_owned(),
                    msg: "no tabs".into(),
                }
            })?,
        )
    };

    Ok(MappedChannelContent {
        list_node,
        has_shorts,
        has_live,
    })
}

struct MapChannelData {
    header: response::channel::Header,
    metadata: response::channel::ChannelMetadataRenderer,
    microformat: response::channel::MicroformatDataRenderer,
    visitor_data: Option<String>,
    has_shorts: bool,
    has_live: bool,
}

fn map_channel(
    d: MapChannelData,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<Channel<()>>, ExtractionError> {
    if d.metadata.external_id != ctx.id {
        return Err(ExtractionError::WrongResult(format!(
            "got wrong channel id {}, expected {}",
            d.metadata.external_id, ctx.id
        )));
    }

    let handle = d
        .metadata
        .vanity_channel_url
        .as_ref()
        .and_then(|url| Url::parse(url).ok())
        .and_then(|url| {
            url.path()
                .strip_prefix('/')
                .filter(|handle| util::CHANNEL_HANDLE_REGEX.is_match(handle))
                .map(str::to_owned)
        });
    let mut warnings = Vec::new();

    Ok(MapResult {
        c: match d.header {
            response::channel::Header::C4TabbedHeaderRenderer(header) => Channel {
                id: d.metadata.external_id,
                name: d.metadata.title,
                handle,
                subscriber_count: header.subscriber_count_text.and_then(|txt| {
                    util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut warnings)
                }),
                video_count: None,
                avatar: header.avatar.into(),
                verification: header.badges.into(),
                description: d.metadata.description,
                tags: d.microformat.tags,
                banner: header.banner.into(),
                has_shorts: d.has_shorts,
                has_live: d.has_live,
                visitor_data: d.visitor_data,
                content: (),
            },
            response::channel::Header::CarouselHeaderRenderer(carousel) => {
                let hdata = carousel.contents.into_iter().find_map(|item| {
                    match item {
                        response::channel::CarouselHeaderRendererItem::TopicChannelDetailsRenderer {
                            subscriber_count_text,
                            subtitle,
                            avatar,
                        } => Some((subscriber_count_text.or(subtitle), avatar)),
                        response::channel::CarouselHeaderRendererItem::None => None,
                    }
                });

                Channel {
                    id: d.metadata.external_id,
                    name: d.metadata.title,
                    handle,
                    subscriber_count: hdata.as_ref().and_then(|hdata| {
                        hdata.0.as_ref().and_then(|txt| {
                            util::parse_large_numstr_or_warn(txt, ctx.lang, &mut warnings)
                        })
                    }),
                    video_count: None,
                    avatar: hdata.map(|hdata| hdata.1.into()).unwrap_or_default(),
                    verification: crate::model::Verification::Verified,
                    description: d.metadata.description,
                    tags: d.microformat.tags,
                    banner: Vec::new(),
                    has_shorts: d.has_shorts,
                    has_live: d.has_live,
                    visitor_data: d.visitor_data,
                    content: (),
                }
            }
            response::channel::Header::PageHeaderRenderer(header) => {
                let hdata = header.content.page_header_view_model;
                let md_rows = hdata.metadata.content_metadata_view_model.metadata_rows;
                let (sub_part, vc_part) = if md_rows.len() > 1 {
                    let mp = &md_rows[1].metadata_parts;
                    (mp.first(), mp.get(1))
                } else {
                    (
                        md_rows.first().and_then(|md| md.metadata_parts.get(1)),
                        None,
                    )
                };
                let subscriber_count = sub_part.and_then(|t| {
                    util::parse_large_numstr_or_warn::<u64>(t.as_str(), ctx.lang, &mut warnings)
                });
                let video_count = vc_part.and_then(|t| {
                    util::parse_large_numstr_or_warn(t.as_str(), ctx.lang, &mut warnings)
                });

                Channel {
                    id: d.metadata.external_id,
                    name: d.metadata.title,
                    handle: handle.or_else(|| {
                        md_rows
                            .first()
                            .and_then(|md| md.metadata_parts.get(1))
                            .map(|txt| txt.as_str().to_owned())
                            .filter(|txt| util::CHANNEL_HANDLE_REGEX.is_match(txt))
                    }),
                    subscriber_count,
                    video_count,
                    avatar: hdata
                        .image
                        .decorated_avatar_view_model
                        .avatar
                        .avatar_view_model
                        .image
                        .into(),
                    verification: hdata.title.map(Verification::from).unwrap_or_default(),
                    description: d.metadata.description,
                    tags: d.microformat.tags,
                    banner: hdata.banner.image_banner_view_model.image.into(),
                    has_shorts: d.has_shorts,
                    has_live: d.has_live,
                    visitor_data: d.visitor_data,
                    content: (),
                }
            }
        },
        warnings,
    })
}

fn map_channel_shell<'a>(
    root: &JsonNode<'a>,
    ctx: &MapRespCtx<'_>,
    content: MappedChannelContent<'a>,
) -> Result<(MapResult<Channel<()>>, Option<JsonNode<'a>>), ExtractionError> {
    let header = root
        .query(ytq!(.header))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no header".into(),
        })?
        .deserialize::<response::channel::Header>()?;
    let metadata = root
        .query(ytq!(.metadata.channelMetadataRenderer))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no metadata".into(),
        })?
        .deserialize::<response::channel::ChannelMetadataRenderer>()?;
    let microformat = root
        .query(ytq!(.microformat.microformatDataRenderer))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no microformat".into(),
        })?
        .deserialize::<response::channel::MicroformatDataRenderer>()?;
    let visitor_data =
        yt_response_visitor_data(root).or_else(|| ctx.visitor_data.map(str::to_owned));

    let channel_data = map_channel(
        MapChannelData {
            header,
            metadata,
            microformat,
            visitor_data: visitor_data.clone(),
            has_shorts: content.has_shorts,
            has_live: content.has_live,
        },
        ctx,
    )?;

    Ok((channel_data, content.list_node))
}

fn combine_channel_data<T>(channel_data: Channel<()>, content: T) -> Channel<T> {
    Channel {
        id: channel_data.id,
        name: channel_data.name,
        handle: channel_data.handle,
        subscriber_count: channel_data.subscriber_count,
        video_count: channel_data.video_count,
        avatar: channel_data.avatar,
        verification: channel_data.verification,
        description: channel_data.description,
        tags: channel_data.tags,
        banner: channel_data.banner,
        has_shorts: channel_data.has_shorts,
        has_live: channel_data.has_live,
        visitor_data: channel_data.visitor_data,
        content,
    }
}

impl MapJsonResponse<Channel<Paginator<VideoItem>>> for ChannelJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> {
        json.with_root(|root| {
            let content = map_channel_content(ctx.id, &root)?;
            let (channel_data, list_node) = map_channel_shell(&root, ctx, content)?;
            let visitor_data = channel_data.c.visitor_data.clone();

            let mut mapper = response::YouTubeListMapper::<VideoItem>::with_channel(
                ctx.lang,
                &channel_data.c,
                channel_data.warnings,
            );
            if let Some(node) = list_node {
                mapper.map_response_node(&node);
            }
            let p = Paginator::new_ext(
                None,
                mapper.items,
                mapper.ctoken,
                visitor_data,
                ContinuationEndpoint::Browse,
                false,
            );

            Ok(MapResult {
                c: combine_channel_data(channel_data.c, p),
                warnings: mapper.warnings,
            })
        })
    }
}

impl MapJsonResponse<Channel<Paginator<PlaylistItem>>> for ChannelJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<PlaylistItem>>>, ExtractionError> {
        json.with_root(|root| {
            let content = map_channel_content(ctx.id, &root)?;
            let (channel_data, list_node) = map_channel_shell(&root, ctx, content)?;
            let mut mapper = response::YouTubeListMapper::<PlaylistItem>::with_channel(
                ctx.lang,
                &channel_data.c,
                channel_data.warnings,
            );
            if let Some(node) = list_node {
                mapper.map_response_node(&node);
            }
            let p = Paginator::new(None, mapper.items, mapper.ctoken);

            Ok(MapResult {
                c: combine_channel_data(channel_data.c, p),
                warnings: mapper.warnings,
            })
        })
    }
}

impl MapJsonResponse<ChannelInfo> for ChannelAboutJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<ChannelInfo>, ExtractionError> {
        json.with_root(|root| {
            let lang = Language::En;

            if let Some(endpoints) = root.query(ytq!(.onResponseReceivedEndpoints)) {
                let (eps, _) =
                    endpoints.deserialize_items_lossy::<response::ContinuationActionWrap<
                        response::channel::AboutChannelRendererWrap,
                    >>();
                let ep = eps
                    .into_iter()
                    .next()
                    .ok_or(ExtractionError::InvalidData("no received endpoint".into()))?;
                let continuations = ep.append_continuation_items_action.continuation_items;
                let about = continuations
                    .c
                    .into_iter()
                    .next()
                    .ok_or(ExtractionError::InvalidData("no aboutChannel data".into()))?
                    .about_channel_renderer
                    .metadata
                    .about_channel_view_model;
                let mut warnings = continuations.warnings;

                let links = about
                    .links
                    .into_iter()
                    .filter_map(|l| {
                        let lv = l.channel_external_link_view_model;
                        if let TextComponent::Web { url, .. } = lv.link {
                            Some((String::from(lv.title), util::sanitize_yt_url(&url)))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                return Ok(MapResult {
                    c: ChannelInfo {
                        id: about.channel_id,
                        url: about.canonical_channel_url,
                        description: about.description,
                        subscriber_count: about.subscriber_count_text.and_then(|txt| {
                            util::parse_large_numstr_or_warn(&txt, lang, &mut warnings)
                        }),
                        video_count: about
                            .video_count_text
                            .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                        create_date: about.joined_date_text.and_then(|txt| {
                            timeago::parse_textual_date_or_warn(
                                lang,
                                ctx.utc_offset,
                                &txt,
                                &mut warnings,
                            )
                            .map(OffsetDateTime::date)
                        }),
                        view_count: about
                            .view_count_text
                            .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                        country: about.country.and_then(|c| util::country_from_name(&c)),
                        links,
                    },
                    warnings,
                });
            }

            if root.query(ytq!(.contents)).is_some() {
                map_channel_content(ctx.id, &root)?;
                return Err(ExtractionError::InvalidData(
                    "could not extract aboutData".into(),
                ));
            }

            Err(ExtractionError::InvalidData(
                "could not extract aboutData".into(),
            ))
        })
    }
}

/// Get the continuation token to fetch channel videos in the given order
fn order_ctoken(
    channel_id: &str,
    tab: ChannelVideoTab,
    order: ChannelOrder,
    target_id: &str,
) -> String {
    let mut pb_tab = ProtoBuilder::new();
    pb_tab.string(2, target_id);

    match tab {
        ChannelVideoTab::Videos => match order {
            ChannelOrder::Latest => {
                pb_tab.varint(3, 1);
                pb_tab.varint(4, 4);
            }
            ChannelOrder::Popular => {
                pb_tab.varint(3, 2);
                pb_tab.varint(4, 2);
            }
            ChannelOrder::Oldest => {
                pb_tab.varint(3, 4);
                pb_tab.varint(4, 5);
            }
        },
        ChannelVideoTab::Shorts => match order {
            ChannelOrder::Latest => pb_tab.varint(4, 4),
            ChannelOrder::Popular => pb_tab.varint(4, 2),
            ChannelOrder::Oldest => pb_tab.varint(4, 5),
        },
        ChannelVideoTab::Live => match order {
            ChannelOrder::Latest => pb_tab.varint(5, 12),
            ChannelOrder::Popular => pb_tab.varint(5, 14),
            ChannelOrder::Oldest => pb_tab.varint(5, 13),
        },
    }

    let mut pb_3 = ProtoBuilder::new();
    pb_3.embedded(tab.order_ctoken_id(), pb_tab);

    let mut pb_110 = ProtoBuilder::new();
    pb_110.embedded(3, pb_3);

    let mut pbi = ProtoBuilder::new();
    pbi.embedded(110, pb_110);

    let mut pb_80226972 = ProtoBuilder::new();
    pb_80226972.string(2, channel_id);
    pb_80226972.string(3, &pbi.to_base64());

    let mut pb = ProtoBuilder::new();
    pb.embedded(80_226_972, pb_80226972);

    pb.to_base64()
}

/// Get the continuation token to fetch channel
fn channel_info_ctoken(channel_id: &str, target_id: &str) -> String {
    let mut pb_3 = ProtoBuilder::new();
    pb_3.string(19, target_id);

    let mut pb_110 = ProtoBuilder::new();
    pb_110.embedded(3, pb_3);

    let mut pbi = ProtoBuilder::new();
    pbi.embedded(110, pb_110);

    let mut pb_80226972 = ProtoBuilder::new();
    pb_80226972.string(2, channel_id);
    pb_80226972.string(3, &pbi.to_base64());

    let mut pb = ProtoBuilder::new();
    pb.embedded(80_226_972, pb_80226972);

    pb.to_base64()
}

/// Create a random UUId to build continuation tokens
fn random_target() -> String {
    format!("\n${}", util::random_uuid())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        error::{ExtractionError, UnavailabilityReason},
        model::{paginator::Paginator, Channel, ChannelInfo, PlaylistItem, VideoItem},
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    use super::{
        channel_info_ctoken, order_ctoken, ChannelAboutJson, ChannelJson, MapJsonResponse,
        MapRespCtx,
    };
    use crate::{
        json::JsonDoc,
        param::{ChannelOrder, ChannelVideoTab},
    };

    #[rstest]
    #[case::base("videos_base", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::music("videos_music", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::withshorts("videos_shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::live("videos_live", "UChs0pSaEoNLV4mevBFGaoKA")]
    #[case::empty("videos_empty", "UCxBa895m48H5idw5li7h-0g")]
    #[case::upcoming("videos_upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A")]
    #[case::richgrid("videos_20221011_richgrid", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::richgrid2("videos_20221011_richgrid2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::coachella("videos_20230415_coachella", "UCHF66aWLOxBW4l6VkSrS3cQ")]
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::livestreams("livestreams", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::pageheader("shorts_20240129_pageheader", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::pageheader2("videos_20240324_pageheader2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::lockup("shorts_20240910_lockup", "UCh8gHdtzO2tXd593_bjErWg")]
    fn map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Channel<Paginator<VideoItem>>> =
            ChannelJson::map_json_response(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        if name == "videos_upcoming" {
            insta::assert_ron_snapshot!(format!("map_channel_{name}"), map_res.c, {
                ".content.items[1:].publish_date" => "[date]",
            });
        } else {
            insta::assert_ron_snapshot!(format!("map_channel_{name}"), map_res.c, {
                ".content.items[].publish_date" => "[date]",
            });
        }
    }

    #[test]
    fn channel_agegate() {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_agegate.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let res: Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> =
            ChannelJson::map_json_response(&json, &MapRespCtx::test("UCbfnHqxXs_K3kvaH-WlNlig"));
        if let Err(ExtractionError::Unavailable { reason, msg }) = res {
            assert_eq!(reason, UnavailabilityReason::AgeRestricted);
            assert!(msg.starts_with("Laphroaig Whisky: "));
        } else {
            panic!("invalid res: {res:?}")
        }
    }

    #[rstest]
    #[case::base("base")]
    #[case::lockup("20241109_lockup")]
    fn map_channel_playlists(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_playlists_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Channel<Paginator<PlaylistItem>>> =
            ChannelJson::map_json_response(&json, &MapRespCtx::test("UC2DjFE7Xf11URZqWBigcVOQ"))
                .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_channel_playlists_{name}"), map_res.c);
    }

    #[rstest]
    fn map_channel_info() {
        let json_path = path!(*TESTFILES / "channel" / "channel_info.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<ChannelInfo> = ChannelAboutJson::map_json_response(
            &json,
            &MapRespCtx::test("UC2DjFE7Xf11U-RZqWBigcVOQ"),
        )
        .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_info", map_res.c);
    }

    #[test]
    fn t_order_ctoken() {
        let channel_id = "UCXuqSBlHAE6Xw-yeJA0Tunw";

        let videos_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Videos,
            ChannelOrder::Popular,
            "\n$6461d7c8-0000-2040-87aa-089e0827e420",
        );
        assert_eq!(videos_popular_token, "4qmFsgJgEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaRDhnWXdHaTU2TEJJbUNpUTJORFl4WkRkak9DMHdNREF3TFRJd05EQXRPRGRoWVMwd09EbGxNRGd5TjJVME1qQVlBaUFD");

        let shorts_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Shorts,
            ChannelOrder::Popular,
            "\n$64679ffb-0000-26b3-a1bd-582429d2c794",
        );
        assert_eq!(shorts_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXhTS2hJbUNpUTJORFkzT1dabVlpMHdNREF3TFRJMllqTXRZVEZpWkMwMU9ESTBNamxrTW1NM09UUWdBZyUzRCUzRA%3D%3D");

        let live_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Live,
            ChannelOrder::Popular,
            "\n$64693069-0000-2a1e-8c7d-582429bd5ba8",
        );
        assert_eq!(live_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXh5S2hJbUNpUTJORFk1TXpBMk9TMHdNREF3TFRKaE1XVXRPR00zWkMwMU9ESTBNamxpWkRWaVlUZ29EZyUzRCUzRA%3D%3D");
    }

    #[test]
    fn t_channel_info_ctoken() {
        let channel_id = "UCh8gHdtzO2tXd593_bjErWg";

        let token = channel_info_ctoken(channel_id, "\n$655b339a-0000-20b9-92dc-582429d254b4");
        assert_eq!(token, "4qmFsgJgEhhVQ2g4Z0hkdHpPMnRYZDU5M19iakVyV2caRDhnWXJHaW1hQVNZS0pEWTFOV0l6TXpsaExUQXdNREF0TWpCaU9TMDVNbVJqTFRVNE1qUXlPV1F5TlRSaU5BJTNEJTNE");
    }
}
