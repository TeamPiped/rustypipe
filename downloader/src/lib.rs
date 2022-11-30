//! # YouTube audio/video downloader

mod util;

use std::{borrow::Cow, cmp::Ordering, ffi::OsString, ops::Range, path::PathBuf, time::Duration};

use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client};
use rustypipe::{
    model::{AudioCodec, FileFormat, VideoCodec, VideoPlayer},
    param::StreamFilter,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

use util::DownloadError;

type Result<T> = core::result::Result<T, DownloadError>;

const CHUNK_SIZE_MIN: u64 = 9000000;
const CHUNK_SIZE_MAX: u64 = 10000000;

fn get_download_range(offset: u64, size: Option<u64>) -> Range<u64> {
    let mut rng = rand::thread_rng();
    let chunk_size = rng.gen_range(CHUNK_SIZE_MIN..CHUNK_SIZE_MAX);
    let mut chunk_end = offset + chunk_size;

    if let Some(size) = size {
        chunk_end = chunk_end.min(size - 1)
    }

    Range {
        start: offset,
        end: chunk_end,
    }
}

fn parse_cr_header(cr_header: &str) -> Result<(u64, u64)> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"bytes (\d+)-(\d+)/(\d+)"#).unwrap());

    let captures = PATTERN.captures(cr_header).ok_or_else(|| {
        DownloadError::Progressive(
            format!(
                "Content-Range header '{}' does not match pattern",
                cr_header
            )
            .into(),
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

async fn download_single_file<P: Into<PathBuf>>(
    url: &str,
    output: P,
    http: Client,
    pb: ProgressBar,
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

                pb.inc_length(original_size);
                pb.inc(offset);
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
        download_chunks_by_param(http, &mut file, url, size.unwrap(), offset, pb).await?;
    } else {
        download_chunks_by_header(http, &mut file, url, size, offset, pb).await?;
    }

    fs::rename(&output_path_tmp, &output_path).await?;
    Ok(())
}

// Use the HTTP range header to download a stream in chunks.
// This is the standardized method that works on all web servers,
// but I have observed throttling using this method.
async fn download_chunks_by_header(
    http: Client,
    file: &mut File,
    url: &str,
    size: Option<u64>,
    offset: u64,
    pb: ProgressBar,
) -> Result<()> {
    let mut offset = offset;
    let mut size = size;

    loop {
        let range = get_download_range(offset, size);
        debug!("Fetching range {}-{}", range.start, range.end);

        let res = http
            .get(url.to_owned())
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
            pb.inc_length(parsed_size);
        }

        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
            pb.inc(chunk.len() as u64);
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
    http: Client,
    file: &mut File,
    url: &str,
    size: u64,
    offset: u64,
    pb: ProgressBar,
) -> Result<()> {
    let mut offset = offset;
    pb.inc_length(size);

    loop {
        let range = get_download_range(offset, Some(size));
        debug!("Fetching range {}-{}", range.start, range.end);

        let res = http
            .get(format!("{}&range={}-{}", url, range.start, range.end))
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com/")
            .send()
            .await?
            .error_for_status()?;

        let clen = res.content_length().unwrap();

        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = item?;
            pb.inc(chunk.len() as u64);
            file.write_all_buf(&mut chunk).await?;
        }

        offset += clen;
        debug!("offset inc by {}, new: {}", clen, offset);
        if offset >= size {
            break;
        }
    }
    Ok(())
}

#[allow(dead_code)]
struct StreamDownload {
    file: PathBuf,
    // track_name: String TODO: add for multiple audio languages,
    url: String,
    audio_codec: Option<AudioCodec>,
    video_codec: Option<VideoCodec>,
}

#[allow(clippy::too_many_arguments)]
pub async fn download_video(
    player_data: &VideoPlayer,
    output_dir: &str,
    output_fname: Option<String>,
    output_format: Option<String>,
    filter: &StreamFilter<'_>,
    ffmpeg: &str,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    // Download filepath
    let download_dir = PathBuf::from(output_dir);
    let title = player_data.details.title.to_owned();
    let output_fname_set = output_fname.is_some();
    let output_fname = output_fname.unwrap_or_else(|| {
        filenamify::filenamify(format!("{} [{}]", title, player_data.details.id))
    });

    // Select streams to download
    let (video, audio) = player_data.select_video_audio_stream(filter);

    if video.is_none() && audio.is_none() {
        return Err(DownloadError::Input("no stream found".into()));
    }

    let format = output_format.unwrap_or(
        match video {
            Some(_) => "mp4",
            None => match audio {
                Some(audio) => match audio.codec {
                    AudioCodec::Mp4a => "m4a",
                    AudioCodec::Opus => "opus",
                    _ => return Err(DownloadError::Input("unknown audio codec".into())),
                },
                None => unreachable!(),
            },
        }
        .to_owned(),
    );

    let output_path = download_dir.join(&output_fname).with_extension(&format);
    if output_path.exists() {
        // If the downloaded video already exists, only error if the download path was
        // chosen explicitly.
        if output_fname_set {
            return Err(DownloadError::Input(
                format!("File {} already exists", output_path.to_string_lossy()).into(),
            ))?;
        } else {
            info!(
                "Downloaded video {} already exists",
                output_path.to_string_lossy()
            );
            return Ok(());
        }
    }

    match (video, audio) {
        // Downloading combined video/audio stream (no conversion)
        (Some(video), None) => {
            pb.set_message(format!("Downloading {}", title));
            download_single_file(
                &video.url,
                download_dir.join(output_fname).with_extension(&format),
                http,
                pb.clone(),
            )
            .await?;
        }
        // Downloading split video/audio streams (requires conversion with ffmpeg)
        _ => {
            let mut downloads: Vec<StreamDownload> = Vec::new();

            if let Some(v) = video {
                downloads.push(StreamDownload {
                    file: download_dir.join(format!(
                        "{}.video{}",
                        output_fname,
                        v.format.extension()
                    )),
                    url: v.url.to_owned(),
                    video_codec: Some(v.codec),
                    audio_codec: None,
                });
            }
            if let Some(a) = audio {
                downloads.push(StreamDownload {
                    file: download_dir.join(format!(
                        "{}.audio{}",
                        output_fname,
                        a.format.extension()
                    )),
                    url: a.url.to_owned(),
                    video_codec: None,
                    audio_codec: Some(a.codec),
                })
            }

            pb.set_message(format!("Downloading {}", title));
            download_streams(&downloads, http, pb.clone()).await?;

            pb.set_message(format!("Converting {}", title));
            pb.set_style(
                ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}]")
                    .unwrap(),
            );
            pb.enable_steady_tick(Duration::from_millis(100));
            convert_streams(&downloads, output_path, ffmpeg).await?;
            pb.disable_steady_tick();

            // Delete original files
            stream::iter(&downloads)
                .map(|d| fs::remove_file(d.file.to_owned()))
                .buffer_unordered(downloads.len())
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<core::result::Result<_, _>>()?;
        }
    }

    pb.finish_and_clear();
    Ok(())
}

async fn download_streams(
    downloads: &Vec<StreamDownload>,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    let n = downloads.len();

    stream::iter(downloads)
        .map(|d| download_single_file(&d.url, d.file.to_owned(), http.clone(), pb.clone()))
        .buffer_unordered(n)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    Ok(())
}

async fn convert_streams<P: Into<PathBuf>>(
    downloads: &Vec<StreamDownload>,
    output: P,
    ffmpeg: &str,
) -> Result<()> {
    let output_path: PathBuf = output.into();

    let mut args: Vec<OsString> = vec![];
    let mut mapping_args: Vec<OsString> = vec![];

    downloads.iter().enumerate().for_each(|(i, d)| {
        args.push("-i".into());
        args.push(d.file.to_owned().into());

        mapping_args.push("-map".into());
        mapping_args.push(i.to_string().into());
    });

    args.append(&mut mapping_args);

    // Combining multiple streams, keep codecs
    if downloads.len() > 1 {
        args.push("-c".into());
        args.push("copy".into());
    }

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
