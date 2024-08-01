#![warn(clippy::todo, clippy::dbg_macro)]

use std::{path::PathBuf, str::FromStr, time::Duration};

use clap::{Parser, Subcommand, ValueEnum};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{UrlTarget, YouTubeItem},
    param::{search_filter, ChannelVideoTab, Country, Language, StreamFilter},
};
use rustypipe_downloader::{DownloadError, DownloadQuery, DownloadVideo, DownloaderBuilder};
use serde::Serialize;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

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

#[derive(Parser)]
#[group(multiple = false)]
struct DownloadTarget {
    #[clap(short, long)]
    output: Option<PathBuf>,
    #[clap(long)]
    output_file: Option<PathBuf>,
    #[clap(long)]
    template: Option<String>,
}

impl DownloadTarget {
    fn assert_dir(&self) {
        if self.output_file.is_some() {
            panic!("Cannot download multiple videos to a single file")
        } else if let Some(template) = &self.template {
            if !template.contains("{id}") && !template.contains("{title}") {
                panic!("Template must contain {{id}} or {{title}} variables")
            }
        }
    }

    fn apply(&self, q: DownloadQuery) -> DownloadQuery {
        if let Some(output_file) = &self.output_file {
            q.to_file(output_file)
        } else if let Some(output) = &self.output {
            q.to_dir(output)
        } else if let Some(template) = &self.template {
            q.to_template(template)
        } else {
            q
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Download a video, playlist, album or channel
    #[clap(alias = "dl")]
    Download {
        /// ID or URL
        id: String,
        #[clap(flatten)]
        target: DownloadTarget,
        /// Video resolution (e.g. 720, 1080). Set to 0 for audio-only.
        #[clap(short, long)]
        resolution: Option<u32>,
        /// Number of videos downloaded in parallel
        #[clap(short, long, default_value_t = 8)]
        parallel: usize,
        /// Use YouTube Music for downloading playlists
        #[clap(long)]
        music: bool,
        /// Limit the number of videos to download
        #[clap(long, default_value_t = 1000)]
        limit: usize,
        #[clap(long)]
        player_type: Option<PlayerType>,
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
    Vdata,
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
    TvEmbed,
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
            PlayerType::TvEmbed => Self::TvHtml5Embed,
            PlayerType::Tv => Self::Tv,
            PlayerType::Android => Self::Android,
            PlayerType::Ios => Self::Ios,
        }
    }
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
    target: &DownloadTarget,
    resolution: Option<u32>,
    player_type: Option<PlayerType>,
    multi: MultiProgress,
) {
    let mut filter = StreamFilter::new();
    if let Some(res) = resolution {
        if res == 0 {
            filter = filter.no_video();
        } else {
            filter = filter.video_max_res(res);
        }
    }
    let dl = DownloaderBuilder::new()
        .rustypipe(rp)
        .stream_filter(filter)
        .progress_bar(multi)
        .audio_tag()
        .crop_cover()
        .build();
    let mut q = target.apply(dl.download_id(id));
    if let Some(player_type) = player_type {
        q = q.player_type(player_type.into());
    }
    let res = q.download().await;
    if let Err(e) = res {
        tracing::error!("[{id}]: {e}")
    }
}

async fn download_videos(
    rp: &RustyPipe,
    videos: Vec<DownloadVideo>,
    target: &DownloadTarget,
    resolution: Option<u32>,
    parallel: usize,
    player_type: Option<PlayerType>,
    multi: MultiProgress,
) {
    let mut filter = StreamFilter::new();
    if let Some(res) = resolution {
        if res == 0 {
            filter = filter.no_video();
        } else {
            filter = filter.video_max_res(res);
        }
    }
    let dl = DownloaderBuilder::new()
        .rustypipe(rp)
        .stream_filter(filter)
        .progress_bar(multi.clone())
        .audio_tag()
        .crop_cover()
        .path_precheck()
        .build();

    // Indicatif setup
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
        .for_each_concurrent(parallel, |video| {
            let dl = dl.clone();
            let main = main.clone();
            let id = video.id().to_owned();

            let mut q = target.apply(dl.download_video(video));
            if let Some(player_type) = player_type {
                q = q.player_type(player_type.into());
            }

            async move {
                if let Err(e) = q.download().await {
                    if !matches!(e, DownloadError::Exists(_)) {
                        tracing::error!("[{id}]: {e}");
                    }
                }
                main.inc(1);
            }
        })
        .await;
}

/// Stderr writer that suspends the progress bars before printing logs
#[derive(Clone)]
struct ProgWriter(MultiProgress);

impl<'a> MakeWriter<'a> for ProgWriter {
    type Writer = ProgWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl std::io::Write for ProgWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.suspend(|| std::io::stderr().write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let multi = MultiProgress::new();

    tracing_subscriber::fmt::SubscriberBuilder::default()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(ProgWriter(multi.clone()))
        .init();

    let mut rp = RustyPipe::builder()
        .visitor_data_opt(cli.vdata)
        .timeout(Duration::from_secs(15));
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
            target,
            resolution,
            parallel,
            music,
            limit,
            player_type,
        } => {
            let url_target = rp.query().resolve_string(&id, false).await.unwrap();
            match url_target {
                UrlTarget::Video { id, .. } => {
                    download_video(&rp, &id, &target, resolution, player_type, multi).await;
                }
                UrlTarget::Channel { id } => {
                    target.assert_dir();
                    let mut channel = rp.query().channel_videos(id).await.unwrap();
                    channel
                        .content
                        .extend_limit(&rp.query(), limit)
                        .await
                        .unwrap();
                    let videos = channel
                        .content
                        .items
                        .into_iter()
                        .take(limit)
                        .map(|v| DownloadVideo::from_entity(&v))
                        .collect();
                    download_videos(
                        &rp,
                        videos,
                        &target,
                        resolution,
                        parallel,
                        player_type,
                        multi,
                    )
                    .await;
                }
                UrlTarget::Playlist { id } => {
                    target.assert_dir();
                    let videos = if music {
                        let mut playlist = rp.query().music_playlist(id).await.unwrap();
                        playlist
                            .tracks
                            .extend_limit(&rp.query(), limit)
                            .await
                            .unwrap();
                        playlist
                            .tracks
                            .items
                            .into_iter()
                            .take(limit)
                            .map(|v| DownloadVideo::from_track(&v))
                            .collect()
                    } else {
                        let mut playlist = rp.query().playlist(id).await.unwrap();
                        playlist
                            .videos
                            .extend_limit(&rp.query(), limit)
                            .await
                            .unwrap();
                        playlist
                            .videos
                            .items
                            .into_iter()
                            .take(limit)
                            .map(|v| DownloadVideo::from_entity(&v))
                            .collect()
                    };
                    download_videos(
                        &rp,
                        videos,
                        &target,
                        resolution,
                        parallel,
                        player_type,
                        multi,
                    )
                    .await;
                }
                UrlTarget::Album { id } => {
                    target.assert_dir();
                    let album = rp.query().music_album(id).await.unwrap();
                    let videos = album
                        .tracks
                        .into_iter()
                        .take(limit)
                        .map(|v| DownloadVideo::from_track(&v))
                        .collect();
                    download_videos(
                        &rp,
                        videos,
                        &target,
                        resolution,
                        parallel,
                        player_type,
                        multi,
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
        Commands::Vdata => {
            let vd = rp.query().get_visitor_data().await.unwrap();
            println!("{vd}");
        }
    };
}
