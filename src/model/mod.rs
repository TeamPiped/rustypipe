pub mod locale;
mod ordering;
pub mod stream_filter;

pub use locale::{Country, Language};

use std::ops::Range;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

pub trait FileFormat {
    fn extension(&self) -> &str;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoPlayer {
    pub info: VideoInfo,
    pub video_streams: Vec<VideoStream>,
    pub video_only_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitles: Vec<Subtitle>,
    pub expires_in_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub videos: Vec<Video>,
    pub n_videos: u32,
    pub ctoken: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub description: Option<String>,
    pub channel: Option<Channel>,
    pub last_update: Option<DateTime<Local>>,
    pub last_update_txt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub length: u32,
    pub thumbnails: Vec<Thumbnail>,
    pub channel: Channel,
    pub publish_date: Option<DateTime<Local>>,
    pub view_count: u64,
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub is_live_content: bool,
    pub is_family_safe: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoStream {
    pub url: String,
    pub itag: u32,
    pub bitrate: u32,
    pub average_bitrate: u32,
    pub size: Option<u64>,
    pub index_range: Option<Range<u32>>,
    pub init_range: Option<Range<u32>>,
    pub width: u32,
    pub height: u32,
    pub fps: u8,
    pub quality: String,
    pub hdr: bool,
    pub mime: String,
    pub format: VideoFormat,
    pub codec: VideoCodec,
    pub throttled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioStream {
    pub url: String,
    pub itag: u32,
    pub bitrate: u32,
    pub average_bitrate: u32,
    pub size: u64,
    pub index_range: Option<Range<u32>>,
    pub init_range: Option<Range<u32>>,
    pub mime: String,
    pub format: AudioFormat,
    pub codec: AudioCodec,
    pub throttled: bool,
    pub track: Option<AudioTrack>,
}

#[derive(
    Default, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoCodec {
    #[default]
    Unknown,
    /// MPEG-4 Part 14 <https://en.wikipedia.org/wiki/MPEG-4_Part_14>
    Mp4v,
    /// avc1 aka H.264: <https://en.wikipedia.org/wiki/Advanced_Video_Coding>
    Avc1,
    /// VP9: <https://en.wikipedia.org/wiki/VP9>
    Vp9,
    /// AV1, the latest codec: <https://en.wikipedia.org/wiki/AV1>
    Av01,
}

#[derive(
    Default, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioCodec {
    #[default]
    Unknown,
    /// MP4A aka AAC: <https://en.wikipedia.org/wiki/Advanced_Audio_Coding>
    Mp4a,
    /// Opus: <https://en.wikipedia.org/wiki/Opus_(audio_format)>
    Opus,
}

/// The video file format
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoFormat {
    #[serde(rename = "3gp")]
    ThreeGp,
    Mp4,
    Webm,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioTrack {
    pub id: String,
    pub lang: Option<String>,
    pub lang_name: String,
    pub is_default: bool,
}

impl FileFormat for VideoFormat {
    fn extension(&self) -> &str {
        match self {
            VideoFormat::ThreeGp => ".3gp",
            VideoFormat::Mp4 => ".mp4",
            VideoFormat::Webm => ".webm",
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioFormat {
    M4a,
    Webm,
}

impl FileFormat for AudioFormat {
    fn extension(&self) -> &str {
        match self {
            AudioFormat::M4a => ".m4a",
            AudioFormat::Webm => ".webm",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subtitle {
    pub url: String,
    pub lang: String,
    pub lang_name: String,
    pub auto_generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub length: u32,
    pub thumbnails: Vec<Thumbnail>,
    pub channel: Channel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub name: String,
}
