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
- **Subscriptions**
- **Playback history**

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
- **Saved items**
- **Playback history**

## Getting started

The RustyPipe library works as follows: at first you have to instantiate a RustyPipe
client. You can either create it with default options or use the `RustyPipe::builder()`
to customize it.

For fetching data you have to start with a new RustyPipe query object (`rp.query()`).
The query object holds options for an individual query (e.g. content language or
country). You can adjust these options with setter methods. Finally call your query
method to fetch the data you need.

All query methods are async, you need the tokio runtime to execute them.

```rust ignore
let rp = RustyPipe::new();
let rp = RustyPipe::builder().storage_dir("/app/data").build().unwrap();
let channel = rp.query().lang(Language::De).channel_videos("UCl2mFZoRqjw_ELax4Yisf6w").await.unwrap();
```

Here are a few examples to get you started:

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

## Crate features

Some features of RustyPipe are gated behind features to avoid compiling unneeded
dependencies.

- `rss` Fetch a channel's RSS feed, which is faster than fetching the channel page
- `userdata` Add functions to fetch YouTube user data (watch history, subscriptions,
  music library)

You can also choose the TLS library used for making web requests using the same features
as the reqwest crate (`default-tls`, `native-tls`, `native-tls-alpn`,
`native-tls-vendored`, `rustls-tls-webpki-roots`, `rustls-tls-native-roots`).

## Cache storage

The RustyPipe cache holds the current version numbers for all clients, the JavaScript
code used to deobfuscate video URLs and the authentication token/cookies. Never share
the contents of the cache if you are using authentication.

By default the cache is written to a JSON file named `rustypipe_cache.json` in the
current working directory. This path can be changed with the `storage_dir` option of the
RustyPipeBuilder. The RustyPipe CLI stores its cache in the userdata folder. The full
path on Linux is `~/.local/share/rustypipe/rustypipe_cache.json`.

You can integrate your own cache storage backend (e.g. database storage) by implementing
the `CacheStorage` trait.

## Reports

RustyPipe has a builtin error reporting system. If a YouTube response cannot be
deserialized or parsed, the original response data along with some request metadata is
written to a JSON file in the folder `rustypipe_reports`, located in RustyPipe's storage
directory (current folder by default, `~/.local/share/rustypipe` for the CLI).

When submitting a bug report to the RustyPipe project, you can share this report to help
resolve the issue.

RustyPipe reports come in 3 severity levels:

- DBG (no error occurred, report creation was enabled by the `RustyPipeQuery::report`
  query option)
- WRN (parts of the response could not be deserialized/parsed, response data may be
  incomplete)
- ERR (entire response could not be deserialized/parsed, RustyPipe returned an error)

## PO tokens

Since August 2024 YouTube requires PO tokens to access streams from web-based clients
(Desktop, Mobile). Otherwise streams will return a 403 error.

Generating PO tokens requires a simulated browser environment, which would be too large
to include in RustyPipe directly.

Therefore, the PO token generation is handled by a seperate CLI application
([rustypipe-botguard](https://codeberg.org/ThetaDev/rustypipe-botguard)) which is called
by the RustyPipe crate. RustyPipe automatically detects the rustypipe-botguard binary if
it is located in PATH or the current working directory. If your rustypipe-botguard
binary is located at a different path, you can specify it with the `.botguard_bin(path)`
option.

## Authentication

RustyPipe supports authenticating with your YouTube account to access
age-restricted/private videos and user information. There are 2 supported authentication
methods: OAuth and cookies.

To execute a query with authentication, use the `.authenticated()` query option. This
option is enabled by default for queries that always require authentication like
fetching user data. RustyPipe may automatically use authentication in case a video is
age-restricted or your IP address is banned by YouTube. If you never want to use
authentication, set the `.unauthenticated()` query option.

### OAuth

OAuth is the authentication method used by the YouTube TV client. It is more
user-friendly than extracting cookies, however it only works with the TV client. This
means that you can only fetch videos and not access any user data.

To login using OAuth, you first have to get a new device code using the
`rp.user_auth_get_code()` function. You can then enter the code on
<https://google.com/device> and log in with your Google account. After generating the
code, you can call the `rp.user_auth_wait_for_login()` function which waits until the
user has logged in and stores the authentication token in the cache.

### Cookies

Authenticating with cookies allows you to use the functionality of the YouTube/YouTube
Music Desktop client. You can fetch your subscribed channels, playlists and your music
collection. You can also fetch videos using the Desktop client, including private
videos, as long as you have access to them.

To authenticate with cookies you have to log into YouTube in a fresh browser session
(open Incognito/Private mode). Then extract the cookies from the developer tools or by
using browser plugins like "Get cookies.txt LOCALLY"
([Firefox](https://addons.mozilla.org/de/firefox/addon/get-cookies-txt-locally/))
([Chromium](https://chromewebstore.google.com/detail/get-cookiestxt-locally/cclelndahbckbenkjhflpdbgdldlbecc)).
Close the browser window after extracting the cookies to prevent YouTube from rotating
the cookies.

You can then add the cookies to your RustyPipe client using the `user_auth_set_cookie`
or `user_auth_set_cookie_txt` function. The cookies are stored in the cache file. To log
out, use the function `user_auth_remove_cookie`.
