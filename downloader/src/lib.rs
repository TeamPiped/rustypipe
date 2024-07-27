#![warn(missing_docs, clippy::todo, clippy::dbg_macro)]

//! # YouTube audio/video downloader

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

use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client, StatusCode};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{
        traits::{FileFormat, YtEntity},
        AudioCodec, VideoCodec, VideoPlayer,
    },
    param::StreamFilter,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

use util::DownloadError;

type Result<T> = core::result::Result<T, DownloadError>;

const CHUNK_SIZE_MIN: u64 = 9_000_000;
const CHUNK_SIZE_MAX: u64 = 10_000_000;

/// RustyPipe audio/video downloader
///
/// The downloader uses an [`Arc`] internally, so if you are using the client
/// at multiple locations, you can just clone it.
#[derive(Clone)]
pub struct Downloader {
    i: Arc<DownloaderInner>,
}

/// Builder to construct a new downloader
pub struct DownloaderBuilder {
    rp: Option<RustyPipe>,
    ffmpeg: String,
    multi: Option<MultiProgress>,
    filter: StreamFilter,
    video_format: DownloadVideoFormat,
    n_retries: u32,
    path_precheck: bool,
}

struct DownloaderInner {
    /// YT client
    rp: RustyPipe,
    /// Path to the ffmpeg binary
    ffmpeg: String,
    /// Global progress
    multi: Option<MultiProgress>,
    /// Default stream filter
    filter: StreamFilter,
    /// Default video format
    video_format: DownloadVideoFormat,
    /// Number of retries in case of 403 error
    n_retries: u32,
    /// Check if destination path exists before player is fetched
    path_precheck: bool,
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
    multi: Option<MultiProgress>,
    /// Stream filter
    filter: Option<StreamFilter>,
    /// Target video format
    video_format: Option<DownloadVideoFormat>,
    /// ClientType type for fetching videos
    player_type: Option<ClientType>,
}

#[derive(Default)]
struct DownloadVideo {
    id: String,
    name: Option<String>,
    channel_id: Option<String>,
    channel_name: Option<String>,
}

impl DownloadVideo {
    fn from_video(video: &impl YtEntity) -> Self {
        DownloadVideo {
            id: video.id().to_owned(),
            name: Some(video.name().to_owned()),
            channel_id: video.channel_id().map(str::to_owned),
            channel_name: video
                .channel_name()
                .map(|n| n.strip_suffix(" - Topic").unwrap_or(n).to_owned()),
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
    filenamify_lim(&format!(
        "{} [{}]",
        v.name.as_deref().unwrap_or_default(),
        v.id
    ))
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
        match self {
            DownloadDest::Default => PathBuf::from(video_filename(v)),
            DownloadDest::File(p) => p.clone(),
            DownloadDest::Dir(p) => p.join(video_filename(v)),
            DownloadDest::Template(t) => t
                .iter()
                .map(|part| {
                    let s = part.to_string_lossy();
                    let mut s = s.replace("{id}", &v.id);
                    if let Some(name) = &v.name {
                        s = s.replace("{title}", name)
                    }
                    if let Some(channel) = &v.channel_name {
                        s = s.replace("{channel}", channel)
                    }
                    if let Some(id) = &v.channel_id {
                        s = s.replace("{channelId}", id);
                    }
                    filenamify_lim(&s)
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
            multi: None,
            filter: StreamFilter::new(),
            video_format: DownloadVideoFormat::Mp4,
            n_retries: 3,
            path_precheck: false,
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
    pub fn client(mut self, rp: &RustyPipe) -> Self {
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
    #[must_use]
    pub fn progress_bar(mut self, progress: MultiProgress) -> Self {
        self.multi = Some(progress);
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

    /// Set the [`VideoFormat`] of downloaded videos
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

    /// Create a new, configured [`Downloader`] instance
    pub fn build(self) -> Downloader {
        Downloader {
            i: Arc::new(DownloaderInner {
                rp: self.rp.unwrap_or_default(),
                ffmpeg: self.ffmpeg,
                multi: self.multi,
                filter: self.filter,
                video_format: self.video_format,
                n_retries: self.n_retries,
                path_precheck: self.path_precheck,
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
        DownloaderBuilder::new().client(rp).build()
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
            multi: None,
            filter: None,
            video_format: None,
            player_type: None,
        }
    }

    /// Download a video with the given ID
    pub fn download_id<S: Into<String>>(&self, video_id: S) -> DownloadQuery {
        self.query(DownloadVideo {
            id: video_id.into(),
            ..Default::default()
        })
    }

    /// Download a video from a [`YtEntity`] object (e.g. playlist/channel video)
    ///
    /// Providing an entity has the advantage that the download path can be determined before the video
    /// is fetched, so already downloaded videos get skipped right away.
    pub fn download_entity(&self, video: &impl YtEntity) -> DownloadQuery {
        self.query(DownloadVideo::from_video(video))
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
    pub fn to_file<P: Into<PathBuf>>(mut self, file: P) -> Self {
        let file = file.into();
        self.update_video_format(&file);
        self.dest = DownloadDest::File(file);
        self
    }

    /// Download to the given directory
    ///
    /// The filename is created by this template: `{title} [{id}]`.
    ///
    /// You can use a custom filename template using [`DownloadQuery::to_template`]
    pub fn to_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.dest = DownloadDest::Dir(dir.into());
        self
    }

    /// Download to the given filename template
    ///
    /// Templates are paths that may contain variables for video metadata.
    ///
    /// ## Variables
    /// - `{id}` Video ID
    /// - `{title}` Video title
    /// - `{channel}` Channel name
    /// - `{channel_id}` Channel ID
    ///
    /// Note that the file extension may be changed to fit the reuested video/audio format.
    /// Refer to the [`DownloadResult`] to get the actual path after downloading.
    pub fn to_template<P: Into<PathBuf>>(mut self, tmpl: P) -> Self {
        let tmpl = tmpl.into();
        self.update_video_format(&tmpl);
        self.dest = DownloadDest::Template(tmpl);
        self
    }

    /// Use a [`MultiProgress`] progress bar for all downloads
    pub fn progress_bar(mut self, progress: MultiProgress) -> Self {
        self.multi = Some(progress);
        self
    }

    /// Set a [`StreamFilter`] for choosing a stream to be downloaded
    pub fn stream_filter(mut self, filter: StreamFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set the [`VideoFormat`] of downloaded videos
    pub fn video_format(mut self, video_format: DownloadVideoFormat) -> Self {
        self.video_format = Some(video_format);
        self
    }

    /// Set the [`ClientType`] used to fetch the YT player
    pub fn player_type(mut self, player_type: ClientType) -> Self {
        self.player_type = Some(player_type);
        self
    }

    /// Download the video
    #[tracing::instrument(skip(self), fields(id = self.video.id))]
    pub async fn download(&self) -> Result<DownloadResult> {
        let mut last_err = None;

        // Progress bar
        let multi = self.multi.clone().or_else(|| self.dl.i.multi.clone());
        let pb = multi.map(|m| {
            let pb = ProgressBar::new(1);
            pb.set_style(ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
                .progress_chars("#>-"));
            m.add(pb)
        });

        for n in 0..=self.dl.i.n_retries {
            let err = match self.download_attempt(&pb, n).await {
                Ok(res) => return Ok(res),
                Err(DownloadError::Http(e)) => {
                    if e.status() != Some(StatusCode::FORBIDDEN) {
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

    async fn download_attempt(&self, pb: &Option<ProgressBar>, n: u32) -> Result<DownloadResult> {
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

        let attempt_suffix = if n > 0 {
            format!(" (retry #{n})")
        } else {
            String::new()
        };
        if let Some(pb) = pb {
            pb.set_message(format!(
                "Fetching player data for {}{}",
                self.video.name.as_deref().unwrap_or_default(),
                attempt_suffix
            ))
        }

        let q = self.dl.i.rp.query();
        let player_data = match self.player_type {
            Some(player_type) => q.player_from_client(&self.video.id, player_type).await?,
            None => q.player(&self.video.id).await?,
        };
        let user_agent = q.user_agent(player_data.client_type);

        // Select streams to download
        let (video, audio) = player_data.select_video_audio_stream(filter);

        if video.is_none() && audio.is_none() {
            return Err(DownloadError::Input("no stream found".into()));
        }

        let extension = match video {
            Some(_) => video_format.extension(),
            None => match audio {
                Some(audio) => match audio.codec {
                    AudioCodec::Mp4a => "m4a",
                    AudioCodec::Opus => "opus",
                    _ => return Err(DownloadError::Input("unknown audio codec".into())),
                },
                None => unreachable!(),
            },
        };

        let pv = DownloadVideo::from_video(&player_data);
        let output_path = self.dest.get_dest_path(&pv).with_extension(extension);

        if output_path.exists() {
            return Err(DownloadError::Exists(output_path));
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
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

        if let Some(pb) = pb {
            pb.set_message(format!(
                "Downloading {}{}",
                player_data.name(),
                attempt_suffix
            ))
        }
        download_streams(
            &downloads,
            self.dl.i.rp.http_client(),
            &user_agent,
            pb.clone(),
        )
        .await?;

        if let Some(pb) = &pb {
            pb.set_message(format!("Converting {}", player_data.name()));
            pb.set_style(
                ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}]")
                    .unwrap(),
            );
            pb.enable_steady_tick(Duration::from_millis(500));
        }

        convert_streams(
            &downloads,
            &output_path,
            &self.dl.i.ffmpeg,
            player_data.name(),
        )
        .await?;
        if let Some(pb) = pb {
            pb.disable_steady_tick();
        }

        // Delete original files
        stream::iter(&downloads)
            .map(|d| fs::remove_file(d.file.clone()))
            .buffer_unordered(downloads.len())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<core::result::Result<_, _>>()?;

        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        Ok(DownloadResult {
            dest: output_path,
            player_data,
        })
    }
}

fn get_download_range(offset: u64, size: Option<u64>) -> Range<u64> {
    let mut rng = rand::thread_rng();
    let chunk_size = rng.gen_range(CHUNK_SIZE_MIN..CHUNK_SIZE_MAX);
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

async fn download_single_file<P: Into<PathBuf>>(
    url: &str,
    output: P,
    http: &Client,
    user_agent: &str,
    pb: Option<ProgressBar>,
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

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&output_path_tmp)
        .await?;

    if is_gvideo && size.is_some() {
        download_chunks_by_param(http, &mut file, url, size.unwrap(), offset, user_agent, pb)
            .await?;
    } else {
        download_chunks_by_header(http, &mut file, url, size, offset, user_agent, pb).await?;
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
    pb: Option<ProgressBar>,
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
            if let Some(pb) = &pb {
                pb.inc_length(parsed_size);
            }
        }

        tracing::debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
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
async fn download_chunks_by_param(
    http: &Client,
    file: &mut File,
    url: &str,
    size: u64,
    offset: u64,
    user_agent: &str,
    pb: Option<ProgressBar>,
) -> Result<()> {
    let mut offset = offset;
    if let Some(pb) = &pb {
        pb.inc_length(size);
    }

    loop {
        let range = get_download_range(offset, Some(size));
        tracing::debug!("Fetching range {}-{}", range.start, range.end);

        let res = http
            .get(format!("{}&range={}-{}", url, range.start, range.end))
            .header(header::USER_AGENT, user_agent)
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com/")
            .send()
            .await?
            .error_for_status()?;

        let clen = res.content_length().unwrap();

        tracing::debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
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
    downloads: &Vec<StreamDownload>,
    http: &Client,
    user_agent: &str,
    pb: Option<ProgressBar>,
) -> Result<()> {
    let n = downloads.len();

    stream::iter(downloads)
        .map(|d| download_single_file(&d.url, d.file.clone(), http, user_agent, pb.clone()))
        .buffer_unordered(n)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    Ok(())
}

async fn convert_streams<P: Into<PathBuf>>(
    downloads: &[StreamDownload],
    output: P,
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
