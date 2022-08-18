use std::{cmp::Ordering, ffi::OsString, ops::Range, path::PathBuf};

use anyhow::{anyhow, bail, Result};
use fancy_regex::Regex;
use futures::stream::{self, StreamExt};
use indicatif::ProgressBar;
use log::{debug, info};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

use crate::{
    model::{AudioCodec, FileFormat, PlayerData, VideoCodec},
    util,
};

const CHUNK_SIZE_MIN: u64 = 9000000;
const CHUNK_SIZE_MAX: u64 = 10000000;

fn get_download_range(offset: u64, size: Option<u64>) -> Range<u64> {
    let mut rng = rand::thread_rng();
    let chunk_size = rng.gen_range(CHUNK_SIZE_MIN..CHUNK_SIZE_MAX);
    let mut chunk_end = offset + chunk_size;

    if size.is_some() {
        chunk_end = chunk_end.min(size.unwrap() - 1)
    }

    Range {
        start: offset,
        end: chunk_end,
    }
}

fn parse_cr_header(cr_header: &str) -> Result<(u64, u64)> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"bytes (\d+)-(\d+)/(\d+)"#).unwrap());

    let captures = some_or_bail!(
        PATTERN.captures(&cr_header).ok().flatten(),
        Err(anyhow!(
            "Content-Range header '{}' does not match pattern.",
            cr_header
        ))
    );

    Ok((
        captures.get(2).unwrap().as_str().parse()?,
        captures.get(3).unwrap().as_str().parse()?,
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
    let (url_base, url_params) = util::url_to_params(url)?;
    let is_gvideo = url_base.ends_with(".googlevideo.com/videoplayback");
    if is_gvideo {
        size = url_params
            .get("clen")
            .map(|s| s.parse::<u64>().ok())
            .flatten();
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

        let cr_header = some_or_bail!(
            res.headers().get(header::CONTENT_RANGE),
            Err(anyhow!("Did not get Content-Range header"))
        )
        .to_str()?;

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
                return Err(anyhow!(
                    "Already downloaded file {} is larger than original",
                    output_path_tmp.to_str().unwrap_or_default()
                ));
            }
        }
    }

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(output_path_tmp.to_owned())
        .await?;

    if is_gvideo && size.is_some() {
        download_chunks_by_param(http, &mut file, url, size.unwrap(), offset, pb).await?;
    } else {
        download_chunks_by_header(http, &mut file, url, size, offset, pb).await?;
    }

    fs::rename(output_path_tmp, output_path).await?;
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
        let cr_header = some_or_bail!(
            res.headers().get(header::CONTENT_RANGE),
            Err(anyhow!("Did not get Content-Range header"))
        )
        .to_str()?;

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

struct StreamDownload {
    file: PathBuf,
    // track_name: String TODO: add for multiple audio languages,
    url: String,
    audio_codec: Option<AudioCodec>,
    video_codec: Option<VideoCodec>,
}

pub async fn download_video(
    player_data: &PlayerData,
    output_dir: &str,
    output_fname: Option<String>,
    resolution: Option<u32>,
    ffmpeg: &str,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    // Download filepath
    let download_dir = PathBuf::from(output_dir);
    let title = player_data.info.title.to_owned();
    let output_fname_set = output_fname.is_some();
    let output_fname = output_fname
        .unwrap_or_else(|| filenamify::filenamify(format!("{} [{}]", title, player_data.info.id)));

    // Select streams to download
    let video = match resolution {
        Some(r) => Some(some_or_bail!(
            player_data
                .video_only_streams
                .iter()
                .rev()
                .find(|s| s.height <= r && !s.hdr)
                .clone(),
            Err(anyhow!("no video stream matching res"))
        )),
        None => None,
    };

    let audio = some_or_bail!(
        player_data
            .audio_streams
            .iter()
            .rev()
            .filter(|a| a.codec != AudioCodec::Unknown)
            .next(),
        Err(anyhow!("no audio stream"))
    );

    let format = match video {
        Some(_) => "mp4",
        None => match audio.codec {
            AudioCodec::Unknown => panic!(),
            AudioCodec::Mp4a => "m4a",
            AudioCodec::Opus => "opus",
        },
    };

    let output_path = download_dir.join(&output_fname).with_extension(format);
    if output_path.exists() {
        // If the downloaded video already exists, only error if the download path was
        // chosen explicitly.
        if output_fname_set {
            bail!("File {} already exists", output_path.to_string_lossy());
        } else {
            info!(
                "Downloaded video {} already exists",
                output_path.to_string_lossy()
            );
            return Ok(());
        }
    }

    let mut downloads: Vec<StreamDownload> = Vec::new();

    video.map(|v| {
        downloads.push(StreamDownload {
            file: download_dir.join(format!("{}.video{}", output_fname, v.format.extension())),
            url: v.url.to_owned(),
            video_codec: Some(v.codec),
            audio_codec: None,
        });
    });
    downloads.push(StreamDownload {
        file: download_dir.join(format!(
            "{}.audio{}",
            output_fname,
            audio.format.extension()
        )),
        url: audio.url.to_owned(),
        video_codec: None,
        audio_codec: Some(audio.codec),
    });

    pb.set_message(format!("Downloading {}", title));
    download_streams(&downloads, http, pb.clone()).await?;

    pb.set_message(format!("Converting {}", title));
    convert_streams(&downloads, output_path, ffmpeg).await?;

    // Delete original files
    stream::iter(&downloads)
        .map(|d| fs::remove_file(d.file.to_owned()))
        .buffer_unordered(downloads.len())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<_, _>>()?;

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

// ffmpeg -i TAEYEON\ 태연\ \'INVU\'\ MV.video.mp4
// -i TAEYEON\ 태연\ \'INVU\'\ MV.audio.webm -i hypa_audio.webm
// -map 0:v -map 1:a -map 2:a -metadata:s:a:1 language=en
// -metadata:s:a:2 language=de -c copy multiaudio.mp4
async fn convert_streams<P: Into<PathBuf>>(
    downloads: &Vec<StreamDownload>,
    output: P,
    ffmpeg: &str,
) -> Result<()> {
    let output_path: PathBuf = output.into();

    let mut args: Vec<OsString> = vec![];
    let mut mapping_args: Vec<OsString> = vec![];
    // let mut meta_args: Vec<OsString> = vec![];

    downloads.iter().enumerate().for_each(|(i, d)| {
        args.push("-i".into());
        args.push(d.file.to_owned().into());

        mapping_args.push("-map".into());
        mapping_args.push(i.to_string().into());
    });

    args.append(&mut mapping_args);

    args.push("-c".into());
    args.push("copy".into());
    args.push(output_path.into());

    let res = Command::new(ffmpeg).args(args).output().await?;

    if !res.status.success() {
        bail!(
            "ffmpeg error: {}",
            std::str::from_utf8(&res.stderr).unwrap_or_default()
        )
    }
    Ok(())
}

/*
#[cfg(test)]
mod tests {
    use crate::client::RustyTube;

    use super::*;
    use indicatif::{ProgressDrawTarget, ProgressStyle};
    use reqwest::ClientBuilder;

    // #[test_log::test(tokio::test)]
    #[tokio::test]
    async fn t_download_video() {
        let http = ClientBuilder::new()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0",
            )
            .gzip(true)
            .brotli(true)
            .build()
            .expect("unable to build the HTTP client");

        // Indicatif setup
        let pb = ProgressBar::new(0);

        let rt = RustyTube::new();
        let player_data = rt
            .get_player("AbZH7XWDW_k", crate::client::ClientType::Desktop)
            .await
            .unwrap();

        // download_video(&player_data, "tmp", "INVU", Some(1080), "ffmpeg", http, pb)
        //     .await
        //     .unwrap();
    }
}
*/
