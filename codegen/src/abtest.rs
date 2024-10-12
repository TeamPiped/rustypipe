use std::collections::BTreeMap;

use anyhow::{bail, Result};
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use num_enum::TryFromPrimitive;
use once_cell::sync::Lazy;
use regex::Regex;
use rustypipe::client::{ClientType, RustyPipe, RustyPipeQuery, YTContext};
use rustypipe::model::{MusicItem, YouTubeItem};
use rustypipe::param::search_filter::{ItemType, SearchFilter};
use rustypipe::param::ChannelVideoTab;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::model::QCont;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, Serialize, Deserialize,
)]
#[repr(u16)]
pub enum ABTest {
    AttributedTextDescription = 1,
    ThreeTabChannelLayout = 2,
    ChannelHandlesInSearchResults = 3,
    TrendsVideoTab = 4,
    TrendsPageHeaderRenderer = 5,
    DiscographyPage = 6,
    ShortDateFormat = 7,
    TrackViewcount = 8,
    PlaylistsForShorts = 9,
    ChannelAboutModal = 10,
    LikeButtonViewmodel = 11,
    ChannelPageHeader = 12,
    MusicPlaylistTwoColumn = 13,
    CommentsFrameworkUpdate = 14,
    ChannelShortsLockup = 15,
    PlaylistPageHeader = 16,
}

/// List of active A/B tests that are run when none is manually specified
const TESTS_TO_RUN: [ABTest; 3] = [
    ABTest::ChannelPageHeader,
    ABTest::MusicPlaylistTwoColumn,
    ABTest::CommentsFrameworkUpdate,
];

#[derive(Debug, Serialize, Deserialize)]
pub struct ABTestRes {
    id: u16,
    name: ABTest,
    tests: usize,
    occurrences: usize,
    vd_present: Option<String>,
    vd_absent: Option<String>,
}

#[derive(Debug, Serialize)]
struct QVideo<'a> {
    context: YTContext<'a>,
    video_id: &'a str,
    content_check_ok: bool,
    racy_check_ok: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowse<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a str>,
}

pub async fn run_test(
    ab: ABTest,
    n: usize,
    concurrency: usize,
) -> (usize, Option<String>, Option<String>) {
    eprintln!("🧪 A/B test #{}: {:?}", ab as u16, ab);

    let rp = RustyPipe::new();
    let pb = ProgressBar::new(n as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len}",
        )
        .unwrap(),
    );

    let results = stream::iter(0..n)
        .map(|_| {
            let rp = rp.clone();
            let pb = pb.clone();
            async move {
                let visitor_data = rp.query().get_visitor_data().await.unwrap();
                let query = rp.query().visitor_data(&visitor_data);
                let is_present = match ab {
                    ABTest::AttributedTextDescription => attributed_text_description(&query).await,
                    ABTest::ThreeTabChannelLayout => three_tab_channel_layout(&query).await,
                    ABTest::ChannelHandlesInSearchResults => {
                        channel_handles_in_search_results(&query).await
                    }
                    ABTest::TrendsVideoTab => trends_video_tab(&query).await,
                    ABTest::TrendsPageHeaderRenderer => trends_page_header_renderer(&query).await,
                    ABTest::DiscographyPage => discography_page(&query).await,
                    ABTest::ShortDateFormat => short_date_format(&query).await,
                    ABTest::PlaylistsForShorts => playlists_for_shorts(&query).await,
                    ABTest::TrackViewcount => track_viewcount(&query).await,
                    ABTest::ChannelAboutModal => channel_about_modal(&query).await,
                    ABTest::LikeButtonViewmodel => like_button_viewmodel(&query).await,
                    ABTest::ChannelPageHeader => channel_page_header(&query).await,
                    ABTest::MusicPlaylistTwoColumn => music_playlist_two_column(&query).await,
                    ABTest::CommentsFrameworkUpdate => comments_framework_update(&query).await,
                    ABTest::ChannelShortsLockup => channel_shorts_lockup(&query).await,
                    ABTest::PlaylistPageHeader => playlist_page_header_renderer(&query).await,
                }
                .unwrap();
                pb.inc(1);
                (is_present, visitor_data)
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    let count = results.iter().filter(|(p, _)| *p).count();
    let vd_present = results
        .iter()
        .find_map(|(p, vd)| if *p { Some(vd.clone()) } else { None });
    let vd_absent = results
        .iter()
        .find_map(|(p, vd)| if *p { None } else { Some(vd.clone()) });

    (count, vd_present, vd_absent)
}

pub async fn run_all_tests(n: usize, concurrency: usize) -> Vec<ABTestRes> {
    let mut results = Vec::new();

    for ab in TESTS_TO_RUN {
        let (occurrences, vd_present, vd_absent) = run_test(ab, n, concurrency).await;
        results.push(ABTestRes {
            id: ab as u16,
            name: ab,
            tests: n,
            occurrences,
            vd_present,
            vd_absent,
        });
    }
    results
}

pub async fn attributed_text_description(rp: &RustyPipeQuery) -> Result<bool> {
    let context = rp.get_context(ClientType::Desktop, true, None).await;
    let q = QVideo {
        context,
        video_id: "ZeerrnuLi5E",
        content_check_ok: false,
        racy_check_ok: false,
    };
    let response_txt = rp.raw(ClientType::Desktop, "next", &q).await.unwrap();

    if !response_txt.contains("\"Black Mamba\"") {
        bail!("invalid response data");
    }

    Ok(response_txt.contains("\"attributedDescription\""))
}

pub async fn three_tab_channel_layout(rp: &RustyPipeQuery) -> Result<bool> {
    let channel = rp.channel_videos("UCR-DXc1voovS8nhAvccRZhg").await.unwrap();
    Ok(channel.has_live || channel.has_shorts)
}

pub async fn channel_handles_in_search_results(rp: &RustyPipeQuery) -> Result<bool> {
    let search = rp
        .search_filter("rust", &SearchFilter::new().item_type(ItemType::Channel))
        .await
        .unwrap();

    Ok(search.items.items.iter().any(|itm| match itm {
        YouTubeItem::Channel(channel) => channel
            .subscriber_count
            .map(|sc| sc > 100 && channel.handle.is_some())
            .unwrap_or_default(),
        _ => false,
    }))
}

pub async fn trends_video_tab(rp: &RustyPipeQuery) -> Result<bool> {
    let context = rp.get_context(ClientType::Desktop, true, None).await;
    let res = rp
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context,
                browse_id: "FEtrending",
                params: None,
            },
        )
        .await?;

    Ok(res.contains("\"4gIOGgxtb3N0X3BvcHVsYXI%3D\""))
}

pub async fn trends_page_header_renderer(rp: &RustyPipeQuery) -> Result<bool> {
    let context = rp.get_context(ClientType::Desktop, true, None).await;
    let res = rp
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context,
                browse_id: "FEtrending",
                params: None,
            },
        )
        .await?;

    #[derive(Debug, Deserialize)]
    struct D {
        header: BTreeMap<String, IgnoredAny>,
    }

    let data = serde_json::from_str::<D>(&res)?;

    Ok(data.header.contains_key("pageHeaderRenderer"))
}

pub async fn discography_page(rp: &RustyPipeQuery) -> Result<bool> {
    let id = "UC7cl4MmM6ZZ2TcFyMk_b4pg";
    let res = rp
        .raw(
            ClientType::DesktopMusic,
            "browse",
            &QBrowse {
                context: rp.get_context(ClientType::DesktopMusic, true, None).await,
                browse_id: id,
                params: None,
            },
        )
        .await
        .unwrap();
    Ok(res.contains(&format!("\"MPAD{id}\"")))
}

pub async fn short_date_format(rp: &RustyPipeQuery) -> Result<bool> {
    static SHORT_DATE: Lazy<Regex> = Lazy::new(|| Regex::new("\\d(?:y|mo|w|d|h|min) ").unwrap());
    let channel = rp.channel_videos("UC2DjFE7Xf11URZqWBigcVOQ").await?;

    Ok(channel.content.items.iter().any(|itm| {
        itm.publish_date_txt
            .as_deref()
            .map(|d| SHORT_DATE.is_match(d))
            .unwrap_or_default()
    }))
}

pub async fn playlists_for_shorts(rp: &RustyPipeQuery) -> Result<bool> {
    let playlist = rp.playlist("UUSHh8gHdtzO2tXd593_bjErWg").await?;
    let v1 = playlist
        .videos
        .items
        .first()
        .ok_or_else(|| anyhow::anyhow!("no videos"))?;
    Ok(v1.publish_date_txt.is_none())
}

pub async fn track_viewcount(rp: &RustyPipeQuery) -> Result<bool> {
    let res = rp.music_search_main("lieblingsmensch namika").await?;

    let track = &res
        .items
        .items
        .iter()
        .find_map(|itm| {
            if let MusicItem::Track(track) = itm {
                if track.id == "6485PhOtHzY" {
                    Some(track)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            panic!("could not find track, got {:#?}", &res.items.items);
        });

    Ok(track.view_count.is_some())
}

pub async fn channel_about_modal(rp: &RustyPipeQuery) -> Result<bool> {
    let id = "UC2DjFE7Xf11URZqWBigcVOQ";
    let res = rp
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context: rp.get_context(ClientType::Desktop, true, None).await,
                browse_id: id,
                params: None,
            },
        )
        .await
        .unwrap();
    Ok(!res.contains("\"EgVhYm91dPIGBAoCEgA%3D\""))
}

pub async fn like_button_viewmodel(rp: &RustyPipeQuery) -> Result<bool> {
    let res = rp
        .raw(
            ClientType::Desktop,
            "next",
            &QVideo {
                context: rp.get_context(ClientType::Desktop, true, None).await,
                video_id: "ZeerrnuLi5E",
                content_check_ok: true,
                racy_check_ok: true,
            },
        )
        .await?;
    Ok(res.contains("\"segmentedLikeDislikeButtonViewModel\""))
}

pub async fn channel_page_header(rp: &RustyPipeQuery) -> Result<bool> {
    let channel = rp
        .channel_videos_tab("UCh8gHdtzO2tXd593_bjErWg", ChannelVideoTab::Shorts)
        .await?;
    Ok(channel.video_count.is_some())
}

pub async fn music_playlist_two_column(rp: &RustyPipeQuery) -> Result<bool> {
    let id = "VLRDCLAK5uy_kb7EBi6y3GrtJri4_ZH56Ms786DFEimbM";
    let res = rp
        .raw(
            ClientType::DesktopMusic,
            "browse",
            &QBrowse {
                context: rp.get_context(ClientType::DesktopMusic, true, None).await,
                browse_id: id,
                params: None,
            },
        )
        .await
        .unwrap();
    Ok(res.contains("\"musicResponsiveHeaderRenderer\""))
}

pub async fn comments_framework_update(rp: &RustyPipeQuery) -> Result<bool> {
    let continuation =
        "Eg0SC3dMZHBSN2d1S3k4GAYyJSIRIgt3TGRwUjdndUt5ODAAeAJCEGNvbW1lbnRzLXNlY3Rpb24%3D";
    let res = rp
        .raw(
            ClientType::Desktop,
            "next",
            &QCont {
                context: rp.get_context(ClientType::Desktop, true, None).await,
                continuation,
            },
        )
        .await
        .unwrap();
    Ok(res.contains("\"frameworkUpdates\""))
}

pub async fn channel_shorts_lockup(rp: &RustyPipeQuery) -> Result<bool> {
    let id = "UCh8gHdtzO2tXd593_bjErWg";
    let res = rp
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context: rp.get_context(ClientType::Desktop, true, None).await,
                browse_id: id,
                params: Some("EgZzaG9ydHPyBgUKA5oBAA%3D%3D"),
            },
        )
        .await
        .unwrap();
    Ok(res.contains("\"shortsLockupViewModel\""))
}

pub async fn playlist_page_header_renderer(rp: &RustyPipeQuery) -> Result<bool> {
    let id = "VLPLZN_exA7d4RVmCQrG5VlWIjMOkMFZVVOc";
    let res = rp
        .raw(
            ClientType::Desktop,
            "browse",
            &QBrowse {
                context: rp.get_context(ClientType::Desktop, true, None).await,
                browse_id: id,
                params: None,
            },
        )
        .await
        .unwrap();
    Ok(res.contains("\"pageHeaderRenderer\""))
}
