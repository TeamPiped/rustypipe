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

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use self::richtext::RichText;

/*
#PLAYER
*/

pub trait FileFormat {
    fn extension(&self) -> &str;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoPlayer {
    pub details: VideoPlayerDetails,
    pub video_streams: Vec<VideoStream>,
    pub video_only_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitles: Vec<Subtitle>,
    pub expires_in_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoPlayerDetails {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub length: u32,
    pub thumbnails: Vec<Thumbnail>,
    pub channel: ChannelId,
    pub publish_date: Option<DateTime<Local>>,
    pub view_count: u64,
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub is_live_content: bool,
    pub is_family_safe: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Subtitle {
    pub url: String,
    pub lang: String,
    pub lang_name: String,
    pub auto_generated: bool,
}

/*
#PLAYLIST
*/

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub videos: Paginator<PlaylistVideo>,
    pub video_count: u32,
    pub thumbnail: Vec<Thumbnail>,
    pub description: Option<String>,
    pub channel: Option<ChannelId>,
    pub last_update: Option<DateTime<Local>>,
    pub last_update_txt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlaylistVideo {
    pub id: String,
    pub title: String,
    pub length: u32,
    pub thumbnail: Vec<Thumbnail>,
    pub channel: ChannelId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelId {
    pub id: String,
    pub name: String,
}

/*
#VIDEO DETAILS
*/

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoDetails {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video description
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
    pub top_comments: Paginator<Comment>,
    /// Paginator to fetch comments (latest first)
    pub latest_comments: Paginator<Comment>,
}

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
    pub fn verified(&self) -> bool {
        self != &Self::None
    }
}

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
