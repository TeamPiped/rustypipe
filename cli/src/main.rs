use anyhow::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, ClientBuilder};
use rusty_tube::client::{ClientType, RustyTube};

async fn download_video(
    video_id: String,
    output_dir: &str,
    output_fname: &str,
    resolution: Option<u32>,
    ffmpeg: &str,
    rt: &RustyTube,
    http: Client,
    multi: MultiProgress,
    main: ProgressBar,
) -> Result<()> {
    let pb = multi.add(ProgressBar::new(1));
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb.set_message("Fetching player data");

    let player_data = rt
        .get_player(video_id.as_str(), ClientType::Android)
        .await?;

    rusty_tube::download::download_video(
        &player_data,
        output_dir,
        // output_fname,
        resolution,
        ffmpeg,
        http,
        pb,
    )
    .await?;

    main.inc(1);
    Ok(())
}

#[tokio::main]
async fn main() {
    let http = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0")
        .gzip(true)
        .brotli(true)
        .build()
        .expect("unable to build the HTTP client");

    let rt = RustyTube::new();
    let playlist = rt
        .get_playlist("PL4fGSI1pDJn4X-OicSCOy-dChXWdTgziQ", ClientType::Desktop)
        .await
        .unwrap();

    // Indicatif setup
    let multi = MultiProgress::new();
    let main = multi.add(ProgressBar::new(
        playlist.len().try_into().unwrap_or_default(),
    ));
    main.set_style(
        ProgressStyle::default_bar()
            .template("Downloading {pos:>}/{len} Videos [{wide_bar:.blue}]")
            .unwrap()
            .progress_chars("#>-"),
    );
    main.tick();

    stream::iter(playlist)
        .map(|item| {
            download_video(
                item.video_id.to_owned(),
                "tmp",
                "xyz",
                None,
                "ffmpeg",
                &rt,
                http.clone(),
                multi.clone(),
                main.clone(),
            )
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<(), _>>()
        .unwrap();
}
