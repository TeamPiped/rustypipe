use std::collections::BTreeMap;

use anyhow::{bail, Result};
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use num_enum::TryFromPrimitive;
use rustypipe::client::{ClientType, RustyPipe, YTContext};
use rustypipe::model::YouTubeItem;
use rustypipe::param::search_filter::{ItemType, SearchFilter};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

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
}

const TESTS_TO_RUN: [ABTest; 1] = [ABTest::TrendsVideoTab];

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
    let http = reqwest::Client::default();
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
            let http = http.clone();
            async move {
                let visitor_data = get_visitor_data(&http).await;
                let is_present = match ab {
                    ABTest::AttributedTextDescription => {
                        attributed_text_description(&rp, &visitor_data).await
                    }
                    ABTest::ThreeTabChannelLayout => {
                        three_tab_channel_layout(&rp, &visitor_data).await
                    }
                    ABTest::ChannelHandlesInSearchResults => {
                        channel_handles_in_search_results(&rp, &visitor_data).await
                    }
                    ABTest::TrendsVideoTab => trends_video_tab(&rp, &visitor_data).await,
                    ABTest::TrendsPageHeaderRenderer => {
                        trends_page_header_renderer(&rp, &visitor_data).await
                    }
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
        .find_map(|(p, vd)| if *p { Some(vd.to_owned()) } else { None });
    let vd_absent = results
        .iter()
        .find_map(|(p, vd)| if !*p { Some(vd.to_owned()) } else { None });

    (count, vd_present, vd_absent)
}

async fn get_visitor_data(http: &reqwest::Client) -> String {
    let resp = http.get("https://www.youtube.com").send().await.unwrap();
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .find_map(|c| {
            if let Ok(cookie) = c.to_str() {
                if let Some(after) = cookie.strip_prefix("__Secure-YEC=") {
                    return after.split_once(';').map(|s| s.0.to_owned());
                }
            }
            None
        })
        .unwrap()
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

pub async fn attributed_text_description(rp: &RustyPipe, visitor_data: &str) -> Result<bool> {
    let query = rp.query();
    let context = query
        .get_context(ClientType::Desktop, true, Some(visitor_data))
        .await;
    let q = QVideo {
        context,
        video_id: "ZeerrnuLi5E",
        content_check_ok: false,
        racy_check_ok: false,
    };
    let response_txt = query.raw(ClientType::Desktop, "next", &q).await.unwrap();

    if !response_txt.contains("\"Black Mamba\"") {
        bail!("invalid response data");
    }

    Ok(response_txt.contains("\"attributedDescription\""))
}

pub async fn three_tab_channel_layout(rp: &RustyPipe, visitor_data: &str) -> Result<bool> {
    let channel = rp
        .query()
        .visitor_data(visitor_data)
        .channel_videos("UCR-DXc1voovS8nhAvccRZhg")
        .await
        .unwrap();
    Ok(channel.has_live || channel.has_shorts)
}

pub async fn channel_handles_in_search_results(rp: &RustyPipe, visitor_data: &str) -> Result<bool> {
    let search = rp
        .query()
        .visitor_data(visitor_data)
        .search_filter("rust", &SearchFilter::new().item_type(ItemType::Channel))
        .await
        .unwrap();

    Ok(search.items.items.iter().any(|itm| match itm {
        YouTubeItem::Channel(channel) => channel
            .subscriber_count
            .map(|sc| sc > 100 && channel.video_count.is_none())
            .unwrap_or_default(),
        _ => false,
    }))
}

pub async fn trends_video_tab(rp: &RustyPipe, visitor_data: &str) -> Result<bool> {
    let query = rp.query().visitor_data(visitor_data);
    let context = query.get_context(ClientType::Desktop, true, None).await;
    let res = query
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

pub async fn trends_page_header_renderer(rp: &RustyPipe, visitor_data: &str) -> Result<bool> {
    let query = rp.query().visitor_data(visitor_data);
    let context = query.get_context(ClientType::Desktop, true, None).await;
    let res = query
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
