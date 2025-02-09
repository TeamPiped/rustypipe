# ![RustyPipe](https://codeberg.org/ThetaDev/rustypipe/raw/branch/main/notes/logo.svg) CLI

[![Current crates.io version](https://img.shields.io/crates/v/rustypipe-cli.svg)](https://crates.io/crates/rustypipe-cli)
[![License](https://img.shields.io/badge/License-GPL--3-blue.svg?style=flat)](https://opensource.org/licenses/GPL-3.0)
[![CI status](https://codeberg.org/ThetaDev/rustypipe/actions/workflows/ci.yaml/badge.svg?style=flat&label=CI)](https://codeberg.org/ThetaDev/rustypipe/actions/?workflow=ci.yaml)

The RustyPipe CLI is a powerful YouTube client for the command line. It allows you to
access most of the features of the RustyPipe crate: getting data from YouTube and
downloading videos.

## Installation

You can download a compiled version of RustyPipe here:
<https://codeberg.org/ThetaDev/rustypipe/releases>

Alternatively, you can compile it yourself by installing [Rust](https://rustup.rs/) and
running `cargo install rustypipe-cli`.

To be able to access streams from web-based clients (Desktop, Mobile) you need to
download [rustypipe-botguard](https://codeberg.org/ThetaDev/rustypipe-botguard/releases)
and place the binary either in the PATH or the current working directory.

For downloading videos you also need to have ffmpeg installed.

## `get`: Fetch information

You can call the get command with any YouTube entity ID or URL and RustyPipe will fetch
the associated metadata. It can fetch channels, playlists, albums and videos.

**Usage:** `rustypipe get UC2TXq_t06Hjdr2g_KdKpHQg`

- `-l`, `--limit` Limit the number of list items to fetch
- `-t`, `--tab` Channel tab (options: **videos**, shorts, live, playlists, info)
- `-m, --music` Use the YouTube Music API
- `--rss`Fetch the RSS feed of a channel
- `--comments` Get comments (options: top, latest)
- `--lyrics` Get the lyrics for YTM tracks
- `--player` Get the player data instead of the video details when fetching videos
- `-c`, `--client-type` YT clients used to fetch player data (options: desktop, tv,
  tv-embed, android, ios; if multiple clients are specified, they are attempted in
  order)

## `search`: Search YouTube

With the search command you can search the entire YouTube platform or individual
channels. YouTube Music search is also supported.

Note that search filters are only supported when searching YouTube. They have no effect
when searching YTM or individual channels.

**Usage:** `rustypipe search "query"`

### Options

- `-l`, `--limit` Limit the number of list items to fetch

- `--item-type` Filter results by item type
- `--length` Filter results by video length
- `--date` Filter results by upload date (options: hour, day, week, month, year)
- `--order` Sort search results (options: rating, date, views)
- `--channel` Channel ID for searching channel videos
- `-m`, `--music` Search YouTube Music in the given category (options: all, tracks,
  videos, artists, albums, playlists-ytm, playlists-community)

## `dl`: Download videos

The downloader can download individual videos, playlists, albums and channels. Multiple
videos can be downloaded in parallel for improved performance.

**Usage:** `rustypipe dl eRsGyueVLvQ`

### Options

- `-o`, `--output` Download to the given directory
- `--output-file` Download to the given file
- `--template` Download to a path determined by a template

- `-r`, `--resolution` Video resolution (e.g. 720, 1080). Set to 0 for audio-only
- `-a`, `--audio` Download only the audio track and write track metadata + album cover
- `-p`, `--parallel` Number of videos downloaded in parallel (default: 8)
- `-m`, `--music` Use YouTube Music for downloading playlists
- `-l`, `--limit` Limit the number of videos to download (default: 1000)
- `-c`, `--client-type` YT clients used to fetch player data (options: desktop, tv,
  tv-embed, android, ios; if multiple clients are specified, they are attempted in
  order)

## `vdata`: Get visitor data

You can use the vdata command to get a new visitor data ID. This feature may come in
handy for testing and reproducing A/B tests.

## `releases` Get YouTube Music new releases

Get a list of new albums or music videos on YouTube Music

**Usage:** `rustypipe releases` or `rustypipe releases --videos`

## `charts`: Get YouTube Music charts

Get a list of the most popular tracks and artists for a given country

**Usage:** `rustypipe charts DE`

## `history`: Get YouTube playback history

Get a list of recently played videos or tracks

### Options

- `-l`, `--limit` Limit the number of list items to fetch
- `--search` Search the playback history (unavailable on YouTube Music)
- `-m`, `--music` Get the YouTube Music playback history

## `subscriptions`: Get subscribed channels

You can use the RustyPipe CLI to get a list of the channels you subscribed to. With the
`--format` flag you can export then in different formats, including OPML and NewPipe
JSON.

With the `--feed` option you can output a list of the latest videos from your
subscription feed instead.

### Options

- `-l`, `--limit` Limit the number of list items to fetch
- `-m`, `--music` Get a list of subscribed YouTube Music artists
- `--feed` Output YouTube Music subscription feed

## `playlists`, `albums`, `tracks`: Get your YouTube library

Fetch a list of all the items saved in your YouTube/YouTube Music profile.

### Options

- `-l`, `--limit` Limit the number of list items to fetch
- `-m`, `--music` (only for playlists): Get your YouTube Music playlists

## Global options

- **Proxy:** RustyPipe respects the environment variables `HTTP_PROXY`, `HTTPS_PROXY`
  and `ALL_PROXY`
- **Logging:** You can change the log level with the `RUST_LOG` environment variable, it
  is set to `info` by default
- **Visitor data:** A custom visitor data ID can be used with the `--vdata` flag
- **Authentication:** Use the commands `rustypipe login` and `rustypipe login --cookie`
  to log into your Google account using either OAuth or YouTube cookies. With the
  `--auth` flag you can use authentication for any request.
- `--lang` Change the YouTube content language
- `--country` Change the YouTube content country
- `--tz` Use a specific
  [timezone](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones) (e.g.
  Europe/Berlin, Australia/Sydney)

  **Note:** this requires building rustypipe-cli with the `timezone` feature

- `--local-tz` Use the local timezone instead of UTC
- `--report` Generate a report on every request and store it in a `rustypipe_reports`
  folder in the current directory
- `--cache-file` Change the RustyPipe cache file location (Default:
  `~/.local/share/rustypipe/rustypipe_cache.json`)
- `--report-dir` Change the RustyPipe report directory location (Default:
  `~/.local/share/rustypipe/rustypipe_reports`)
- `--botguard-bin` Use a
  [rustypipe-botguard](https://codeberg.org/ThetaDev/rustypipe-botguard) binary from the
  given path for generating PO tokens
- `--no-botguard` Disable Botguard, only download videos using clients that dont require
  it
- `--pot-cache` Enable caching for session-bound PO tokens

### Output format

By default, the CLI outputs YouTube data in a human-readable text format. If you want to
store the data or process it with a script, you should choose a machine readable output
format. You can choose both JSON and YAML with the `-f, --format` flag.
