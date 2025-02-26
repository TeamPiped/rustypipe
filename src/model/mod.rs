//! YouTube API response models

mod convert;
mod frameset;
mod ordering;

pub mod paginator;
pub mod richtext;
pub mod traits;
pub use frameset::{Frameset, FramesetUrls};

use std::{collections::BTreeSet, ops::Range};

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use self::{paginator::Paginator, richtext::RichText};
use crate::{client::ClientType, error::Error, param::Country, validate};

/*
#COMMON
*/

/// Video thumbnail or other image
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Thumbnail {
    /// Thumbnail URL
    pub url: String,
    /// Thumbnail image width
    pub width: u32,
    /// Thumbnail image height
    pub height: u32,
}

/// Entities extracted from a YouTube URL
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UrlTarget {
    /// YouTube video
    ///
    /// Example: <https://youtube.com/watch?v=ZeerrnuLi5E>
    Video {
        /// Unique YouTube video ID
        id: String,
        /// Video start time in seconds
        start_time: u32,
    },
    /// YouTube channel
    ///
    /// Example: <https://www.youtube.com/channel/UC2DjFE7Xf11URZqWBigcVOQ>
    Channel {
        /// Unique YouTube channel ID
        id: String,
    },
    /// YouTube playlist
    ///
    /// Example: <https://www.youtube.com/playlist?list=PLKUA473MWUv2jmkqIxzQR3YL4kuPArj4G>
    Playlist {
        /// Unique YouTube playlist ID
        id: String,
    },
    /// YouTube Music album
    ///
    /// Example: <https://music.youtube.com/browse/MPREb_nlBWQROfvjo>
    Album {
        /// Unique YouTube album ID
        id: String,
    },
}

impl std::fmt::Display for UrlTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_url())
    }
}

impl UrlTarget {
    /// Convert the URL target to a YouTube URL
    ///
    /// Is equivalent to `url_target.to_string()`
    pub fn to_url(&self) -> String {
        self.to_url_yt_host("https://www.youtube.com")
    }

    /// Convert the URL target to a YouTube URL with a specified YouTube host.
    ///
    /// Used to redirect to alternative YouTube frontends like Piped or Invidious.
    ///
    /// **Note:** Music album URL targets are still converted to `music.youtube.com/browse/*`,
    /// since these URLs are not supported by Piped or Invidious.
    pub fn to_url_yt_host(&self, yt_host: &str) -> String {
        match self {
            UrlTarget::Video { id, start_time, .. } => match start_time {
                0 => format!("{yt_host}/watch?v={id}"),
                n => format!("{yt_host}/watch?v={id}&t={n}s"),
            },
            UrlTarget::Channel { id } => {
                format!("{yt_host}/channel/{id}")
            }
            UrlTarget::Playlist { id } => {
                format!("{yt_host}/playlist?list={id}")
            }
            UrlTarget::Album { id } => {
                format!("https://music.youtube.com/browse/{id}")
            }
        }
    }

    /// Validate the YouTube ID from the URL target
    pub(crate) fn validate(&self) -> Result<(), Error> {
        match self {
            UrlTarget::Video { id, .. } => validate::video_id(id),
            UrlTarget::Channel { id } => validate::channel_id(id),
            UrlTarget::Playlist { id } => validate::playlist_id(id),
            UrlTarget::Album { id } => validate::album_id(id),
        }
    }
}

/*
#PLAYER
*/

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
    ///
    /// **Note:** use the `valid_until` value to check if the stream URLs are still valid,
    /// since it takes PO token lifetime into account.
    pub expires_in_seconds: u32,
    /// Date until which the stream URLs are valid
    #[serde(with = "time::serde::rfc3339")]
    pub valid_until: OffsetDateTime,
    /// HLS manifest URL (for livestreams)
    pub hls_manifest_url: Option<String>,
    /// Dash manifest URL (for livestreams)
    pub dash_manifest_url: Option<String>,
    /// Video frames for seek preview
    pub preview_frames: Vec<Frameset>,
    /// Video player DRM config
    ///
    /// [`None`] if the video is not DRM-protected
    pub drm: Option<VideoPlayerDrm>,
    /// Client type with which the player was fetched
    pub client_type: ClientType,
    /// YouTube visitor data ID
    pub visitor_data: Option<String>,
}

/// Video metadata from the player
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoPlayerDetails {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub name: Option<String>,
    /// Video description in plaintext format
    pub description: Option<String>,
    /// Video duration in seconds
    ///
    /// Is zero for livestreams
    pub duration: u32,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel ID of the video
    pub channel_id: String,
    /// Channel name of the video
    pub channel_name: Option<String>,
    /// Number of views / current viewers in case of a livestream.
    pub view_count: Option<u64>,
    /// List of words that describe the topic of the video
    pub keywords: Vec<String>,
    /// True if the video is an active livestream
    pub is_live: bool,
    /// True if the video is/was livestreamed
    pub is_live_content: bool,
}

/// Video stream
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoStream {
    /// Video stream URL
    pub url: String,
    /// YouTube stream format identifier
    pub itag: u32,
    /// Stream bitrate (in bits/second)
    pub bitrate: u32,
    /// Average stream bitrate (in bits/second)
    pub average_bitrate: u32,
    /// Video file size in bytes
    pub size: Option<u64>,
    /// Index range (used for DASH streaming)
    pub index_range: Option<Range<u32>>,
    /// Init range (used for DASH streaming)
    pub init_range: Option<Range<u32>>,
    /// Video duration in milliseconds
    pub duration_ms: Option<u32>,
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
    /// DRM track type
    ///
    /// [`None`] if the track is not DRM-protected
    pub drm_track_type: Option<DrmTrackType>,
    /// List of DRM systems that can decrypt this track
    pub drm_systems: Vec<DrmSystem>,
}

/// Audio stream
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct AudioStream {
    /// Audio stream URL
    pub url: String,
    /// YouTube stream format identifier
    pub itag: u32,
    /// Stream bitrate (in bits/second)
    pub bitrate: u32,
    /// Average stream bitrate (in bits/second)
    pub average_bitrate: u32,
    /// Audio file size in bytes
    pub size: u64,
    /// Index range (used for DASH streaming)
    pub index_range: Option<Range<u32>>,
    /// Init range (used for DASH streaming)
    pub init_range: Option<Range<u32>>,
    /// Audio duration in milliseconds
    pub duration_ms: Option<u32>,
    /// MIME file type
    pub mime: String,
    /// Audio file format
    pub format: AudioFormat,
    /// Audio codec
    pub codec: AudioCodec,
    /// Number of audio channels
    pub channels: Option<u8>,
    /// Audio loudness for volume normalization
    ///
    /// The track volume correction factor (0-1) can be calculated using this formula
    ///
    /// `10^(-loudness_db/20)`
    ///
    /// Note that the `loudness_db` value is the inverse of the usual ReplayGain track gain
    /// parameter, i.e. a value of 6 means the volume should be reduced by 6dB and the
    /// track gain parameter would be -6.
    ///
    /// More information about ReplayGain and how to apply this infomation to audio files
    /// can be found here: <https://wiki.hydrogenaud.io/index.php?title=ReplayGain_1.0_specification>.
    ///
    /// The loudness parameter is not available when using the Android client.
    pub loudness_db: Option<f32>,
    /// Audio track information
    ///
    /// Videos can have multiple audio tracks (different languages).
    /// In this case, this object shows to which track the stream belongs to.
    ///
    /// This is None if the video contains only 1 audio track.
    pub track: Option<AudioTrack>,
    /// DRM track type
    ///
    /// [`None`] if the track is not DRM-protected
    pub drm_track_type: Option<DrmTrackType>,
    /// List of DRM systems that can decrypt this track
    pub drm_systems: Vec<DrmSystem>,
}

/// Video player DRM parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct VideoPlayerDrm {
    /// Widevine service certificate
    pub widevine_service_cert: Option<Vec<u8>>,
    /// DRM parameters for the license API
    pub drm_params: String,
    /// DRM session id parameter for the license API
    pub drm_session_id: String,
    /// List of track types available for playback
    pub authorized_track_types: Vec<DrmTrackType>,
}

/// Video player DRM parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DrmLicense {
    /// DRM license
    pub license: Vec<u8>,
    /// List of authorized formats with track type and 16-byte key ID
    pub authorized_formats: Vec<(DrmTrackType, [u8; 16])>,
}

/// Video codec
#[derive(
    Default, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VideoCodec {
    /// Unknown codec
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
    /// Unknown codec
    #[default]
    Unknown,
    /// MP4A aka AAC: <https://en.wikipedia.org/wiki/Advanced_Audio_Coding>
    Mp4a,
    /// Opus: <https://en.wikipedia.org/wiki/Opus_(audio_format)>
    Opus,
    /// Dolby Digital / AC-3: <https://en.wikipedia.org/wiki/Dolby_Digital>
    #[serde(rename = "ac-3")]
    Ac3,
    /// Dolby Digital Plus / EC-3: <https://en.wikipedia.org/wiki/Dolby_Digital_Plus>
    #[serde(rename = "ec-3")]
    Ec3,
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

/// DRM track type
///
/// Depending on the purchased video and the device's DRM capabilites, only a subset of track
/// types may be available for playback.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DrmTrackType {
    /// Audio track
    Audio,
    /// Standard definition video (max. 480p)
    Sd,
    /// High definition video (max. 1080p)
    Hd,
    /// Ultra high definition video (2160p)
    Uhd1,
}

/// DRM system used to protect tracks
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DrmSystem {
    /// Google Widevine
    ///
    /// <https://en.wikipedia.org/wiki/Widevine>
    Widevine,
    /// Microsoft PlayReady
    ///
    /// <https://en.wikipedia.org/wiki/PlayReady>
    Playready,
    /// Apple FairPlay
    ///
    /// <https://en.wikipedia.org/wiki/FairPlay>
    Fairplay,
}

impl DrmSystem {
    pub(crate) fn req_param(self) -> &'static str {
        match self {
            DrmSystem::Widevine => "DRM_SYSTEM_WIDEVINE",
            DrmSystem::Playready => "DRM_SYSTEM_PLAYREADY",
            DrmSystem::Fairplay => "DRM_SYSTEM_FAIRPLAY",
        }
    }
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
    /// Language code (e.g. `en-US`, `de`)
    pub lang: Option<String>,
    /// Language name (e.g. "English")
    pub lang_name: String,
    /// True if this is the default audio track chosen by YouTube
    ///
    /// Note that YouTube's selection depends on the client type used to fetch the player.
    /// Some players
    pub is_default: bool,
    /// Audio track type (e.g. *Original*, *Dubbed*)
    pub track_type: Option<AudioTrackType>,
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

/// Audio track type
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum AudioTrackType {
    /// An original audio track of the video
    Original,
    /// An audio track with the original voices replaced, typically in a different language
    Dubbed,
    /// An audio track dubbed using YouTube's AI powered AutoDub feature
    DubbedAuto,
    /// A descriptive audio track
    ///
    /// A descriptive audio track is an audio track in which descriptions of visual elements of
    /// a video are added to the original audio, with the goal to make a video more accessible to
    /// blind and visually impaired people.
    ///
    /// See <https://en.wikipedia.org/wiki/Audio_description>
    Descriptive,
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
    pub videos: Paginator<VideoItem>,
    /// Number of videos in the playlist
    pub video_count: u64,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Playlist description in rich text format
    pub description: Option<RichText>,
    /// Channel of the playlist
    pub channel: Option<ChannelId>,
    /// Last update date
    pub last_update: Option<Date>,
    /// Textual last update date
    pub last_update_txt: Option<String>,
    /// YouTube visitor data ID
    pub visitor_data: Option<String>,
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
    pub name: String,
    /// Video description in rich text format
    pub description: RichText,
    /// Channel of the video
    pub channel: ChannelTag,
    /// Number of views / current viewers in case of a livestream.
    pub view_count: u64,
    /// Number of likes
    ///
    /// [`None`] if the like count was hidden by the creator.
    pub like_count: Option<u32>,
    /// Video publishing date. Start date in case of a livestream.
    ///
    /// [`None`] if the date could not be parsed.
    #[serde(with = "time::serde::rfc3339::option")]
    pub publish_date: Option<OffsetDateTime>,
    /// Textual video publishing date (e.g. `Aug 2, 2013`, depends on language)
    pub publish_date_txt: Option<String>,
    /// Is the video a livestream?
    pub is_live: bool,
    /// Is the video published under the Creative Commons BY 3.0 license?
    ///
    /// Information about the license:
    ///
    /// <https://www.youtube.com/t/creative_commons>
    ///
    /// <https://creativecommons.org/licenses/by/3.0/>
    pub is_ccommons: bool,
    /// Chapters of the video
    pub chapters: Vec<Chapter>,
    /// Recommended videos
    ///
    /// Note: Recommendations are not available for age-restricted videos
    pub recommended: Paginator<VideoItem>,
    /// Paginator to fetch comments (most liked first)
    ///
    /// Is initially empty.
    pub top_comments: Paginator<Comment>,
    /// Paginator to fetch comments (latest first)
    ///
    /// Is initially empty.
    pub latest_comments: Paginator<Comment>,
    /// YouTube visitor data ID
    pub visitor_data: Option<String>,
}

/// Chapter of a video
///
/// Videos can consist of different chapters, which YouTube shows
/// on the seek bar and below the description text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Chapter {
    /// Chapter name
    pub name: String,
    /// Chapter position in seconds
    pub position: u32,
    /// Chapter thumbnail
    pub thumbnail: Vec<Thumbnail>,
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
    /// [`None`] if hidden by the owner or not present.
    ///
    /// Info: This is only present in the `VideoDetails` response
    pub subscriber_count: Option<u64>,
}

/*
#COMMENTS
*/

/// Verification status of a channel
#[derive(
    Default, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
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
    /// [`None`] if the date could not be parsed.
    #[serde(with = "time::serde::rfc3339::option")]
    pub publish_date: Option<OffsetDateTime>,
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
    /// YouTube channel handle (e.g. `@EEVblog`)
    pub handle: Option<String>,
    /// Channel subscriber count
    ///
    /// [`None`] if the subscriber count was hidden by the owner
    /// or could not be parsed.
    pub subscriber_count: Option<u64>,
    /// Number of videos
    pub video_count: Option<u64>,
    /// Channel avatar / profile picture
    pub avatar: Vec<Thumbnail>,
    /// Channel verification mark
    pub verification: Verification,
    /// Channel description text
    pub description: String,
    /// List of words to describe the topic of the channel
    pub tags: Vec<String>,
    /// Banner image shown above the channel
    pub banner: Vec<Thumbnail>,
    /// Does the channel have a *Shorts* tab?
    pub has_shorts: bool,
    /// Does the channel have a *Live* tab?
    pub has_live: bool,
    /// YouTube visitor data ID
    pub visitor_data: Option<String>,
    /// Content fetched from the channel
    pub content: T,
}

/// Detailed channel information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelInfo {
    /// Unique YouTube Channel-ID (e.g. `UC-lHJZR3Gqxm24_Vd_AJ5Yw`)
    pub id: String,
    /// Channel URL
    pub url: String,
    /// Channel description text
    pub description: String,
    /// Channel subscriber count
    ///
    /// [`None`] if the subscriber count was hidden by the owner
    /// or could not be parsed.
    pub subscriber_count: Option<u64>,
    /// Channel video count
    pub video_count: Option<u64>,
    /// Channel creation date
    pub create_date: Option<Date>,
    /// Channel view count
    pub view_count: Option<u64>,
    /// Channel origin country
    pub country: Option<Country>,
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
    #[serde(with = "time::serde::rfc3339")]
    pub create_date: OffsetDateTime,
}

/// YouTube video fetched from a channel's RSS feed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelRssVideo {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub name: String,
    /// Video description in plaintext format
    pub description: String,
    /// Video thumbnail
    pub thumbnail: Thumbnail,
    /// Video publishing date (second-accurate).
    #[serde(with = "time::serde::rfc3339")]
    pub publish_date: OffsetDateTime,
    /// Date and time when the RSS feed entry was last updated.
    #[serde(with = "time::serde::rfc3339")]
    pub update_date: OffsetDateTime,
    /// Number of views / current viewers in case of a livestream.
    pub view_count: u64,
    /// Number of likes
    ///
    /// Zero if the like count was hidden by the creator.
    pub like_count: u64,
}

/// YouTube search result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult<T> {
    /// Search result items
    pub items: Paginator<T>,
    /// Corrected search query
    ///
    /// If the search term containes a typo, YouTube instead searches
    /// for the corrected search term and displays it on top of the
    /// search results page.
    pub corrected_query: Option<String>,
    /// YouTube visitor data ID
    pub visitor_data: Option<String>,
}

/// YouTube item (Video/Channel/Playlist)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum YouTubeItem {
    /// YouTube video item
    Video(VideoItem),
    /// YouTube playlist item
    Playlist(PlaylistItem),
    /// YouTube channel item
    Channel(ChannelItem),
}

/// YouTube video list item (from search results, recommendations, playlists)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoItem {
    /// Unique YouTube video ID
    pub id: String,
    /// Video title
    pub name: String,
    /// Video duration in seconds.
    ///
    /// Is [`None`] for livestreams.
    pub duration: Option<u32>,
    /// Video thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the video
    pub channel: Option<ChannelTag>,
    /// Video publishing date.
    ///
    /// [`None`] if the date could not be parsed.
    #[serde(with = "time::serde::rfc3339::option")]
    pub publish_date: Option<OffsetDateTime>,
    /// Textual video publish date (e.g. `11 months ago`, depends on language)
    ///
    /// Is [`None`] for livestreams and upcoming videos.
    pub publish_date_txt: Option<String>,
    /// View count
    ///
    /// [`None`] if it could not be extracted.
    pub view_count: Option<u64>,
    /// Is the video an active livestream?
    pub is_live: bool,
    /// Is the video a YouTube Short video (vertical and <60s)?
    pub is_short: bool,
    /// Is the video announced, but not released yet (YouTube Premiere)?
    pub is_upcoming: bool,
    /// Abbreviated video description
    pub short_description: Option<String>,
}

/// YouTube channel list item (from search results)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelItem {
    /// Unique YouTube channel ID
    pub id: String,
    /// Channel name
    pub name: String,
    /// YouTube channel handle (e.g. `@EEVblog`)
    pub handle: Option<String>,
    /// Channel avatar/profile picture
    pub avatar: Vec<Thumbnail>,
    /// Channel verification mark
    pub verification: Verification,
    /// Approximate number of subscribers
    ///
    /// [`None`] if hidden by the owner or not present.
    pub subscriber_count: Option<u64>,
    /// Abbreviated channel description
    pub short_description: String,
}

/// YouTube playlist list item (from search results)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlaylistItem {
    /// Unique YouTube playlist ID (e.g. `PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ`)
    pub id: String,
    /// Playlist name
    pub name: String,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the playlist
    pub channel: Option<ChannelTag>,
    /// Number of playlist videos
    pub video_count: Option<u64>,
}

/// YouTube video identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoId {
    /// Video ID
    pub id: String,
    /// Video title
    pub name: String,
}

/*
#MUSIC
*/

/// YouTube Music track list item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct TrackItem {
    /// Unique YouTube video ID
    pub id: String,
    /// Track name
    pub name: String,
    /// Track duration in seconds
    ///
    /// [`None`] when extracted from an artist page or a featured video.
    pub duration: Option<u32>,
    /// Album cover
    pub cover: Vec<Thumbnail>,
    /// Artists of the track
    pub artists: Vec<ArtistId>,
    /// Primary artist ID
    pub artist_id: Option<String>,
    /// Album of the track
    pub album: Option<AlbumId>,
    /// View count
    ///
    /// [`None`] if it is a not a video or the view count could not be extracted.
    pub view_count: Option<u64>,
    /// Type of the track (YTM track / music video / podcast episode)
    pub track_type: TrackType,
    /// Album track number
    ///
    /// [`None`] if the track is not fetched from an album.
    pub track_nr: Option<u16>,
    /// Is the track by 'Various artists'?
    pub by_va: bool,
}

/// YouTube Music artist list item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtistItem {
    /// Unique YouTube channel ID
    pub id: String,
    /// Artist name
    pub name: String,
    /// Artist avatar/profile picture
    pub avatar: Vec<Thumbnail>,
    /// Approximate number of subscribers
    ///
    /// [`None`] if hidden by the owner or not present.
    pub subscriber_count: Option<u64>,
}

/// YouTube Music user item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserItem {
    /// Unique YouTube user ID
    pub id: String,
    /// User name
    pub name: String,
    /// YouTube channel handle (e.g. `@EEVblog`)
    pub handle: Option<String>,
    /// User avatar/profile picture
    pub avatar: Vec<Thumbnail>,
}

/// YouTube Music artist identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtistId {
    /// Unique YouTube channel ID
    pub id: Option<String>,
    /// Artist name
    pub name: String,
}

/// YouTube Music album list item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct AlbumItem {
    /// Unique YouTube album ID (e.g. `MPREb_T5s950Swfdy`)
    pub id: String,
    /// Album name
    pub name: String,
    /// Album cover
    pub cover: Vec<Thumbnail>,
    /// Artists of the album
    pub artists: Vec<ArtistId>,
    /// Primary artist ID
    pub artist_id: Option<String>,
    /// Album type (Album/Single/EP)
    pub album_type: AlbumType,
    /// Release year of the album
    pub year: Option<u16>,
    /// Is the album by 'Various artists'?
    pub by_va: bool,
}

/// YouTube Music playlist list item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicPlaylistItem {
    /// Unique YouTube playlist ID (e.g. `PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ`)
    pub id: String,
    /// Playlist name
    pub name: String,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the playlist
    pub channel: Option<ChannelId>,
    /// Number of tracks in the playlist
    pub track_count: Option<u64>,
    /// True if the playlist is from YouTube Music
    pub from_ytm: bool,
    /// True if the playlist is a podcast
    pub is_podcast: bool,
}

/// YouTube Music album type
#[derive(
    Default, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AlbumType {
    /// Regular album (default)
    #[default]
    Album,
    /// Extended play
    Ep,
    /// Single
    Single,
    /// Audiobook
    Audiobook,
    /// Show (audio drama)
    Show,
}

/// YouTube Music track type
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TrackType {
    /// Official YouTube Music track without video
    Track,
    /// Music video
    Video,
    /// Podcast episode
    Episode,
}

impl TrackType {
    /// Return true if the track is an official YouTube Music track without video
    pub fn is_track(&self) -> bool {
        self == &Self::Track
    }

    /// Return true if the track is a YouTube video
    pub fn is_video(&self) -> bool {
        self != &Self::Track
    }
}

/// Album identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct AlbumId {
    /// Unique YouTube album ID (e.g. `MPREb_O2gXCdCVGsZ`)
    pub id: String,
    /// Album name
    pub name: String,
}

/// YouTube Music playlist object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicPlaylist {
    /// Unique YouTube playlist ID (e.g. `PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ`)
    pub id: String,
    /// Playlist/album name
    pub name: String,
    /// Playlist thumbnail
    pub thumbnail: Vec<Thumbnail>,
    /// Channel of the playlist
    pub channel: Option<ChannelId>,
    /// Playlist description in rich text format
    pub description: Option<RichText>,
    /// Number of tracks in the playlist
    pub track_count: Option<u64>,
    /// True if the playlist is from YouTube Music
    pub from_ytm: bool,
    /// Playlist tracks
    pub tracks: Paginator<TrackItem>,
    /// Related playlists
    pub related_playlists: Paginator<MusicPlaylistItem>,
}

/// YouTube Music album object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicAlbum {
    /// Unique YouTube album ID (e.g. `MPREb_O2gXCdCVGsZ`)
    pub id: String,
    /// Unique YouTube playlist ID (e.g. `OLAK5uy_nZpcQys48R0aNb046hV-n1OAHGE4reftQ`)
    pub playlist_id: Option<String>,
    /// Album name
    pub name: String,
    /// Album cover
    pub cover: Vec<Thumbnail>,
    /// Artists of the album
    pub artists: Vec<ArtistId>,
    /// Primary artist ID
    pub artist_id: Option<String>,
    /// Album description in rich text format
    pub description: Option<RichText>,
    /// Album type (Album/Single/EP)
    pub album_type: AlbumType,
    /// Release year
    pub year: Option<u16>,
    /// Is the album by 'Various artists'?
    pub by_va: bool,
    /// Number of album tracks
    pub track_count: u16,
    /// Album tracks
    pub tracks: Vec<TrackItem>,
    /// Album variants
    pub variants: Vec<AlbumItem>,
}

/// YouTube Music artist object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicArtist {
    /// Unique YouTube channel ID (e.g. `UCRD-INDaHvHlO8K_33uKetQ`)
    pub id: String,
    /// Artist name
    pub name: String,
    /// Artist header image
    pub header_image: Vec<Thumbnail>,
    /// Artist description
    pub description: Option<String>,
    /// URL of the artist's wikipedia page
    pub wikipedia_url: Option<String>,
    /// Artist subscriber count
    ///
    /// [`None`] if the subscriber count was hidden by the owner
    /// or could not be parsed.
    pub subscriber_count: Option<u64>,
    /// The most popular tracks of the artist
    pub tracks: Vec<TrackItem>,
    /// The artist's albums
    pub albums: Vec<AlbumItem>,
    /// Playlists featuring the artist
    pub playlists: Vec<MusicPlaylistItem>,
    /// Similar artists
    pub similar_artists: Vec<ArtistItem>,
    /// ID of the playlist containging the artist's tracks
    pub tracks_playlist_id: Option<String>,
    /// ID of the playlist containging the artist's videos
    pub videos_playlist_id: Option<String>,
    /// ID of the artist radio
    pub radio_id: Option<String>,
}

/// Generic YouTube Music item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum MusicItem {
    Track(TrackItem),
    Album(AlbumItem),
    Artist(ArtistItem),
    Playlist(MusicPlaylistItem),
    User(UserItem),
}

/// YouTube Music item type
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum MusicItemType {
    Track,
    Album,
    Artist,
    Playlist,
    User,
}

/// YouTube Music search result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicSearchResult<T> {
    /// Search items
    pub items: Paginator<T>,
    /// Corrected search query
    ///
    /// If the search term containes a typo, YouTube instead searches
    /// for the corrected search term and displays it on top of the
    /// search results page.
    pub corrected_query: Option<String>,
}

/// Music track details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct TrackDetails {
    /// Track metadata
    pub track: TrackItem,
    /// ID to fetch lyrics
    pub lyrics_id: Option<String>,
    /// ID to fetch related tracks
    pub related_id: Option<String>,
}

/// Song lyrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Lyrics {
    /// Lyrics text
    pub body: String,
    /// Footer (contains lyrics source)
    pub footer: String,
}

/// YouTube Music items related to a track
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicRelated {
    /// Related tracks
    pub tracks: Vec<TrackItem>,
    /// Other versions of the same track
    pub other_versions: Vec<TrackItem>,
    /// Related albums
    pub albums: Vec<AlbumItem>,
    /// Related artists
    pub artists: Vec<ArtistItem>,
    /// Related playlists
    pub playlists: Vec<MusicPlaylistItem>,
}

/// YouTube Music charts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicCharts {
    /// List of top music videos
    pub top_tracks: Vec<TrackItem>,
    /// List of trending music videos
    pub trending_tracks: Vec<TrackItem>,
    /// List of top artists
    pub artists: Vec<ArtistItem>,
    /// List of playlists (charts by genre, currently only available in US)
    pub playlists: Vec<MusicPlaylistItem>,
    /// ID of the playlist containing top music videos
    pub top_playlist_id: Option<String>,
    /// ID of the playlist containing trending music videos
    pub trending_playlist_id: Option<String>,
    /// Set of available countries to fetch charts from
    pub available_countries: BTreeSet<Country>,
}

/// YouTube Music genre/mood list item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicGenreItem {
    /// Unique YouTube Music genre ID
    pub id: String,
    /// Genre name
    pub name: String,
    /// Is it a mood (e.g. Chill, Focus, Party)
    pub is_mood: bool,
    /// Color of the genre button
    ///
    /// Encoded as a 32-bit integer with the following format (8 bits per number):
    ///
    /// `[Alpha][R][G][B]`
    ///
    /// **Note:** Alpa/Opacity is always set to 0xFF.
    pub color: u32,
}

/// YouTube Music genre/mood content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicGenre {
    /// Unique YouTube Music genre ID
    pub id: String,
    /// Genre name
    pub name: String,
    /// List of sections containing the content
    pub sections: Vec<MusicGenreSection>,
}

/// YouTube Music genre/mood content section
///
/// Genre pages in YouTube Music are split into sections. These have a name
/// and contain several playlists. If the section is showing a subgenre
/// (e.g "2000s" from "Decades"), the section also includes a `subgenre_id`
/// for fetching more content of that subgenre.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicGenreSection {
    /// Name of the genre section
    pub name: String,
    /// Subgenre ID to fetch more content
    pub subgenre_id: Option<String>,
    /// List of playlists of the genre section
    pub playlists: Vec<MusicPlaylistItem>,
}

/// YouTube Music suggested search terms/items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicSearchSuggestion {
    /// Suggested search terms
    pub terms: Vec<String>,
    /// Suggested music items
    pub items: Vec<MusicItem>,
}

/// YouTube playback history entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HistoryItem<T> {
    /// History item
    pub item: T,
    /// Playback date
    pub playback_date: Option<Date>,
    /// Textual playback date
    pub playback_date_txt: Option<String>,
}
