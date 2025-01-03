#![warn(clippy::todo)]

mod abtest;
mod collect_album_types;
mod collect_chan_prefixes;
mod collect_history_dates;
mod collect_large_numbers;
mod collect_playlist_dates;
mod collect_video_dates;
mod collect_video_durations;
mod download_testfiles;
mod gen_dictionary;
mod gen_locales;
mod model;
mod util;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    #[clap(short, default_value = "8")]
    concurrency: usize,
}

#[derive(Subcommand)]
enum Commands {
    CollectPlaylistDates,
    CollectLargeNumbers,
    CollectAlbumTypes,
    CollectVideoDurations,
    CollectVideoDates,
    CollectHistoryDates,
    CollectChanPrefixes,
    ParsePlaylistDates,
    ParseHistoryDates,
    ParseLargeNumbers,
    ParseAlbumTypes,
    ParseVideoDurations,
    ParseChanPrefixes,
    GenLocales,
    GenDict,
    DownloadTestfiles,
    AbTest {
        #[clap(value_parser)]
        id: Option<u16>,
        #[clap(short, default_value = "100")]
        n: usize,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::CollectPlaylistDates => {
            collect_playlist_dates::collect_dates(cli.concurrency).await;
        }
        Commands::CollectLargeNumbers => {
            collect_large_numbers::collect_large_numbers(cli.concurrency).await;
        }
        Commands::CollectAlbumTypes => {
            collect_album_types::collect_album_types(cli.concurrency).await;
        }
        Commands::CollectVideoDurations => {
            collect_video_durations::collect_video_durations(cli.concurrency).await;
        }
        Commands::CollectVideoDates => {
            collect_video_dates::collect_video_dates(cli.concurrency).await;
        }
        Commands::CollectHistoryDates => {
            collect_history_dates::collect_dates().await;
        }
        Commands::CollectChanPrefixes => {
            collect_chan_prefixes::collect_chan_prefixes().await;
        }
        Commands::ParsePlaylistDates => collect_playlist_dates::write_samples_to_dict(),
        Commands::ParseHistoryDates => collect_history_dates::write_samples_to_dict(),
        Commands::ParseLargeNumbers => collect_large_numbers::write_samples_to_dict(),
        Commands::ParseAlbumTypes => collect_album_types::write_samples_to_dict(),
        Commands::ParseVideoDurations => collect_video_durations::parse_video_durations(),
        Commands::ParseChanPrefixes => collect_chan_prefixes::write_samples_to_dict(),
        Commands::GenLocales => {
            gen_locales::generate_locales().await;
        }
        Commands::GenDict => gen_dictionary::generate_dictionary(),
        Commands::DownloadTestfiles => download_testfiles::download_testfiles().await,
        Commands::AbTest { id, n } => {
            match id {
                Some(id) => {
                    let ab = abtest::ABTest::try_from(id).expect("invalid A/B test id");
                    let (occurrences, vd_present, vd_absent) =
                        abtest::run_test(ab, n, cli.concurrency).await;
                    eprintln!(
                        "{}/{} occurences ({:.1}%)",
                        occurrences,
                        n,
                        occurrences as f32 / n as f32 * 100.0
                    );
                    eprintln!(
                        "visitor_data (present): {}",
                        vd_present.as_deref().unwrap_or("n/a")
                    );
                    eprintln!(
                        "visitor_data (absent):  {}",
                        vd_absent.as_deref().unwrap_or("n/a")
                    );
                }
                None => {
                    let res = abtest::run_all_tests(n, cli.concurrency).await;
                    println!("{}", serde_json::to_string_pretty(&res).unwrap());
                }
            };
        }
    };
}
