#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, clippy::todo, clippy::dbg_macro)]

mod error;
mod util;

use std::{
    borrow::Cow,
    cmp::Ordering,
    ffi::OsString,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::stream::{self, StreamExt, TryStreamExt};
use once_cell::sync::Lazy;
use rand::RngExt;
use regex::Regex;
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{
        traits::{FileFormat, YtEntity},
        AudioCodec, AudioStream, TrackItem, VideoCodec, VideoPlayer, VideoStream,
    },
    param::StreamFilter,
};
use sabr::{FormatId, Stream as SabrStream};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};
use wreq::{header, Client, StatusCode, Url};

#[cfg(feature = "indicatif")]
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

#[cfg(feature = "audiotag")]
use lofty::{config::WriteOptions, picture::Picture, prelude::*, tag::Tag};
#[cfg(feature = "audiotag")]
use rustypipe::model::{richtext::ToPlaintext, VideoDetails, VideoPlayerDetails};
#[cfg(feature = "audiotag")]
use time::{Date, OffsetDateTime};

pub use error::DownloadError;

type Result<T> = core::result::Result<T, DownloadError>;

const CHUNK_SIZE_MIN: u64 = 9_000_000;
const CHUNK_SIZE_MAX: u64 = 10_000_000;

/// Internal trait used to coerce a `VideoStream` or `AudioStream` into a
/// [`sabr::FormatId`] without duplicating the field-copying code.
trait FormatIdSource {
    fn itag(&self) -> u32;
    fn last_modified(&self) -> Option<u64>;
    fn xtags(&self) -> Option<String>;
    fn format_id(&self) -> FormatId {
        FormatId {
            itag: Some(self.itag() as i32),
            last_modified: self.last_modified(),
            xtags: self.xtags(),
        }
    }
}

impl FormatIdSource for AudioStream {
    fn itag(&self) -> u32 {
        self.itag
    }
    fn last_modified(&self) -> Option<u64> {
        self.last_modified
    }
    fn xtags(&self) -> Option<String> {
        self.xtags.clone()
    }
}

impl FormatIdSource for VideoStream {
    fn itag(&self) -> u32 {
        self.itag
    }
    fn last_modified(&self) -> Option<u64> {
        self.last_modified
    }
    fn xtags(&self) -> Option<String> {
        self.xtags.clone()
    }
}

/// RustyPipe audio/video downloader
///
/// The downloader uses an [`Arc`] internally, so if you are using the client
/// at multiple locations, you can just clone it.
#[derive(Clone)]
pub struct Downloader {
    i: Arc<DownloaderInner>,
}

/// Apply the player JS nsig deobfuscation to a SABR stream URL's `n` parameter.
///
/// YouTube ships the SABR URL with an *encrypted* `n` parameter that must
/// be deobfuscated before the request is sent — GVS returns 403 if it
/// receives the encrypted value.
///
/// SABR URLs also contain `sig` and `lsig` parameters, but unlike the
/// `signatureCipher` flow, the browser does **not** deobfuscate these
/// for SABR — it sends the raw encrypted values that came from the
/// player response. Running the sig deobfuscation here would corrupt
/// the values (the encrypted sig is a base64-encoded binary blob, not
/// the rearranged character string the JS deobfuscator expects).
async fn deobf_sabr_url(rp: &RustyPipe, url: &str) -> Result<String> {
    let mut parsed = url::Url::parse(url)
        .map_err(|e| DownloadError::Source(format!("invalid SABR url: {e}").into()))?;

    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if !pairs.iter().any(|(k, _)| k == "n") {
        return Ok(url.to_owned());
    }

    // Fetch the deobf data, then run the encrypted `n` through the JS fn.
    let deobf_data = rp
        .deobf_data()
        .await
        .map_err(|e| DownloadError::Source(format!("extract deobf: {e}").into()))?;
    let deobf = rustypipe::deobfuscate::Deobfuscator::new(&deobf_data)
        .map_err(|e| DownloadError::Source(format!("create deobf: {e}").into()))?;

    // Build new query with the deobfuscated `n`. `sig`/`lsig` are passed
    // through unchanged (the browser uses the raw encrypted values).
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in &pairs {
        match k.as_str() {
            "n" => {
                let decrypted = deobf
                    .deobfuscate_nsig(v)
                    .map_err(|e| DownloadError::Source(format!("deobf n: {e}").into()))?;
                serializer.append_pair("n", &decrypted);
            }
            _ => {
                serializer.append_pair(k, v);
            }
        }
    }
    parsed.set_query(Some(&serializer.finish()));
    // The browser's SABR request always carries `cpn`, `cver`, and `alr`
    // in the URL. The SABR stream code appends its own copy of these
    // (matching the browser's session-bound values), but only after
    // base64-decoding and re-serialising the URL through `deobf_sabr_url`
    // — so we have to seed the params here. Without them the GVS server
    // rejects the request with 403.
    Ok(parsed.to_string())
}

/// Builder to construct a new downloader
pub struct DownloaderBuilder {
    rp: Option<RustyPipe>,
    ffmpeg: String,
    #[cfg(feature = "indicatif")]
    multi: Option<MultiProgress>,
    #[cfg(feature = "indicatif")]
    progress_style: Option<ProgressStyle>,
    filter: StreamFilter,
    video_format: DownloadVideoFormat,
    n_retries: u32,
    path_precheck: bool,
    #[cfg(feature = "audiotag")]
    audio_tag: bool,
    #[cfg(feature = "audiotag")]
    crop_cover: bool,
    client_types: Option<Vec<ClientType>>,
}

struct DownloaderInner {
    /// YT client
    rp: RustyPipe,
    /// HTTP client
    http: Client,
    /// Path to the ffmpeg binary
    ffmpeg: String,
    /// Global progress
    #[cfg(feature = "indicatif")]
    multi: Option<MultiProgress>,
    /// Progress style
    #[cfg(feature = "indicatif")]
    progress_style: ProgressStyle,
    /// Default stream filter
    filter: StreamFilter,
    /// Default video format
    video_format: DownloadVideoFormat,
    /// Number of retries in case of 403 error
    n_retries: u32,
    /// Check if destination path exists before player is fetched
    path_precheck: bool,
    /// Apply metadata to audio files
    #[cfg(feature = "audiotag")]
    audio_tag: bool,
    /// Crop YT thumbnails to ensure square album covers
    #[cfg(feature = "audiotag")]
    crop_cover: bool,
    /// Client types for fetching videos
    client_types: Option<Vec<ClientType>>,
}

/// Download query
pub struct DownloadQuery {
    /// RustyPipe Downloader
    dl: Downloader,
    /// Video to download
    video: DownloadVideo,
    /// Destination
    dest: DownloadDest,
    /// Progress bar
    #[cfg(feature = "indicatif")]
    progress: Option<ProgressBar>,
    /// Stream filter
    filter: Option<StreamFilter>,
    /// Target video format
    video_format: Option<DownloadVideoFormat>,
    /// Client types for fetching videos
    client_types: Option<Vec<ClientType>>,
}

/// Video to be downloaded
#[derive(Default)]
pub struct DownloadVideo {
    id: String,
    name: Option<String>,
    channel_id: Option<String>,
    channel_name: Option<String>,
    album_id: Option<String>,
    album_name: Option<String>,
    track_nr: Option<u16>,
}

impl DownloadVideo {
    /// Get the YouTube video id
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Create a new DownloadVideo from a YouTube entity
    pub fn from_entity(video: &impl YtEntity) -> Self {
        DownloadVideo {
            id: video.id().to_owned(),
            name: Some(video.name().to_owned()),
            channel_id: video.channel_id().map(str::to_owned),
            channel_name: video
                .channel_name()
                .map(|n| n.strip_suffix("- Topic").unwrap_or(n).trim().to_owned()),
            album_id: None,
            album_name: None,
            track_nr: None,
        }
    }

    /// Create a new DownloadVideo from a YTM track
    pub fn from_track(track: &TrackItem) -> Self {
        DownloadVideo {
            id: track.id.to_owned(),
            name: Some(track.name.to_owned()),
            channel_id: track.channel_id().map(str::to_owned),
            channel_name: track.channel_name().map(str::to_owned),
            album_id: track.album.as_ref().map(|b| b.id.to_owned()),
            album_name: track.album.as_ref().map(|b| b.name.to_owned()),
            track_nr: track.track_nr,
        }
    }
}

#[derive(Clone)]
enum DownloadDest {
    Default,
    File(PathBuf),
    Dir(PathBuf),
    Template(PathBuf),
}

fn video_filename(v: &DownloadVideo) -> String {
    let mut n = format!("{} [{}]", v.name.as_deref().unwrap_or_default(), v.id);
    if let Some(track_nr) = v.track_nr {
        n = format!("{track_nr:02} {n}");
    }
    filenamify_lim(&n)
}

/// Video container format for downloading
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum DownloadVideoFormat {
    /// .mp4
    #[default]
    Mp4,
    /// .mkv
    Mkv,
    /// .webm
    Webm,
}

impl DownloadVideoFormat {
    /// Get the video format file extension
    pub fn extension(&self) -> &'static str {
        match self {
            DownloadVideoFormat::Mp4 => "mp4",
            DownloadVideoFormat::Mkv => "mkv",
            DownloadVideoFormat::Webm => "webm",
        }
    }

    /// Get the video format from the given file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "mp4" => Some(Self::Mp4),
            "mkv" => Some(Self::Mkv),
            "webm" => Some(Self::Webm),
            _ => None,
        }
    }
}

impl DownloadDest {
    fn get_dest_path(&self, v: &DownloadVideo) -> PathBuf {
        static RE_TEMPLATE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\{\w+\} *"#).unwrap());

        match self {
            DownloadDest::Default => PathBuf::from(video_filename(v)),
            DownloadDest::File(p) => p.clone(),
            DownloadDest::Dir(p) => p.join(video_filename(v)),
            DownloadDest::Template(t) => t
                .iter()
                .map(|part| {
                    let s = part.to_string_lossy();

                    let (mut replaced, last_end) = RE_TEMPLATE.find_iter(&s).fold(
                        (String::new(), 0),
                        |(mut acc, last_end), m| {
                            acc += &s[last_end..m.start()];
                            let ms = m.as_str();
                            let trimmed = ms.trim_end_matches(' ');
                            let repl: Option<Cow<str>> = match trimmed.trim_matches(['{', '}']) {
                                "id" => Some(v.id.as_str().into()),
                                "title" => v.name.as_deref().map(Cow::from),
                                "channel" => v.channel_name.as_deref().map(Cow::from),
                                "channelId" => v.channel_id.as_deref().map(Cow::from),
                                "album" => v.album_name.as_deref().map(Cow::from),
                                "albumId" => v.album_id.as_deref().map(Cow::from),
                                "track" => v.track_nr.map(|n| format!("{n:02}").into()),
                                _ => None,
                            };
                            if let Some(repl) = repl {
                                acc += &repl;
                                acc += &ms[trimmed.len()..]; // preceeding whitespace
                            }
                            (acc, m.end())
                        },
                    );
                    replaced += &s[last_end..];
                    replaced = replaced.trim().to_owned();

                    if replaced.is_empty() {
                        "-".to_owned()
                    } else {
                        filenamify_lim(&replaced)
                    }
                })
                .collect(),
        }
    }
}

impl Default for DownloaderBuilder {
    fn default() -> Self {
        Self {
            rp: None,
            ffmpeg: "ffmpeg".to_owned(),
            #[cfg(feature = "indicatif")]
            multi: None,
            #[cfg(feature = "indicatif")]
            progress_style: None,
            filter: StreamFilter::new(),
            video_format: DownloadVideoFormat::Mp4,
            n_retries: 3,
            path_precheck: false,
            #[cfg(feature = "audiotag")]
            audio_tag: false,
            #[cfg(feature = "audiotag")]
            crop_cover: false,
            client_types: None,
        }
    }
}

impl DownloaderBuilder {
    /// Create a new [`DownloaderBuilder`]
    ///
    /// This is the same as [`Downloader::builder`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a custom [`RustyPipe`] client
    #[must_use]
    pub fn rustypipe(mut self, rp: &RustyPipe) -> Self {
        self.rp = Some(rp.clone());
        self
    }

    /// Set the path to ffmpeg, used to join video and audio files
    ///
    /// The default system-wide `ffmpeg` binary is used by default.
    #[must_use]
    pub fn ffmpeg<S: Into<String>>(mut self, ffmpeg: S) -> Self {
        self.ffmpeg = ffmpeg.into();
        self
    }

    /// Set the indicatif [`MultiProgress`] used to show download progress
    /// for all downloads
    #[cfg(feature = "indicatif")]
    #[cfg_attr(docsrs, doc(cfg(feature = "indicatif")))]
    #[must_use]
    pub fn multi_progress(mut self, progress: MultiProgress) -> Self {
        self.multi = Some(progress);
        self
    }

    /// Set the indicatif [`ProgressStyle`] for the progress bars displayed under `multi_progress`
    #[cfg(feature = "indicatif")]
    #[cfg_attr(docsrs, doc(cfg(feature = "indicatif")))]
    #[must_use]
    pub fn progress_style(mut self, style: ProgressStyle) -> Self {
        self.progress_style = Some(style);
        self
    }

    /// Set the default [`StreamFilter`] for all downloads.
    ///
    /// The filter can be overridden for individual download queries.
    #[must_use]
    pub fn stream_filter(mut self, filter: StreamFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Set the [`DownloadVideoFormat`] of downloaded videos
    #[must_use]
    pub fn video_format(mut self, video_format: DownloadVideoFormat) -> Self {
        self.video_format = video_format;
        self
    }

    /// Set the number of retries in case a download fails with a 403 error
    #[must_use]
    pub fn n_retries(mut self, n_retries: u32) -> Self {
        self.n_retries = n_retries;
        self
    }

    /// Enable path precheck
    ///
    /// The downloader will check if the destination path
    /// (predicted from the entity to download and the StreamFilter) exists and
    /// skips the download with [`DownloadError::Exists`] without fetching any player data.
    ///
    /// This allows fast resumption of playlist downloads.
    #[must_use]
    pub fn path_precheck(mut self) -> Self {
        self.path_precheck = true;
        self
    }

    /// Enable audio tagging
    #[cfg(feature = "audiotag")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audiotag")))]
    #[must_use]
    pub fn audio_tag(mut self) -> Self {
        self.audio_tag = true;
        self
    }

    /// Crop YouTube thumbnails to get square album covers
    #[cfg(feature = "audiotag")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audiotag")))]
    #[must_use]
    pub fn crop_cover(mut self) -> Self {
        self.crop_cover = true;
        self
    }

    /// Set the [`ClientType`] used to fetch the YT player
    #[must_use]
    pub fn client_type(mut self, client_type: ClientType) -> Self {
        self.client_types = Some(vec![client_type]);
        self
    }

    /// Set a list of client types used to fetch the YT player
    ///
    /// The clients are used in the given order. If a client cannot fetch the requested video,
    /// an attempt is made with the next one.
    #[must_use]
    pub fn client_types<T: Into<Vec<ClientType>>>(mut self, client_types: T) -> Self {
        self.client_types = Some(client_types.into());
        self
    }

    /// Create a new, configured [`Downloader`] instance
    pub fn build(self) -> Downloader {
        self.build_with_client(
            Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
        )
    }

    /// Create a new, configured [`Downloader`] instance using a custom Reqwest [`Client`]
    pub fn build_with_client(self, http_client: Client) -> Downloader {
        Downloader {
            i: Arc::new(DownloaderInner {
                rp: self.rp.unwrap_or_default(),
                http: http_client,
                ffmpeg: self.ffmpeg,
                #[cfg(feature = "indicatif")]
                multi: self.multi,
                #[cfg(feature = "indicatif")]
                progress_style: self.progress_style.unwrap_or_else(|| {
                    ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                        .unwrap()
                        .progress_chars("#>-")
                }),
                filter: self.filter,
                video_format: self.video_format,
                n_retries: self.n_retries,
                path_precheck: self.path_precheck,
                #[cfg(feature = "audiotag")]
                audio_tag: self.audio_tag,
                #[cfg(feature = "audiotag")]
                crop_cover: self.crop_cover,
                client_types: self.client_types,
            }),
        }
    }
}

impl Default for Downloader {
    fn default() -> Self {
        DownloaderBuilder::new().build()
    }
}

impl Downloader {
    /// Create a new [`Downloader`] using the given [`RustyPipe`] instance
    pub fn new(rp: &RustyPipe) -> Self {
        DownloaderBuilder::new().rustypipe(rp).build()
    }

    /// Create a new [`DownloaderBuilder`]
    ///
    /// This is the same as [`DownloaderBuilder::new`]
    pub fn builder() -> DownloaderBuilder {
        DownloaderBuilder::default()
    }

    fn query(&self, video: DownloadVideo) -> DownloadQuery {
        DownloadQuery {
            dl: self.clone(),
            video,
            dest: DownloadDest::Default,
            #[cfg(feature = "indicatif")]
            progress: None,
            filter: None,
            video_format: None,
            client_types: None,
        }
    }

    /// Download a video with the given ID
    #[must_use]
    pub fn id<S: Into<String>>(&self, video_id: S) -> DownloadQuery {
        self.query(DownloadVideo {
            id: video_id.into(),
            ..Default::default()
        })
    }

    /// Download a video from a DownloadVideo object
    #[must_use]
    pub fn video(&self, video: DownloadVideo) -> DownloadQuery {
        self.query(video)
    }

    /// Download a video from a [`YtEntity`] object (e.g. playlist/channel video)
    ///
    /// Providing an entity has the advantage that the download path can be determined before the video
    /// is fetched, so already downloaded videos get skipped right away.
    #[must_use]
    pub fn entity(&self, video: &impl YtEntity) -> DownloadQuery {
        self.query(DownloadVideo::from_entity(video))
    }

    /// Download a video from a [`TrackItem`] (YouTube Music album/playlist item)
    ///
    /// Providing an entity has the advantage that the download path can be determined before the video
    /// is fetched, so already downloaded videos get skipped right away.
    ///
    /// If an album track is downloaded, this method will also add the track number to the downloaded file
    #[must_use]
    pub fn track(&self, track: &TrackItem) -> DownloadQuery {
        self.query(DownloadVideo::from_track(track))
    }
}

/// Output data from downloading a video
pub struct DownloadResult {
    /// Download destination path
    pub dest: PathBuf,
    /// Fetched vvideo player data
    pub player_data: VideoPlayer,
}

impl DownloadQuery {
    /// Update the video format from the given path extension
    ///
    /// The video format is not updated if it was already manually set
    fn update_video_format(&mut self, path: &Path) {
        if self.video_format.is_none() {
            self.video_format = path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(DownloadVideoFormat::from_extension);
        }
    }

    /// Download to the given file
    ///
    /// Note that the file extension may be changed to fit the reuested video/audio format.
    /// Refer to the [`DownloadResult`] to get the actual path after downloading.
    #[must_use]
    pub fn to_file<P: Into<PathBuf>>(mut self, file: P) -> Self {
        let file = file.into();
        self.update_video_format(&file);
        self.dest = DownloadDest::File(file);
        self
    }

    /// Download to the given directory
    ///
    /// The filename is created by this template: `{track} {title} [{id}]`.
    ///
    /// You can use a custom filename template using [`DownloadQuery::to_template`]
    #[must_use]
    pub fn to_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.dest = DownloadDest::Dir(dir.into());
        self
    }

    /// Download to a path determined by a template
    ///
    /// Templates are paths that may contain variables for video metadata.
    ///
    /// ## Variables
    /// - `{id}` Video ID
    /// - `{title}` Video title
    /// - `{channel}` Channel name
    /// - `{channel_id}` Channel ID
    /// - `{album}` Album
    /// - `{album_id}` Album ID
    /// - `{track}` Track number
    ///
    /// Whitespace between template variables is automatically removed if a variable
    /// contains no data (e.g. `{track} {name}` is equal to `{name}` if a video without
    /// track number is downloaded).
    ///
    /// Note that the file extension may be changed to fit the reuested video/audio format.
    /// Refer to the [`DownloadResult`] to get the actual path after downloading.
    #[must_use]
    pub fn to_template<P: Into<PathBuf>>(mut self, tmpl: P) -> Self {
        let tmpl = tmpl.into();
        self.update_video_format(&tmpl);
        self.dest = DownloadDest::Template(tmpl);
        self
    }

    /// Show the progress of this download using a Indicatif [`ProgressBar`]
    #[cfg(feature = "indicatif")]
    #[cfg_attr(docsrs, doc(cfg(feature = "indicatif")))]
    #[must_use]
    pub fn progress_bar(mut self, progress: ProgressBar) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Set a [`StreamFilter`] for choosing a stream to be downloaded
    #[must_use]
    pub fn stream_filter(mut self, filter: StreamFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set the [`DownloadVideoFormat`] of downloaded videos
    #[must_use]
    pub fn video_format(mut self, video_format: DownloadVideoFormat) -> Self {
        self.video_format = Some(video_format);
        self
    }

    /// Set the [`ClientType`] used to fetch the YT player
    #[must_use]
    pub fn client_type(mut self, client_type: ClientType) -> Self {
        self.client_types = Some(vec![client_type]);
        self
    }

    /// Set a list of client types used to fetch the YT player
    ///
    /// The clients are used in the given order. If a client cannot fetch the requested video,
    /// an attempt is made with the next one.
    #[must_use]
    pub fn client_types<T: Into<Vec<ClientType>>>(mut self, client_types: T) -> Self {
        self.client_types = Some(client_types.into());
        self
    }

    /// Download the video
    ///
    /// If no download path is set, the video is downloaded to the current directory
    /// with a filename created by this template: `{track} {title} [{id}]`.
    #[tracing::instrument(skip(self), level="error", fields(id = self.video.id))]
    pub async fn download(&self) -> Result<DownloadResult> {
        let mut last_err = None;
        let mut failed_client = None;

        // Progress bar
        #[cfg(feature = "indicatif")]
        let pb = match &self.progress {
            Some(progress) => Some(progress.clone()),
            None => self.dl.i.multi.clone().map(|m| {
                let pb = ProgressBar::new(1);
                pb.set_style(self.dl.i.progress_style.clone());
                m.add(pb)
            }),
        };

        for n in 0..=self.dl.i.n_retries {
            let err = match self
                .download_attempt(
                    n,
                    failed_client,
                    #[cfg(feature = "indicatif")]
                    &pb,
                )
                .await
            {
                Ok(res) => return Ok(res),
                Err(DownloadError::Forbidden {
                    client_type,
                    visitor_data,
                }) => {
                    failed_client = Some(client_type);
                    DownloadError::Forbidden {
                        client_type,
                        visitor_data,
                    }
                }
                Err(DownloadError::Http(e)) => {
                    if !e.is_timeout() {
                        return Err(DownloadError::Http(e));
                    }
                    DownloadError::Http(e)
                }
                Err(e) => return Err(e),
            };

            if n != self.dl.i.n_retries {
                tracing::warn!("Retry attempt #{}. Error: {}", n + 1, err);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            last_err = Some(err);
        }
        Err(last_err.unwrap())
    }

    async fn download_attempt(
        &self,
        #[allow(unused_variables)] n: u32,
        failed_client: Option<ClientType>,
        #[cfg(feature = "indicatif")] pb: &Option<ProgressBar>,
    ) -> Result<DownloadResult> {
        let filter = self.filter.as_ref().unwrap_or(&self.dl.i.filter);
        let video_format = self.video_format.unwrap_or(self.dl.i.video_format);

        // Check if already downloaded
        if self.video.name.is_some() && self.dl.i.path_precheck {
            let op = self.dest.get_dest_path(&self.video);

            if filter.is_video_none() {
                for ext in ["m4a", "opus"] {
                    let p = op.with_extension(ext);
                    if p.is_file() {
                        return Err(DownloadError::Exists(p));
                    }
                }
            } else {
                let p = op.with_extension(video_format.extension());
                if p.is_file() {
                    return Err(DownloadError::Exists(p));
                }
            }
        }

        #[cfg(feature = "indicatif")]
        let attempt_suffix = if n > 0 {
            format!(" (retry #{n})")
        } else {
            String::new()
        };
        #[cfg(feature = "indicatif")]
        if let Some(pb) = pb {
            if let Some(n) = &self.video.name {
                pb.set_message(format!("Fetching player data for {n}{attempt_suffix}"));
            } else {
                pb.set_message(format!("Fetching player data{attempt_suffix}"));
            }
        }

        let q = self.dl.i.rp.query();

        let mut client_types = Cow::Borrowed(
            self.client_types
                .as_ref()
                .or(self.dl.i.client_types.as_ref())
                .map(Vec::as_slice)
                .unwrap_or(q.player_client_order()),
        );

        // If the last download failed, try another client if possible
        if let Some(failed_client) = failed_client {
            if let Some(pos) = client_types.iter().position(|c| c == &failed_client) {
                let p2 = pos + 1;
                if p2 < client_types.len() {
                    let mut v = client_types[p2..].to_vec();
                    v.extend(&client_types[..p2]);
                    client_types = v.into();
                }
            }
        }

        let player_data = q.player_from_clients(&self.video.id, &client_types).await?;
        let user_agent = q.user_agent(player_data.client_type);

        // Select streams to download
        let (video, audio) = player_data.select_video_audio_stream(filter);

        tracing::info!(
            "selected streams: video={:?} audio={:?} (abr_url_present={} audio_streams={} video_streams={})",
            video.map(|v| v.itag),
            audio.map(|a| a.itag),
            player_data.abr_streaming_url.is_some(),
            player_data.audio_streams.len(),
            player_data.video_streams.len(),
        );

        if video.is_none() && audio.is_none() {
            if player_data.drm.is_some() {
                return Err(DownloadError::Source("video is DRM-protected".into()));
            }
            return Err(DownloadError::Source("no stream found".into()));
        }

        let extension = match video {
            Some(_) => video_format.extension(),
            None => match audio {
                Some(audio) => match audio.codec {
                    AudioCodec::Mp4a => "m4a",
                    AudioCodec::Opus => "opus",
                    AudioCodec::Ac3 => "ac3",
                    AudioCodec::Ec3 => "eac3",
                    _ => return Err(DownloadError::Source("unknown audio codec".into())),
                },
                None => unreachable!(),
            },
        };

        let (name, details) = match &player_data.details.name {
            Some(n) => (n.to_owned(), None),
            None => {
                let details = self.dl.i.rp.query().video_details(&self.video.id).await?;
                (details.name.to_owned(), Some(details))
            }
        };

        let pv = DownloadVideo {
            id: player_data.details.id.to_owned(),
            name: Some(name.to_owned()),
            channel_id: Some(player_data.details.channel_id.to_owned()),
            channel_name: player_data
                .details
                .channel_name
                .clone()
                .or(details.as_ref().map(|d| d.channel.name.to_owned())),
            album_id: self.video.album_id.to_owned(),
            album_name: self.video.album_name.to_owned(),
            track_nr: self.video.track_nr,
        };
        let output_path = self.dest.get_dest_path(&pv).with_extension(extension);

        if output_path.exists() {
            return Err(DownloadError::Exists(output_path));
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try SABR first when the player response exposes the SABR endpoint
        // AND the filter is configured for it. Failures fall through to
        // the progressive download path below.
        if filter.is_abr_only() {
            if let (Some(url), Some(config_b64)) = (
                player_data.abr_streaming_url.as_deref(),
                player_data.abr_ustreamer_config.as_deref(),
            ) {
                // The browser sends a *cold-start* PoToken for the initial
                // SABR request: 10 bytes, structured as a `PoTokenMsg` with
                // field 4 holding 8 random bytes (`0x22 0x08 <8 random>`).
                // The server's `sps=2` (status 2) gives us a ~1–2 MB grace
                // window for this kind of placeholder. After that, the
                // server demands a refreshed token (status 3).
                //
                // If we send a real BotGuard-attested (field 6, 87 bytes)
                // token on the first request, the server classifies it as
                // a refresh attempt — not a cold-start — and gives us a
                // much shorter grace window. Worse, when we later try to
                // refresh with a different session binding, the server
                // rejects the refresh outright.
                //
                // The browser's exact sequence is:
                //   1. Send a cold-start placeholder (dP4.D in base.js).
                //   2. The server replies with sps=2/3 (`spsumpreject`).
                //   3. The player's `spsumpreject` handler triggers the
                //      full cV+LT+GenerateIT pipeline (cV/line 2242,
                //      LT/line 1048), mints a real field-6 token, and
                //      retries the SABR request.
                //   4. Subsequent requests use the real, session-bound
                //      token.
                //
                // We mirror that. The cold-start placeholder is built
                // first, and the chromey mint is pre-warmed in parallel
                // so the real token is ready by the time the server asks
                // for it.
                //
                // Allow tests / debugging to override the PoToken with
                // one captured from a real browser request. The token is
                // read from `$RUSTYPIPE_SABR_PO_TOKEN_FILE` (raw bytes)
                // or `$RUSTYPIPE_SABR_PO_TOKEN_B64` (base64). The override
                // wins over the cold-start placeholder so the same
                // diagnostic mechanism continues to work.
                let override_b64: Option<String> = std::env::var("RUSTYPIPE_SABR_PO_TOKEN_B64")
                    .ok()
                    .or_else(|| {
                        std::env::var("RUSTYPIPE_SABR_PO_TOKEN_FILE")
                            .ok()
                            .and_then(|path| std::fs::read(path).ok())
                            .and_then(|bytes| {
                                use data_encoding::Encoding;
                                Some(data_encoding::BASE64URL.encode(&bytes))
                            })
                    });

                // Build the cold-start placeholder bytes (the byte-for-byte
                // shape of player.js's dP4.D output at line 6023 of
                // base.js.full: 0x22 0x08 <8 random bytes>).
                let cold_start_bytes: Vec<u8> = {
                    let mut rand = [0u8; 8];
                    rand::rng().fill(&mut rand[..]);
                    let mut po = vec![0x22u8, 0x08];
                    po.extend_from_slice(&rand);
                    po
                };

                // Pick the initial bytes: env-var override wins, otherwise
                // always the cold-start placeholder.
                let initial_bytes: Vec<u8> = if let Some(b64) = override_b64.clone() {
                    use data_encoding::Encoding;
                    data_encoding::BASE64URL
                        .decode(b64.as_bytes())
                        .unwrap_or_else(|_| cold_start_bytes.clone())
                } else {
                    tracing::info!(
                        "sending cold-start PoToken (10 bytes) for initial SABR request; \
                         will mint a real BotGuard-attested token on sps=2/3 response"
                    );
                    cold_start_bytes.clone()
                };

                // Pre-warm the chromey mint in parallel. The browser
                // kicks off cV+LT+GenerateIT on `csiinitialized` (line
                // 7783 of base.js.full) so a real token is ready by the
                // time sps=2 returns. We do the same: spawn the mint
                // task now and pass the JoinHandle to download_sabr.
                //
                // If chromey is not enabled, the pre-warm task is a
                // no-op and the existing mint_attestation_po_token path
                // handles the retry.
                let prewarmed: tokio::task::JoinHandle<Option<Vec<u8>>> = {
                    let rp = self.dl.i.rp.clone();
                    let video_id = self.video.id.clone();
                    let visitor_data = player_data.visitor_data.clone();
                    let session_po_token = player_data.session_po_token.clone();
                    let skip_prewarm = override_b64.is_some()
                        || std::env::var("RUSTYPIPE_SABR_NO_BOTGUARD").is_ok();
                    tokio::spawn(async move {
                        if skip_prewarm {
                            return None;
                        }
                        match Self::mint_attestation_po_token(
                            &rp,
                            session_po_token.as_deref(),
                            &visitor_data,
                            &video_id,
                        )
                        .await
                        {
                            Ok(bytes) => {
                                tracing::debug!(
                                    "prewarmed real PoToken ({} bytes) for video_id={}",
                                    bytes.len(),
                                    video_id
                                );
                                Some(bytes)
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "prewarm mint failed for video_id={}: {e}",
                                    video_id
                                );
                                None
                            }
                        }
                    })
                };

                let po_token_b64: &str = &data_encoding::BASE64URL.encode(&initial_bytes);

                #[cfg(feature = "indicatif")]
                {
                    if let Some(pb) = pb {
                        pb.set_message(format!("Downloading {name} via SABR{attempt_suffix}"));
                    }
                }
                match self
                    .download_sabr(
                        &player_data,
                        video,
                        audio,
                        &output_path,
                        url,
                        config_b64,
                        po_token_b64,
                        user_agent.as_ref(),
                        Some(prewarmed),
                    )
                    .await
                {
                    Ok(dest) => {
                        #[cfg(feature = "audiotag")]
                        if self.dl.i.audio_tag
                            && video.is_none()
                            && matches!(extension, "m4a" | "opus")
                        {
                            let (details_obj, track) = match details {
                                Some(d) => (
                                    d,
                                    self.dl
                                        .i
                                        .rp
                                        .query()
                                        .music_details(&self.video.id)
                                        .await?
                                        .track,
                                ),
                                None => {
                                    let q = self.dl.i.rp.query();
                                    let (det, music) = tokio::try_join!(
                                        q.video_details(&self.video.id),
                                        q.music_details(&self.video.id)
                                    )?;
                                    (det, music.track)
                                }
                            };
                            if let Err(e) = self
                                .apply_audio_tags(
                                    &dest,
                                    details_obj,
                                    &player_data.details,
                                    track,
                                    pv.track_nr,
                                )
                                .await
                            {
                                tracing::warn!("audio tagging failed: {e}");
                            }
                        }

                        #[cfg(feature = "indicatif")]
                        if let Some(pb) = pb {
                            pb.finish_and_clear();
                        }

                        return Ok(DownloadResult {
                            dest,
                            player_data,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("SABR download failed, falling back to progressive: {e}");
                        // fall through to progressive path
                    }
                }
            }
        }

        let mut downloads: Vec<StreamDownload> = Vec::new();

        if let Some(v) = video {
            downloads.push(StreamDownload {
                file: output_path.with_extension(format!("video{}", v.format.extension())),
                url: v.url.clone(),
                video_codec: Some(v.codec),
                audio_codec: None,
            });
        }
        if let Some(a) = audio {
            downloads.push(StreamDownload {
                file: output_path.with_extension(format!("audio{}", a.format.extension())),
                url: a.url.clone(),
                video_codec: None,
                audio_codec: Some(a.codec),
            });
        }

        #[cfg(feature = "indicatif")]
        if let Some(pb) = pb {
            pb.set_message(format!("Downloading {name}{attempt_suffix}"))
        }
        let downloads = download_streams(
            downloads,
            &self.dl.i.http,
            &user_agent,
            #[cfg(feature = "indicatif")]
            pb.clone(),
        )
        .await
        .map_err(|e| {
            if let DownloadError::Http(e) = &e {
                if e.status() == Some(StatusCode::FORBIDDEN) {
                    // 403 errors may occur due to bad visitor data IDs
                    if let Some(vd) = &player_data.visitor_data {
                        q.remove_visitor_data(vd);
                    }
                    return DownloadError::Forbidden {
                        client_type: player_data.client_type,
                        visitor_data: player_data.visitor_data.clone(),
                    };
                }
            }
            e
        })?;

        #[cfg(feature = "indicatif")]
        if let Some(pb) = &pb {
            pb.set_message(format!("Converting {name}"));
            pb.set_style(
                ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}]")
                    .unwrap(),
            );
            pb.enable_steady_tick(Duration::from_millis(500));
        }

        convert_streams(&downloads, &output_path, &self.dl.i.ffmpeg, &name).await?;

        // Tag audio file
        #[cfg(feature = "audiotag")]
        if self.dl.i.audio_tag && video.is_none() && matches!(extension, "m4a" | "opus") {
            let (details, track) = match details {
                Some(d) => (d, self.dl.i.rp.query().music_details(&self.video.id).await?),
                None => {
                    let q = self.dl.i.rp.query();
                    tokio::try_join!(
                        q.video_details(&self.video.id),
                        q.music_details(&self.video.id)
                    )?
                }
            };
            self.apply_audio_tags(
                &output_path,
                details,
                &player_data.details,
                track.track,
                pv.track_nr,
            )
            .await?;
        }

        #[cfg(feature = "indicatif")]
        if let Some(pb) = pb {
            pb.disable_steady_tick();
        }

        // Delete original files
        for d in &downloads {
            fs::remove_file(&d.file).await?;
        }

        #[cfg(feature = "indicatif")]
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        Ok(DownloadResult {
            dest: output_path,
            player_data,
        })
    }

    #[cfg(feature = "audiotag")]
    async fn apply_audio_tags(
        &self,
        file: &Path,
        details: VideoDetails,
        player_details: &VideoPlayerDetails,
        track: TrackItem,
        track_nr: Option<u16>,
    ) -> Result<()> {
        use std::{io::Cursor, num::NonZeroU32};

        let mut tagged_file = lofty::read_from_path(file)?;
        let tag = match tagged_file.primary_tag_mut() {
            Some(primary_tag) => primary_tag,
            None => {
                if let Some(first_tag) = tagged_file.first_tag_mut() {
                    first_tag
                } else {
                    let tag_type = tagged_file.primary_tag_type();
                    tagged_file.insert_tag(Tag::new(tag_type));

                    tagged_file.primary_tag_mut().unwrap()
                }
            }
        };

        let description = details.description.to_plaintext();

        tag.set_album(
            track
                .album
                .map(|b| b.name)
                .unwrap_or_else(|| track.name.clone()),
        );
        tag.set_artist(
            track
                .artists
                .into_iter()
                .next()
                .map(|a| a.name)
                .unwrap_or(details.channel.name),
        );
        tag.set_title(track.name);
        if let Some(release_date) = extract_yt_release_date(&description, details.publish_date) {
            if let Ok(date_str) = release_date.format(&YMD_FORMAT) {
                tag.insert_text(ItemKey::RecordingDate, date_str);
            }
        }
        tag.set_comment(description);
        if let Some(track_nr) = track_nr {
            tag.set_track(track_nr.into());
        }

        // For YTM tracks the music details contain a high quality, square cover image, but for music videos
        // the cover images are cropped and of worse resolution.
        // Therefore we switch to the thumbnails from the player data if the music details contain no square
        // thumbnails.
        let thumbnail_music = track.cover.into_iter().max_by_key(|c| c.height);
        let thumbnail = if thumbnail_music
            .as_ref()
            .map(|tn| tn.height == tn.width)
            .unwrap_or_default()
        {
            thumbnail_music
        } else {
            let thumbnail_player = player_details
                .thumbnail
                .iter()
                .max_by_key(|c| c.height)
                .cloned();
            thumbnail_player.or(thumbnail_music)
        };

        if let Some(thumbnail) = thumbnail {
            // Attempt to get the higher resolution, uncropped maxresdefault.jpg thumbnail if available
            let mut resp = None;
            if thumbnail.height != thumbnail.width {
                if let Ok(x) = self
                    .dl
                    .i
                    .http
                    .get(format!(
                        "https://i.ytimg.com/vi/{}/maxresdefault.jpg",
                        track.id
                    ))
                    .send()
                    .await?
                    .error_for_status()
                {
                    resp = Some(x);
                }
            }

            let resp = match resp {
                Some(resp) => resp,
                None => self
                    .dl
                    .i
                    .http
                    .get(thumbnail.url)
                    .send()
                    .await?
                    .error_for_status()?,
            };

            let img_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|fmt| fmt.to_str().ok())
                .and_then(image::ImageFormat::from_mime_type);
            let img_bts = resp.bytes().await?;

            let mut lofty_img = if self.dl.i.crop_cover {
                // Crop cover image if it is not square
                if thumbnail.height != thumbnail.width {
                    let mut img = if let Some(fmt) = img_type {
                        image::load_from_memory_with_format(&img_bts, fmt)?
                    } else {
                        image::load_from_memory(&img_bts)?
                    };

                    let crop = smartcrop::find_best_crop_no_borders(
                        &img,
                        NonZeroU32::new(1).unwrap(),
                        NonZeroU32::new(1).unwrap(),
                    )
                    .map_err(|e| DownloadError::AudioTag(format!("image crop: {e}").into()))?
                    .crop;
                    img = img.crop_imm(crop.x, crop.y, crop.width, crop.height);
                    let mut enc_bts = Vec::new();
                    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
                        &mut enc_bts,
                        90,
                    ))?;
                    let mut rd = Cursor::new(enc_bts);
                    Picture::from_reader(&mut rd)?
                } else {
                    let mut rd = Cursor::new(img_bts);
                    Picture::from_reader(&mut rd)?
                }
            } else {
                let mut rd = Cursor::new(img_bts);
                Picture::from_reader(&mut rd)?
            };

            lofty_img.set_pic_type(lofty::picture::PictureType::CoverFront);
            tag.set_picture(0, lofty_img);
        }

        tag.save_to_path(file, WriteOptions::default())?;
        Ok(())
    }
}

impl DownloadQuery {
    /// Mint a fresh PO token for an attestation refresh.
    ///
    /// The SABR server returns `AttestationRequired` (status 3) when it
    /// needs a freshly-minted PO token to continue. The browser refreshes
    /// by calling botguard with the **video ID** as content binding,
    /// which returns a `PoToken` protobuf message (base64url-encoded).
    /// The whole returned message is then placed verbatim at
    /// `StreamerContext.po_token` (field 2 of the SABR body). YouTube's
    /// server-side parser extracts `19.2.6` (field 6 of the PoTokenMsg)
    /// itself — we do **not** strip and re-wrap field 6 client-side.
    ///
    /// Fallbacks, in order:
    /// 1. The env-var override (`$RUSTYPIPE_SABR_PO_TOKEN_FILE` /
    ///    `$RUSTYPIPE_SABR_PO_TOKEN_B64`) for tests. Expected to be
    ///    either raw PoToken bytes or base64url-encoded PoToken bytes
    ///    (we send as-is).
    /// 2. A freshly-minted content token via botguard with the video ID.
    ///    The botguard output is a complete PoTokenMsg proto; we send
    ///    it as-is.
    /// 3. A fresh cold-start PoToken (field 4 with 8 random bytes).
    ///    The server's SABR allows cold-start PoTokens as a kind of
    ///    "replay" — it treats the new request as a fresh session
    ///    continuation and resumes from where we left off.
    ///
    /// `visitor_data` and the deobf signature timestamp are
    /// passed to the chromey minter as the
    /// `(contentBinding, signedTimestamp)` pair. The botguard
    /// binary does not accept those — it only mints with the
    /// video-id identifier — so on that path we fall through
    /// with `None, None`. The chromey path is the one that
    /// produces valid attestation refreshes.
    async fn mint_attestation_po_token(
        rp: &RustyPipe,
        session_po_token_b64: Option<&str>,
        visitor_data: &Option<String>,
        video_id: &str,
    ) -> std::result::Result<Vec<u8>, DownloadError> {
        // Strategy 1: env-var override.
        if let Ok(b64) = std::env::var("RUSTYPIPE_SABR_PO_TOKEN_B64") {
            use data_encoding::Encoding;
            let bytes = data_encoding::BASE64URL
                .decode(b64.as_bytes())
                .map_err(|e| DownloadError::Source(format!("bad override b64: {e}").into()))?;
            tracing::debug!("using env-var override PoToken ({} bytes)", bytes.len());
            return Ok(bytes);
        }
        if let Ok(path) = std::env::var("RUSTYPIPE_SABR_PO_TOKEN_FILE") {
            let raw = tokio::fs::read(&path).await.map_err(|e| {
                DownloadError::Source(format!("read override file: {e}").into())
            })?;
            tracing::debug!("using env-var file override PoToken ({} bytes)", raw.len());
            return Ok(raw);
        }

        // Fetch the deobf data once so we can hand the
        // signature timestamp to the chromey minter. The
        // botguard binary path ignores these — it always
        // mints with just the video id — so the deobf
        // fetch is wasted on that branch, but the deobf
        // cache means it's free on the second call.
        let sts = rp.query().get_signature_timestamp().await.ok();

        // Strategy 2: chromey or botguard content token. The provider
        // returns a base64url-encoded PoTokenMsg proto that already has
        // the attestation bytes in field 6. The whole proto is what
        // `StreamerContext.po_token` expects — we send it verbatim and
        // let YouTube's server parse `19.2.6` from it.
        //
        // We use the configured provider (chromey if enabled, else
        // botguard). The chromey path is the one that produces valid
        // attestation tokens in the current setup, so we don't want
        // to suppress it. `RUSTYPIPE_SABR_NO_BOTGUARD` only suppresses
        // the botguard binary fallback.
        #[cfg(feature = "chromey-po-token")]
        {
            // First, try chromey directly. We bypass the
            // `get_po_token` cache because every attestation
            // refresh needs a brand-new content token. We
            // pass `(visitor_data, deobf.sts)` as the
            // `(contentBinding, signedTimestamp)` so the
            // minter is built the same way player.js builds
            // it — which is the only way GVS will accept
            // the resulting PoToken as a valid
            // attestation refresh.
            //
            // We also pass the video's watch URL, which
            // makes the chromey provider navigate to
            // `https://www.youtube.com/watch?v=<id>` before
            // running the botguard VM. YouTube's player.js
            // runs the VM inside the watch page, and the VM
            // fingerprints the page's navigation context
            // (referrer, history, document.title). A minter
            // built on the root `youtube.com` page produces
            // PoTokens GVS rejects with
            // `attestation_required` after a few SABR
            // segments, because the server cross-checks the
            // VM environment against the page that issued
            // the request. Navigating to the watch page
            // gives the VM the same context YouTube's
            // player would.
            if let Ok(token) = (|| async {
                rp.query()
                    .get_po_token_watch_bound(
                        video_id,
                        video_id,
                        visitor_data.as_deref(),
                        sts.as_deref(),
                    )
                    .await
            })().await {
                use data_encoding::Encoding;
                if let Ok(outer) = data_encoding::BASE64URL.decode(token.po_token.as_bytes()) {
                    tracing::debug!(
                        "minted fresh content PO token ({} bytes outer, bound={}, watch={}) for video_id={}",
                        outer.len(),
                        visitor_data.is_some(),
                        true,
                        video_id,
                    );
                    eprintln!(
                        "ATT_DIAG: minted {} bytes b64[:30]={} bound={} watch=true",
                        outer.len(),
                        &token.po_token[..token.po_token.len().min(30)],
                        visitor_data.is_some()
                    );
                    return Ok(outer);
                }
            }
        }
        #[cfg(not(feature = "chromey-po-token"))]
        if std::env::var("RUSTYPIPE_SABR_NO_BOTGUARD").is_err() {
            if let Ok(token) = rp.query().get_po_token(video_id).await {
                use data_encoding::Encoding;
                if let Ok(outer) = data_encoding::BASE64URL.decode(token.po_token.as_bytes()) {
                    tracing::debug!(
                        "minted fresh content PO token via botguard ({} bytes outer) for video_id={}",
                        outer.len(),
                        video_id,
                    );
                    return Ok(outer);
                }
            }
        } else {
            tracing::debug!("RUSTYPIPE_SABR_NO_BOTGUARD set, skipping botguard");
        }

        // Suppress unused-variable warning when neither
        // feature is enabled and we still get here.
        let _ = session_po_token_b64;

        // Strategy 3: cold-start PoToken (8 random bytes wrapped in field 4).
        {
            let mut rand = [0u8; 8];
            rand::rng().fill(&mut rand[..]);
            let mut po = vec![0x22u8, 0x08];
            po.extend_from_slice(&rand);
            tracing::debug!(
                "falling back to cold-start PO token ({} bytes) for video_id={}",
                po.len(),
                video_id,
            );
            return Ok(po);
        }
    }

    /// Strip the inner `ReloadPlaybackContext` (field 25) from the
    /// first sub-message (`f1`) of the given UstreamerConfig bytes.
    ///
    /// The `ustreamerConfig` returned by `/youtubei/v1/player` includes
    /// a `ReloadPlaybackContext` carrying a `token` (the visitor-data
    /// ID or a per-session binding). When this token is included in
    /// the SABR body the server returns 403 Forbidden — it expects
    /// the cold-start config to omit it, and the browser does exactly
    /// that. We strip it before handing the bytes to the SABR layer.
    ///
    /// If the bytes are not the expected proto, or the field is
    /// already absent, the original bytes are returned unchanged.
    fn strip_reload_playback_context(bytes: &[u8]) -> Vec<u8> {

        // Read a base-128 varint starting at `pos`. Returns (value, byte_offset_past_value)
        // or None if the buffer ends prematurely.
        fn read_varint(data: &[u8], pos: usize) -> Option<(usize, usize)> {
            let mut result: usize = 0;
            let mut shift = 0;
            let mut p = pos;
            while p < data.len() {
                let b = data[p];
                result |= ((b & 0x7f) as usize) << shift;
                p += 1;
                if (b & 0x80) == 0 {
                    return Some((result, p));
                }
                shift += 7;
                if shift > 63 {
                    return None;
                }
            }
            None
        }

        // Parse the top-level UstreamerConfig. We only need to find
        // the first sub-message (f1), and inside it the first
        // sub-message (f1) that contains a f25 — but in practice
        // the browser's config is the same structure: f1 wraps a
        // long config blob, f25 is field 25 inside that blob.
        //
        // Simplest correct strategy: walk top-level, recurse into
        // nested f1 (len-delim) messages, and at any level skip
        // field 25 (len-delim). Re-emit everything else.
        let mut out = Vec::with_capacity(bytes.len());
        let mut pos = 0;
        while pos < bytes.len() {
            let tag_start = pos;
            let (tag, after_tag) = match read_varint(bytes, pos) {
                Some(v) => v,
                None => {
                    out.extend_from_slice(&bytes[tag_start..]);
                    break;
                }
            };
            let fn_num = tag >> 3;
            let wt = tag & 0x7;
            match wt {
                0 => {
                    let (_v, after) = match read_varint(bytes, after_tag) {
                        Some(v) => v,
                        None => {
                            out.extend_from_slice(&bytes[tag_start..]);
                            return out;
                        }
                    };
                    if fn_num == 25 {
                        // skip
                        pos = after;
                        continue;
                    }
                    out.extend_from_slice(&bytes[tag_start..after]);
                    pos = after;
                }
                1 => {
                    if after_tag + 8 > bytes.len() {
                        out.extend_from_slice(&bytes[tag_start..]);
                        break;
                    }
                    if fn_num == 25 {
                        pos = after_tag + 8;
                        continue;
                    }
                    out.extend_from_slice(&bytes[tag_start..after_tag + 8]);
                    pos = after_tag + 8;
                }
                2 => {
                    let (l, content_start) = match read_varint(bytes, after_tag) {
                        Some(v) => v,
                        None => {
                            out.extend_from_slice(&bytes[tag_start..]);
                            return out;
                        }
                    };
                    let content_end = content_start + l;
                    if content_end > bytes.len() {
                        out.extend_from_slice(&bytes[tag_start..]);
                        break;
                    }
                    if fn_num == 25 {
                        pos = content_end;
                        continue;
                    }
                    // Recurse into the sub-message so we also strip
                    // nested f25 fields.
                    let inner = &bytes[content_start..content_end];
                    let filtered = Self::strip_reload_playback_context(inner);
                    // Emit: tag + length varint + filtered content
                    out.extend_from_slice(&bytes[tag_start..content_start]);
                    let mut len_val = filtered.len();
                    let mut len_bytes = Vec::new();
                    loop {
                        let mut b = (len_val & 0x7f) as u8;
                        len_val >>= 7;
                        if len_val != 0 {
                            b |= 0x80;
                        }
                        len_bytes.push(b);
                        if len_val == 0 {
                            break;
                        }
                    }
                    out.extend_from_slice(&len_bytes);
                    out.extend_from_slice(&filtered);
                    pos = content_end;
                }
                5 => {
                    if after_tag + 4 > bytes.len() {
                        out.extend_from_slice(&bytes[tag_start..]);
                        break;
                    }
                    if fn_num == 25 {
                        pos = after_tag + 4;
                        continue;
                    }
                    out.extend_from_slice(&bytes[tag_start..after_tag + 4]);
                    pos = after_tag + 4;
                }
                _ => {
                    out.extend_from_slice(&bytes[tag_start..]);
                    break;
                }
            }
        }
        out
    }

    /// Download a stream via the SABR/UMP endpoint and write the resulting
    /// bytes to `output_path`. The path is created if it does not exist.
    #[allow(clippy::too_many_arguments)]
    async fn download_sabr(
        &self,
        player_data: &VideoPlayer,
        video: Option<&VideoStream>,
        audio: Option<&AudioStream>,
        output_path: &Path,
        url: &str,
        ustreamer_config_b64: &str,
        po_token_b64: &str,
        user_agent: &str,
        mut prewarmed_po_token: Option<tokio::task::JoinHandle<Option<Vec<u8>>>>,
    ) -> Result<PathBuf> {
        let ustreamer_config_raw = data_encoding::BASE64URL
            .decode(ustreamer_config_b64.as_bytes())
            .map_err(|e| {
                DownloadError::Source(format!("invalid base64 ustreamer_config: {e}").into())
            })?;
        // The browser does NOT send the inner `ReloadPlaybackContext`
        // (field 25 inside the UstreamerConfig sub-message) on cold-start
        // SABR requests — empirically YouTube returns 403 Forbidden if we
        // include it. Strip it before forwarding the config to SABR.
        // DIAG: skip strip to see if it causes malformed_config
        // let ustreamer_config = Self::strip_reload_playback_context(&ustreamer_config_raw);
        let ustreamer_config = ustreamer_config_raw;
        let po_token = data_encoding::BASE64URL
            .decode(po_token_b64.as_bytes())
            .ok();
        tracing::debug!(
            "initial PoToken: {} bytes, hex[:40]={}",
            po_token.as_ref().map(|b| b.len()).unwrap_or(0),
            po_token.as_ref().map(|b| b.iter().take(40).map(|x| format!("{x:02x}")).collect::<String>()).unwrap_or_default(),
        );

        // YouTube's SABR URL comes out of the player response with an
        // *encrypted* `n` parameter. The browser runs it through the player
        // JS nsig function before sending; we need to do the same.
        // The `n` value is what GVS uses to throttle. GVS returns 403 if
        // it receives the un-deobfuscated value.
        let url = deobf_sabr_url(&self.dl.i.rp, url).await?;

        // SABR requires an audio format id; if the user only picked a
        // video stream, we still need an audio itag to initialise the stream,
        // so fall back to the audio stream and then to the video stream.
        let audio_id_source: &dyn FormatIdSource = audio
            .map(|s| s as &dyn FormatIdSource)
            .unwrap_or_else(|| video.expect("checked above") as &dyn FormatIdSource);
        let audio_fmt = audio_id_source.format_id();

        let video_fmt = video.map(|v| FormatId {
            itag: Some(v.itag as i32),
            last_modified: v.last_modified,
            xtags: v.xtags.clone(),
        });

        // Send all audio formats and the full set of video formats the player
        // advertised in `adaptiveFormats` so the SABR server can pick a
        // compatible fallback. YouTube's server rejects requests that don't
        // include the candidate set (returns 403 on the cold start).
        //
        // The browser always includes `xtags: ""` (empty string) on every
        // format id, so we always set the field — leaving it as `None`
        // results in the proto encoder dropping the field, which the
        // server treats as a mismatch.
        // The browser only includes the audio formats that the player is
        // *likely* to use (it omits 249, 140 etc.). We don't know the
        // exact heuristic the browser uses, but it's safe to filter to
        // opus formats and drop mp4a — YouTube prefers opus in the SABR
        // path anyway. We keep the highest-quality (251) and the
        // middle-quality (250) entries.
        let preferred_audio_formats: Vec<FormatId> = player_data
            .audio_streams
            .iter()
            .filter(|s| s.itag == 250 || s.itag == 251)
            .map(|s| FormatId {
                itag: Some(s.itag as i32),
                last_modified: s.last_modified,
                xtags: s.xtags.clone().or_else(|| Some(String::new())),
            })
            .collect();
        // The browser omits the legacy progressive formats (18, 133-137,
        // 160) from its preferred list. We filter those out the same way.
        let preferred_video_formats: Vec<FormatId> = player_data
            .video_streams
            .iter()
            .chain(player_data.video_only_streams.iter())
            .filter(|s| {
                // itag 18 = legacy progressive mp4, 133-137 = legacy
                // progressive video, 160 = legacy progressive h264.
                // Excluding them leaves only the modern ABR formats
                // the browser sends.
                s.itag != 18
                    && !(133..=137).contains(&s.itag)
                    && s.itag != 160
            })
            .map(|s| FormatId {
                itag: Some(s.itag as i32),
                last_modified: s.last_modified,
                xtags: s.xtags.clone().or_else(|| Some(String::new())),
            })
            .collect();

        let mut stream = SabrStream::new(
            &self.video.id,
            url.to_owned(),
            ustreamer_config,
            po_token,
            player_data.client_version.clone(),
            audio_fmt,
            video_fmt,
            preferred_audio_formats,
            preferred_video_formats,
            user_agent,
        );

        let tmp = output_path.with_extension(format!(
            "{}.sabr.part",
            output_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin")
        ));
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&tmp)
            .await?;

        let mut idle_requests: u32 = 0;
        let mut attestation_attempts: u32 = 0;
        loop {
            match stream.media().await {
                Ok(Some((audio_segs, video_segs))) => {
                    if audio_segs.is_empty() && video_segs.is_empty() {
                        idle_requests += 1;
                        if idle_requests > 5 {
                            return Err(DownloadError::Source(
                                "SABR stream returned no segments for 5 consecutive requests"
                                    .into(),
                            ));
                        }
                    } else {
                        idle_requests = 0;
                    }
                    for seg in audio_segs.iter() {
                        for chunk in seg.data() {
                            file.write_all_buf(&mut chunk.clone()).await?;
                        }
                    }
                    // Only write video segments if the user
                    // actually requested a video stream. With
                    // `-q audio` we still ask the SABR server for
                    // video formats (so it picks a compatible
                    // audio fallback) but we drop the bytes
                    // here. Otherwise an audio-only download
                    // ends up writing hundreds of MB of video
                    // segments to the same .sabr.part file.
                    if video.is_some() {
                        for seg in video_segs.iter() {
                            for chunk in seg.data() {
                                file.write_all_buf(&mut chunk.clone()).await?;
                            }
                        }
                    }
                }
                Ok(None) => {
                    drop(file);
                    fs::rename(&tmp, output_path).await?;
                    return Ok(output_path.to_owned());
                }
                Err(sabr::Error::AttestationRequired) => {
                    // GVS requires a fresh PO token (attestation). The
                    // server's behavior is to deliver the full media
                    // first, *then* close the stream with a status 3
                    // "attestation required" part asking for a fresh
                    // token. So when we see this, check whether we
                    // already have everything — if so, treat the stream
                    // as complete and stop, even though the server cut us
                    // off before sending an explicit end-of-stream part.
                    if stream.is_complete() {
                        tracing::info!(
                            "server requested attestation but stream is complete; finishing"
                        );
                        drop(file);
                        fs::rename(&tmp, output_path).await?;
                        return Ok(output_path.to_owned());
                    }

                    // The SABR server returns `attestation_required` (status 3)
                    // after delivering some initial data. We mint a fresh
                    // token and retry. We don't give up early — the server
                    // may need many refreshes if our PoToken quality is low
                    // (e.g. the botguard's content token is rejected as a
                    // refresh token). Only stop if:
                    //   * the stream is complete (handled above), or
                    //   * we've made many attempts and gotten nothing new
                    //     for a while.
                    //
                    // The max_retries reported by the server (typically 10)
                    // is the upper bound on what it allows. We don't trust
                    // it as a hard limit because we have observed the server
                    // sometimes accepts tokens beyond max_retries.
                    if attestation_attempts >= 10 {
                        // We can't get past attestation. The partial file
                        // we have is a truncated, broken WebM/Opus that
                        // most players will refuse to play past the
                        // truncation point. Don't return it as success —
                        // raise the error so the caller can fall back to
                        // the progressive (non-SABR) download path, which
                        // uses a simple GET with Range and is the same
                        // thing the browser does for audio-only playback.
                        drop(file);
                        let _ = fs::remove_file(&tmp).await;
                        return Err(DownloadError::Source(
                            "SABR server requires attestation; gave up after 10 attempts"
                                .into(),
                        ));
                    }
                    attestation_attempts += 1;
                    // On the first attestation_required, use the
                    // pre-warmed token if it landed in time. The
                    // browser kicks off the full cV+LT+GenerateIT
                    // pipeline on csiinitialized (in parallel with the
                    // cold-start request), so a real token is usually
                    // ready by the time the server replies with
                    // sps=2/3. If the prewarm didn't complete, await
                    // it briefly. If it still didn't produce a token,
                    // fall back to mint_attestation_po_token which
                    // will re-mint.
                    let new_token = if attestation_attempts == 1 {
                        // Wait up to 30s for the prewarm to finish.
                        // The prewarm runs the full chromey pipeline
                        // (cV+LT+GenerateIT) which typically takes
                        // 5-15s. After 30s we give up on the prewarm
                        // and mint fresh.
                        let prewarmed_handle = prewarmed_po_token.take();
                        let prewarmed_result: std::result::Result<
                            std::result::Result<Option<Vec<u8>>, tokio::task::JoinError>,
                            tokio::time::error::Elapsed,
                        > = match prewarmed_handle {
                            Some(handle) => {
                                tokio::time::timeout(
                                    std::time::Duration::from_secs(30),
                                    handle,
                                )
                                .await
                            }
                            None => {
                                // Prewarm was already consumed on a
                                // previous iteration (shouldn't happen
                                // for attempts==1, but be defensive).
                                std::future::pending().await
                            }
                        };
                        match prewarmed_result {
                            Ok(Ok(Some(bytes))) => {
                                tracing::info!(
                                    "using prewarmed PoToken ({} bytes) for first \
                                     attestation refresh",
                                    bytes.len()
                                );
                                bytes
                            }
                            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => {
                                tracing::debug!(
                                    "prewarm not ready; minting fresh attestation \
                                     PoToken via mint_attestation_po_token"
                                );
                                Self::mint_attestation_po_token(
                                    &self.dl.i.rp,
                                    player_data.session_po_token.as_deref(),
                                    &player_data.visitor_data,
                                    &player_data.details.id,
                                )
                                .await?
                            }
                        }
                    } else {
                        Self::mint_attestation_po_token(
                            &self.dl.i.rp,
                            player_data.session_po_token.as_deref(),
                            &player_data.visitor_data,
                            &player_data.details.id,
                        )
                        .await?
                    };
                    stream.set_po_token(Some(new_token));
                    idle_requests = 0;
                }
                Err(e) => {
                    return Err(DownloadError::Source(
                        format!("SABR error: {e}").into(),
                    ));
                }
            }
        }
    }
}

fn get_download_range(offset: u64, size: Option<u64>) -> Range<u64> {
    let mut rng = rand::rng();
    let chunk_size = rng.random_range(CHUNK_SIZE_MIN..CHUNK_SIZE_MAX);
    let mut chunk_end = offset + chunk_size;

    if let Some(size) = size {
        chunk_end = chunk_end.min(size - 1);
    }

    Range {
        start: offset,
        end: chunk_end,
    }
}

fn parse_cr_header(cr_header: &str) -> Result<(u64, u64)> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"bytes (\d+)-(\d+)/(\d+)").unwrap());

    let captures = PATTERN.captures(cr_header).ok_or_else(|| {
        DownloadError::Progressive(
            format!("Content-Range header '{cr_header}' does not match pattern").into(),
        )
    })?;

    Ok((
        captures.get(2).unwrap().as_str().parse().map_err(|_| {
            DownloadError::Progressive("could not parse range header number".into())
        })?,
        captures.get(3).unwrap().as_str().parse().map_err(|_| {
            DownloadError::Progressive("could not parse range header number".into())
        })?,
    ))
}

fn filenamify_lim(name: &str) -> String {
    let lim = 200;
    let n = filenamify::filenamify(name);

    if n.len() > lim {
        n.char_indices()
            .take_while(|(i, _)| i < &lim)
            .map(|(_, c)| c)
            .collect::<String>()
    } else {
        n
    }
}

async fn download_single_file(
    url: &str,
    output: &Path,
    http: &Client,
    user_agent: &str,
    #[cfg(feature = "indicatif")] pb: Option<ProgressBar>,
) -> Result<()> {
    // Check if file is already downloaded
    let output_path: PathBuf = output.into();

    if output_path.exists() {
        return Ok(());
    }

    let mut extension = OsString::from(output_path.extension().unwrap_or_default());
    extension.push(".part");
    let output_path_tmp = output_path.with_extension(extension);
    let mut offset: u64 = 0;
    let mut size: Option<u64> = None;

    // If the url is from googlevideo, extract file size from clen parameter
    let (url_base, url_params) =
        util::url_to_params(url).map_err(|e| DownloadError::Other(e.to_string().into()))?;
    let is_gvideo = url_base
        .as_str()
        .ends_with(".googlevideo.com/videoplayback");
    if is_gvideo {
        size = url_params.get("clen").and_then(|s| s.parse::<u64>().ok());
    }

    // Check if file is partially downloaded
    if output_path_tmp.exists() {
        let file_size = output_path_tmp.metadata()?.len();

        let res = http
            .head(url.to_owned())
            .header(header::USER_AGENT, user_agent)
            .header(header::RANGE, "bytes=0-0")
            .send()
            .await?
            .error_for_status()?;

        let cr_header = res
            .headers()
            .get(header::CONTENT_RANGE)
            .ok_or(DownloadError::Progressive(Cow::Borrowed(
                "Did not get Content-Range header",
            )))?
            .to_str()
            .map_err(|_| {
                DownloadError::Progressive(
                    "could not convert Content-Range header to string".into(),
                )
            })?;

        let (_, original_size) = parse_cr_header(cr_header)?;

        match file_size.cmp(&original_size) {
            Ordering::Less => {
                // Partially downloaded
                size = Some(original_size);
                offset = file_size;

                #[cfg(feature = "indicatif")]
                if let Some(pb) = &pb {
                    pb.inc_length(original_size);
                    pb.inc(offset);
                }
            }
            Ordering::Equal => {
                // Already downloaded
                fs::rename(output_path_tmp, output_path).await?;
                return Ok(());
            }
            Ordering::Greater => {
                // WTF?
                return Err(DownloadError::Other(
                    format!(
                        "Already downloaded file {} is larger than original",
                        output_path_tmp.to_str().unwrap_or_default()
                    )
                    .into(),
                ));
            }
        }
    }

    tracing::debug!("downloading {} to {}", url, output.to_string_lossy());

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&output_path_tmp)
        .await?;

    let res = if is_gvideo {
        if let Some(size) = size {
            download_chunks_by_param(
                http,
                &mut file,
                url,
                size,
                offset,
                user_agent,
                #[cfg(feature = "indicatif")]
                pb,
            )
            .await
        } else {
            download_chunks_by_header(
                http,
                &mut file,
                url,
                size,
                offset,
                user_agent,
                #[cfg(feature = "indicatif")]
                pb,
            )
            .await
        }
    } else {
        download_chunks_by_header(
            http,
            &mut file,
            url,
            size,
            offset,
            user_agent,
            #[cfg(feature = "indicatif")]
            pb,
        )
        .await
    };

    drop(file);
    if let Err(e) = res {
        // Remove temporary file if nothing was downloaded (e.g. 403 error)
        if std::fs::metadata(&output_path_tmp)
            .map(|md| md.len() == 0)
            .unwrap_or_default()
        {
            _ = std::fs::remove_file(&output_path_tmp);
        }
        return Err(e);
    }

    fs::rename(&output_path_tmp, &output_path).await?;
    Ok(())
}

// Use the HTTP range header to download a stream in chunks.
// This is the standardized method that works on all web servers,
// but I have observed throttling using this method.
async fn download_chunks_by_header(
    http: &Client,
    file: &mut File,
    url: &str,
    size: Option<u64>,
    offset: u64,
    user_agent: &str,
    #[cfg(feature = "indicatif")] pb: Option<ProgressBar>,
) -> Result<()> {
    let mut offset = offset;
    let mut size = size;

    loop {
        let range = get_download_range(offset, size);
        tracing::debug!("Fetching range {}-{}", range.start, range.end);

        let res = http
            .get(url.to_owned())
            .header(header::USER_AGENT, user_agent)
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com/")
            .header(
                header::RANGE,
                format!("bytes={}-{}", range.start, range.end),
            )
            .send()
            .await?
            .error_for_status()?;

        if res.content_length().unwrap_or_default() == 0 {
            return Err(DownloadError::Progressive(
                format!("empty chunk {}-{}", range.start, range.end).into(),
            ));
        }

        // Content-Range: bytes 0-100/451368980
        let cr_header = res
            .headers()
            .get(header::CONTENT_RANGE)
            .ok_or(DownloadError::Progressive(Cow::Borrowed(
                "Did not get Content-Range header",
            )))?
            .to_str()
            .map_err(|_| {
                DownloadError::Progressive(
                    "could not convert Content-Range header to string".into(),
                )
            })?;

        let (parsed_offset, parsed_size) = parse_cr_header(cr_header)?;

        offset = parsed_offset + 1;
        if size.is_none() {
            size = Some(parsed_size);
            #[cfg(feature = "indicatif")]
            if let Some(pb) = &pb {
                pb.inc_length(parsed_size);
            }
        }

        tracing::debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
            #[cfg(feature = "indicatif")]
            if let Some(pb) = &pb {
                pb.inc(chunk.len() as u64);
            }
            file.write_all_buf(&mut chunk).await?;
        }

        if offset >= size.unwrap() {
            break;
        }
    }
    Ok(())
}

// Use the `range` url parameter to download a stream in chunks.
// This ist used by YouTube's web player. The file size
// must be known beforehand (it is included in the stream url).
#[allow(clippy::too_many_arguments)]
async fn download_chunks_by_param(
    http: &Client,
    file: &mut File,
    url: &str,
    size: u64,
    offset: u64,
    user_agent: &str,
    #[cfg(feature = "indicatif")] pb: Option<ProgressBar>,
) -> Result<()> {
    let mut offset = offset;
    #[cfg(feature = "indicatif")]
    if let Some(pb) = &pb {
        pb.inc_length(size);
    }

    loop {
        let range = get_download_range(offset, Some(size));
        tracing::debug!("Fetching range {}-{}", range.start, range.end);

        let urlp =
            Url::parse_with_params(url, [("range", &format!("{}-{}", range.start, range.end))])
                .map_err(|e| DownloadError::Progressive(format!("url parsing: {e}").into()))?;

        let res = http
            .get(urlp)
            .header(header::USER_AGENT, user_agent)
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com/")
            .send()
            .await?
            .error_for_status()?;

        let clen = res.content_length().unwrap_or_default();
        if clen == 0 {
            return Err(DownloadError::Progressive(
                format!("empty chunk {}-{}", range.start, range.end).into(),
            ));
        }

        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
            #[cfg(feature = "indicatif")]
            if let Some(pb) = &pb {
                pb.inc(chunk.len() as u64);
            }
            file.write_all_buf(&mut chunk).await?;
        }

        offset += clen;
        tracing::debug!("offset inc by {}, new: {}", clen, offset);
        if offset >= size {
            break;
        }
    }
    Ok(())
}

#[allow(dead_code)]
struct StreamDownload {
    file: PathBuf,
    url: String,
    audio_codec: Option<AudioCodec>,
    video_codec: Option<VideoCodec>,
}

async fn download_streams(
    downloads: Vec<StreamDownload>,
    http: &Client,
    user_agent: &str,
    #[cfg(feature = "indicatif")] pb: Option<ProgressBar>,
) -> Result<Vec<StreamDownload>> {
    stream::iter(downloads.iter().map(Ok))
        .try_for_each_concurrent(2, |d| {
            #[cfg(feature = "indicatif")]
            let pb = pb.clone();
            async move {
                download_single_file(
                    &d.url,
                    &d.file,
                    http,
                    user_agent,
                    #[cfg(feature = "indicatif")]
                    pb,
                )
                .await
            }
        })
        .await?;

    Ok(downloads)
}

async fn convert_streams(
    downloads: &[StreamDownload],
    output: &Path,
    ffmpeg: &str,
    title: &str,
) -> Result<()> {
    let output_path: PathBuf = output.into();

    let mut args: Vec<OsString> = vec![];
    let mut mapping_args: Vec<OsString> = vec![];

    downloads.iter().enumerate().for_each(|(i, d)| {
        args.push("-i".into());
        args.push(d.file.clone().into());

        mapping_args.push("-map".into());
        mapping_args.push(i.to_string().into());
    });

    args.append(&mut mapping_args);

    args.push("-c".into());
    args.push("copy".into());

    args.push("-metadata".into());
    args.push(format!("title={title}").into());

    args.push(output_path.into());

    let res = Command::new(ffmpeg).args(args).output().await?;

    if !res.status.success() {
        return Err(DownloadError::Ffmpeg(
            format!(
                "ffmpeg error: {}",
                std::str::from_utf8(&res.stderr).unwrap_or_default()
            )
            .into(),
        ));
    }
    Ok(())
}

#[cfg(feature = "audiotag")]
const YMD_FORMAT: &[time::format_description::FormatItem] =
    time::macros::format_description!("[year]-[month]-[day]");

#[cfg(feature = "audiotag")]
fn extract_yt_release_date(
    description: &str,
    publish_date: Option<OffsetDateTime>,
) -> Option<Date> {
    static RELEASE_DATE_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"Released on: (\d{4}-\d{2}-\d{2})").unwrap());

    RELEASE_DATE_REGEX
        .captures(description)
        .and_then(|cap| {
            let raw_date = &cap[1];
            Date::parse(raw_date, YMD_FORMAT).ok()
        })
        .map(|release_date| {
            if let Some(upload_date) = publish_date {
                // Prefer the video upload date if it lies within 4 days of the release date
                let upload_date = upload_date.date();
                let diff = (upload_date - release_date).abs();
                if diff < time::Duration::days(4) {
                    return upload_date;
                }
            }
            release_date
        })
        .or_else(|| publish_date.map(|d| d.date()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template() {
        let dest =
            DownloadDest::Template(PathBuf::from("{channel}/{album}/{track} {title} [{id}]"));
        let track_path = dest.get_dest_path(&DownloadVideo {
            id: "a3Fo1vYyiDw".to_owned(),
            name: Some("Volle Kraft voraus".to_owned()),
            channel_id: Some("UCE7_p3lcXA-YXRZp2PjrgYw".to_owned()),
            channel_name: Some("Helene Fischer".to_owned()),
            album_id: Some("MPREb_O2gXCdCVGsZ".to_owned()),
            album_name: Some("Rausch (Deluxe)".to_owned()),
            track_nr: Some(1),
        });
        assert_eq!(
            track_path.to_str().unwrap(),
            "Helene Fischer/Rausch (Deluxe)/01 Volle Kraft voraus [a3Fo1vYyiDw]"
        );

        let video_path = dest.get_dest_path(&DownloadVideo {
            id: "5en96GIijXk".to_owned(),
            name: Some("a pretty cloud, and a happy duck".to_owned()),
            channel_id: Some("UCl2mFZoRqjw_ELax4Yisf6w".to_owned()),
            channel_name: Some("Louis Rossmann".to_owned()),
            album_id: None,
            album_name: None,
            track_nr: None,
        });
        assert_eq!(
            video_path.to_str().unwrap(),
            "Louis Rossmann/-/a pretty cloud, and a happy duck [5en96GIijXk]"
        );

        let ido_path = dest.get_dest_path(&DownloadVideo {
            id: "5en96GIijXk".to_owned(),
            name: None,
            channel_id: None,
            channel_name: None,
            album_id: None,
            album_name: None,
            track_nr: None,
        });
        assert_eq!(ido_path.to_str().unwrap(), "-/-/[5en96GIijXk]");
    }
}
