# ![RustyPipe](https://codeberg.org/ThetaDev/rustypipe/raw/branch/main/notes/logo.svg) Downloader

[![Current crates.io version](https://img.shields.io/crates/v/rustypipe-downloader.svg)](https://crates.io/crates/rustypipe-downloader)
[![License](https://img.shields.io/badge/License-GPL--3-blue.svg?style=flat)](http://opensource.org/licenses/MIT)
[![CI status](https://codeberg.org/ThetaDev/rustypipe/actions/workflows/ci.yaml/badge.svg?style=flat&label=CI)](https://codeberg.org/ThetaDev/rustypipe/actions/?workflow=ci.yaml)

The downloader is a companion crate for RustyPipe that allows for easy and fast
downloading of video and audio files.

## Features

- Fast download of streams, bypassing YouTube's throttling
- Join video and audio streams using ffmpeg
- [Indicatif](https://crates.io/crates/indicatif) support to show download progress bars
  (enable `indicatif` feature to use)
- Tag audio files with title, album, artist, date, description and album cover (enable
  `audiotag` feature to use)
- Album covers are automatically cropped using smartcrop to ensure they are square

## How to use

For the downloader to work, you need to have ffmpeg installed on your system. If your
ffmpeg binary is located at a non-standard path, you can configure the location using
[`DownloaderBuilder::ffmpeg`].

At first you have to instantiate and configure the downloader using either
[`Downloader::new`] or the [`DownloaderBuilder`].

Then you can build a new download query with a video ID, stream filter and destination
path and finally download the video.

```rust ignore
use rustypipe::param::StreamFilter;
use rustypipe_downloader::DownloaderBuilder;

let dl = DownloaderBuilder::new()
    .audio_tag()
    .crop_cover()
    .build();

let filter_audio = StreamFilter::new().no_video();
dl.id("eRsGyueVLvQ").stream_filter(filter_audio).to_file("audio.opus").download().await;

let filter_video = StreamFilter::new().video_max_res(720);
dl.id("eRsGyueVLvQ").stream_filter(filter_video).to_file("video.mp4").download().await;
```
