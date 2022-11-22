mod abtest;
mod collect_album_types;
mod collect_large_numbers;
mod collect_playlist_dates;
mod download_testfiles;
mod gen_dictionary;
mod gen_locales;
mod util;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    #[clap(short = 'd', default_value = "..")]
    project_root: PathBuf,
    #[clap(short, default_value = "8")]
    concurrency: usize,
}

#[derive(Subcommand)]
enum Commands {
    CollectPlaylistDates,
    CollectLargeNumbers,
    CollectAlbumTypes,
    ParsePlaylistDates,
    ParseLargeNumbers,
    ParseAlbumTypes,
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
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::CollectPlaylistDates => {
            collect_playlist_dates::collect_dates(&cli.project_root, cli.concurrency).await;
        }
        Commands::CollectLargeNumbers => {
            collect_large_numbers::collect_large_numbers(&cli.project_root, cli.concurrency).await;
        }
        Commands::CollectAlbumTypes => {
            collect_album_types::collect_album_types(&cli.project_root, cli.concurrency).await;
        }
        Commands::ParsePlaylistDates => {
            collect_playlist_dates::write_samples_to_dict(&cli.project_root)
        }
        Commands::ParseLargeNumbers => {
            collect_large_numbers::write_samples_to_dict(&cli.project_root)
        }
        Commands::ParseAlbumTypes => collect_album_types::write_samples_to_dict(&cli.project_root),
        Commands::GenLocales => {
            gen_locales::generate_locales(&cli.project_root).await;
        }
        Commands::GenDict => gen_dictionary::generate_dictionary(&cli.project_root),
        Commands::DownloadTestfiles => {
            download_testfiles::download_testfiles(&cli.project_root).await
        }
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
                    eprintln!("visitor_data (present): {:?}", vd_present);
                    eprintln!("visitor_data (absent):  {:?}", vd_absent);
                }
                None => {
                    let res = abtest::run_all_tests(n, cli.concurrency).await;
                    println!("{}", serde_json::to_string(&res).unwrap())
                }
            };
        }
    };
}
