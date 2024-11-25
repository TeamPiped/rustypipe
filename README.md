# ![RustyPipe](https://codeberg.org/ThetaDev/rustypipe/raw/branch/main/notes/logo.svg)

[![Current crates.io version](https://img.shields.io/crates/v/rustypipe.svg)](https://crates.io/crates/rustypipe)
[![License](https://img.shields.io/badge/License-GPL--3-blue.svg?style=flat)](https://opensource.org/licenses/GPL-3.0)
[![Docs](https://img.shields.io/docsrs/rustypipe/latest?style=flat)](https://docs.rs/rustypipe)
[![CI status](https://codeberg.org/ThetaDev/rustypipe/actions/workflows/ci.yaml/badge.svg?style=flat&label=CI)](https://codeberg.org/ThetaDev/rustypipe/actions/?workflow=ci.yaml)

RustyPipe is a fully featured Rust client for the public YouTube / YouTube Music API
(Innertube), inspired by [NewPipe](https://github.com/TeamNewPipe/NewPipeExtractor).

## Features

### YouTube

- **Player** (video/audio streams, subtitles)
- **VideoDetails** (metadata, comments, recommended videos)
- **Playlist**
- **Channel** (videos, shorts, livestreams, playlists, info, search)
- **ChannelRSS**
- **Search** (with filters)
- **Search suggestions**
- **Trending**
- **URL resolver**

### YouTube Music

- **Playlist**
- **Album**
- **Artist**
- **Search**
- **Search suggestions**
- **Radio**
- **Track details** (lyrics, recommendations)
- **Moods/Genres**
- **Charts**
- **New** (albums, music videos)

## Getting started

### Cargo.toml

```toml
[dependencies]
rustypipe = "0.1.3"
tokio = { version = "1.20.0", features = ["macros", "rt-multi-thread"] }
```

### Watch a video

```rust ignore
use std::process::Command;

use rustypipe::{client::RustyPipe, param::StreamFilter};

#[tokio::main]
async fn main() {
    // Create a client
    let rp = RustyPipe::new();
    // Fetch the player
    let player = rp.query().player("pPvd8UxmSbQ").await.unwrap();
    // Select the best streams
    let (video, audio) = player.select_video_audio_stream(&StreamFilter::default());

    // Open mpv player
    let mut args = vec![video.expect("no video stream").url.to_owned()];
    if let Some(audio) = audio {
        args.push(format!("--audio-file={}", audio.url));
    }
    Command::new("mpv").args(args).output().unwrap();
}
```

### Get a playlist

```rust ignore
use rustypipe::client::RustyPipe

#[tokio::main]
async fn main() {
    // Create a client
    let rp = RustyPipe::new();
    // Get the playlist
    let playlist = rp
        .query()
        .playlist("PL2_OBreMn7FrsiSW0VDZjdq0xqUKkZYHT")
        .await
        .unwrap();
    // Get all items (maximum: 1000)
    playlist.videos.extend_limit(rp.query(), 1000).await.unwrap();

    println!("Name: {}", playlist.name);
    println!("Author: {}", playlist.channel.unwrap().name);
    println!("Last update: {}", playlist.last_update.unwrap());

    playlist
        .videos
        .items
        .iter()
        .for_each(|v| println!("[{}] {} ({}s)", v.id, v.name, v.length));
}
```

**Output:**

```txt
Name: Homelab
Author: Jeff Geerling
Last update: 2023-05-04
[cVWF3u-y-Zg] I put a computer in my computer (720s)
[ecdm3oA-QdQ] 6-in-1: Build a 6-node Ceph cluster on this Mini ITX Motherboard (783s)
[xvE4HNJZeIg] Scrapyard Server: Fastest all-SSD NAS! (733s)
[RvnG-ywF6_s] Nanosecond clock sync with a Raspberry Pi (836s)
[R2S2RMNv7OU] I made the Petabyte Raspberry Pi even faster! (572s)
[FG--PtrDmw4] Hiding Macs in my Rack! (515s)
...
```

### Get a channel

```rust ignore
use rustypipe::client::RustyPipe

#[tokio::main]
async fn main() {
    // Create a client
    let rp = RustyPipe::new();
    // Get the channel
    let channel = rp
        .query()
        .channel_videos("UCl2mFZoRqjw_ELax4Yisf6w")
        .await
        .unwrap();

    println!("Name: {}", channel.name);
    println!("Description: {}", channel.description);
    println!("Subscribers: {}", channel.subscriber_count.unwrap());

    channel
        .content
        .items
        .iter()
        .for_each(|v| println!("[{}] {} ({}s)", v.id, v.name, v.length.unwrap()));
}
```

**Output:**

```txt
Name: Louis Rossmann
Description: I discuss random things of interest to me. (...)
Subscribers: 1780000
[qBHgJx_rb8E] Introducing Rossmann senior, a genuine fossil 😃 (122s)
[TmV8eAtXc3s] Am I wrong about CompTIA? (592s)
[CjOJJc1qzdY] How FUTO projects loosen Google's grip on your life! (588s)
[0A10JtkkL9A] a private moment between a man and his kitten (522s)
[zbHq5_1Cd5U] Is Texas mandating auto repair shops use OEM parts? SB1083 analysis & breakdown; tldr, no. (645s)
[6Fv8bd9ICb4] Who owns this? (199s)
...
```

## Development

**Requirements:**

- Current version of stable Rust
- [`just`](https://github.com/casey/just) task runner
- [`nextest`](https://nexte.st) test runner
- [`pre-commit`](https://pre-commit.com/)
- yq (YAML processor)

### Tasks

**Testing**

- `just test` Run unit+integration tests
- `just unittest` Run unit tests
- `just testyt` Run YouTube integration tests
- `just testintl` Run YouTube integration tests for all supported languages (this takes
  a long time and is therefore not run in CI)
- `YT_LANG=de just testyt` Run YouTube integration tests for a specific language

**Tools**

- `just testfiles` Download missing testfiles for unit tests
- `just report2yaml` Convert RustyPipe reports into a more readable yaml format
  (requires `yq`)
