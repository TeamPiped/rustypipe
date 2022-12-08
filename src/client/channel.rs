use std::borrow::Cow;

use serde::Serialize;
use url::Url;

use crate::{
    error::{Error, ExtractionError},
    model::{Channel, ChannelInfo, Paginator, PlaylistItem, VideoItem, YouTubeItem},
    param::Language,
    serializer::MapResult,
    util,
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
    params: Params,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
}

#[derive(Debug, Serialize)]
enum Params {
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

impl RustyPipeQuery {
    async fn _channel_videos<S: AsRef<str>>(
        &self,
        channel_id: S,
        params: Params,
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

    pub async fn channel_videos<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, Params::Videos, None, "channel_videos")
            .await
    }

    pub async fn channel_shorts<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, Params::Shorts, None, "channel_shorts")
            .await
    }

    pub async fn channel_livestreams<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(channel_id, Params::Live, None, "channel_livestreams")
            .await
    }

    pub async fn channel_search<S: AsRef<str>, S2: AsRef<str>>(
        &self,
        channel_id: S,
        query: S2,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self._channel_videos(
            channel_id,
            Params::Search,
            Some(query.as_ref()),
            "channel_search",
        )
        .await
    }

    pub async fn channel_playlists<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<Paginator<PlaylistItem>>, Error> {
        let channel_id = channel_id.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: Params::Playlists,
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

    pub async fn channel_info<S: AsRef<str>>(
        &self,
        channel_id: S,
    ) -> Result<Channel<ChannelInfo>, Error> {
        let channel_id = channel_id.as_ref();
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: Params::Info,
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
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> {
        let content = map_channel_content(self.contents, self.alerts)?;

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: self.response_context.visitor_data.clone(),
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            id,
            lang,
        )?;

        let mut mapper =
            response::YouTubeListMapper::<VideoItem>::with_channel(lang, &channel_data);
        mapper.map_response(content.content);
        let p = Paginator::new_ext(
            None,
            mapper.items,
            mapper.ctoken,
            self.response_context.visitor_data,
            crate::param::ContinuationEndpoint::Browse,
        );

        Ok(MapResult {
            c: combine_channel_data(channel_data, p),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<Channel<Paginator<PlaylistItem>>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Channel<Paginator<PlaylistItem>>>, ExtractionError> {
        let content = map_channel_content(self.contents, self.alerts)?;

        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: self.response_context.visitor_data,
                has_shorts: content.has_shorts,
                has_live: content.has_live,
            },
            id,
            lang,
        )?;

        let mut mapper =
            response::YouTubeListMapper::<PlaylistItem>::with_channel(lang, &channel_data);
        mapper.map_response(content.content);
        let p = Paginator::new(None, mapper.items, mapper.ctoken);

        Ok(MapResult {
            c: combine_channel_data(channel_data, p),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<Channel<ChannelInfo>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Channel<ChannelInfo>>, ExtractionError> {
        let content = map_channel_content(self.contents, self.alerts)?;
        let channel_data = map_channel(
            MapChannelData {
                header: self.header,
                metadata: self.metadata,
                microformat: self.microformat,
                visitor_data: self.response_context.visitor_data,
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
            c: combine_channel_data(channel_data, cinfo),
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
        let _ = parsed_url.set_scheme("https");
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
) -> Result<Channel<()>, ExtractionError> {
    let header = d
        .header
        .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
            "channel not found",
        )))?;
    let metadata = d
        .metadata
        .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
            "channel not found",
        )))?
        .channel_metadata_renderer;
    let microformat = d
        .microformat
        .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
            "channel not found",
        )))?;

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

    Ok(match header {
        response::channel::Header::C4TabbedHeaderRenderer(header) => Channel {
            id: metadata.external_id,
            name: metadata.title,
            subscriber_count: header
                .subscriber_count_text
                .and_then(|txt| util::parse_large_numstr(&txt, lang)),
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
            let hdata = carousel
                .contents
                .into_iter()
                .filter_map(|item| {
                    match item {
                response::channel::CarouselHeaderRendererItem::TopicChannelDetailsRenderer {
                    subscriber_count_text,
                    avatar,
                } => Some((subscriber_count_text, avatar)),
                response::channel::CarouselHeaderRendererItem::None => None,
            }
                })
                .next();

            Channel {
                id: metadata.external_id,
                name: metadata.title,
                subscriber_count: hdata.as_ref().and_then(|hdata| {
                    hdata
                        .0
                        .as_ref()
                        .and_then(|txt| util::parse_large_numstr(txt, lang))
                }),
                avatar: hdata.map(|hdata| hdata.1.into()).unwrap_or_default(),
                verification: crate::model::Verification::None,
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
    })
}

struct MappedChannelContent {
    content: MapResult<Vec<response::YouTubeListItem>>,
    has_shorts: bool,
    has_live: bool,
}

fn map_channel_content(
    contents: Option<response::channel::Contents>,
    alerts: Option<Vec<response::Alert>>,
) -> Result<MappedChannelContent, ExtractionError> {
    match contents {
        Some(contents) => {
            let tabs = contents.two_column_browse_results_renderer.tabs;
            if tabs.is_empty() {
                return Err(ExtractionError::ContentUnavailable(
                    "channel not found".into(),
                ));
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

            let content = match channel_content {
                Some(list) => list.contents,
                None => {
                    // YouTube may show the "Featured" tab if the requested tab is empty/does not exist
                    if featured_tab {
                        MapResult::default()
                    } else {
                        return Err(ExtractionError::InvalidData(Cow::Borrowed(
                            "could not extract content",
                        )));
                    }
                }
            };

            Ok(MappedChannelContent {
                content,
                has_shorts,
                has_live,
            })
        }
        None => Err(response::alerts_to_err(alerts)),
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

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapResponse},
        model::{Channel, ChannelInfo, Paginator, PlaylistItem, VideoItem},
        param::Language,
        serializer::MapResult,
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
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::livestreams("livestreams", "UC2DjFE7Xf11URZqWBigcVOQ")]
    fn map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let json_path = path!("testfiles" / "channel" / format!("channel_{}.json", name));
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<VideoItem>>> =
            channel.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        if name == "videos_upcoming" {
            insta::assert_ron_snapshot!(format!("map_channel_{}", name), map_res.c, {
                ".content.items[1:].publish_date" => "[date]",
            });
        } else {
            insta::assert_ron_snapshot!(format!("map_channel_{}", name), map_res.c, {
                ".content.items[].publish_date" => "[date]",
            });
        }
    }

    #[test]
    fn map_channel_playlists() {
        let json_path = path!("testfiles" / "channel" / "channel_playlists.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<PlaylistItem>>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_playlists", map_res.c);
    }

    #[test]
    fn map_channel_info() {
        let json_path = path!("testfiles" / "channel" / "channel_info.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<ChannelInfo>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_info", map_res.c);
    }
}
