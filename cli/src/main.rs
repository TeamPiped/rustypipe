#![warn(clippy::todo, clippy::dbg_macro)]

use std::{path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, ClientBuilder};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{UrlTarget, VideoId, YouTubeItem},
    param::{search_filter, ChannelVideoTab, Country, Language, StreamFilter},
};
use serde::Serialize;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    /// Always generate a report (used for debugging)
    #[clap(long, global = true)]
    report: bool,
    /// YouTube visitor data cookie
    #[clap(long, global = true)]
    vdata: Option<String>,
    /// YouTube content language
    #[clap(long, global = true)]
    lang: Option<String>,
    /// YouTube content country
    #[clap(long, global = true)]
    country: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download a video, playlist, album or channel
    #[clap(alias = "dl")]
    Download {
        /// ID or URL
        id: String,
        /// Output path
        #[clap(short, default_value = ".")]
        output: PathBuf,
        /// Video resolution (e.g. 720, 1080). Set to 0 for audio-only.
        #[clap(short, long)]
        resolution: Option<u32>,
        /// Number of videos downloaded in parallel
        #[clap(short, long, default_value_t = 8)]
        parallel: usize,
        /// Limit the number of videos to download
        #[clap(long, default_value_t = 1000)]
        limit: usize,
    },
    /// Extract video, playlist, album or channel data
    Get {
        /// ID or URL
        id: String,
        /// Output format
        #[clap(long, value_parser, default_value = "json")]
        format: Format,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(long, default_value_t = 20)]
        limit: usize,
        /// Channel tab
        #[clap(long, default_value = "videos")]
        tab: ChannelTab,
        /// Use YouTube Music
        #[clap(long)]
        music: bool,
        /// Get comments
        #[clap(long)]
        comments: Option<CommentsOrder>,
        /// Get lyrics
        #[clap(long)]
        lyrics: bool,
        /// Get the player
        #[clap(long)]
        player: bool,
        #[clap(long)]
        player_type: Option<PlayerType>,
    },
    /// Search YouTube
    Search {
        /// Search query
        query: String,
        /// Output format
        #[clap(long, value_parser, default_value = "json")]
        format: Format,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(long, default_value_t = 20)]
        limit: usize,
        /// Filter results by item type
        #[clap(long)]
        item_type: Option<SearchItemType>,
        /// Filter results by video length
        #[clap(long)]
        length: Option<SearchLength>,
        /// Filter results by upload date
        #[clap(long)]
        date: Option<SearchUploadDate>,
        /// Sort search resulus
        #[clap(long)]
        order: Option<SearchOrder>,
        /// Channel ID for searching channel videos
        #[clap(long)]
        channel: Option<String>,
        /// YouTube Music search filter
        #[clap(long)]
        music: Option<MusicSearchCategory>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    Json,
    Yaml,
}

#[derive(Copy, Clone, ValueEnum)]
enum ChannelTab {
    Videos,
    Shorts,
    Live,
    Playlists,
    Info,
}

#[derive(Copy, Clone, ValueEnum)]
enum CommentsOrder {
    Top,
    Latest,
}

#[derive(Copy, Clone, ValueEnum)]
enum SearchItemType {
    Video,
    Channel,
    Playlist,
}

#[derive(Copy, Clone, ValueEnum)]
enum SearchLength {
    /// < 4min
    Short,
    /// 4-20min
    Medium,
    /// > 20min
    Long,
}

#[derive(Copy, Clone, ValueEnum)]
enum SearchUploadDate {
    /// 1 hour old or newer
    Hour,
    /// 1 day old or newer
    Day,
    /// 1 week old or newer
    Week,
    /// 1 month old or newer
    Month,
    /// 1 year old or newer
    Year,
}

#[derive(Copy, Clone, ValueEnum)]
enum SearchOrder {
    /// Sort by Like/Dislike ratio
    Rating,
    /// Sort by upload date
    Date,
    /// Sort by view count
    Views,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum MusicSearchCategory {
    All,
    Tracks,
    Videos,
    Artists,
    Albums,
    PlaylistsYtm,
    PlaylistsCommunity,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum PlayerType {
    Desktop,
    Tv,
    Android,
    Ios,
}

impl From<SearchItemType> for search_filter::ItemType {
    fn from(value: SearchItemType) -> Self {
        match value {
            SearchItemType::Video => search_filter::ItemType::Video,
            SearchItemType::Channel => search_filter::ItemType::Channel,
            SearchItemType::Playlist => search_filter::ItemType::Playlist,
        }
    }
}

impl From<SearchLength> for search_filter::Length {
    fn from(value: SearchLength) -> Self {
        match value {
            SearchLength::Short => search_filter::Length::Short,
            SearchLength::Medium => search_filter::Length::Medium,
            SearchLength::Long => search_filter::Length::Long,
        }
    }
}

impl From<SearchUploadDate> for search_filter::UploadDate {
    fn from(value: SearchUploadDate) -> Self {
        match value {
            SearchUploadDate::Hour => search_filter::UploadDate::Hour,
            SearchUploadDate::Day => search_filter::UploadDate::Day,
            SearchUploadDate::Week => search_filter::UploadDate::Week,
            SearchUploadDate::Month => search_filter::UploadDate::Month,
            SearchUploadDate::Year => search_filter::UploadDate::Year,
        }
    }
}

impl From<SearchOrder> for search_filter::Order {
    fn from(value: SearchOrder) -> Self {
        match value {
            SearchOrder::Rating => search_filter::Order::Rating,
            SearchOrder::Date => search_filter::Order::Date,
            SearchOrder::Views => search_filter::Order::Views,
        }
    }
}

impl From<PlayerType> for ClientType {
    fn from(value: PlayerType) -> Self {
        match value {
            PlayerType::Desktop => Self::Desktop,
            PlayerType::Tv => Self::TvHtml5Embed,
            PlayerType::Android => Self::Android,
            PlayerType::Ios => Self::Ios,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn download_single_video(
    video_id: &str,
    video_title: &str,
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
    pb.set_style(ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Fetching player data for {video_title}"));

    let res = async {
        let player_data = rp
            .query()
            .player(video_id)
            .await
            .context(format!("Failed to fetch player data for video {video_id}"))?;

        let mut filter = StreamFilter::new();
        if let Some(res) = resolution {
            if res == 0 {
                filter = filter.no_video();
            } else {
                filter = filter.video_max_res(res);
            }
        }

        rustypipe_downloader::download_video(
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
            player_data.details.name, video_id
        ))
    }
    .await;

    if let Some(main) = main {
        main.inc(1);
    }
    res
}

fn print_data<T: Serialize>(data: &T, format: Format, pretty: bool) {
    let stdout = std::io::stdout().lock();
    match format {
        Format::Json => {
            if pretty {
                serde_json::to_writer_pretty(stdout, data).unwrap();
            } else {
                serde_json::to_writer(stdout, data).unwrap();
            }
        }
        Format::Yaml => serde_yaml::to_writer(stdout, data).unwrap(),
    };
}

async fn download_video(
    rp: &RustyPipe,
    id: &str,
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
) {
    let http = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0")
        .gzip(true)
        .brotli(true)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("unable to build the HTTP client");

    // Indicatif setup
    let multi = MultiProgress::new();

    download_single_video(
        id,
        id,
        output_dir,
        output_fname,
        resolution,
        "ffmpeg",
        rp,
        http,
        multi,
        None,
    )
    .await
    .unwrap_or_else(|e| println!("ERROR: {e:?}"));
}

async fn download_videos(
    rp: &RustyPipe,
    videos: &[VideoId],
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
    parallel: usize,
) {
    let http = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0")
        .gzip(true)
        .brotli(true)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("unable to build the HTTP client");

    // Indicatif setup
    let multi = MultiProgress::new();
    let main = multi.add(ProgressBar::new(
        videos.len().try_into().unwrap_or_default(),
    ));

    main.set_style(
        ProgressStyle::default_bar()
            .template("Downloaded {pos:>}/{len} Videos [{wide_bar:.blue}]")
            .unwrap()
            .progress_chars("#>-"),
    );
    main.tick();

    stream::iter(videos)
        .map(|video| {
            download_single_video(
                &video.id,
                &video.name,
                output_dir,
                output_fname.clone(),
                resolution,
                "ffmpeg",
                rp,
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
                println!("ERROR: {e:?}");
            }
        });
}

#[tokio::main]
async fn main() {
    // env_logger::builder().format_timestamp_micros().init();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let mut rp = RustyPipe::builder().visitor_data_opt(cli.vdata);
    if cli.report {
        rp = rp.report();
    } else {
        let mut storage_dir = dirs::data_dir().expect("no data dir");
        storage_dir.push("rustypipe");
        std::fs::create_dir_all(&storage_dir).expect("could not create data dir");
        rp = rp.storage_dir(storage_dir);
    }
    if let Some(lang) = cli.lang {
        rp = rp.lang(Language::from_str(&lang.to_ascii_lowercase()).expect("invalid language"));
    }
    if let Some(country) = cli.country {
        rp = rp.country(Country::from_str(&country.to_ascii_uppercase()).expect("invalid country"));
    }
    let rp = rp.build().unwrap();

    match cli.command {
        Commands::Download {
            id,
            output,
            resolution,
            parallel,
            limit,
        } => {
            // Cases: Existing folder, non-existing file with existing parent folder,
            // Error cases: non-existing parent folder, existing file
            let output_path = std::fs::canonicalize(output).unwrap();
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

            let target = rp.query().resolve_string(&id, false).await.unwrap();
            match target {
                UrlTarget::Video { id, .. } => {
                    download_video(&rp, &id, &output_dir, output_fname, resolution).await;
                }
                UrlTarget::Channel { id } => {
                    let mut channel = rp.query().channel_videos(id).await.unwrap();
                    channel
                        .content
                        .extend_limit(&rp.query(), limit)
                        .await
                        .unwrap();
                    let videos: Vec<VideoId> = channel
                        .content
                        .items
                        .into_iter()
                        .take(limit)
                        .map(VideoId::from)
                        .collect();
                    download_videos(
                        &rp,
                        &videos,
                        &output_dir,
                        output_fname,
                        resolution,
                        parallel,
                    )
                    .await;
                }
                UrlTarget::Playlist { id } => {
                    let mut playlist = rp.query().playlist(id).await.unwrap();
                    playlist
                        .videos
                        .extend_limit(&rp.query(), limit)
                        .await
                        .unwrap();
                    let videos: Vec<VideoId> = playlist
                        .videos
                        .items
                        .into_iter()
                        .take(limit)
                        .map(VideoId::from)
                        .collect();
                    download_videos(
                        &rp,
                        &videos,
                        &output_dir,
                        output_fname,
                        resolution,
                        parallel,
                    )
                    .await;
                }
                UrlTarget::Album { id } => {
                    let album = rp.query().music_album(id).await.unwrap();
                    let videos: Vec<VideoId> = album
                        .tracks
                        .into_iter()
                        .take(limit)
                        .map(VideoId::from)
                        .collect();
                    download_videos(
                        &rp,
                        &videos,
                        &output_dir,
                        output_fname,
                        resolution,
                        parallel,
                    )
                    .await;
                }
            }
        }
        Commands::Get {
            id,
            format,
            pretty,
            limit,
            tab,
            music,
            comments,
            lyrics,
            player,
            player_type,
        } => {
            let target = rp.query().resolve_string(&id, false).await.unwrap();

            match target {
                UrlTarget::Video { id, .. } => {
                    if lyrics {
                        let details = rp.query().music_details(&id).await.unwrap();
                        match details.lyrics_id {
                            Some(lyrics_id) => {
                                let lyrics = rp.query().music_lyrics(lyrics_id).await.unwrap();
                                print_data(&lyrics, format, pretty);
                            }
                            None => eprintln!("no lyrics found"),
                        }
                    } else if music {
                        let details = rp.query().music_details(&id).await.unwrap();
                        print_data(&details, format, pretty);
                    } else if player {
                        let player = if let Some(player_type) = player_type {
                            rp.query().player_from_client(&id, player_type.into()).await
                        } else {
                            rp.query().player(&id).await
                        }
                        .unwrap();
                        print_data(&player, format, pretty);
                    } else {
                        let mut details = rp.query().video_details(&id).await.unwrap();

                        match comments {
                            Some(CommentsOrder::Top) => {
                                details
                                    .top_comments
                                    .extend_limit(rp.query(), limit)
                                    .await
                                    .unwrap();
                            }
                            Some(CommentsOrder::Latest) => {
                                details
                                    .latest_comments
                                    .extend_limit(rp.query(), limit)
                                    .await
                                    .unwrap();
                            }
                            None => {}
                        }

                        print_data(&details, format, pretty);
                    }
                }
                UrlTarget::Channel { id } => {
                    if music {
                        let artist = rp.query().music_artist(&id, true).await.unwrap();
                        print_data(&artist, format, pretty);
                    } else {
                        match tab {
                            ChannelTab::Videos | ChannelTab::Shorts | ChannelTab::Live => {
                                let video_tab = match tab {
                                    ChannelTab::Videos => ChannelVideoTab::Videos,
                                    ChannelTab::Shorts => ChannelVideoTab::Shorts,
                                    ChannelTab::Live => ChannelVideoTab::Live,
                                    _ => unreachable!(),
                                };
                                let mut channel =
                                    rp.query().channel_videos_tab(&id, video_tab).await.unwrap();

                                channel
                                    .content
                                    .extend_limit(rp.query(), limit)
                                    .await
                                    .unwrap();
                                print_data(&channel, format, pretty);
                            }
                            ChannelTab::Playlists => {
                                let channel = rp.query().channel_playlists(&id).await.unwrap();
                                print_data(&channel, format, pretty);
                            }
                            ChannelTab::Info => {
                                let channel = rp.query().channel_info(&id).await.unwrap();
                                print_data(&channel, format, pretty);
                            }
                        }
                    }
                }
                UrlTarget::Playlist { id } => {
                    if music {
                        let mut playlist = rp.query().music_playlist(&id).await.unwrap();
                        playlist
                            .tracks
                            .extend_limit(rp.query(), limit)
                            .await
                            .unwrap();
                        print_data(&playlist, format, pretty);
                    } else {
                        let mut playlist = rp.query().playlist(&id).await.unwrap();
                        playlist
                            .videos
                            .extend_limit(rp.query(), limit)
                            .await
                            .unwrap();
                        print_data(&playlist, format, pretty);
                    }
                }
                UrlTarget::Album { id } => {
                    let album = rp.query().music_album(&id).await.unwrap();
                    print_data(&album, format, pretty);
                }
            }
        }
        Commands::Search {
            query,
            format,
            pretty,
            limit,
            item_type,
            length,
            date,
            order,
            channel,
            music,
        } => match music {
            None => match channel {
                Some(channel) => {
                    rustypipe::validate::channel_id(&channel).unwrap();
                    let res = rp.query().channel_search(&channel, &query).await.unwrap();
                    print_data(&res, format, pretty);
                }
                None => {
                    let filter = search_filter::SearchFilter::new()
                        .item_type_opt(item_type.map(search_filter::ItemType::from))
                        .length_opt(length.map(search_filter::Length::from))
                        .date_opt(date.map(search_filter::UploadDate::from))
                        .sort_opt(order.map(search_filter::Order::from));
                    let mut res = rp
                        .query()
                        .search_filter::<YouTubeItem, _>(&query, &filter)
                        .await
                        .unwrap();
                    res.items.extend_limit(rp.query(), limit).await.unwrap();
                    print_data(&res, format, pretty);
                }
            },
            Some(MusicSearchCategory::All) => {
                let res = rp.query().music_search_main(&query).await.unwrap();
                print_data(&res, format, pretty);
            }
            Some(MusicSearchCategory::Tracks) => {
                let mut res = rp.query().music_search_tracks(&query).await.unwrap();
                res.items.extend_limit(rp.query(), limit).await.unwrap();
                print_data(&res, format, pretty);
            }
            Some(MusicSearchCategory::Videos) => {
                let mut res = rp.query().music_search_videos(&query).await.unwrap();
                res.items.extend_limit(rp.query(), limit).await.unwrap();
                print_data(&res, format, pretty);
            }
            Some(MusicSearchCategory::Artists) => {
                let mut res = rp.query().music_search_artists(&query).await.unwrap();
                res.items.extend_limit(rp.query(), limit).await.unwrap();
                print_data(&res, format, pretty);
            }
            Some(MusicSearchCategory::Albums) => {
                let mut res = rp.query().music_search_albums(&query).await.unwrap();
                res.items.extend_limit(rp.query(), limit).await.unwrap();
                print_data(&res, format, pretty);
            }
            Some(MusicSearchCategory::PlaylistsYtm | MusicSearchCategory::PlaylistsCommunity) => {
                let mut res = rp
                    .query()
                    .music_search_playlists(
                        &query,
                        music == Some(MusicSearchCategory::PlaylistsCommunity),
                    )
                    .await
                    .unwrap();
                res.items.extend_limit(rp.query(), limit).await.unwrap();
                print_data(&res, format, pretty);
            }
        },
    };
}
