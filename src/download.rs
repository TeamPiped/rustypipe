use std::{cmp::Ordering, ffi::OsString, ops::Range, path::PathBuf};

use anyhow::{anyhow, bail, Result};
use fancy_regex::Regex;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client};
use tokio::{fs, io::AsyncWriteExt, process::Command};

use crate::model::{AudioCodec, FileFormat, PlayerData, VideoCodec};

const CHUNK_SIZE_MIN: u64 = 9000000;
const CHUNK_SIZE_MAX: u64 = 11000000;

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

async fn download_single_file<S: Into<String>, P: Into<PathBuf>>(
    url: S,
    output: P,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    // Check if file is already downloaded
    let output_path: PathBuf = output.into();
    let url: String = url.into();

    if output_path.exists() {
        return Ok(());
    }

    let output_path_tmp = output_path.with_extension(format!(
        "{}.part",
        output_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    let mut offset: u64 = 0;
    let mut size: Option<u64> = None;

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

    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb.set_message("Downloading");

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

    fs::rename(output_path_tmp, output_path).await?;
    Ok(())
}

struct StreamDownload {
    file: PathBuf,
    // track_name: String TODO: add for multiple audio languages,
    url: String,
    audio_codec: Option<AudioCodec>,
    video_codec: Option<VideoCodec>,
}

async fn download_video(
    player_data: &PlayerData,
    output_dir: &str,
    resolution: Option<u32>,
    ffmpeg: &str,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    // Select streams to download
    let video = match resolution {
        Some(r) => Some(some_or_bail!(
            player_data
                .video_only_streams
                .iter()
                .rev()
                .find(|s| s.height == r && !s.hdr)
                .clone(),
            Err(anyhow!("no video stream matching res"))
        )),
        None => None,
    };

    let audio = some_or_bail!(
        player_data.audio_streams.iter().rev().next(),
        Err(anyhow!("no audio stream"))
    );

    let download_dir = PathBuf::from(output_dir);
    let title_fname = player_data.info.title.to_owned(); // TODO: slugify

    let mut downloads: Vec<StreamDownload> = Vec::new();

    video.map(|v| {
        println!("Video: {}", v.url);
        downloads.push(StreamDownload {
            file: download_dir.join(format!("{}.video{}", title_fname, v.format.extension())),
            url: v.url.to_owned(),
            video_codec: Some(v.codec),
            audio_codec: None,
        });
    });
    println!("Audio: {}", audio.url);
    downloads.push(StreamDownload {
        file: download_dir.join(format!("{}.audio{}", title_fname, audio.format.extension())),
        url: audio.url.to_owned(),
        video_codec: None,
        audio_codec: Some(audio.codec),
    });

    download_streams(&downloads, http, pb).await?;

    let output_file = download_dir.join(format!("{}.mp4", title_fname));
    convert_streams(&downloads, output_file, ffmpeg).await?;

    Ok(())
}

async fn download_streams(
    downloads: &Vec<StreamDownload>,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    let n = downloads.len();

    stream::iter(downloads)
        .map(|d| {
            download_single_file(
                d.url.to_owned(),
                d.file.to_owned(),
                http.clone(),
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

// ffmpeg -i TAEYEON\ 태연\ \'INVU\'\ MV.video.mp4
// -i TAEYEON\ 태연\ \'INVU\'\ MV.audio.webm -i hypa_audio.webm
// -map 0:v -map 1:a -map 2:a -metadata:s:a:1 language=en
// -metadata:s:a:2 language=de -c copy multiaudio.mp4
async fn convert_streams<P: Into<PathBuf>>(
    downloads: &Vec<StreamDownload>,
    output: P,
    ffmpeg: &str,
) -> Result<()> {
    let output: PathBuf = output.into();
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
    args.push(output.into());

    let res = Command::new(ffmpeg).args(args).output().await?;

    if !res.status.success() {
        bail!(
            "ffmpeg error: {}",
            std::str::from_utf8(&res.stderr).unwrap_or_default()
        )
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::client::RustyTube;

    use super::*;
    use indicatif::{ProgressDrawTarget, ProgressStyle};
    use reqwest::ClientBuilder;

    const TEST_URL_AUDIO: &str = "https://rr2---sn-h0jelnes.googlevideo.com/videoplayback?c=WEB&clen=3548576&dur=217.281&ei=XLTsYqrjBZWI6dsPpN2piAM&expire=1659701436&fexp=24001373%2C24007246&fvip=3&gir=yes&id=o-ADzcOIYmmZUru2VQVa-K0lhP_Uwt-YB868WY1tQpxP29&initcwndbps=1550000&ip=2003%3Ade%3Aaf09%3A3800%3Adf03%3Aff5b%3A9fbd%3Aef0b&itag=251&keepalive=yes&lmt=1655066322398609&lsig=AG3C_xAwRQIhAPWzFISUntnQVCePCtbi3PwsrztgOM_ACh3OQX333boNAiBHcu5TJj8oQGmgz8sfm_I9jkbiCM1VOq_vW-wN0ARlMg%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=0P&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelnes%2Csn-h0jeened&ms=au%2Crdu&mt=1659679486&mv=m&mvi=2&n=9-E5diT6ORysAQ&ns=z1W4YnCGd7nB7ajH1gDgfDkH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAKd-cnF7ZCwKCi2J4_4R032sNFzquZUsgr0EStdolqETAiEAgBd-yD8HhXKiqll9_Pn_z2aWGBi1rcvqpO-KOsgaTZQ%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khvvt1xML3EE5f7dUNGCF9edAhhQ&txp=5532434&vp";
    const TEST_URL_VIDEO: &str = "https://rr2---sn-h0jelnes.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C137%2C160%2C242%2C243%2C244%2C247%2C248%2C271%2C278%2C313%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401&c=WEB&clen=53812383&dur=217.258&ei=XLTsYqrjBZWI6dsPpN2piAM&expire=1659701436&fexp=24001373%2C24007246&fvip=3&gir=yes&id=o-ADzcOIYmmZUru2VQVa-K0lhP_Uwt-YB868WY1tQpxP29&initcwndbps=1550000&ip=2003%3Ade%3Aaf09%3A3800%3Adf03%3Aff5b%3A9fbd%3Aef0b&itag=399&keepalive=yes&lmt=1655077485544227&lsig=AG3C_xAwRAIgYASOFHKLHNDlad52_t29Vem3WMdSI4n2cDkW_GxxGB0CICb1D5TmmApvKZQP-tf7Mq4pgYyA9ihm7Bx152GjrrFf&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=0P&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelnes%2Csn-h0jeened&ms=au%2Crdu&mt=1659679486&mv=m&mvi=2&n=9-E5diT6ORysAQ&ns=z1W4YnCGd7nB7ajH1gDgfDkH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgHo0czKIjgbtGJS9yQHRMHZyZ8tzRhgbxBAl2N39Ms0ICIQCSTqPrsewj0qYDxjXnp6nIuRkYZU6WTiHPeaXVz1-eEw%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khvvt1xML3EE5f7dUNGCF9edAhhQ&txp=5532434&vprv=1";

    #[test_log::test(tokio::test)]
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

        download_video(&player_data, "tmp", Some(1080), "ffmpeg", http, pb)
            .await
            .unwrap();
    }
}
