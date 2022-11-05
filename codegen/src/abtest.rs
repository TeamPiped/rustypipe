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

pub async fn run_test(ab: ABTest, n: usize, concurrency: usize) -> usize {
    eprintln!("🧪 A/B test #{}: {:?}", ab as u16, ab);

    let rp = RustyPipe::new();
    let pb = ProgressBar::new(n as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len}",
        )
        .unwrap(),
    );
    // let mut count = 0;

    let results = stream::iter(0..n)
        .map(|_| {
            let rp = rp.clone();
            let pb = pb.clone();
            async move {
                let is_present = match ab {
                    ABTest::AttributedTextDescription => attributed_text_description(&rp).await,
                    ABTest::ThreeTabChannelLayout => three_tab_channel_layout(&rp).await,
                }
                .unwrap();
                pb.inc(1);
                is_present
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    let count = results.iter().filter(|x| **x).count();

    count
}

pub async fn run_all_tests(n: usize, concurrency: usize) -> Vec<ABTestRes> {
    let mut results = Vec::new();

    for id in 1..=N_TESTS {
        let ab = ABTest::try_from(id).unwrap();
        let occurrences = run_test(ab, n, concurrency).await;
        results.push(ABTestRes {
            id,
            name: ab,
            tests: n,
            occurrences,
        });
    }
    results
}

pub async fn attributed_text_description(rp: &RustyPipe) -> Result<bool> {
    let query = rp.query();
    let context = query.get_context(ClientType::Desktop, true, None).await;
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

pub async fn three_tab_channel_layout(rp: &RustyPipe) -> Result<bool> {
    let channel = rp
        .query()
        .channel_videos("UCR-DXc1voovS8nhAvccRZhg")
        .await
        .unwrap();
    Ok(channel.has_live || channel.has_shorts)
}
