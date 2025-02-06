#![doc = include_str!("../README.md")]
#![warn(clippy::todo, clippy::dbg_macro)]

use std::{
    ffi::OsString,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use clap::{Parser, Subcommand, ValueEnum};
use const_format::formatcp;
use futures_util::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use rustypipe::{
    cache::FileStorage,
    client::{ClientType, RustyPipe},
    model::{
        richtext::{RichText, ToPlaintext},
        traits::YtEntity,
        ArtistId, AudioCodec, Comment, MusicSearchResult, TrackItem, TrackType, UrlTarget,
        Verification, YouTubeItem,
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
    /// YouTube visitor data ID
    #[clap(long, global = true)]
    vdata: Option<String>,
    /// YouTube content language
    #[clap(long, global = true)]
    lang: Option<String>,
    /// YouTube content country
    #[clap(long, global = true)]
    country: Option<String>,
    /// Use authentication
    #[clap(long, global = true)]
    auth: bool,
    #[clap(long, global = true)]
    /// RustyPipe cache file
    cache_file: Option<PathBuf>,
    /// RustyPipe report folder
    #[clap(long, global = true)]
    report_dir: Option<PathBuf>,
    /// Path to rustypipe-botguard binary
    #[clap(long, global = true)]
    botguard_bin: Option<OsString>,
    /// Disable Botguard
    #[clap(long, global = true)]
    no_botguard: bool,
    /// Enable caching for session-bound PO tokens
    #[clap(long, global = true)]
    pot_cache: bool,
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
    /// Get your playback history
    History {
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
        /// Use YouTube Music
        #[clap(short, long)]
        music: bool,
        /// Search YouTube playback history
        #[clap(long)]
        search: Option<String>,
    },
    /// Get the channels you subscribed to
    Subscriptions {
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<SubscriptionFormat>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
        /// Use YouTube Music
        #[clap(short, long)]
        music: bool,
        /// Output YouTube subscription feed
        #[clap(long)]
        feed: bool,
    },
    /// Get the playlists from your library
    Playlists {
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
        /// Use YouTube Music
        #[clap(short, long)]
        music: bool,
    },
    /// Get the albums from your library
    Albums {
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// Get the tracks from your library
    Tracks {
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
        /// Limit the number of items to fetch
        #[clap(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// Get the latest music releases
    Releases {
        /// Get latest music videos
        #[clap(long)]
        videos: bool,
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
    },
    /// Get YouTube Music charts
    Charts {
        /// Chart country
        country: Option<String>,
        /// Output format
        #[clap(short, long, value_parser)]
        format: Option<Format>,
        /// Pretty-print output
        #[clap(long)]
        pretty: bool,
    },
    /// Get a YouTube visitor data ID
    Vdata,
    /// Log in using your Google account
    Login {
        /// Log in using YouTube cookies (otherwise OAuth is used)
        #[clap(long)]
        cookie: bool,
        /// Path to cookie.txt
        #[clap(long)]
        cookies_txt: Option<PathBuf>,
    },
    /// Log out from your Google account
    Logout {
        /// Remove stored YouTube cookies (otherwise OAuth is used)
        #[clap(long)]
        cookie: bool,
    },
}

#[derive(Default, Copy, Clone, ValueEnum)]
enum Format {
    #[default]
    Json,
    Yaml,
}

#[derive(Debug, Default, Copy, Clone, ValueEnum)]
enum SubscriptionFormat {
    #[default]
    Json,
    Yaml,
    Newpipe,
    Opml,
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

#[derive(Serialize)]
struct NewpipeSubscriptions {
    app_version: &'static str,
    app_version_int: u16,
    subscriptions: Vec<NewpipeSubscription>,
}

#[derive(Serialize)]
struct NewpipeSubscription {
    service_id: u16,
    url: String,
    name: String,
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

impl From<SubscriptionFormat> for Format {
    fn from(value: SubscriptionFormat) -> Self {
        match value {
            SubscriptionFormat::Json => Self::Json,
            SubscriptionFormat::Yaml => Self::Yaml,
            _ => Self::default(),
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
        print_entity(e, with_type);
    }
}

fn print_entity(e: &impl YtEntity, with_type: bool) {
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

fn fmt_print_entities<T: YtEntity + Serialize>(
    items: &[T],
    format: Option<Format>,
    pretty: bool,
    title: &str,
) {
    match format {
        Some(format) => print_data(&items, format, pretty),
        None => {
            print_h1(title);
            print_entities(items, false);
        }
    }
}

fn fmt_print_tracks(tracks: &[TrackItem], format: Option<Format>, pretty: bool, title: &str) {
    match format {
        Some(format) => print_data(&tracks, format, pretty),
        None => {
            print_h1(title);
            print_tracks(tracks);
        }
    }
}

fn fmt_print_subscriptions<T: YtEntity + Serialize>(
    items: &[T],
    format: Option<SubscriptionFormat>,
    pretty: bool,
    title: &str,
) {
    match format {
        Some(SubscriptionFormat::Newpipe) => {
            let subscriptions = items
                .iter()
                .map(|itm| NewpipeSubscription {
                    service_id: 0,
                    url: format!("https://www.youtube.com/channel/{}", itm.id()),
                    name: itm.name().to_owned(),
                })
                .collect();
            let data = NewpipeSubscriptions {
                app_version: "0.24.1",
                app_version_int: 991,
                subscriptions,
            };
            print_data(&data, Format::Json, pretty);
        }
        Some(SubscriptionFormat::Opml) => {
            let mut writer = if pretty {
                quick_xml::Writer::new_with_indent(std::io::stdout(), b' ', 2)
            } else {
                quick_xml::Writer::new(std::io::stdout())
            };
            writer
                .create_element("opml")
                .with_attribute(("version", "1.1"))
                .write_inner_content(|writer| {
                    writer
                        .create_element("body")
                        .write_inner_content(|writer| {
                            writer
                                .create_element("outline")
                                .with_attributes([
                                    ("text", title),
                                    ("title", title),
                                ])
                                .write_inner_content(|writer| {
                                    for itm in items {
                                        writer
                                            .create_element("outline")
                                            .with_attributes([
                                                ("text", itm.name()),
                                                ("title", itm.name()),
                                                ("type", "rss"),
                                                ("xmlUrl", &format!("https://www.youtube.com/feeds/videos.xml?channel_id={}", itm.id())),
                                            ])
                                            .write_empty()?;
                                    }
                                    Ok(())
                                })?;
                            Ok(())
                        })?;
                    Ok(())
                })
                .unwrap();
            println!();
        }
        Some(format) => print_data(&items, format.into(), pretty),
        None => {
            print_h1(title);
            print_entities(items, false);
        }
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
            print_h1("Music search");
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

fn print_h1(title: &str) {
    anstream::println!("{}", format!("{title}:").on_green().black());
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

fn print_comments(comments: &[Comment]) {
    print_h2("Comments");
    for c in comments {
        if let Some(author) = &c.author {
            anstream::print!("{} [{}]", author.name.cyan(), author.id);
            print_verification(author.verification);
        } else {
            anstream::print!("{}", "Unknown author".magenta());
        }
        if c.by_owner {
            print!(" (Owner)");
        }
        println!();
        print_richtext(&c.text);
        anstream::print!("{} {}", "Likes:".blue(), c.like_count.unwrap_or_default());
        if c.hearted {
            anstream::print!(" {}", "♥".red());
        }
        println!();
        if let Some(ctoken) = &c.replies.ctoken {
            println!("replies:{ctoken}");
        }
        println!();
    }
}

fn print_richtext(text: &RichText) {
    for c in &text.0 {
        match c {
            rustypipe::model::richtext::TextComponent::Text { text, style } => {
                if !text.is_empty() {
                    let mut tstyle = owo_colors::Style::new();

                    if style.bold {
                        tstyle = tstyle.bold();
                    }
                    if style.italic {
                        tstyle = tstyle.italic();
                    }
                    if style.strikethrough {
                        tstyle = tstyle.strikethrough();
                    }
                    anstream::print!("{}", text.style(tstyle));
                }
            }
            rustypipe::model::richtext::TextComponent::Web { url, .. } => {
                anstream::print!("{}", url.underline());
            }
            rustypipe::model::richtext::TextComponent::YouTube { text, target } => {
                if matches!(target, UrlTarget::Channel { .. }) {
                    anstream::print!("{}", text.cyan());
                } else {
                    anstream::print!("{}", target.to_url().underline());
                }
            }
            _ => {}
        }
    }
    println!();
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

    if cli.cache_file.is_none() || cli.report_dir.is_none() {
        std::fs::create_dir_all(&storage_dir).expect("could not create data dir");
    }

    let mut rp = RustyPipe::builder()
        .storage_dir(storage_dir)
        .visitor_data_opt(cli.vdata)
        .timeout(Duration::from_secs(15));

    if let Some(cache_file) = cli.cache_file {
        rp = rp.storage(Box::new(FileStorage::new(cache_file)));
    }
    if let Some(report_dir) = cli.report_dir {
        rp = rp.reporter(Box::new(FileReporter::new(report_dir)));
    }

    if cli.report {
        rp = rp
            .report()
            .reporter(Box::new(FileReporter::new("rustypipe_reports")));
    }
    if let Some(lang) = cli.lang {
        rp = rp.lang(Language::from_str(&lang).expect("invalid language"));
    }
    if let Some(country) = cli.country {
        rp = rp.country(Country::from_str(&country.to_ascii_uppercase()).expect("invalid country"));
    }
    if let Some(botguard_bin) = cli.botguard_bin {
        rp = rp.botguard_bin(botguard_bin);
    }
    if cli.no_botguard {
        rp = rp.no_botguard();
    }
    if cli.pot_cache {
        rp = rp.po_token_cache();
    }
    if cli.auth {
        rp = rp.authenticated();
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
        } => {
            let url_target = rp.query().resolve_string(&id, false).await?;

            let mut filter = StreamFilter::new();
            if let Some(res) = resolution {
                if res == 0 {
                    filter = filter
                        .no_video()
                        .audio_codecs([AudioCodec::Mp4a, AudioCodec::Opus]);
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
            if let Some(ctoken) = id.strip_prefix("replies:") {
                let mut replies = rp.query().video_comments(ctoken, None).await?;
                replies.extend_limit(&rp.query(), limit).await?;
                match format {
                    Some(format) => print_data(&replies.items, format, pretty),
                    None => print_comments(&replies.items),
                }
                return Ok(());
            }

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
                                print_richtext(&details.description);
                                if !details.recommended.is_empty() {
                                    print_h2("Recommended");
                                    print_entities(&details.recommended.items, false);
                                }
                                let comment_list = comments.map(|c| match c {
                                    CommentsOrder::Top => &details.top_comments.items,
                                    CommentsOrder::Latest => &details.latest_comments.items,
                                });
                                if let Some(comment_list) = comment_list {
                                    print_comments(comment_list)
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
                            print_h1("Search");
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
        Commands::History {
            format,
            pretty,
            limit,
            music,
            search,
        } => {
            if music {
                let mut history = rp.query().music_history().await?;
                history.extend_limit(rp.query(), limit).await?;

                match format {
                    Some(format) => print_data(&history, format, pretty),
                    None => {
                        anstream::println!("{}", "[Music history]".on_green().black());

                        let mut last_date = None;
                        for item in history.items {
                            if last_date != item.playback_date {
                                println!();
                                if let Some(dt) = item.playback_date {
                                    anstream::println!("{}", dt.green().underline());
                                }
                                last_date = item.playback_date;
                            }

                            let t = item.item;
                            anstream::print!("[{}] {} - ", t.id, t.name.bold());
                            print_artists(&t.artists);
                            print_duration(t.duration);
                            println!();
                        }
                    }
                }
            } else {
                let mut history = match search {
                    Some(query) => rp.query().history_search(query).await?,
                    None => rp.query().history().await?,
                };
                history.extend_limit(rp.query(), limit).await?;

                match format {
                    Some(format) => print_data(&history, format, pretty),
                    None => {
                        anstream::println!("{}", "[History]".on_green().black());

                        let mut last_date = None;
                        for item in history.items {
                            if last_date != item.playback_date {
                                println!();
                                if let Some(dt) = item.playback_date {
                                    anstream::println!("{}", dt.green().underline());
                                }
                                last_date = item.playback_date;
                            }
                            print_entity(&item.item, false);
                        }
                    }
                }
            }
        }
        Commands::Subscriptions {
            format,
            pretty,
            limit,
            music,
            feed,
        } => {
            if music {
                let mut subscriptions = rp.query().music_saved_artists().await?;
                subscriptions.extend_limit(rp.query(), limit).await?;
                fmt_print_subscriptions(
                    &subscriptions.items,
                    format,
                    pretty,
                    "YouTube Music artists",
                );
            } else if feed {
                let mut feed = rp.query().subscription_feed().await?;
                feed.extend_limit(rp.query(), limit).await?;
                fmt_print_entities(&feed.items, format.map(Format::from), pretty, "Feed");
            } else {
                let mut subscriptions = rp.query().subscriptions().await?;
                subscriptions.extend_limit(rp.query(), limit).await?;
                fmt_print_subscriptions(
                    &subscriptions.items,
                    format,
                    pretty,
                    "YouTube subscriptions",
                );
            }
        }
        Commands::Playlists {
            format,
            pretty,
            limit,
            music,
        } => {
            if music {
                let mut playlists = rp.query().music_saved_playlists().await?;
                playlists.extend_limit(rp.query(), limit).await?;
                fmt_print_entities(&playlists.items, format, pretty, "Music playlists");
            } else {
                let mut playlists = rp.query().saved_playlists().await?;
                playlists.extend_limit(rp.query(), limit).await?;
                fmt_print_entities(&playlists.items, format, pretty, "Saved playlists");
            }
        }
        Commands::Albums {
            format,
            pretty,
            limit,
        } => {
            let mut albums = rp.query().music_saved_albums().await?;
            albums.extend_limit(rp.query(), limit).await?;
            fmt_print_entities(&albums.items, format, pretty, "Saved albums");
        }
        Commands::Tracks {
            format,
            pretty,
            limit,
        } => {
            let mut tracks = rp.query().music_saved_tracks().await?;
            tracks.extend_limit(rp.query(), limit).await?;
            fmt_print_tracks(&tracks.items, format, pretty, "Saved tracks");
        }
        Commands::Releases {
            videos,
            format,
            pretty,
        } => {
            if videos {
                let releases = rp.query().music_new_videos().await?;
                fmt_print_tracks(&releases, format, pretty, "New music videos");
            } else {
                let releases = rp.query().music_new_albums().await?;
                fmt_print_entities(&releases, format, pretty, "New albums");
            }
        }
        Commands::Charts {
            country,
            format,
            pretty,
        } => {
            let country = match country {
                Some(c) => Some(Country::from_str(&c)?),
                None => None,
            };
            let charts = rp.query().music_charts(country).await?;
            match format {
                Some(format) => print_data(&charts, format, pretty),
                None => {
                    print_h1("Music charts");
                    if let Some(plid) = &charts.top_playlist_id {
                        print_h2(&format!("Top tracks [{plid}]"));
                    } else {
                        print_h2("Top tracks");
                    }
                    print_tracks(&charts.top_tracks);
                    if let Some(plid) = &charts.trending_playlist_id {
                        print_h2(&format!("Trending [{plid}]"));
                    } else {
                        print_h2("Trending");
                    }
                    print_tracks(&charts.trending_tracks);
                    print_h2("Artists");
                    print_entities(&charts.artists, false);

                    if !charts.playlists.is_empty() {
                        print_h2("Playlists");
                        print_entities(&charts.playlists, false);
                    }
                }
            }
        }
        Commands::Vdata => {
            let vd = rp.query().get_visitor_data(true).await?;
            println!("{vd}");
        }
        Commands::Login {
            cookie,
            cookies_txt,
        } => {
            if cookie || cookies_txt.is_some() {
                match rp.user_auth_check_cookie().await {
                    Ok(_) => {
                        println!("Already logged in.");
                    }
                    Err(rustypipe::error::Error::Auth(_)) => {
                        let cookie_raw = if let Some(cookie_txt) = cookies_txt {
                            std::fs::read_to_string(cookie_txt)?
                        } else {
                            println!("Enter cookie header or cookies.txt:");

                            // Read until 2 consecutive newlines
                            let mut line = String::new();
                            let mut last_len = 0;
                            let mut stop = 0;
                            while stop < 2 {
                                std::io::stdin().read_line(&mut line)?;
                                if line.len() <= last_len + 1 {
                                    stop += 1;
                                } else {
                                    stop = 0;
                                }
                                last_len = line.len();
                            }
                            line
                        };

                        if cookie_raw.contains('\t') {
                            rp.user_auth_set_cookie_txt(&cookie_raw).await?;
                        } else {
                            rp.user_auth_set_cookie(cookie_raw.trim()).await?;
                        }
                        anstream::println!("{}", "Logged in.".green());
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                match rp.user_auth_check_login().await {
                    Ok(_) => {
                        println!("Already logged in.");
                    }
                    Err(rustypipe::error::Error::Auth(_)) => {
                        let device_code = rp.user_auth_get_code().await?;
                        println!(
                            "Open {} and enter the following code:",
                            device_code.verification_url
                        );
                        anstream::println!("{}", device_code.user_code.blue());
                        rp.user_auth_wait_for_login(&device_code).await?;
                        anstream::println!("{}", "Logged in.".green());
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
        Commands::Logout { cookie } => {
            if cookie {
                rp.user_auth_remove_cookie().await?;
            } else {
                rp.user_auth_logout().await?;
            }
            anstream::println!("{}", "Logged out.".red());
        }
    };
    Ok(())
}
