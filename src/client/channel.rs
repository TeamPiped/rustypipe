use std::borrow::Cow;

use serde::Serialize;
use time::OffsetDateTime;
use url::Url;

use crate::{
    error::{Error, ExtractionError},
    model::{Channel, ChannelInfo, Paginator, PlaylistItem, VideoItem},
    param::{ChannelOrder, Language},
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
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
    pub async fn channel_videos(
        &self,
        channel_id: &str,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        self.channel_videos_ordered(channel_id, ChannelOrder::default())
            .await
    }

    pub async fn channel_videos_ordered(
        &self,
        channel_id: &str,
        order: ChannelOrder,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: match order {
                ChannelOrder::Latest => Params::VideosLatest,
                ChannelOrder::Oldest => Params::VideosOldest,
                ChannelOrder::Popular => Params::VideosPopular,
            },
        };

        self.execute_request::<response::Channel, _, _>(
            ClientType::Desktop,
            "channel_videos",
            channel_id,
            "browse",
            &request_body,
        )
        .await
    }

    pub async fn channel_playlists(
        &self,
        channel_id: &str,
    ) -> Result<Channel<Paginator<PlaylistItem>>, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: Params::Playlists,
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

    pub async fn channel_info(&self, channel_id: &str) -> Result<Channel<ChannelInfo>, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
        let request_body = QChannel {
            context,
            browse_id: channel_id,
            params: Params::Info,
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
        let content = map_channel_content(self.contents, id, self.alerts)?;
        let grid = match content {
            response::channel::ChannelContent::GridRenderer { items } => Some(items),
            _ => None,
        };

        let v_res = grid.map(|g| map_videos(g, lang)).unwrap_or_default();

        Ok(MapResult {
            c: map_channel(
                self.header,
                self.metadata,
                self.microformat,
                v_res.c,
                id,
                lang,
            )?,
            warnings: v_res.warnings,
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
        let content = map_channel_content(self.contents, id, self.alerts)?;
        let grid = match content {
            response::channel::ChannelContent::GridRenderer { items } => Some(items),
            _ => None,
        };

        let p_res = grid
            .map(|item| map_playlists(item, lang))
            .unwrap_or_default();

        Ok(MapResult {
            c: map_channel(
                self.header,
                self.metadata,
                self.microformat,
                p_res.c,
                id,
                lang,
            )?,
            warnings: p_res.warnings,
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
        let content = map_channel_content(self.contents, id, self.alerts)?;
        let mut warnings = Vec::new();
        let meta = match content {
            response::channel::ChannelContent::ChannelAboutFullMetadataRenderer(meta) => Some(meta),
            _ => None,
        };

        let cinfo = meta
            .map(|meta| ChannelInfo {
                create_date: timeago::parse_textual_date_or_warn(
                    lang,
                    &meta.joined_date_text,
                    &mut warnings,
                )
                .map(OffsetDateTime::date),
                view_count: meta
                    .view_count_text
                    .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                links: meta
                    .primary_links
                    .into_iter()
                    .filter_map(|l| {
                        l.navigation_endpoint
                            .url_endpoint
                            .map(|url| (l.title, util::sanitize_yt_url(&url.url)))
                    })
                    .collect(),
            })
            .unwrap_or_else(|| {
                warnings.push("no aboutFullMetadata".to_owned());
                ChannelInfo {
                    create_date: None,
                    view_count: None,
                    links: Vec::new(),
                }
            });

        Ok(MapResult {
            c: map_channel(
                self.header,
                self.metadata,
                self.microformat,
                cinfo,
                id,
                lang,
            )?,
            warnings,
        })
    }
}

fn map_videos(
    res: MapResult<Vec<response::YouTubeListItem>>,
    lang: Language,
) -> MapResult<Paginator<VideoItem>> {
    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
    mapper.map_response(res);

    MapResult {
        c: Paginator::new(None, mapper.items, mapper.ctoken),
        warnings: mapper.warnings,
    }
}

fn map_playlists(
    res: MapResult<Vec<response::YouTubeListItem>>,
    lang: Language,
) -> MapResult<Paginator<PlaylistItem>> {
    let mut mapper = response::YouTubeListMapper::<PlaylistItem>::new(lang);
    mapper.map_response(res);

    MapResult {
        c: Paginator::new(None, mapper.items, mapper.ctoken),
        warnings: mapper.warnings,
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

fn map_channel<T>(
    header: Option<response::channel::Header>,
    metadata: Option<response::channel::Metadata>,
    microformat: Option<response::channel::Microformat>,
    content: T,
    id: &str,
    lang: Language,
) -> Result<Channel<T>, ExtractionError> {
    let header = header.ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
        "channel not found",
    )))?;
    let metadata = metadata
        .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
            "channel not found",
        )))?
        .channel_metadata_renderer;
    let microformat = microformat.ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
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
            content,
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
                content,
            }
        }
    })
}

fn map_channel_content(
    contents: Option<response::channel::Contents>,
    id: &str,
    alerts: Option<Vec<response::Alert>>,
) -> Result<response::channel::ChannelContent, ExtractionError> {
    match contents {
        Some(contents) => {
            let tabs = contents.two_column_browse_results_renderer.tabs;
            if tabs.is_empty() {
                return Err(ExtractionError::ContentUnavailable(
                    "channel not found".into(),
                ));
            }

            let (channel_content, target_id) = tabs
                .into_iter()
                .filter_map(|tab| {
                    let content = tab.tab_renderer.content;
                    match (content.section_list_renderer, content.rich_grid_renderer) {
                        (Some(mut section_list_renderer), _) => {
                            let content =
                                section_list_renderer.contents.try_swap_remove(0).and_then(
                                    |mut i| i.item_section_renderer.contents.try_swap_remove(0),
                                );

                            content.map(|c| (c, section_list_renderer.target_id))
                        }
                        (None, Some(rich_grid_renderer)) => Some((
                            response::channel::ChannelContent::GridRenderer {
                                items: rich_grid_renderer.contents,
                            },
                            rich_grid_renderer.target_id,
                        )),
                        (None, None) => None,
                    }
                })
                .next()
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "could not extract content",
                )))?;

            if let Some(target_id) = target_id {
                // YouTube falls back to the featured page if the channel does not have a "videos" tab.
                // This is the case for YouTube Music channels.
                if target_id.starts_with(&format!("browse-feed{}featured", id)) {
                    return Ok(response::channel::ChannelContent::None);
                }
            }

            Ok(channel_content)
        }
        None => Err(response::alerts_to_err(alerts)),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use crate::{
        client::{response, MapResponse},
        model::{Channel, ChannelInfo, Paginator, PlaylistItem, VideoItem},
        param::Language,
        serializer::MapResult,
    };

    #[rstest]
    #[case::base("base", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::music("music", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::live("live", "UChs0pSaEoNLV4mevBFGaoKA")]
    #[case::empty("empty", "UCxBa895m48H5idw5li7h-0g")]
    #[case::upcoming("upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A")]
    #[case::richgrid("20221011_richgrid", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::richgrid2("20221011_richgrid2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    fn map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/channel/channel_videos_{}.json", name);
        let json_path = Path::new(&filename);
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

        if name == "upcoming" {
            insta::assert_ron_snapshot!(format!("map_channel_videos_{}", name), map_res.c, {
                ".content.items[1:].publish_date" => "[date]",
            });
        } else {
            insta::assert_ron_snapshot!(format!("map_channel_videos_{}", name), map_res.c, {
                ".content.items[].publish_date" => "[date]",
            });
        }
    }

    #[test]
    fn map_channel_playlists() {
        let json_path = Path::new("testfiles/channel/channel_playlists.json");
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
        let json_path = Path::new("testfiles/channel/channel_info.json");
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
