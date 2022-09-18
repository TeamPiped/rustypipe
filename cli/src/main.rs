use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, ClientBuilder};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::stream_filter::Filter,
};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    #[clap(short, value_parser, default_value = ".", global = true)]
    output: PathBuf,
    #[clap(long, value_parser, global = true)]
    resolution: Option<u32>,
    #[clap(short, long, value_parser, default_value = "8", global = true)]
    parallel: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Download a playlist
    Playlist {
        /// Playlist ID
        #[clap(value_parser)]
        id: String,
    },
    /// Download a video
    Video {
        /// Video ID
        #[clap(value_parser)]
        id: String,
    },
}

async fn download_single_video(
    video_id: String,
    video_title: String,
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
    ffmpeg: &str,
    rp: &RustyPipe,
    http: Client,
    multi: MultiProgress,
    main: Option<ProgressBar>,
) -> Result<()> {
    let pb = multi.add(ProgressBar::new(1));
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Fetching player data for {}", video_title));

    let res = async {
        let player_data = rp
            .query()
            .player(video_id.as_str(), ClientType::TvHtml5Embed)
            .await
            .context(format!(
                "Failed to fetch player data for video {}",
                video_id
            ))?;

        let mut filter = Filter::default();
        if let Some(res) = resolution {
            if res == 0 {
                filter.no_video();
            } else {
                filter.video_max_res(res);
            }
        }

        rustypipe::download::download_video(
            &player_data,
            output_dir,
            output_fname,
            None,
            &filter,
            ffmpeg,
            http,
            pb,
        )
        .await
        .context(format!(
            "Failed to download video '{}' [{}]",
            player_data.details.title, video_id
        ))
    }
    .await;

    if let Some(main) = main {
        main.inc(1);
    }
    res
}

async fn download_video(
    id: &str,
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
) {
    let http = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0")
        .gzip(true)
        .brotli(true)
        .build()
        .expect("unable to build the HTTP client");

    let rp = RustyPipe::default();

    // Indicatif setup
    let multi = MultiProgress::new();

    download_single_video(
        id.to_owned(),
        id.to_owned(),
        output_dir,
        output_fname,
        resolution,
        "ffmpeg",
        &rp,
        http,
        multi,
        None,
    )
    .await
    .unwrap_or_else(|e| println!("ERROR: {:?}", e));
}

async fn download_playlist(
    id: &str,
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
    parallel: usize,
) {
    let http = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0")
        .gzip(true)
        .brotli(true)
        .build()
        .expect("unable to build the HTTP client");

    let rp = RustyPipe::default();
    let mut playlist = rp.query().playlist(id).await.unwrap();
    playlist.videos.extend_pages(rp.query(), usize::MAX).await.unwrap();

    // Indicatif setup
    let multi = MultiProgress::new();
    let main = multi.add(ProgressBar::new(
        playlist.videos.items.len().try_into().unwrap_or_default(),
    ));

    main.set_style(
        ProgressStyle::default_bar()
            .template("Downloaded {pos:>}/{len} Videos [{wide_bar:.blue}]")
            .unwrap()
            .progress_chars("#>-"),
    );
    main.tick();

    stream::iter(playlist.videos.items)
        .map(|video| {
            download_single_video(
                video.id,
                video.title,
                output_dir,
                output_fname.to_owned(),
                resolution,
                "ffmpeg",
                &rp,
                http.clone(),
                multi.clone(),
                Some(main.clone()),
            )
        })
        .buffer_unordered(parallel)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .for_each(|res| match res {
            Ok(_) => {}
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        });
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();

    // Cases: Existing folder, non-existing file with existing parent folder,
    // Error cases: non-existing parent folder, existing file
    let output_path = std::fs::canonicalize(cli.output).unwrap();
    if output_path.is_file() {
        println!("Output file already exists");
        return;
    }
    let (output_dir, output_fname) = if output_path.is_dir() {
        (output_path.to_string_lossy().to_string(), None)
    } else {
        let output_dir_parent = output_path.parent().unwrap();
        if !output_dir_parent.is_dir() {
            println!(
                "Parent folder {} does not exist",
                output_dir_parent.to_string_lossy()
            );
            return;
        }

        (
            output_dir_parent.to_string_lossy().to_string(),
            Some(
                output_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            ),
        )
    };

    match cli.command {
        Commands::Playlist { id } => {
            download_playlist(&id, &output_dir, output_fname, cli.resolution, cli.parallel).await
        }
        Commands::Video { id } => {
            download_video(&id, &output_dir, output_fname, cli.resolution).await
        }
    };
}
