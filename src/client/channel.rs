use std::fmt::Debug;

use serde::Serialize;
use time::OffsetDateTime;
use url::Url;

use crate::{
    client::response::YouTubeListItem,
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        Channel, ChannelInfo, PlaylistItem, Verification, VideoItem,
    },
    param::{ChannelOrder, ChannelVideoTab, Language},
    serializer::{text::TextComponent, MapResult},
    util::{self, timeago, ProtoBuilder},
};

use super::{
    response, ClientType, MapRespCtx, MapRespOptions, MapResponse, QContinuation, RustyPipeQuery,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel<'a> {
    browse_id: &'a str,
    params: ChannelTab,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
}

#[derive(Debug, Serialize)]
enum ChannelTab {
    #[serde(rename = "EgZ2aWRlb3PyBgQKAjoA")]
    Videos,
    #[serde(rename = "EgZzaG9ydHPyBgUKA5oBAA%3D%3D")]
    Shorts,
    #[serde(rename = "EgdzdHJlYW1z8gYECgJ6AA%3D%3D")]
    Live,
    #[serde(rename = "EglwbGF5bGlzdHMgAQ%3D%3D")]
    Playlists,
    #[serde(rename = "EgZzZWFyY2jyBgQKAloA")]
    Search,
}

impl From<ChannelVideoTab> for ChannelTab {
    fn from(value: ChannelVideoTab) -> Self {
        match value {
            ChannelVideoTab::Videos => Self::Videos,
            ChannelVideoTab::Shorts => Self::Shorts,
            ChannelVideoTab::Live => Self::Live,
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
        let request_body = QChannel {
            browse_id: channel_id,
            params,
            query,
        };

        self.execute_request::<response::Channel, _, _>(
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
        let request_body = QChannel {
            browse_id: channel_id,
            params: ChannelTab::Playlists,
            query: None,
        };

        self.execute_request::<response::Channel, _, _>(
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
        let request_body = QContinuation {
            continuation: &channel_info_ctoken(channel_id, &random_target()),
        };

        self.execute_request_ctx::<response::ChannelAbout, _, _>(
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

impl MapResponse<Channel<Paginator<VideoItem>>> for response::Channel {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> {
        let content = map_channel_content(ctx.id, self.contents, self.alerts)?;
        let visitor_data = self
            .response_context
            .visitor_data
            .or_else(|| ctx.visitor_data.map(str::to_owned));

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: visitor_data.clone(),
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            ctx,
        )?;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::with_channel(
            ctx.lang,
            &channel_data.c,
            channel_data.warnings,
        );
        mapper.map_response(content.content);
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
    }
}

impl MapResponse<Channel<Paginator<PlaylistItem>>> for response::Channel {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<PlaylistItem>>>, ExtractionError> {
        let content = map_channel_content(ctx.id, self.contents, self.alerts)?;
        let visitor_data = self
            .response_context
            .visitor_data
            .or_else(|| ctx.visitor_data.map(str::to_owned));

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data,
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            ctx,
        )?;

        let mut mapper = response::YouTubeListMapper::<PlaylistItem>::with_channel(
            ctx.lang,
            &channel_data.c,
            channel_data.warnings,
        );
        mapper.map_response(content.content);
        let p = Paginator::new(None, mapper.items, mapper.ctoken);

        Ok(MapResult {
            c: combine_channel_data(channel_data.c, p),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<ChannelInfo> for response::ChannelAbout {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<ChannelInfo>, ExtractionError> {
        // Channel info is always fetched in English. There is no localized data
        // and it allows parsing the country name.
        let lang = Language::En;

        let ep = match self {
            response::ChannelAbout::ReceivedEndpoints {
                on_response_received_endpoints,
            } => on_response_received_endpoints
                .into_iter()
                .next()
                .ok_or(ExtractionError::InvalidData("no received endpoint".into()))?,
            response::ChannelAbout::Content { contents } => {
                // Handle errors (e.g. age restriction) when regular channel content was returned
                map_channel_content(ctx.id, contents, None)?;
                return Err(ExtractionError::InvalidData(
                    "could not extract aboutData".into(),
                ));
            }
        };
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

        Ok(MapResult {
            c: ChannelInfo {
                id: about.channel_id,
                url: about.canonical_channel_url,
                description: about.description,
                subscriber_count: about
                    .subscriber_count_text
                    .and_then(|txt| util::parse_large_numstr_or_warn(&txt, lang, &mut warnings)),
                video_count: about
                    .video_count_text
                    .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                create_date: about.joined_date_text.and_then(|txt| {
                    timeago::parse_textual_date_or_warn(lang, ctx.utc_offset, &txt, &mut warnings)
                        .map(OffsetDateTime::date)
                }),
                view_count: about
                    .view_count_text
                    .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                country: about.country.and_then(|c| util::country_from_name(&c)),
                links,
            },
            warnings,
        })
    }
}

struct MapChannelData {
    header: Option<response::channel::Header>,
    metadata: Option<response::channel::Metadata>,
    microformat: Option<response::channel::Microformat>,
    visitor_data: Option<String>,
    has_shorts: bool,
    has_live: bool,
}

fn map_channel(
    d: MapChannelData,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<Channel<()>>, ExtractionError> {
    let header = d.header.ok_or_else(|| ExtractionError::NotFound {
        id: ctx.id.to_owned(),
        msg: "no header".into(),
    })?;
    let metadata = d
        .metadata
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no metadata".into(),
        })?
        .channel_metadata_renderer;
    let microformat = d.microformat.ok_or_else(|| ExtractionError::NotFound {
        id: ctx.id.to_owned(),
        msg: "no microformat".into(),
    })?;

    if metadata.external_id != ctx.id {
        return Err(ExtractionError::WrongResult(format!(
            "got wrong channel id {}, expected {}",
            metadata.external_id, ctx.id
        )));
    }

    let handle = metadata
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
        c: match header {
            response::channel::Header::C4TabbedHeaderRenderer(header) => Channel {
                id: metadata.external_id,
                name: metadata.title,
                handle,
                subscriber_count: header.subscriber_count_text.and_then(|txt| {
                    util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut warnings)
                }),
                video_count: None,
                avatar: header.avatar.into(),
                verification: header.badges.into(),
                description: metadata.description,
                tags: microformat.microformat_data_renderer.tags,
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
                    id: metadata.external_id,
                    name: metadata.title,
                    handle,
                    subscriber_count: hdata.as_ref().and_then(|hdata| {
                        hdata.0.as_ref().and_then(|txt| {
                            util::parse_large_numstr_or_warn(txt, ctx.lang, &mut warnings)
                        })
                    }),
                    video_count: None,
                    avatar: hdata.map(|hdata| hdata.1.into()).unwrap_or_default(),
                    // Since the carousel header is only used for YT-internal channels or special events
                    // (World Cup, Coachella, etc.) we can assume the channel to be verified
                    verification: crate::model::Verification::Verified,
                    description: metadata.description,
                    tags: microformat.microformat_data_renderer.tags,
                    banner: Vec::new(),
                    has_shorts: d.has_shorts,
                    has_live: d.has_live,
                    visitor_data: d.visitor_data,
                    content: (),
                }
            }
            response::channel::Header::PageHeaderRenderer(header) => {
                let hdata = header.content.page_header_view_model;
                // channel handle - subscriber count - video count
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
                let video_count =
                    vc_part.and_then(|t| util::parse_numeric_or_warn(t.as_str(), &mut warnings));

                Channel {
                    id: metadata.external_id,
                    name: metadata.title,
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
                    description: metadata.description,
                    tags: microformat.microformat_data_renderer.tags,
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

struct MappedChannelContent {
    content: MapResult<Vec<response::YouTubeListItem>>,
    has_shorts: bool,
    has_live: bool,
}

fn map_channel_content(
    id: &str,
    contents: Option<response::channel::Contents>,
    alerts: Option<Vec<response::Alert>>,
) -> Result<MappedChannelContent, ExtractionError> {
    match contents {
        Some(contents) => {
            let tabs = contents.two_column_browse_results_renderer.contents;
            let cmp_url_suffix = |endpoint: &response::channel::ChannelTabEndpoint,
                                  expect: &str| {
                endpoint
                    .command_metadata
                    .web_command_metadata
                    .url
                    .ends_with(expect)
            };

            let mut has_shorts = false;
            let mut has_live = false;
            let mut featured_tab = false;

            for tab in &tabs {
                if let Some(endpoint) = &tab.tab_renderer.endpoint {
                    if cmp_url_suffix(endpoint, "/featured")
                        && (tab.tab_renderer.content.section_list_renderer.is_some()
                            || tab.tab_renderer.content.rich_grid_renderer.is_some())
                    {
                        featured_tab = true;
                    } else if cmp_url_suffix(endpoint, "/shorts") {
                        has_shorts = true;
                    } else if cmp_url_suffix(endpoint, "/streams") {
                        has_live = true;
                    }
                } else {
                    // Check for age gate
                    if let Some(YouTubeListItem::ChannelAgeGateRenderer {
                        channel_title,
                        main_text,
                    }) = &tab
                        .tab_renderer
                        .content
                        .section_list_renderer
                        .as_ref()
                        .and_then(|c| c.contents.c.first())
                    {
                        return Err(ExtractionError::Unavailable {
                            reason: crate::error::UnavailabilityReason::AgeRestricted,
                            msg: format!("{channel_title}: {main_text}"),
                        });
                    }
                }
            }

            let channel_content = tabs
                .into_iter()
                .filter(|t| t.tab_renderer.endpoint.is_some())
                .find_map(|tab| {
                    tab.tab_renderer
                        .content
                        .rich_grid_renderer
                        .or(tab.tab_renderer.content.section_list_renderer)
                });

            // YouTube may show the "Featured" tab if the requested tab is empty/does not exist
            let content = if featured_tab {
                MapResult::default()
            } else {
                match channel_content {
                    Some(list) => list.contents,
                    None => {
                        return Err(ExtractionError::NotFound {
                            id: id.to_owned(),
                            msg: "no tabs".into(),
                        });
                    }
                }
            };

            Ok(MappedChannelContent {
                content,
                has_shorts,
                has_live,
            })
        }
        None => Err(response::alerts_to_err(id, alerts)),
    }
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
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapRespCtx, MapResponse},
        error::{ExtractionError, UnavailabilityReason},
        model::{paginator::Paginator, Channel, ChannelInfo, PlaylistItem, VideoItem},
        param::{ChannelOrder, ChannelVideoTab},
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    use super::{channel_info_ctoken, order_ctoken};

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
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<VideoItem>>> =
            channel.map_response(&MapRespCtx::test(id)).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let res: Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> =
            channel.map_response(&MapRespCtx::test("UCbfnHqxXs_K3kvaH-WlNlig"));
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
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<PlaylistItem>>> = channel
            .map_response(&MapRespCtx::test("UC2DjFE7Xf11URZqWBigcVOQ"))
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
        let json_file = File::open(json_path).unwrap();

        let channel: response::ChannelAbout =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<ChannelInfo> = channel
            .map_response(&MapRespCtx::test("UC2DjFE7Xf11U-RZqWBigcVOQ"))
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
