use std::{cmp::Ordering, ops::Range, path::PathBuf};

use anyhow::{anyhow, bail, Result};
use fancy_regex::Regex;
use futures::stream::StreamExt;
use indicatif::ProgressBar;
use log::debug;
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client};
use tokio::{fs, io::AsyncWriteExt, process::Command};

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

async fn download_single_file(
    url: &str,
    output: &str,
    http: Client,
    pb: ProgressBar,
) -> Result<()> {
    // Check if file is already downloaded
    let output_path = PathBuf::from(output);
    if output_path.exists() {
        return Ok(());
    }

    let output_path_tmp = PathBuf::from(output.to_owned() + ".part");
    let mut offset: u64 = 0;
    let mut size: Option<u64> = None;

    // Check if file is partially downloaded
    if output_path_tmp.exists() {
        let file_size = output_path_tmp.metadata()?.len();

        let res = http
            .head(url)
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

    loop {
        let range = get_download_range(offset, size);
        debug!("Fetching range {}-{}", range.start, range.end);

        let res = http
            .get(url)
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

        offset = parsed_offset;
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

        if offset >= size.unwrap() - 1 {
            break;
        }
    }

    fs::rename(output_path_tmp, output_path).await?;
    Ok(())
}

// ffmpeg -i video.webm -i audio.webm -c copy output.mp4
async fn join_video_audio(
    video_file: &str,
    audio_file: &str,
    output_file: &str,
    ffmpeg: &str,
) -> Result<()> {
    let res = Command::new(ffmpeg)
        .args([
            "-i",
            video_file,
            "-i",
            audio_file,
            "-c",
            "copy",
            output_file,
        ])
        .output()
        .await?;

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
    use super::*;
    use indicatif::ProgressStyle;
    use reqwest::ClientBuilder;

    const TEST_URL_AUDIO: &str = "https://rr5---sn-h0jeenl6.googlevideo.com/videoplayback?c=WEB&clen=3781277&dur=229.301&ei=Wd3oYqnIHLKYx_APgPCeoAM&expire=1659449785&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AH9zSQFJUzAo61SdF1m4PUcknacuL35Mm8TgOmD5lfwF&initcwndbps=1597500&ip=2003%3Ade%3Aaf0e%3A2f00%3Ade47%3A297%3Aa6db%3A774e&itag=251&keepalive=yes&lmt=1655510291473933&lsig=AG3C_xAwRgIhAOPc6qa-C6x1GOFxx5hpiP_ZFFeCAdHSr43mq4PujcasAiEA8NHcpNsurS187Gjg1WseiaQ_kslkKWU4fylIVGr4p8Y%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=audio%2Fwebm&mm=31%2C26&mn=sn-h0jeenl6%2Csn-4g5ednsl&ms=au%2Conr&mt=1659427257&mv=m&mvi=5&n=cRL0RZUaCeszsQ&ns=1UbvTJx8sEFT4vlb0jQyd68H&pl=37&requiressl=yes&sig=AOq0QJ8wRQIgRY8UR_GHs7T2ZX-0g6vRzvQS5MqpAMOs3sBpPthEzMUCIQDkh7aZOGpgzy82ha2CN2yiYS9NVHBd5WGa1e3K8GYKKg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khs9YQMiuc_CePC7R74ycrwd1hNk&txp=4532434&vprv=1";
    const TEST_URL_VIDEO: &str = "https://rr5---sn-h0jeenl6.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C137%2C160%2C242%2C243%2C244%2C247%2C248%2C271%2C278%2C313%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401&c=WEB&clen=66413039&dur=229.270&ei=Wd3oYqnIHLKYx_APgPCeoAM&expire=1659449785&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AH9zSQFJUzAo61SdF1m4PUcknacuL35Mm8TgOmD5lfwF&initcwndbps=1597500&ip=2003%3Ade%3Aaf0e%3A2f00%3Ade47%3A297%3Aa6db%3A774e&itag=248&keepalive=yes&lmt=1655512874472691&lsig=AG3C_xAwRAIgbmq3hI3VDXrOvENhCotYujpiKaJODqLVq-Il8K9OIwwCIHk-H0SzI4tH1w3TzKnVSbpjghk3AByD9VD75Ywii1F_&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=video%2Fwebm&mm=31%2C26&mn=sn-h0jeenl6%2Csn-4g5ednsl&ms=au%2Conr&mt=1659427257&mv=m&mvi=5&n=cRL0RZUaCeszsQ&ns=1UbvTJx8sEFT4vlb0jQyd68H&pl=37&requiressl=yes&sig=AOq0QJ8wRgIhAOuxn8gnk3FFCPPpEoylYPcLyas52BvyT7DzSAsbmJMIAiEAzUAnieCK31ZVQydfExQ5FSrCGJR3AzcwqgpENBzunjA%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khs9YQMiuc_CePC7R74ycrwd1hNk&txp=4537434&vprv=1";

    // #[tokio::test]
    async fn test() {
        // download_file(TEST_URL_LARGE, ".tmp/test.webm").await;
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
        pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
        pb.set_message("Downloading");

        let (r1, r2) = tokio::join!(
            download_single_file(TEST_URL_VIDEO, "tmp/test.webm", http.clone(), pb.clone()),
            download_single_file(TEST_URL_AUDIO, "tmp/test_audio.webm", http, pb)
        );
        r1.unwrap();
        r2.unwrap();

        join_video_audio(
            "tmp/test.webm",
            "tmp/test_audio.webm",
            "tmp/test.mp4",
            "ffmpeg",
        )
        .await
        .unwrap();
    }
}
