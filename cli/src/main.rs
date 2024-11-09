#![doc = include_str!("../README.md")]
#![warn(clippy::todo, clippy::dbg_macro)]

use std::{
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use clap::{Parser, Subcommand, ValueEnum};
use const_format::formatcp;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{
        richtext::ToPlaintext, traits::YtEntity, ArtistId, MusicSearchResult, TrackItem, TrackType,
        UrlTarget, Verification, YouTubeItem,
    },
    param::{search_filter, ChannelVideoTab, Country, Language, StreamFilter},
    report::FileReporter,
};
use rustypipe_downloader::{
    DownloadError, DownloadQuery, DownloadVideo, Downloader, DownloaderBuilder,
};
use serde::Serialize;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

#[derive(Parser)]
#[clap(
    author,
    version = formatcp!("{}\nrustypipe {}", env!("CARGO_PKG_VERSION"), rustypipe::VERSION),
    about,
    long_about = None
)]
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
    /// Download to the given directory
    #[clap(short, long)]
    output: Option<PathBuf>,
    /// Download to the given file
    #[clap(long)]
    output_file: Option<PathBuf>,
    /// Download to a path determined by a template
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
        /// Download only the audio track and write track information
        #[clap(short, long)]
        audio: bool,
        /// Number of videos downloaded in parallel
        #[clap(short, long, default_value_t = 8)]
        parallel: usize,
        /// Use YouTube Music for downloading playlists
        #[clap(short, long)]
        music: bool,
        /// Limit the number of videos to download
        #[clap(short, long, default_value_t = 1000)]
        limit: usize,
        /// YT Client used to fetch player data
        #[clap(short, long)]
        client_type: Option<Vec<ClientTypeArg>>,
        /// `pot` token to circumvent bot detection
        #[clap(long)]
        pot: Option<String>,
    },
    /// Extract video, playlist, album or channel data
    Get {
        /// ID or URL
        id: String,
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
        /// Channel tab
        #[clap(short, long, default_value = "videos")]
        tab: ChannelTab,
        /// Use YouTube Music
        #[clap(short, long)]
        music: bool,
        /// Fetch the RSS feed of a channel
        #[clap(long)]
        rss: bool,
        /// Get comments
        #[clap(long)]
        comments: Option<CommentsOrder>,
        /// Get lyrics for YTM tracks
        #[clap(long)]
        lyrics: bool,
        /// Get the player data instead of the video details
        #[clap(long)]
        player: bool,
        /// YT Client used to fetch player data
        #[clap(short, long)]
        client_type: Option<Vec<ClientTypeArg>>,
    },
    /// Search YouTube
    Search {
        /// Search query
        query: String,
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
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
        /// Search YouTube Music in the given category
        #[clap(short, long)]
        music: Option<MusicSearchCategory>,
    },
    /// Get a YouTube visitor data cookie
    Vdata,
    /// Log in using your Google account
    Login,
    /// Log out from your Google account
    Logout,
}

#[derive(Default, Copy, Clone, ValueEnum)]
enum Format {
    #[default]
    Json,
    Yaml,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
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
    Users,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ClientTypeArg {
    Desktop,
    Mobile,
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

impl From<ClientTypeArg> for ClientType {
    fn from(value: ClientTypeArg) -> Self {
        match value {
            ClientTypeArg::Desktop => Self::Desktop,
            ClientTypeArg::Mobile => Self::Mobile,
            ClientTypeArg::Tv => Self::Tv,
            ClientTypeArg::Android => Self::Android,
            ClientTypeArg::Ios => Self::Ios,
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

fn print_entities(items: &[impl YtEntity], with_type: bool) {
    for e in items {
        if with_type {
            if let Some(t) = e.music_item_type() {
                anstream::print!("{: >8} ", format!("{t:?}").dimmed());
            }
        }
        anstream::print!("[{}] {}", e.id(), e.name().bold());
        if let Some(n) = e.channel_name() {
            anstream::print!(" - {}", n.cyan());
        }
        println!();
    }
}

fn print_tracks(tracks: &[TrackItem]) {
    for t in tracks {
        if let Some(n) = t.track_nr {
            anstream::print!("{} ", format!("{n:02}").yellow().bold());
        }
        anstream::print!("[{}] {} - ", t.id, t.name.bold());
        print_artists(&t.artists);
        print_duration(t.duration);
        println!();
    }
}

fn print_artists(artists: &[ArtistId]) {
    for (i, a) in artists.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        anstream::print!("{}", a.name.cyan());
        if let Some(id) = &a.id {
            print!(" [{id}]");
        }
    }
}

fn print_duration(duration: Option<u32>) {
    if let Some(d) = duration {
        print!(" ");
        let hours = d / 3600;
        let minutes = (d / 60) % 60;
        let seconds = d % 60;
        if hours > 0 {
            anstream::print!("{}", format!("{hours:02}:").yellow());
        }
        anstream::print!("{}", format!("{minutes:02}:{seconds:02}").yellow());
    }
}

fn print_music_search<T: Serialize + YtEntity>(
    data: &MusicSearchResult<T>,
    format: Option<Format>,
    pretty: bool,
    with_type: bool,
) {
    match format {
        Some(format) => print_data(data, format, pretty),
        None => {
            if let Some(corr) = &data.corrected_query {
                anstream::println!("Did you mean `{}`?", corr.magenta());
            }
            print_entities(&data.items.items, with_type);
        }
    }
}

fn print_description(desc: Option<String>) {
    if let Some(desc) = desc {
        if !desc.is_empty() {
            print_h2("Description");
            println!("{}", desc.trim());
        }
    }
}

fn print_h2(title: &str) {
    anstream::println!("\n{}", format!("{title}:").green().underline());
}

fn print_verification(verification: Verification) {
    match verification {
        Verification::None => {}
        Verification::Verified => print!(" ✓"),
        Verification::Artist => print!(" ♪"),
    }
}

async fn download_video(
    dl: &Downloader,
    id: &str,
    target: &DownloadTarget,
    client_types: Option<&[ClientType]>,
) {
    let mut q = target.apply(dl.id(id));
    if let Some(client_types) = client_types {
        q = q.client_types(client_types);
    }
    let res = q.download().await;
    if let Err(e) = res {
        tracing::error!("[{id}]: {e}")
    }
}

async fn download_videos(
    dl: &Downloader,
    videos: Vec<DownloadVideo>,
    target: &DownloadTarget,
    parallel: usize,
    client_types: Option<&[ClientType]>,
    multi: MultiProgress,
) -> anyhow::Result<()> {
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

    let n_failed = Arc::new(AtomicUsize::default());

    stream::iter(videos)
        .for_each_concurrent(parallel, |video| {
            let dl = dl.clone();
            let main = main.clone();
            let id = video.id().to_owned();
            let n_failed = n_failed.clone();

            let mut q = target.apply(dl.video(video));
            if let Some(client_types) = client_types {
                q = q.client_types(client_types);
            }

            async move {
                if let Err(e) = q.download().await {
                    if !matches!(e, DownloadError::Exists(_)) {
                        tracing::error!("[{id}]: {e}");
                        n_failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                } else {
                    main.inc(1);
                }
            }
        })
        .await;

    let n_failed = n_failed.load(std::sync::atomic::Ordering::Relaxed);
    if n_failed > 0 {
        anyhow::bail!("{n_failed} downloads failed");
    }
    Ok(())
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
    if let Err(e) = run().await {
        println!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
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

    let mut storage_dir = dirs::data_dir().expect("no data dir");
    storage_dir.push("rustypipe");
    std::fs::create_dir_all(&storage_dir).expect("could not create data dir");

    let mut rp = RustyPipe::builder()
        .storage_dir(storage_dir)
        .visitor_data_opt(cli.vdata)
        .timeout(Duration::from_secs(15));
    if cli.report {
        rp = rp
            .report()
            .reporter(Box::new(FileReporter::new("rustypipe_reports")));
    }
    if let Some(lang) = cli.lang {
        rp = rp.lang(Language::from_str(&lang.to_ascii_lowercase()).expect("invalid language"));
    }
    if let Some(country) = cli.country {
        rp = rp.country(Country::from_str(&country.to_ascii_uppercase()).expect("invalid country"));
    }
    let rp = rp.build()?;

    match cli.command {
        Commands::Download {
            id,
            target,
            resolution,
            audio,
            parallel,
            music,
            limit,
            client_type,
            pot,
        } => {
            let url_target = rp.query().resolve_string(&id, false).await?;

            let mut filter = StreamFilter::new();
            if let Some(res) = resolution {
                if res == 0 {
                    filter = filter.no_video();
                } else {
                    filter = filter.video_max_res(res);
                }
            }
            let mut dl = DownloaderBuilder::new()
                .rustypipe(&rp)
                .multi_progress(multi.clone())
                .path_precheck();
            if audio {
                dl = dl.audio_tag().crop_cover();
                filter = filter.no_video();
            }
            if let Some(pot) = pot {
                dl = dl.pot(pot);
            }
            let dl = dl.stream_filter(filter).build();

            let cts = client_type.map(|c| c.into_iter().map(ClientType::from).collect::<Vec<_>>());

            match url_target {
                UrlTarget::Video { id, .. } => {
                    download_video(&dl, &id, &target, cts.as_deref()).await;
                }
                UrlTarget::Channel { id } => {
                    target.assert_dir();
                    let mut channel = rp.query().channel_videos(id).await?;
                    channel.content.extend_limit(&rp.query(), limit).await?;
                    let videos = channel
                        .content
                        .items
                        .into_iter()
                        .take(limit)
                        .map(|v| DownloadVideo::from_entity(&v))
                        .collect();
                    download_videos(&dl, videos, &target, parallel, cts.as_deref(), multi).await?;
                }
                UrlTarget::Playlist { id } => {
                    target.assert_dir();
                    let videos = if music {
                        let mut playlist = rp.query().music_playlist(id).await?;
                        playlist.tracks.extend_limit(&rp.query(), limit).await?;
                        playlist
                            .tracks
                            .items
                            .into_iter()
                            .take(limit)
                            .map(|v| DownloadVideo::from_track(&v))
                            .collect()
                    } else {
                        let mut playlist = rp.query().playlist(id).await?;
                        playlist.videos.extend_limit(&rp.query(), limit).await?;
                        playlist
                            .videos
                            .items
                            .into_iter()
                            .take(limit)
                            .map(|v| DownloadVideo::from_entity(&v))
                            .collect()
                    };
                    download_videos(&dl, videos, &target, parallel, cts.as_deref(), multi).await?;
                }
                UrlTarget::Album { id } => {
                    target.assert_dir();
                    let album = rp.query().music_album(id).await?;
                    let videos = album
                        .tracks
                        .into_iter()
                        .take(limit)
                        .map(|v| DownloadVideo::from_track(&v))
                        .collect();
                    download_videos(&dl, videos, &target, parallel, cts.as_deref(), multi).await?;
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
            rss,
            comments,
            lyrics,
            player,
            client_type,
        } => {
            let target = rp.query().resolve_string(&id, false).await?;

            match target {
                UrlTarget::Video { id, .. } => {
                    if lyrics {
                        let details = rp.query().music_details(&id).await?;
                        match details.lyrics_id {
                            Some(lyrics_id) => {
                                let lyrics = rp.query().music_lyrics(lyrics_id).await?;
                                match format {
                                    Some(format) => print_data(&lyrics, format, pretty),
                                    None => println!("{}\n\n{}", lyrics.body, lyrics.footer.blue()),
                                }
                            }
                            None => eprintln!("no lyrics found"),
                        }
                    } else if music {
                        let details = rp.query().music_details(&id).await?;
                        match format {
                            Some(format) => print_data(&details, format, pretty),
                            None => {
                                let typ_str = match details.track.track_type {
                                    TrackType::Track => "[Track]",
                                    TrackType::Video => "[MV]",
                                    TrackType::Episode => "[Episode]",
                                };
                                anstream::println!("{}", typ_str.on_green().black());
                                anstream::print!(
                                    "{} [{}]",
                                    details.track.name.green().bold(),
                                    details.track.id
                                );
                                print_duration(details.track.duration);
                                println!();
                                print_artists(&details.track.artists);
                                println!();
                                if details.track.track_type.is_track() {
                                    anstream::println!(
                                        "{} {}",
                                        "Album:".blue(),
                                        details
                                            .track
                                            .album
                                            .as_ref()
                                            .map(|b| b.id.as_str())
                                            .unwrap_or("None")
                                    )
                                }
                                if let Some(view_count) = details.track.view_count {
                                    anstream::println!("{} {}", "Views:".blue(), view_count);
                                }
                            }
                        }
                    } else if player {
                        let player = if let Some(client_types) = client_type {
                            let cts = client_types
                                .into_iter()
                                .map(ClientType::from)
                                .collect::<Vec<_>>();
                            rp.query().player_from_clients(&id, &cts).await
                        } else {
                            rp.query().player(&id).await
                        }?;
                        print_data(&player, format.unwrap_or_default(), pretty);
                    } else {
                        let mut details = rp.query().video_details(&id).await?;

                        match comments {
                            Some(CommentsOrder::Top) => {
                                details.top_comments.extend_limit(rp.query(), limit).await?;
                            }
                            Some(CommentsOrder::Latest) => {
                                details
                                    .latest_comments
                                    .extend_limit(rp.query(), limit)
                                    .await?;
                            }
                            None => {}
                        }

                        match format {
                            Some(format) => print_data(&details, format, pretty),
                            None => {
                                anstream::println!(
                                    "{}\n{} [{}]",
                                    "[Video]".on_green().black(),
                                    details.name.green().bold(),
                                    details.id
                                );
                                anstream::println!(
                                    "{} {} [{}]",
                                    "Channel:".blue(),
                                    details.channel.name,
                                    details.channel.id
                                );
                                if let Some(subs) = details.channel.subscriber_count {
                                    anstream::println!("{} {}", "Subscribers:".blue(), subs);
                                }
                                if let Some(date) = details.publish_date {
                                    anstream::println!("{} {}", "Date:".blue(), date);
                                }
                                anstream::println!("{} {}", "Views:".blue(), details.view_count);
                                if let Some(likes) = details.like_count {
                                    anstream::println!("{} {}", "Likes:".blue(), likes);
                                }
                                if let Some(comments) = details.top_comments.count {
                                    anstream::println!("{} {}", "Comments:".blue(), comments);
                                }
                                if details.is_ccommons {
                                    anstream::println!("{}", "Creative Commons".green());
                                }
                                if details.is_live {
                                    anstream::println!("{}", "Livestream".red());
                                }
                                print_description(Some(details.description.to_plaintext()));
                                if !details.recommended.is_empty() {
                                    print_h2("Recommended");
                                    print_entities(&details.recommended.items, false);
                                }
                                let comment_list = comments.map(|c| match c {
                                    CommentsOrder::Top => &details.top_comments.items,
                                    CommentsOrder::Latest => &details.latest_comments.items,
                                });
                                if let Some(comment_list) = comment_list {
                                    print_h2("Comments");
                                    for c in comment_list {
                                        if let Some(author) = &c.author {
                                            anstream::print!(
                                                "{} [{}]",
                                                author.name.cyan(),
                                                author.id
                                            );
                                            print_verification(author.verification);
                                        } else {
                                            anstream::print!("{}", "Unknown author".magenta());
                                        }
                                        if c.by_owner {
                                            print!(" (Owner)");
                                        }
                                        println!();
                                        println!("{}", c.text.to_plaintext());
                                        anstream::print!(
                                            "{} {}",
                                            "Likes:".blue(),
                                            c.like_count.unwrap_or_default()
                                        );
                                        if c.hearted {
                                            anstream::print!(" {}", "♥".red());
                                        }
                                        println!("\n");
                                    }
                                }
                            }
                        }
                    }
                }
                UrlTarget::Channel { id } => {
                    if music {
                        let artist = rp.query().music_artist(&id, true).await?;
                        match format {
                            Some(format) => print_data(&artist, format, pretty),
                            None => {
                                anstream::println!(
                                    "{}\n{} [{}]",
                                    "[Artist]".on_green().black(),
                                    artist.name.green().bold(),
                                    artist.id
                                );
                                if let Some(subs) = artist.subscriber_count {
                                    anstream::println!("{} {}", "Subscribers:".blue(), subs);
                                }
                                if let Some(url) = artist.wikipedia_url {
                                    anstream::println!("{} {}", "Wikipedia:".blue(), url);
                                }
                                if let Some(id) = artist.tracks_playlist_id {
                                    anstream::println!("{} {}", "All tracks:".blue(), id);
                                }
                                if let Some(id) = artist.videos_playlist_id {
                                    anstream::println!("{} {}", "All videos:".blue(), id);
                                }
                                if let Some(id) = artist.radio_id {
                                    anstream::println!("{} {}", "Radio:".blue(), id);
                                }
                                print_description(artist.description);
                                if !artist.albums.is_empty() {
                                    print_h2("Albums");
                                    for b in artist.albums {
                                        anstream::print!(
                                            "[{}] {} ({:?}",
                                            b.id,
                                            b.name.bold(),
                                            b.album_type
                                        );
                                        if let Some(y) = b.year {
                                            print!(", {y}");
                                        }
                                        println!(")");
                                    }
                                }
                                if !artist.playlists.is_empty() {
                                    print_h2("Playlists");
                                    print_entities(&artist.playlists, false);
                                }
                                if !artist.similar_artists.is_empty() {
                                    print_h2("Similar artists");
                                    print_entities(&artist.similar_artists, false);
                                }
                            }
                        }
                    } else if rss {
                        let rss = rp.query().channel_rss(&id).await?;

                        match format {
                            Some(format) => print_data(&rss, format, pretty),
                            None => {
                                anstream::println!(
                                    "{}\n{} [{}]\n{} {}",
                                    "[Channel RSS]".on_green().black(),
                                    rss.name.green().bold(),
                                    rss.id,
                                    "Created on:".blue(),
                                    rss.create_date,
                                );
                                if let Some(v) = rss.videos.first() {
                                    anstream::println!(
                                        "{} {} [{}]",
                                        "Latest video:".blue(),
                                        v.publish_date,
                                        v.id
                                    );
                                }
                                println!();
                                print_entities(&rss.videos, false);
                            }
                        }
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
                                    rp.query().channel_videos_tab(&id, video_tab).await?;

                                channel.content.extend_limit(rp.query(), limit).await?;

                                match format {
                                    Some(format) => print_data(&channel, format, pretty),
                                    None => {
                                        anstream::print!(
                                            "{}\n{} {} [{}]",
                                            format!("[Channel {tab:?}]").on_green().black(),
                                            channel.name.green().bold(),
                                            channel.handle.unwrap_or_default(),
                                            channel.id
                                        );
                                        print_verification(channel.verification);
                                        println!();
                                        if let Some(subs) = channel.subscriber_count {
                                            anstream::println!(
                                                "{} {}",
                                                "Subscribers:".blue(),
                                                subs
                                            );
                                        }
                                        if let Some(vids) = channel.video_count {
                                            anstream::println!("{} {}", "Videos:".blue(), vids);
                                        }
                                        print_description(Some(channel.description));
                                        println!();
                                        print_entities(&channel.content.items, false);
                                    }
                                }
                            }
                            ChannelTab::Playlists => {
                                let channel = rp.query().channel_playlists(&id).await?;

                                match format {
                                    Some(format) => print_data(&channel, format, pretty),
                                    None => {
                                        anstream::println!(
                                            "{}\n{} {} [{}]",
                                            format!("[Channel {tab:?}]").on_green().black(),
                                            channel.name.green().bold(),
                                            channel.handle.unwrap_or_default(),
                                            channel.id
                                        );
                                        print_description(Some(channel.description));
                                        if let Some(subs) = channel.subscriber_count {
                                            anstream::println!(
                                                "{} {}",
                                                "Subscribers:".blue(),
                                                subs
                                            );
                                        }
                                        if let Some(vids) = channel.video_count {
                                            anstream::println!("{} {}", "Videos:".blue(), vids);
                                        }
                                        println!();
                                        print_entities(&channel.content.items, false);
                                    }
                                }
                            }
                            ChannelTab::Info => {
                                let info = rp.query().channel_info(&id).await?;

                                match format {
                                    Some(format) => print_data(&info, format, pretty),
                                    None => {
                                        anstream::println!(
                                            "{}\n<b>ID:</b>{}",
                                            "[Channel info]".on_green().black(),
                                            info.id
                                        );
                                        print_description(Some(info.description));
                                        if let Some(subs) = info.subscriber_count {
                                            anstream::println!(
                                                "{} {}",
                                                "Subscribers:".blue(),
                                                subs
                                            );
                                        }
                                        if let Some(vids) = info.video_count {
                                            anstream::println!("{} {}", "Videos:".blue(), vids);
                                        }
                                        if let Some(views) = info.view_count {
                                            anstream::println!("{} {}", "Views:".blue(), views);
                                        }
                                        if let Some(created) = info.create_date {
                                            anstream::println!(
                                                "{} {}",
                                                "Created on:".blue(),
                                                created
                                            );
                                        }
                                        if let Some(country) = info.country {
                                            anstream::println!("{} {}", "Country:".blue(), country);
                                        }
                                        if !info.links.is_empty() {
                                            print_h2("Links");
                                            for (name, url) in &info.links {
                                                anstream::println!("{} {}", name.blue(), url);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                UrlTarget::Playlist { id } => {
                    if music {
                        let mut playlist = rp.query().music_playlist(&id).await?;
                        playlist.tracks.extend_limit(rp.query(), limit).await?;
                        match format {
                            Some(format) => print_data(&playlist, format, pretty),
                            None => {
                                anstream::println!(
                                    "{}\n{} [{}]\n{} {}",
                                    "[MusicPlaylist]".on_green().black(),
                                    playlist.name.green().bold(),
                                    playlist.id,
                                    "Tracks:".blue(),
                                    playlist.track_count.unwrap_or_default(),
                                );
                                if let Some(n) = playlist.channel_name() {
                                    anstream::print!("{} {}", "Author:".blue(), n.bold());
                                    if let Some(id) = playlist.channel_id() {
                                        print!(" [{id}]");
                                    }
                                    println!();
                                }
                                print_description(playlist.description.map(|d| d.to_plaintext()));
                                println!();
                                print_tracks(&playlist.tracks.items);
                            }
                        }
                    } else {
                        let mut playlist = rp.query().playlist(&id).await?;
                        playlist.videos.extend_limit(rp.query(), limit).await?;
                        match format {
                            Some(format) => print_data(&playlist, format, pretty),
                            None => {
                                anstream::println!(
                                    "{}\n{} [{}]\n{} {}",
                                    "[Playlist]".on_green().black(),
                                    playlist.name.green().bold(),
                                    playlist.id,
                                    "Videos:".blue(),
                                    playlist.video_count,
                                );
                                if let Some(n) = playlist.channel_name() {
                                    anstream::print!("{} {}", "Author:".blue(), n.bold());
                                    if let Some(id) = playlist.channel_id() {
                                        print!(" [{id}]");
                                    }
                                    println!();
                                }
                                if let Some(last_update) = playlist.last_update {
                                    anstream::println!("{} {}", "Last update:".blue(), last_update);
                                }
                                print_description(playlist.description.map(|d| d.to_plaintext()));
                                println!();
                                print_entities(&playlist.videos.items, false);
                            }
                        }
                    }
                }
                UrlTarget::Album { id } => {
                    let album = rp.query().music_album(&id).await?;
                    match format {
                        Some(format) => print_data(&album, format, pretty),
                        None => {
                            anstream::print!(
                                "{}\n{} [{}] ({:?}",
                                "[Album]".on_green().black(),
                                album.name.green().bold(),
                                album.id,
                                album.album_type
                            );
                            if let Some(year) = album.year {
                                print!(", {year}");
                            }
                            println!(")");
                            if let Some(n) = album.channel_name() {
                                anstream::print!("{} {}", "Artist:".blue(), n);
                                if let Some(id) = album.channel_id() {
                                    print!(" [{id}]");
                                }
                            }
                            print_description(album.description.map(|d| d.to_plaintext()));
                            println!();
                            print_tracks(&album.tracks);
                        }
                    }
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
                Some(channel_id) => {
                    rustypipe::validate::channel_id(&channel_id)?;
                    let channel = rp.query().channel_search(&channel_id, &query).await?;

                    match format {
                        Some(format) => print_data(&channel, format, pretty),
                        None => {
                            anstream::print!(
                                "{}\n{} [{}]",
                                "[Channel search]".on_green().black(),
                                channel.name.green().bold(),
                                channel.id
                            );
                            print_verification(channel.verification);
                            println!();
                            if let Some(subs) = channel.subscriber_count {
                                anstream::println!("{} {}", "Subscribers:".blue(), subs);
                            }
                            print_description(Some(channel.description));
                            println!();
                            print_entities(&channel.content.items, false);
                        }
                    }
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
                        .await?;
                    res.items.extend_limit(rp.query(), limit).await?;

                    match format {
                        Some(format) => print_data(&res, format, pretty),
                        None => {
                            if let Some(corr) = res.corrected_query {
                                anstream::println!("Did you mean `{}`?", corr.magenta());
                            }
                            print_entities(&res.items.items, false);
                        }
                    }
                }
            },
            Some(MusicSearchCategory::All) => {
                let res = rp.query().music_search_main(&query).await?;
                print_music_search(&res, format, pretty, true);
            }
            Some(MusicSearchCategory::Tracks) => {
                let mut res = rp.query().music_search_tracks(&query).await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
            Some(MusicSearchCategory::Videos) => {
                let mut res = rp.query().music_search_videos(&query).await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
            Some(MusicSearchCategory::Artists) => {
                let mut res = rp.query().music_search_artists(&query).await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
            Some(MusicSearchCategory::Albums) => {
                let mut res = rp.query().music_search_albums(&query).await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
            Some(MusicSearchCategory::PlaylistsYtm | MusicSearchCategory::PlaylistsCommunity) => {
                let mut res = rp
                    .query()
                    .music_search_playlists(
                        &query,
                        music == Some(MusicSearchCategory::PlaylistsCommunity),
                    )
                    .await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
            Some(MusicSearchCategory::Users) => {
                let mut res = rp.query().music_search_users(&query).await?;
                res.items.extend_limit(rp.query(), limit).await?;
                print_music_search(&res, format, pretty, false);
            }
        },
        Commands::Vdata => {
            let vd = rp.query().get_visitor_data().await?;
            println!("{vd}");
        }
        Commands::Login => {
            match rp.user_auth_check_login().await {
                Ok(_) => {}
                Err(rustypipe::error::Error::Auth(_)) => {
                    let device_code = rp.user_auth_get_code().await?;
                    println!(
                        "Open {} and enter the following code:",
                        device_code.verification_url
                    );
                    anstream::println!("{}", device_code.user_code.blue());
                    rp.user_auth_wait_for_login(&device_code).await?;
                }
                Err(e) => return Err(e.into()),
            }
            anstream::println!("{}", "Logged in.".green());
        }
        Commands::Logout => {
            rp.user_auth_logout().await?;
            anstream::println!("{}", "Logged out.".red());
        }
    };
    Ok(())
}
