use std::{fs, os::unix::fs::MetadataExt, path::Path, process::Command};

use path_macro::path;
use rstest::{fixture, rstest};
use rustypipe::{client::RustyPipe, model::AudioCodec, param::StreamFilter};
use rustypipe_downloader::Downloader;
use temp_testdir::TempDir;

/// Get a new RusttyPipe instance
#[fixture]
fn rp() -> RustyPipe {
    let vdata = std::env::var("YT_VDATA").ok();
    RustyPipe::builder()
        .strict()
        .storage_dir(path!(env!("CARGO_MANIFEST_DIR") / ".."))
        .visitor_data_opt(vdata)
        .build()
        .unwrap()
}

#[rstest]
#[tokio::test]
async fn download_video(rp: RustyPipe) {
    let td = TempDir::default();
    let td_path = td.to_path_buf();

    let dl = Downloader::builder().rustypipe(&rp).build();

    let res = dl
        .id("UXqq0ZvbOnk")
        .to_dir(&td_path)
        .stream_filter(StreamFilter::new().video_max_res(480))
        .download()
        .await
        .unwrap();

    assert_eq!(
        res.dest,
        path!(td_path / "CHARGE - Blender Open Movie [UXqq0ZvbOnk].mp4")
    );
    assert_eq!(res.player_data.details.id, "UXqq0ZvbOnk");
}

#[rstest]
#[tokio::test]
async fn download_music(rp: RustyPipe) {
    let td = TempDir::default();
    let td_path = td.to_path_buf();

    let dl = Downloader::builder()
        .audio_tag()
        .crop_cover()
        .rustypipe(&rp)
        .build();

    let res = dl
        .id("bVtv3st8bgc")
        .to_dir(&td_path)
        .stream_filter(
            StreamFilter::new()
                .no_video()
                .audio_codecs([AudioCodec::Opus]),
        )
        .download()
        .await
        .unwrap();

    assert_eq!(
        res.dest,
        path!(td_path / "Lord of the Riffs [bVtv3st8bgc].opus")
    );
    assert_eq!(res.player_data.details.id, "bVtv3st8bgc");
    let fm = fs::metadata(&res.dest).unwrap();
    assert_gte(fm.size(), 6_000_000, "file size");
    assert_audio_meta(
        &res.dest,
        "Lord of the Riffs",
        "Alexander Nakarada - CreatorChords",
        "Lord of the Riffs",
        "2022-02-05",
    );
}

/// Assert that number A is greater than or equal to number B
#[track_caller]
fn assert_gte<T: PartialOrd + std::fmt::Display>(a: T, b: T, msg: &str) {
    assert!(a >= b, "expected >= {b} {msg}, got {a}");
}

#[track_caller]
fn assert_audio_meta(p: &Path, title: &str, artist: &str, album: &str, date: &str) {
    let res = Command::new("ffprobe")
        .args([
            "-loglevel",
            "error",
            "-show_entries",
            "stream_tags",
            "-of",
            "json",
        ])
        .arg(p)
        .output()
        .unwrap();
    if !res.status.success() {
        panic!("ffprobe error\n{}", String::from_utf8_lossy(&res.stderr))
    }
    let res_json = serde_json::from_slice::<serde_json::Value>(&res.stdout).unwrap();
    let tags = &res_json["streams"][0]["tags"];
    assert_eq!(tags["TITLE"].as_str(), Some(title));
    assert_eq!(tags["ARTIST"].as_str(), Some(artist));
    assert_eq!(tags["ALBUM"].as_str(), Some(album));
    assert_eq!(tags["DATE"].as_str(), Some(date));
}
