use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, ClientBuilder};
use rustypipe::client::{ClientType, RustyTube};

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
}

async fn download_video(
    video_id: String,
    video_title: String,
    output_dir: &str,
    output_fname: Option<String>,
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
    pb.set_message(format!("Fetching player data for {}", video_title));

    let res = async {
        let player_data = rt
            .get_player(video_id.as_str(), ClientType::Desktop)
            .await
            .context(format!(
                "Failed to fetch player data for video {}",
                video_id
            ))?;

        rustypipe::download::download_video(
            &player_data,
            output_dir,
            output_fname,
            resolution,
            ffmpeg,
            http,
            pb,
        )
        .await
        .context(format!(
            "Failed to download video '{}' [{}]",
            player_data.info.title, video_id
        ))
    }
    .await;

    main.inc(1);
    res
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

    let rt = RustyTube::new();
    let playlist = rt.get_playlist(id, ClientType::Desktop).await.unwrap();

    // Indicatif setup
    let multi = MultiProgress::new();
    let main = multi.add(ProgressBar::new(
        playlist.len().try_into().unwrap_or_default(),
    ));

    main.set_style(
        ProgressStyle::default_bar()
            .template("Downloaded {pos:>}/{len} Videos [{wide_bar:.blue}]")
            .unwrap()
            .progress_chars("#>-"),
    );
    main.tick();

    stream::iter(playlist)
        .map(|item| {
            download_video(
                item.video_id.to_owned(),
                item.title.to_owned(),
                output_dir,
                output_fname.to_owned(),
                resolution,
                "ffmpeg",
                &rt,
                http.clone(),
                multi.clone(),
                main.clone(),
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
    };
}
