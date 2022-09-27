//! YouTube API request and response models

pub mod locale;
mod ordering;
mod paginator;
mod param;
pub mod richtext;
pub mod stream_filter;

pub use locale::{Country, Language};
pub use paginator::Paginator;
pub use param::ChannelOrder;

use std::ops::Range;

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use self::richtext::RichText;

/*
#COMMON
*/

/// Video thumbnail or other image
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/*
#PLAYER
*/

pub trait FileFormat {
    /// Get the file extension (".xyz") of the file format
    fn extension(&self) -> &str;
}

/// Video player data
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoPlayer {
    /// Video metadata
    pub details: VideoPlayerDetails,
    /// List of streams containing both audio and video
    pub video_streams: Vec<VideoStream>,
    /// List of streams containing video only
    pub video_only_streams: Vec<VideoStream>,
    /// List of streams containing audio only
    pub audio_streams: Vec<AudioStream>,
    /// List of subtitles
    pub subtitles: Vec<Subtitle>,
    /// Lifetime of the stream URLs in seconds
    pub expires_in_seconds: u32,
}

/// Video metadata from the player
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoPlayerDetails {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video description in plaintext format
    pub description: Option<String>,
    /// Video length in seconds
    pub length: u32,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the video
    pub channel: ChannelId,
    /// Video publishing date. Start date in case of a livestream.
    pub publish_date: Option<DateTime<Local>>,
    /// Number of views / current viewers in case of a livestream.
    pub view_count: u64,
    /// List of words that describe the topic of the video
    pub keywords: Vec<String>,
    pub category: Option<String>,
    /// True if the video is/was livestreamed
    pub is_live_content: bool,
    /// True if the video is not age-restricted
    pub is_family_safe: Option<bool>,
}

/// Video stream
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoStream {
    /// Video stream URL
    pub url: String,
    /// YouTube stream format identifier
    pub itag: u32,
    pub bitrate: u32,
    pub average_bitrate: u32,
    /// Video file size in bytes
    pub size: Option<u64>,
    pub index_range: Option<Range<u32>>,
    pub init_range: Option<Range<u32>>,
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Video frames per second
    pub fps: u8,
    /// Quality text (e.g. "1080p60")
    pub quality: String,
    /// True if the video is HDR
    pub hdr: bool,
    /// MIME file type
    pub mime: String,
    /// Video file format
    pub format: VideoFormat,
    /// Video codec
    pub codec: VideoCodec,
    /// True if the deobfuscation of the nsig url parameter failed
    /// and the stream will be throttled
    pub throttled: bool,
}

/// Audio stream
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct AudioStream {
    /// Audio stream URL
    pub url: String,
    /// YouTube stream format identifier
    pub itag: u32,
    pub bitrate: u32,
    pub average_bitrate: u32,
    /// Audio file size in bytes
    pub size: u64,
    pub index_range: Option<Range<u32>>,
    pub init_range: Option<Range<u32>>,
    /// MIME file type
    pub mime: String,
    /// Audio file format
    pub format: AudioFormat,
    /// Audio codec
    pub codec: AudioCodec,
    /// True if the deobfuscation of the nsig url parameter failed
    /// and the stream will be throttled
    pub throttled: bool,
    /// Audio track information
    ///
    /// Videos can have multiple audio tracks (different languages).
    /// In this case, this object shows to which track the stream belongs to.
    ///
    /// This is None if the video contains only 1 audio track.
    pub track: Option<AudioTrack>,
}

/// Video codec
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

/// Audio codec
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

/// Video file type
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoFormat {
    /// `*.3gp`
    #[serde(rename = "3gp")]
    ThreeGp,
    /// `*.mp4`
    Mp4,
    /// `*.webm`
    Webm,
}

/// Audio track information
///
/// Videos can have multiple audio tracks (different languages).
/// In this case, this object shows to which track the stream belongs to.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct AudioTrack {
    /// Track ID (e.g. `en.0`)
    pub id: String,
    /// 2/3 letter language code (e.g. `en`)
    ///
    /// Extracted from the track ID
    pub lang: Option<String>,
    /// Language name (e.g. "English")
    pub lang_name: String,
    /// True if this is the default audio track
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

/// Audio file type
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioFormat {
    /// `*.m4a`
    M4a,
    /// `*.webm`
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

/// YouTube provides subtitles in different formats.
///
/// srv1 (XML) is the default format, to request a different format you have
/// to append `&fmt=<Format>` to the URL.
///
/// # Subtitle formats
///
/// ### `srv1` (default)
///
/// ```xml
/// <?xml version="1.0" encoding="utf-8"?>
/// <transcript>
///     <text start="0.12" dur="1.59">- [Mr Beast] I built two massive circles</text>
///     <text start="1.71" dur="3.39">and put 100 boys in one
/// and 100 girls in the other</text>
/// </transcript>
/// ```
///
/// ### `srv2`
///
/// ```xml
/// <?xml version="1.0" encoding="utf-8"?>
/// <timedtext>
///   <text t="120" d="1590">- [Mr Beast] I built two massive circles</text>
///   <text t="1710" d="3390">and put 100 boys in one
/// and 100 girls in the other</text>
/// </timedtext>
/// ```
///
/// ### `srv3`
///
/// ```xml
/// <?xml version="1.0" encoding="utf-8"?>
/// <timedtext format="3">
///   <body>
///     <p t="120" d="1590">- [Mr Beast] I built two massive circles</p>
///     <p t="1710" d="3390">and put 100 boys in one
/// and 100 girls in the other</p>
///   </body>
/// </timedtext>
/// ```
///
/// ### `json3`
///
/// ```json
/// {
///   "wireMagic": "pb3",
///   "pens": [{}],
///   "wsWinStyles": [{}],
///   "wpWinPositions": [{}],
///   "events": [
///     {
///       "tStartMs": 120,
///       "dDurationMs": 1590,
///       "segs": [
///         {
///           "utf8": "- [Mr Beast] I built two massive circles"
///         }
///       ]
///     },
///     {
///       "tStartMs": 1710,
///       "dDurationMs": 3390,
///       "segs": [
///         {
///           "utf8": "and put 100 boys in one\nand 100 girls in the other"
///         }
///       ]
///     }
///   ]
/// }
/// ```
///
/// ### Timed Text Markup Language (`ttml`)
///
/// ```xml
/// <?xml version="1.0" encoding="utf-8" ?>
/// <tt xml:lang="en-US" xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xmlns:tts="http://www.w3.org/ns/ttml#styling" xmlns:ttp="http://www.w3.org/ns/ttml#parameter" ttp:profile="http://www.w3.org/TR/profile/sdp-us" >
/// <head>
/// <styling>
/// <style xml:id="s1" tts:textAlign="center" tts:extent="90% 90%" tts:origin="5% 5%" tts:displayAlign="after"/>
/// <style xml:id="s2" tts:fontSize=".72c" tts:backgroundColor="black" tts:color="white"/>
/// </styling>
/// <layout>
/// <region xml:id="r1" style="s1"/>
/// </layout>
/// </head>
/// <body region="r1">
/// <div>
/// <p begin="00:00:00.120" end="00:00:01.710" style="s2">- [Mr Beast] I built two massive circles</p>
/// <p begin="00:00:01.710" end="00:00:05.100" style="s2">and put 100 boys in one<br />and 100 girls in the other</p>
/// </div>
/// </body>
/// </tt>
/// ```
///
/// ### WebVTT (`vtt`)
///
/// ```txt
/// WEBVTT
/// Kind: captions
/// Language: en-US
///
/// 00:00:00.120 --> 00:00:01.710
/// - [Mr Beast] I built two massive circles
///
/// 00:00:01.710 --> 00:00:05.100
/// and put 100 boys in one
/// and 100 girls in the other
/// ```
///
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Subtitle {
    /// URL of the subtitle file
    pub url: String,
    /// Subtitle language code (e.g. "en")
    pub lang: String,
    /// Subtitle language name (e.g. "English")
    pub lang_name: String,
    /// True if the subtitle was automatically generated
    /// by YouTube's speech recognition
    pub auto_generated: bool,
}

/*
#PLAYLIST
*/

/// YouTube playlist
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Playlist {
    /// Unique YouTube playlist ID
    pub id: String,
    /// Playlist name
    pub name: String,
    /// Playlist videos
    pub videos: Paginator<PlaylistVideo>,
    /// Number of videos in the playlist
    pub video_count: u32,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Playlist description in plaintext format
    pub description: Option<String>,
    /// Channel of the playlist
    pub channel: Option<ChannelId>,
    /// Last update date
    pub last_update: Option<DateTime<Local>>,
    /// Textual last update date
    pub last_update_txt: Option<String>,
}

/// YouTube video extracted from a playlist
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlaylistVideo {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video length in seconds
    pub length: u32,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the video
    pub channel: ChannelId,
}

/// Channel identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelId {
    /// Unique YouTube channel ID
    pub id: String,
    /// Channel name
    pub name: String,
}

/*
#VIDEO DETAILS
*/

/// VideoDetails contains additional information that YouTube shows next
/// to the video player.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoDetails {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video description in rich text format
    pub description: RichText,
    /// Channel of the video
    pub channel: ChannelTag,
    /// Number of views / current viewers in case of a livestream.
    pub view_count: u64,
    /// Number of likes
    ///
    /// `None` if the like count was hidden by the creator.
    pub like_count: Option<u32>,
    /// Video publishing date. Start date in case of a livestream.
    ///
    /// `None` if the date could not be parsed.
    pub publish_date: Option<DateTime<Local>>,
    /// Textual video publishing date (e.g. `Aug 2, 2013`, depends on language)
    pub publish_date_txt: String,
    /// Is the video a livestream?
    pub is_live: bool,
    /// Is the video published under the Creative Commons BY 3.0 license?
    ///
    /// Information about the license:
    ///
    /// https://www.youtube.com/t/creative_commons
    ///
    /// https://creativecommons.org/licenses/by/3.0/
    pub is_ccommons: bool,
    /// Chapters of the video
    pub chapters: Vec<Chapter>,
    /// Recommended videos
    ///
    /// Note: Recommendations are not available for age-restricted videos
    pub recommended: Paginator<RecommendedVideo>,
    /// Paginator to fetch comments (most liked first)
    ///
    /// Is initially empty.
    pub top_comments: Paginator<Comment>,
    /// Paginator to fetch comments (latest first)
    ///
    /// Is initially empty.
    pub latest_comments: Paginator<Comment>,
}

/// Chapter of a video
///
/// Videos can consist of different chapters, which YouTube shows
/// on the seek bar and below the description text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Chapter {
    /// Chapter title
    pub title: String,
    /// Chapter position in seconds
    pub position: u32,
    /// Chapter thumbnail
    pub thumbnail: Vec<Thumbnail>,
}

/*
@RECOMMENDATIONS
*/

/// YouTube video fetched from the recommendations next to a video
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecommendedVideo {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video length in seconds.
    ///
    /// Is `None` for livestreams.
    pub length: Option<u32>,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the video
    pub channel: ChannelTag,
    /// Video publishing date.
    ///
    /// `None` if the date could not be parsed.
    pub publish_date: Option<DateTime<Local>>,
    /// Textual video publish date (e.g. `11 months ago`, depends on language)
    ///
    /// Is `None` for livestreams.
    pub publish_date_txt: Option<String>,
    /// View count
    ///
    /// `None` if it could not be extracted.
    pub view_count: Option<u64>,
    /// Is the video an active livestream?
    pub is_live: bool,
}

/// Channel information attached to a video or comment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelTag {
    /// Unique YouTube channel ID
    pub id: String,
    /// Channel name
    pub name: String,
    /// Channel avatar/profile picture
    pub avatar: Vec<Thumbnail>,
    /// Channel verification mark
    pub verification: Verification,
    /// Approximate number of subscribers
    ///
    /// `None` if hidden by the owner or not present.
    ///
    /// Info: This is only present in the `VideoDetails` response
    pub subscriber_count: Option<u64>,
}

/*
@COMMENTS
*/

/// Verification status of a channel
#[derive(Default, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Verification {
    #[default]
    /// Unverified channel (default)
    None,
    /// Verified channel (✓ checkmark symbol)
    Verified,
    /// Verified music artist (♪ music note symbol)
    Artist,
}

impl Verification {
    /// Returns true if the verification status is not None
    /// (either Verified or Artist).
    pub fn verified(&self) -> bool {
        self != &Self::None
    }
}

/// Comment under a YouTube video
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Comment {
    /// Unique YouTube Comment-ID (e.g. `UgynScMrsqGSL8qvePl4AaABAg`)
    pub id: String,
    /// Comment text
    pub text: RichText,
    /// Comment author
    ///
    /// There may be comments with missing authors (possibly deleted users?).
    pub author: Option<ChannelTag>,
    /// Comment publishing date.
    ///
    /// `None` if the date could not be parsed.
    pub publish_date: Option<DateTime<Local>>,
    /// Textual comment publish date (e.g. `14 hours ago`), depends on language setting
    pub publish_date_txt: String,
    /// Number of comment likes
    pub like_count: Option<u32>,
    /// Number of replies
    pub reply_count: u32,
    /// Paginator to fetch comment replies
    pub replies: Paginator<Comment>,
    /// Is the comment from the channel owner?
    pub by_owner: bool,
    /// Has the channel owner pinned the comment to the top?
    pub pinned: bool,
    /// Has the channel owner marked the comment with a ❤️ heart ?
    pub hearted: bool,
}

/*
#CHANNEL
*/

/// YouTube channel object.
///
/// Contains channel metadata as well as additional content
/// depending on which channel tab is fetched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Channel<T> {
    /// Unique YouTube Channel-ID (e.g. `UC-lHJZR3Gqxm24_Vd_AJ5Yw`)
    pub id: String,
    /// Channel name
    pub name: String,
    /// Channel subscriber count
    ///
    /// `None` if the subscriber count was hidden by the owner
    /// or could not be parsed.
    pub subscriber_count: Option<u64>,
    /// Channel avatar / profile picture
    pub avatar: Vec<Thumbnail>,
    /// Channel description text
    pub description: String,
    /// List of words to describe the topic of the channel
    pub tags: Vec<String>,
    /// Custom URL set by the channel owner
    /// (e.g. <https://www.youtube.com/c/EevblogDave>)
    pub vanity_url: Option<String>,
    /// Banner image shown above the channel
    pub banner: Vec<Thumbnail>,
    /// Banner image shown above the channel (small format for mobile)
    pub mobile_banner: Vec<Thumbnail>,
    /// Banner image shown above the channel (16:9 fullscreen format for TV)
    pub tv_banner: Vec<Thumbnail>,
    /// Content fetched from the channel
    pub content: T,
}

/// Video fetched from a YouTube channel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelVideo {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video length in seconds.
    ///
    /// Is `None` for livestreams.
    pub length: Option<u32>,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Video publishing date.
    ///
    /// `None` if the date could not be parsed.
    pub publish_date: Option<DateTime<Local>>,
    /// Textual video publish date (e.g. `11 months ago`, depends on language)
    ///
    /// Is `None` for livestreams.
    pub publish_date_txt: Option<String>,
    /// View count
    ///
    /// `None` if it could not be extracted.
    pub view_count: Option<u64>,
    /// Is the video an active livestream?
    pub is_live: bool,
    /// Is the video a YouTube Short video (vertical and <60s)?
    pub is_short: bool,
}

/// Playlist fetched from a YouTube channel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelPlaylist {
    /// Unique YouTube Playlist-ID (e.g. `PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ`)
    pub id: String,
    /// Playlist name
    pub name: String,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Number of playlist videos
    pub video_count: Option<u32>,
}

/// Additional channel metadata fetched from the "About" tab.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelInfo {
    /// Channel creation date
    pub create_date: Option<DateTime<Local>>,
    /// Channel view count
    pub view_count: Option<u64>,
    /// Links to other websites or social media profiles
    pub links: Vec<(String, String)>,
}

/// YouTube channel RSS feed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelRss {
    /// Unique YouTube Channel-ID (e.g. `UC-lHJZR3Gqxm24_Vd_AJ5Yw`)
    pub id: String,
    /// Channel name
    pub name: String,
    /// List of the latest channel videos
    pub videos: Vec<ChannelRssVideo>,
    /// Channel creation date (second-accurate).
    pub create_date: DateTime<Utc>,
}

/// YouTube video fetched from a channel's RSS feed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelRssVideo {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video description in plaintext format
    pub description: String,
    /// Video thumbnail
    pub thumbnail: Thumbnail,
    /// Video publishing date (second-accurate).
    pub publish_date: DateTime<Utc>,
    /// Date and time when the RSS feed entry was last updated.
    pub update_date: DateTime<Utc>,
    /// View count
    pub view_count: u64,
    /// Number of likes
    ///
    /// Zero if the like count was hidden by the creator.
    pub like_count: u32,
}
