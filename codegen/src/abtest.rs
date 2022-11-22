use anyhow::{bail, Result};
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use num_enum::TryFromPrimitive;
use rustypipe::client::{ClientType, RustyPipe, YTContext};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, Serialize, Deserialize,
)]
#[repr(u16)]
pub enum ABTest {
    AttributedTextDescription = 1,
    ThreeTabChannelLayout = 2,
}

const N_TESTS: u16 = 2;

#[derive(Debug, Serialize, Deserialize)]
pub struct ABTestRes {
    id: u16,
    name: ABTest,
    tests: usize,
    occurrences: usize,
}

#[derive(Debug, Serialize)]
struct QVideo<'a> {
    context: YTContext<'a>,
    video_id: &'a str,
    content_check_ok: bool,
    racy_check_ok: bool,
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

    for id in 1..=N_TESTS {
        let ab = ABTest::try_from(id).unwrap();
        let (occurrences, _, _) = run_test(ab, n, concurrency).await;
        results.push(ABTestRes {
            id,
            name: ab,
            tests: n,
            occurrences,
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
