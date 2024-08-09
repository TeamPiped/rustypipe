#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::todo, clippy::dbg_macro)]

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
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client, StatusCode, Url};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{
        traits::{FileFormat, YtEntity},
        AudioCodec, TrackItem, VideoCodec, VideoPlayer,
    },
    param::StreamFilter,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

#[cfg(feature = "indicatif")]
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

#[cfg(feature = "audiotag")]
use lofty::{config::WriteOptions, picture::Picture, prelude::*, tag::Tag};
#[cfg(feature = "audiotag")]
use rustypipe::model::{richtext::ToPlaintext, VideoDetails, VideoPlayerDetails};
#[cfg(feature = "audiotag")]
use time::{Date, OffsetDateTime};

pub use util::DownloadError;

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
    pot: Option<String>,
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
    /// Pot token to circumvent bot detection
    pot: Option<String>,
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
    /// ClientType type for fetching videos
    client_type: Option<ClientType>,
    /// Pot token to circumvent bot detection
    pot: Option<String>,
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
                .map(|n| n.strip_suffix(" - Topic").unwrap_or(n).to_owned()),
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
            pot: None,
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
    #[must_use]
    pub fn multi_progress(mut self, progress: MultiProgress) -> Self {
        self.multi = Some(progress);
        self
    }

    /// Set the indicatif [`ProgressStyle`] for the progress bars displayed under `multi_progress`
    #[cfg(feature = "indicatif")]
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

    /// Enable audio tagging
    #[cfg(feature = "audiotag")]
    #[must_use]
    pub fn audio_tag(mut self) -> Self {
        self.audio_tag = true;
        self
    }

    /// Crop YouTube thumbnails to get square album covers
    #[cfg(feature = "audiotag")]
    #[must_use]
    pub fn crop_cover(mut self) -> Self {
        self.crop_cover = true;
        self
    }

    /// Set the `pot` token to circumvent bot detection
    ///
    /// YouTube has implemented the token to prevent other clients from downloading YouTube videos.
    /// The token is generated using YouTube's botguard. Therefore you need a full browser environment
    /// to obtain one.
    ///
    /// The Invidious project has created a script to extract this token: <https://github.com/iv-org/youtube-trusted-session-generator>
    ///
    /// The `pot` token is only used for the [`ClientType::Desktop`] and [`ClientType::DesktopMusic`] clients.
    #[must_use]
    pub fn pot<S: Into<String>>(mut self, pot: S) -> Self {
        self.pot = Some(pot.into());
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
                pot: self.pot,
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
            client_type: None,
            pot: None,
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

    /// Download a video from a [`TrackItem`] (YouTube music album/playlist item)
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

    /// Set the [`VideoFormat`] of downloaded videos
    #[must_use]
    pub fn video_format(mut self, video_format: DownloadVideoFormat) -> Self {
        self.video_format = Some(video_format);
        self
    }

    /// Set the [`ClientType`] used to fetch the YT player
    #[must_use]
    pub fn client_type(mut self, client_type: ClientType) -> Self {
        self.client_type = Some(client_type);
        self
    }

    /// Set the `pot` token to circumvent bot detection
    ///
    /// YouTube has implemented the token to prevent other clients from downloading YouTube videos.
    /// The token is generated using YouTube's botguard. Therefore you need a full browser environment
    /// to obtain one.
    ///
    /// The Invidious project has created a script to extract this token: <https://github.com/iv-org/youtube-trusted-session-generator>
    ///
    /// The `pot` token is only used for the [`ClientType::Desktop`] and [`ClientType::DesktopMusic`] clients.
    #[must_use]
    pub fn pot<S: Into<String>>(mut self, pot: S) -> Self {
        self.pot = Some(pot.into());
        self
    }

    /// Download the video
    ///
    /// If no download path is set, the video is downloaded to the current directory
    /// with a filename created by this template: `{track} {title} [{id}]`.
    #[tracing::instrument(skip(self), fields(id = self.video.id))]
    pub async fn download(&self) -> Result<DownloadResult> {
        let mut last_err = None;

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
                    #[cfg(feature = "indicatif")]
                    &pb,
                )
                .await
            {
                Ok(res) => return Ok(res),
                Err(DownloadError::Http(e)) => {
                    if !e.is_timeout() && e.status() != Some(StatusCode::FORBIDDEN) {
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
            pb.set_message(format!(
                "Fetching player data for {}{}",
                self.video.name.as_deref().unwrap_or_default(),
                attempt_suffix
            ))
        }

        let q = self.dl.i.rp.query();
        let player_data = match self.client_type {
            Some(client_type) => q.player_from_client(&self.video.id, client_type).await?,
            None => q.player(&self.video.id).await?,
        };
        let user_agent = q.user_agent(player_data.client_type);
        let pot = if matches!(
            player_data.client_type,
            ClientType::Desktop | ClientType::DesktopMusic
        ) {
            self.pot.as_deref().or(self.dl.i.pot.as_deref())
        } else {
            None
        };

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
        download_streams(
            &downloads,
            &self.dl.i.http,
            &user_agent,
            pot,
            #[cfg(feature = "indicatif")]
            pb.clone(),
        )
        .await?;

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
        if self.dl.i.audio_tag && video.is_none() {
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
        stream::iter(&downloads)
            .map(|d| fs::remove_file(d.file.clone()))
            .buffer_unordered(downloads.len())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<core::result::Result<(), _>>()?;

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
            let resp = self
                .dl
                .i
                .http
                .get(thumbnail.url)
                .send()
                .await?
                .error_for_status()?;
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

                    let crop = smartcrop::find_best_crop(&img, NonZeroU32::MIN, NonZeroU32::MIN)
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

async fn download_single_file(
    url: &str,
    output: &Path,
    http: &Client,
    user_agent: &str,
    pot: Option<&str>,
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

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&output_path_tmp)
        .await?;

    if is_gvideo && size.is_some() {
        download_chunks_by_param(
            http,
            &mut file,
            url,
            size.unwrap(),
            offset,
            user_agent,
            pot,
            #[cfg(feature = "indicatif")]
            pb,
        )
        .await?;
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
        .await?;
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
    pot: Option<&str>,
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

        let mut urlp =
            Url::parse_with_params(url, [("range", &format!("{}-{}", range.start, range.end))])
                .map_err(|e| DownloadError::Progressive(format!("url parsing: {e}").into()))?;
        if let Some(pot) = pot {
            urlp.query_pairs_mut().append_pair("pot", pot);
        }

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
    pot: Option<&str>,
    #[cfg(feature = "indicatif")] pb: Option<ProgressBar>,
) -> Result<()> {
    let n = downloads.len();

    stream::iter(downloads)
        .map(|d| {
            download_single_file(
                &d.url,
                &d.file,
                http,
                user_agent,
                pot,
                #[cfg(feature = "indicatif")]
                pb.clone(),
            )
        })
        .buffer_unordered(n)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    Ok(())
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
