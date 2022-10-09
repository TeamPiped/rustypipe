use chrono::TimeZone;
use serde::Serialize;
use url::Url;

use crate::{
    error::{Error, ExtractionError},
    model::{
        Channel, ChannelInfo, ChannelOrder, ChannelPlaylist, ChannelVideo, Language, Paginator,
    },
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IsLive, IsShort},
    ClientType, MapResponse, QContinuation, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QChannel<'a> {
    context: YTContext,
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
    ) -> Result<Channel<Paginator<ChannelVideo>>, Error> {
        self.channel_videos_ordered(channel_id, ChannelOrder::default())
            .await
    }

    pub async fn channel_videos_ordered(
        &self,
        channel_id: &str,
        order: ChannelOrder,
    ) -> Result<Channel<Paginator<ChannelVideo>>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
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

    pub async fn channel_videos_continuation(
        &self,
        ctoken: &str,
    ) -> Result<Paginator<ChannelVideo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::ChannelCont, _, _>(
            ClientType::Desktop,
            "channel_videos_continuation",
            ctoken,
            "browse",
            &request_body,
        )
        .await
    }

    pub async fn channel_playlists(
        &self,
        channel_id: &str,
    ) -> Result<Channel<Paginator<ChannelPlaylist>>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
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

    pub async fn channel_playlists_continuation(
        &self,
        ctoken: &str,
    ) -> Result<Paginator<ChannelPlaylist>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::ChannelCont, _, _>(
            ClientType::Desktop,
            "channel_videos_continuation",
            ctoken,
            "browse",
            &request_body,
        )
        .await
    }

    pub async fn channel_info(&self, channel_id: &str) -> Result<Channel<ChannelInfo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
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

impl MapResponse<Channel<Paginator<ChannelVideo>>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Channel<Paginator<ChannelVideo>>>, ExtractionError> {
        let content = map_channel_content(self.contents, id);
        let mut warnings = content.warnings;
        let grid = match content.c {
            response::channel::ChannelContent::GridRenderer { items } => Some(items),
            _ => None,
        };

        let mut v_res = grid.map(|g| map_videos(g, lang)).unwrap_or_default();
        warnings.append(&mut v_res.warnings);

        Ok(MapResult {
            c: map_channel(
                self.header,
                self.metadata,
                self.microformat,
                v_res.c,
                id,
                lang,
            )?,
            warnings,
        })
    }
}

impl MapResponse<Channel<Paginator<ChannelPlaylist>>> for response::Channel {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Channel<Paginator<ChannelPlaylist>>>, ExtractionError> {
        let content = map_channel_content(self.contents, id);
        let mut warnings = content.warnings;
        let grid = match content.c {
            response::channel::ChannelContent::GridRenderer { items } => Some(items),
            _ => None,
        };

        let mut p_res = grid.map(map_playlists).unwrap_or_default();
        warnings.append(&mut p_res.warnings);

        Ok(MapResult {
            c: map_channel(
                self.header,
                self.metadata,
                self.microformat,
                p_res.c,
                id,
                lang,
            )?,
            warnings,
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
        let content = map_channel_content(self.contents, id);
        let mut warnings = content.warnings;
        let meta = match content.c {
            response::channel::ChannelContent::ChannelAboutFullMetadataRenderer(meta) => Some(meta),
            _ => None,
        };

        let cinfo = meta
            .map(|meta| ChannelInfo {
                create_date: timeago::parse_textual_date_or_warn(
                    lang,
                    &meta.joined_date_text,
                    &mut warnings,
                ),
                view_count: util::parse_numeric_or_warn(&meta.view_count_text, &mut warnings),
                links: meta
                    .primary_links
                    .into_iter()
                    .map(|l| {
                        (
                            l.title,
                            util::sanitize_yt_url(&l.navigation_endpoint.url_endpoint.url),
                        )
                    })
                    .collect(),
            })
            .unwrap_or_else(|| {
                warnings.push("no metadata".to_owned());
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

impl MapResponse<Paginator<ChannelVideo>> for response::ChannelCont {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<ChannelVideo>>, ExtractionError> {
        let mut actions = self.on_response_received_actions;
        let res = some_or_bail!(
            actions.try_swap_remove(0),
            Err(ExtractionError::InvalidData("no received action".into()))
        )
        .append_continuation_items_action
        .continuation_items;

        Ok(map_videos(res, lang))
    }
}

impl MapResponse<Paginator<ChannelPlaylist>> for response::ChannelCont {
    fn map_response(
        self,
        _id: &str,
        _lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<ChannelPlaylist>>, ExtractionError> {
        let mut actions = self.on_response_received_actions;
        let res = some_or_bail!(
            actions.try_swap_remove(0),
            Err(ExtractionError::InvalidData("no received action".into()))
        )
        .append_continuation_items_action
        .continuation_items;

        Ok(map_playlists(res))
    }
}

fn map_videos(
    res: MapResult<Vec<response::VideoListItem>>,
    lang: Language,
) -> MapResult<Paginator<ChannelVideo>> {
    let mut warnings = res.warnings;

    let mut ctoken = None;
    let videos = res
        .c
        .into_iter()
        .filter_map(|item| match item {
            response::VideoListItem::GridVideoRenderer(video) => {
                let mut toverlays = video.thumbnail_overlays;
                let is_live = toverlays.is_live();
                let is_short = toverlays.is_short();
                let to = toverlays.try_swap_remove(0);

                Some(ChannelVideo {
                    id: video.video_id,
                    title: video.title,
                    // Time text is `LIVE` for livestreams, so we ignore parse errors
                    length: to.and_then(|to| {
                        util::parse_video_length(&to.thumbnail_overlay_time_status_renderer.text)
                    }),
                    thumbnail: video.thumbnail.into(),
                    publish_date: video
                        .upcoming_event_data
                        .as_ref()
                        .map(|upc| {
                            chrono::Local.from_utc_datetime(&chrono::NaiveDateTime::from_timestamp(
                                upc.start_time,
                                0,
                            ))
                        })
                        .or_else(|| {
                            video.published_time_text.as_ref().and_then(|txt| {
                                timeago::parse_timeago_or_warn(lang, txt, &mut warnings)
                            })
                        }),
                    publish_date_txt: video.published_time_text,
                    view_count: video
                        .view_count_text
                        .map(|txt| util::parse_numeric(&txt).unwrap_or_default()),
                    is_live,
                    is_short,
                    is_upcoming: video.upcoming_event_data.is_some(),
                })
            }
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            _ => None,
        })
        .collect();

    MapResult {
        c: Paginator::new(None, videos, ctoken),
        warnings,
    }
}

fn map_playlists(
    res: MapResult<Vec<response::VideoListItem>>,
) -> MapResult<Paginator<ChannelPlaylist>> {
    let mut ctoken = None;
    let playlists = res
        .c
        .into_iter()
        .filter_map(|item| match item {
            response::VideoListItem::GridPlaylistRenderer(playlist) => Some(ChannelPlaylist {
                id: playlist.playlist_id,
                name: playlist.title,
                thumbnail: playlist.thumbnail.into(),
                video_count: util::parse_numeric(&playlist.video_count_short_text).ok(),
            }),
            response::VideoListItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                ctoken = Some(continuation_endpoint.continuation_command.token);
                None
            }
            _ => None,
        })
        .collect();

    MapResult {
        c: Paginator::new(None, playlists, ctoken),
        warnings: res.warnings,
    }
}

fn map_vanity_url(url: &str, id: &str) -> Option<String> {
    if url.contains(id) {
        return None;
    }

    let mut parsed_url = ok_or_bail!(Url::parse(url), None);

    // The vanity URL from YouTube is http for some reason
    let _ = parsed_url.set_scheme("https");
    Some(parsed_url.to_string())
}

fn map_channel<T>(
    header: response::channel::Header,
    metadata: response::channel::Metadata,
    microformat: response::channel::Microformat,
    content: T,
    id: &str,
    lang: Language,
) -> Result<Channel<T>, ExtractionError> {
    let header = header.c4_tabbed_header_renderer;

    if header.channel_id != id {
        return Err(ExtractionError::WrongResult(format!(
            "got wrong channel id {}, expected {}",
            header.channel_id, id
        )));
    }

    Ok(Channel {
        id: header.channel_id,
        name: header.title,
        subscriber_count: header
            .subscriber_count_text
            .and_then(|txt| util::parse_large_numstr(&txt, lang)),
        avatar: header.avatar.into(),
        description: metadata.channel_metadata_renderer.description,
        tags: microformat.microformat_data_renderer.tags,
        vanity_url: metadata
            .channel_metadata_renderer
            .vanity_channel_url
            .as_ref()
            .and_then(|url| map_vanity_url(url, id)),
        banner: header.banner.into(),
        mobile_banner: header.mobile_banner.into(),
        tv_banner: header.tv_banner.into(),
        content,
    })
}

fn map_channel_content(
    contents: response::channel::Contents,
    id: &str,
) -> MapResult<response::channel::ChannelContent> {
    let mut tabs = contents.two_column_browse_results_renderer.tabs;
    let mut sectionlist = some_or_bail!(
        tabs.try_swap_remove(0),
        MapResult::error("no tab".to_owned())
    )
    .tab_renderer
    .content
    .section_list_renderer;

    if let Some(target_id) = sectionlist.target_id {
        // YouTube falls back to the featured page if the channel does not have a "videos" tab.
        // This is the case for YouTube Music channels.
        if target_id.starts_with(&format!("browse-feed{}featured", id)) {
            return MapResult::ok(response::channel::ChannelContent::None);
        }
    }

    let mut itemsection = some_or_bail!(
        sectionlist.contents.try_swap_remove(0),
        MapResult::error("no sectionlist".to_owned())
    )
    .item_section_renderer
    .contents;

    let content = some_or_bail!(
        itemsection.try_swap_remove(0),
        MapResult::error("no channel content".to_owned())
    );

    MapResult::ok(content)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use chrono::Datelike;
    use rstest::rstest;

    use crate::{
        client::{response, MapResponse, RustyPipe},
        model::{
            Channel, ChannelInfo, ChannelOrder, ChannelPlaylist, ChannelVideo, Language, Paginator,
        },
        serializer::MapResult,
    };

    #[rstest]
    #[case::latest(ChannelOrder::Latest)]
    #[case::oldest(ChannelOrder::Oldest)]
    #[case::popular(ChannelOrder::Popular)]
    #[tokio::test]
    async fn get_videos(#[case] order: ChannelOrder) {
        let rp = RustyPipe::builder().strict().build();
        let channel = rp
            .query()
            .channel_videos_ordered("UC2DjFE7Xf11URZqWBigcVOQ", order)
            .await
            .unwrap();

        // dbg!(&channel);
        check_channel(&channel);

        assert!(
            !channel.content.items.is_empty() && !channel.content.is_exhausted(),
            "got no videos"
        );

        let first_video = &channel.content.items[0];
        let first_video_date = first_video.publish_date.unwrap();
        let age_days = (chrono::Local::now() - first_video_date).num_days();

        match order {
            ChannelOrder::Latest => {
                assert!(age_days < 60, "latest video older than 60 days")
            }
            ChannelOrder::Oldest => {
                assert!(age_days > 4700, "oldest video newer than 4700 days")
            }
            ChannelOrder::Popular => {
                assert!(
                    first_video.view_count.unwrap() > 2300000,
                    "most popular video < 2.3M views"
                )
            }
        }

        let next = channel.content.next(rp.query()).await.unwrap().unwrap();
        assert!(
            !next.is_exhausted() && !next.items.is_empty(),
            "no more videos"
        );
    }

    #[tokio::test]
    async fn get_playlists() {
        let rp = RustyPipe::builder().strict().build();
        let channel = rp
            .query()
            .channel_playlists("UC2DjFE7Xf11URZqWBigcVOQ")
            .await
            .unwrap();

        // dbg!(&channel);
        check_channel(&channel);

        assert!(
            !channel.content.items.is_empty() && !channel.content.is_exhausted(),
            "got no playlists"
        );

        let next = channel.content.next(rp.query()).await.unwrap().unwrap();
        assert!(
            !next.is_exhausted() && !next.items.is_empty(),
            "no more playlists"
        );
    }

    #[tokio::test]
    async fn get_info() {
        let rp = RustyPipe::builder().strict().build();
        let channel = rp
            .query()
            .channel_info("UC2DjFE7Xf11URZqWBigcVOQ")
            .await
            .unwrap();

        dbg!(&channel);
        check_channel(&channel);

        let created = channel.content.create_date.unwrap();
        assert_eq!(created.year(), 2009);
        assert_eq!(created.month(), 4);
        assert_eq!(created.day(), 4);

        assert!(
            channel.content.view_count.unwrap() > 186854340,
            "exp >186M views, got {}",
            channel.content.view_count.unwrap()
        );

        insta::assert_ron_snapshot!(channel.content.links, @r###"
        [
          ("EEVblog Web Site", "http://www.eevblog.com/"),
          ("Twitter", "http://www.twitter.com/eevblog"),
          ("Facebook", "http://www.facebook.com/EEVblog"),
          ("EEVdiscover", "https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ"),
          ("The EEVblog Forum", "http://www.eevblog.com/forum"),
          ("EEVblog Merchandise (T-Shirts)", "http://www.eevblog.com/merch"),
          ("EEVblog Donations", "http://www.eevblog.com/donations/"),
          ("Patreon", "https://www.patreon.com/eevblog"),
          ("SubscribeStar", "https://www.subscribestar.com/eevblog"),
          ("The AmpHour Radio Show", "http://www.theamphour.com/"),
          ("Flickr", "http://www.flickr.com/photos/eevblog"),
          ("EEVblog AMAZON Store", "http://www.amazon.com/gp/redirect.html?ie=UTF8&location=http%3A%2F%2Fwww.amazon.com%2F&tag=ee04-20&linkCode=ur2&camp=1789&creative=390957"),
          ("2nd EEVblog Channel", "http://www.youtube.com/EEVblog2"),
        ]
        "###);
    }

    fn check_channel<T>(channel: &Channel<T>) {
        assert_eq!(channel.id, "UC2DjFE7Xf11URZqWBigcVOQ");
        assert_eq!(channel.name, "EEVblog");
        assert!(
            channel.subscriber_count.unwrap() > 880000,
            "exp >880K subscribers, got {}",
            channel.subscriber_count.unwrap()
        );
        assert!(!channel.avatar.is_empty(), "got no thumbnails");
        assert_eq!(channel.description, "NO SCRIPT, NO FEAR, ALL OPINION\nAn off-the-cuff Video Blog about Electronics Engineering, for engineers, hobbyists, enthusiasts, hackers and Makers\nHosted by Dave Jones from Sydney Australia\n\nDONATIONS:\nBitcoin: 3KqyH1U3qrMPnkLufM2oHDU7YB4zVZeFyZ\nEthereum: 0x99ccc4d2654ba40744a1f678d9868ecb15e91206\nPayPal: david@alternatezone.com\n\nPatreon: https://www.patreon.com/eevblog\n\nEEVblog2: http://www.youtube.com/EEVblog2\nEEVdiscover: https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ\n\nEMAIL:\nAdvertising/Commercial: eevblog+business@gmail.com\nFan mail: eevblog+fan@gmail.com\nHate Mail: eevblog+hate@gmail.com\n\nI DON'T DO PAID VIDEO SPONSORSHIPS, DON'T ASK!\n\nPLEASE:\nDo NOT ask for personal advice on something, post it in the EEVblog forum.\nI read ALL email, but please don't be offended if I don't have time to reply, I get a LOT of email.\n\nMailbag\nPO Box 7949\nBaulkham Hills NSW 2153\nAUSTRALIA");
        assert!(!channel.tags.is_empty(), "got no tags");
        assert_eq!(
            channel.vanity_url.as_ref().unwrap(),
            "https://www.youtube.com/c/EevblogDave"
        );
        assert!(!channel.banner.is_empty(), "got no banners");
        assert!(!channel.mobile_banner.is_empty(), "got no mobile banners");
        assert!(!channel.tv_banner.is_empty(), "got no tv banners");
    }

    #[rstest]
    #[case::base("base", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::music("music", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::live("live", "UChs0pSaEoNLV4mevBFGaoKA")]
    #[case::empty("empty", "UCxBa895m48H5idw5li7h-0g")]
    #[case::upcoming("upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A")]
    fn t_map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/channel/channel_videos_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<ChannelVideo>>> =
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
    fn t_map_channel_videos_cont() {
        let json_path = Path::new("testfiles/channel/channel_videos_cont.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::ChannelCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<ChannelVideo>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_videos_cont", map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn t_map_channel_playlists() {
        let json_path = Path::new("testfiles/channel/channel_playlists.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::Channel =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Channel<Paginator<ChannelPlaylist>>> = channel
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
    fn t_map_channel_playlists_cont() {
        let json_path = Path::new("testfiles/channel/channel_playlists_cont.json");
        let json_file = File::open(json_path).unwrap();

        let channel: response::ChannelCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<ChannelPlaylist>> = channel
            .map_response("UC2DjFE7Xf11URZqWBigcVOQ", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_playlists_cont", map_res.c);
    }

    #[test]
    fn t_map_channel_info() {
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
