use serde::Serialize;
use url::Url;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        Channel, ChannelInfo, PlaylistItem, VideoItem, YouTubeItem,
    },
    param::{ChannelOrder, ChannelVideoTab, Language},
    serializer::MapResult,
    util::{self, ProtoBuilder},
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel<'a> {
    context: YTContext<'a>,
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
    #[serde(rename = "EgVhYm91dPIGBAoCEgA%3D")]
    Info,
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
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
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
    pub async fn channel_videos<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, ChannelTab::Videos, None, "channel_videos")
            .await
    }

    /// Get a ordered list of videos from a YouTube channel
    ///
    /// This function does not return channel metadata.
    pub async fn channel_videos_order<S: AsRef<str>>(
        &self,
        channel_id: S,
        order: ChannelOrder,
    ) -> Result<Paginator<VideoItem>, Error> {
        self.channel_videos_tab_order(channel_id, ChannelVideoTab::Videos, order)
            .await
    }

    /// Get the videos of the given tab (Shorts, Livestreams) from a YouTube channel
    pub async fn channel_videos_tab<S: AsRef<str>>(
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
    pub async fn channel_videos_tab_order<S: AsRef<str>>(
        &self,
        channel_id: S,
        tab: ChannelVideoTab,
        order: ChannelOrder,
    ) -> Result<Paginator<VideoItem>, Error> {
        let visitor_data = match tab {
            ChannelVideoTab::Shorts => Some(self.get_visitor_data().await?),
            _ => None,
        };

        self.continuation(
            order_ctoken(channel_id.as_ref(), tab, order),
            ContinuationEndpoint::Browse,
            visitor_data.as_deref(),
        )
        .await
    }

    /// Search the videos of a channel
    pub async fn channel_search<S: AsRef<str>, S2: AsRef<str>>(
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
    pub async fn channel_playlists<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<PlaylistItem>>, Error> {
        let channel_id = channel_id.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
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
    pub async fn channel_info<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<ChannelInfo>, Error> {
        let channel_id = channel_id.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: ChannelTab::Info,
            query: None,
        };

        self.execute_request::<response::Channel, _, _>(
            ClientType::Desktop,
            "channel_info",
            channel_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<Channel<Paginator<VideoItem>>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
        vdata: Option<&str>,
    ) -> Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> {
        let content = map_channel_content(id, self.contents, self.alerts)?;
        let visitor_data = self
            .response_context
            .visitor_data
            .or_else(|| vdata.map(str::to_owned));

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: visitor_data.clone(),
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            id,
            lang,
        )?;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::with_channel(
            lang,
            &channel_data.c,
            channel_data.warnings,
        );
        mapper.map_response(content.content);
        let p = Paginator::new_ext(
            None,
            mapper.items,
            mapper.ctoken,
            visitor_data,
            crate::model::paginator::ContinuationEndpoint::Browse,
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
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
        vdata: Option<&str>,
    ) -> Result<MapResult<Channel<Paginator<PlaylistItem>>>, ExtractionError> {
        let content = map_channel_content(id, self.contents, self.alerts)?;
        let visitor_data = self
            .response_context
            .visitor_data
            .or_else(|| vdata.map(str::to_owned));

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data,
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            id,
            lang,
        )?;

        let mut mapper = response::YouTubeListMapper::<PlaylistItem>::with_channel(
            lang,
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

impl MapResponse<Channel<ChannelInfo>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
        vdata: Option<&str>,
    ) -> Result<MapResult<Channel<ChannelInfo>>, ExtractionError> {
        let content = map_channel_content(id, self.contents, self.alerts)?;
        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: self
                    .response_context
                    .visitor_data
                    .or_else(|| vdata.map(str::to_owned)),
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            id,
            lang,
        )?;

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(lang);
        mapper.map_response(content.content);
        let mut warnings = mapper.warnings;

        let cinfo = mapper.channel_info.unwrap_or_else(|| {
            warnings.push("no aboutFullMetadata".to_owned());
            ChannelInfo {
                create_date: None,
                view_count: None,
                links: Vec::new(),
            }
        });

        Ok(MapResult {
            c: combine_channel_data(channel_data.c, cinfo),
            warnings,
        })
    }
}

fn map_vanity_url(url: &str, id: &str) -> Option<String> {
    if url.contains(id) {
        return None;
    }

    Url::parse(url).ok().map(|mut parsed_url| {
        // The vanity URL from YouTube is http for some reason
        _ = parsed_url.set_scheme("https");
        parsed_url.to_string()
    })
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
    id: &str,
    lang: Language,
) -> Result<MapResult<Channel<()>>, ExtractionError> {
    let header = d.header.ok_or_else(|| ExtractionError::NotFound {
        id: id.to_owned(),
        msg: "no header".into(),
    })?;
    let metadata = d
        .metadata
        .ok_or_else(|| ExtractionError::NotFound {
            id: id.to_owned(),
            msg: "no metadata".into(),
        })?
        .channel_metadata_renderer;
    let microformat = d.microformat.ok_or_else(|| ExtractionError::NotFound {
        id: id.to_owned(),
        msg: "no microformat".into(),
    })?;

    if metadata.external_id != id {
        return Err(ExtractionError::WrongResult(format!(
            "got wrong channel id {}, expected {}",
            metadata.external_id, id
        )));
    }

    let vanity_url = metadata
        .vanity_channel_url
        .as_ref()
        .and_then(|url| map_vanity_url(url, id));
    let mut warnings = Vec::new();

    Ok(MapResult {
        c: match header {
            response::channel::Header::C4TabbedHeaderRenderer(header) => Channel {
                id: metadata.external_id,
                name: metadata.title,
                subscriber_count: header
                    .subscriber_count_text
                    .and_then(|txt| util::parse_large_numstr_or_warn(&txt, lang, &mut warnings)),
                avatar: header.avatar.into(),
                verification: header.badges.into(),
                description: metadata.description,
                tags: microformat.microformat_data_renderer.tags,
                vanity_url,
                banner: header.banner.into(),
                mobile_banner: header.mobile_banner.into(),
                tv_banner: header.tv_banner.into(),
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
                    subscriber_count: hdata.as_ref().and_then(|hdata| {
                        hdata.0.as_ref().and_then(|txt| {
                            util::parse_large_numstr_or_warn(txt, lang, &mut warnings)
                        })
                    }),
                    avatar: hdata.map(|hdata| hdata.1.into()).unwrap_or_default(),
                    verification: crate::model::Verification::Verified,
                    description: metadata.description,
                    tags: microformat.microformat_data_renderer.tags,
                    vanity_url,
                    banner: Vec::new(),
                    mobile_banner: Vec::new(),
                    tv_banner: Vec::new(),
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
            if tabs.is_empty() {
                return Err(ExtractionError::NotFound {
                    id: id.to_owned(),
                    msg: "no tabs".into(),
                });
            }

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
                if cmp_url_suffix(&tab.tab_renderer.endpoint, "/featured")
                    && (tab.tab_renderer.content.section_list_renderer.is_some()
                        || tab.tab_renderer.content.rich_grid_renderer.is_some())
                {
                    featured_tab = true;
                } else if cmp_url_suffix(&tab.tab_renderer.endpoint, "/shorts") {
                    has_shorts = true;
                } else if cmp_url_suffix(&tab.tab_renderer.endpoint, "/streams") {
                    has_live = true;
                }
            }

            let channel_content = tabs.into_iter().find_map(|tab| {
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
                        return Err(ExtractionError::InvalidData(
                            "could not extract content".into(),
                        ))
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
        subscriber_count: channel_data.subscriber_count,
        avatar: channel_data.avatar,
        verification: channel_data.verification,
        description: channel_data.description,
        tags: channel_data.tags,
        vanity_url: channel_data.vanity_url,
        banner: channel_data.banner,
        mobile_banner: channel_data.mobile_banner,
        tv_banner: channel_data.tv_banner,
        has_shorts: channel_data.has_shorts,
        has_live: channel_data.has_live,
        visitor_data: channel_data.visitor_data,
        content,
    }
}

/// Get the continuation token to fetch channel videos in the given order
fn order_ctoken(channel_id: &str, tab: ChannelVideoTab, order: ChannelOrder) -> String {
    _order_ctoken(
        channel_id,
        tab,
        order,
        &format!("\n${}", util::random_uuid()),
    )
}

/// Get the continuation token to fetch channel videos in the given order
/// (fixed targetId for testing)
fn _order_ctoken(
    channel_id: &str,
    tab: ChannelVideoTab,
    order: ChannelOrder,
    target_id: &str,
) -> String {
    let mut pb_tab = ProtoBuilder::new();
    pb_tab.string(2, target_id);
    pb_tab.varint(3, order as u64);

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

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapResponse},
        model::{paginator::Paginator, Channel, ChannelInfo, PlaylistItem, VideoItem},
        param::{ChannelOrder, ChannelVideoTab, Language},
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    use super::_order_ctoken;

    #[rstest]
    #[case::base("videos_base", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::music("videos_music", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::withshorts("videos_shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::live("videos_live", "UChs0pSaEoNLV4mevBFGaoKA")]
    #[case::empty("videos_empty", "UCxBa895m48H5idw5li7h-0g")]
    #[case::upcoming("videos_upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A")]
    #[case::richgrid("videos_20221011_richgrid", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::richgrid2("videos_20221011_richgrid2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::richgrid2("videos_20230415_coachella", "UCHF66aWLOxBW4l6VkSrS3cQ")]
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::livestreams("livestreams", "UC2DjFE7Xf11URZqWBigcVOQ")]
    fn map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<VideoItem>>> =
            channel.map_response(id, Language::En, None, None).unwrap();

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

    #[rstest]
    fn map_channel_playlists() {
        let json_path = path!(*TESTFILES / "channel" / "channel_playlists.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<PlaylistItem>>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_playlists", map_res.c);
    }

    #[rstest]
    fn map_channel_info() {
        let json_path = path!(*TESTFILES / "channel" / "channel_info.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<ChannelInfo>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_info", map_res.c);
    }

    #[test]
    fn order_ctoken() {
        let channel_id = "UCXuqSBlHAE6Xw-yeJA0Tunw";

        let videos_popular_token = _order_ctoken(
            channel_id,
            ChannelVideoTab::Videos,
            ChannelOrder::Popular,
            "\n$6461d7c8-0000-2040-87aa-089e0827e420",
        );
        assert_eq!(videos_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXg2S2hJbUNpUTJORFl4WkRkak9DMHdNREF3TFRJd05EQXRPRGRoWVMwd09EbGxNRGd5TjJVME1qQVlBZyUzRCUzRA%3D%3D");

        let shorts_popular_token = _order_ctoken(
            channel_id,
            ChannelVideoTab::Shorts,
            ChannelOrder::Popular,
            "\n$64679ffb-0000-26b3-a1bd-582429d2c794",
        );
        assert_eq!(shorts_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXhTS2hJbUNpUTJORFkzT1dabVlpMHdNREF3TFRJMllqTXRZVEZpWkMwMU9ESTBNamxrTW1NM09UUVlBZyUzRCUzRA%3D%3D");

        let live_popular_token = _order_ctoken(
            channel_id,
            ChannelVideoTab::Live,
            ChannelOrder::Popular,
            "\n$64693069-0000-2a1e-8c7d-582429bd5ba8",
        );
        assert_eq!(live_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXh5S2hJbUNpUTJORFk1TXpBMk9TMHdNREF3TFRKaE1XVXRPR00zWkMwMU9ESTBNamxpWkRWaVlUZ1lBZyUzRCUzRA%3D%3D");
    }
}
