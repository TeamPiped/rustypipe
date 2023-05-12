//! # Input validation
//!
//! The extraction functions of RustyPipe will produce errors when fed with invalid input data
//! (e.g. YouTube ID's with invalid format). Therefore you will need to validate all untrusted
//! input data beforehand. The library offers two options for this:
//!
//! - The [URL resolver](crate::client::RustyPipeQuery::resolve_url) or
//!   [string resolver](crate::client::RustyPipeQuery::resolve_string) is great for handling
//!   arbitrary input and returns a [`UrlTarget`](crate::model::UrlTarget) enum that tells you
//!   whether the given URL points to a video, channel, playlist, etc.
//! - The validation functions of this module are meant vor validating specific data (video IDs,
//!   channel IDs, playlist IDs) and return [`true`] if the given input is valid

use crate::util;
use once_cell::sync::Lazy;
use regex::Regex;

/// Validate the given video ID
///
/// YouTube video IDs are exactly 11 characters long and consist of the charactes `A-Za-z0-9_-`.
///
/// # Examples
/// ```
/// # use rustypipe::validate;
/// assert!(validate::video_id("dQw4w9WgXcQ"));
/// assert!(!validate::video_id("Abcd"));
/// assert!(!validate::video_id("dQw4w9WgXc@"));
/// ```
pub fn video_id<S: AsRef<str>>(video_id: S) -> bool {
    util::VIDEO_ID_REGEX.is_match(video_id.as_ref())
}

/// Validate the given channel ID
///
/// YouTube channel IDs are exactly 24 characters long, start with the characters `UC`,
/// followed by 22 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
/// ```
/// # use rustypipe::validate;
/// assert!(validate::channel_id("UC2DjFE7Xf11URZqWBigcVOQ"));
/// assert!(!validate::channel_id("Abcd"));
/// assert!(!validate::channel_id("XY2DjFE7Xf11URZqWBigcVOQ"));
/// ```
pub fn channel_id<S: AsRef<str>>(channel_id: S) -> bool {
    util::CHANNEL_ID_REGEX.is_match(channel_id.as_ref())
}

/// Validate the given playlist ID
///
/// YouTube playlist IDs start with the characters `PL` (user-created playlist),
/// `RDCLAK` (YouTube Music-curated playlist) or `OLAK` (YouTube Music album),
/// followed by at least 30 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
/// ```
/// # use rustypipe::validate;
/// assert!(validate::playlist_id("PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI"));
/// assert!(validate::playlist_id("RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"));
/// assert!(validate::playlist_id("OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE"));
///
/// assert!(!validate::playlist_id("Abcd"));
/// ```
pub fn playlist_id<S: AsRef<str>>(playlist_id: S) -> bool {
    util::PLAYLIST_ID_REGEX.is_match(playlist_id.as_ref())
}

/// Validate the given album ID
///
/// YouTube Music album IDs are exactly 17 characters long, start with the characters `MPREB_`,
/// followed by 11 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
/// ```
/// # use rustypipe::validate;
/// assert!(validate::album_id("MPREb_GyH43gCvdM5"));
/// assert!(!validate::album_id("Abcd_GyH43gCvdM5"));
/// ```
///
/// # Note
///
/// Albums on YouTube Music have an album ID (`MPREB_...`) and a playlist ID
/// (`OLAK...`). If you open an album on the YouTube Music website, the address bar shows
/// the playlist ID, not the album ID.
///
/// If you have the playlist ID of an album and need the album ID, you can use the
/// [string resolver](crate::client::RustyPipeQuery::resolve_string) with the `resolve_albums`
/// option enabled.
pub fn album_id<S: AsRef<str>>(album_id: S) -> bool {
    util::ALBUM_ID_REGEX.is_match(album_id.as_ref())
}

/// Validate the given radio ID
///
/// YouTube radio IDs start with the characters `RD`,
/// followed by at least 22 of these characters: `A-Za-z0-9_-`.
///
/// # Radio types
///
/// - Artist radio: `RDEMSuoM_jxfse1_g8uCO7MCtg`
/// - Genre radio: `RDQM1xqCV6EdPUw`
/// - Shuffle radio: `RDAOVeZA-2uzuUKdoB81Ha3srw`
/// - Playlist radio (`RDAMPL` + playlist ID): `RDAMPLPL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI`
/// - Track radio (`RDAMVM` + video ID): `RDAMVMZeerrnuLi5E`
///
/// # Examples
///
/// ```
/// # use rustypipe::validate;
/// assert!(validate::radio_id("RDEMSuoM_jxfse1_g8uCO7MCtg"));
/// assert!(!validate::radio_id("Abcd"));
/// assert!(!validate::radio_id("XYEMSuoM_jxfse1_g8uCO7MCtg"));
/// ```
pub fn radio_id<S: AsRef<str>>(radio_id: S) -> bool {
    static RADIO_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^RD[A-Za-z0-9_-]{22,50}$").unwrap());

    RADIO_ID_REGEX.is_match(radio_id.as_ref())
}

/// Validate the given genre ID
///
/// YouTube genre IDs are exactly 24 characters long, start with the characters `ggMPO`,
/// followed by 19 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
///
/// ```
/// # use rustypipe::validate;
/// assert!(validate::genre_id("ggMPOg1uX1JOQWZFeDByc2Jm"));
/// assert!(!validate::genre_id("Abcd"));
/// assert!(!validate::genre_id("ggAbcg1uX1JOQWZFeDByc2Jm"));
/// ```
pub fn genre_id<S: AsRef<str>>(genre_id: S) -> bool {
    static GENRE_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^ggMPO[A-Za-z0-9_-]{19}$").unwrap());

    GENRE_ID_REGEX.is_match(genre_id.as_ref())
}

/// Validate the given related tracks ID
///
/// YouTube related IDs are exactly 17 characters long, start with the characters `MPTRt_`,
/// followed by 11 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
///
/// ```
/// # use rustypipe::validate;
/// assert!(validate::track_related_id("MPTRt_wrKjTn9hmry"));
/// assert!(!validate::track_related_id("Abcd"));
/// assert!(!validate::track_related_id("Abcdt_wrKjTn9hmry"));
/// ```
pub fn track_related_id<S: AsRef<str>>(related_id: S) -> bool {
    static RELATED_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^MPTRt_[A-Za-z0-9_-]{11}$").unwrap());

    RELATED_ID_REGEX.is_match(related_id.as_ref())
}

/// Validate the given lyrics ID
///
/// YouTube lyrics IDs are exactly 17 characters long, start with the characters `MPLYt_`,
/// followed by 11 of these characters: `A-Za-z0-9_-`.
///
/// # Examples
///
/// ```
/// # use rustypipe::validate;
/// assert!(validate::track_lyrics_id("MPLYt_wrKjTn9hmry"));
/// assert!(!validate::track_lyrics_id("Abcd"));
/// assert!(!validate::track_lyrics_id("Abcdt_wrKjTn9hmry"));
/// ```
pub fn track_lyrics_id<S: AsRef<str>>(lyrics_id: S) -> bool {
    static LYRICS_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^MPLYt_[A-Za-z0-9_-]{11}$").unwrap());

    LYRICS_ID_REGEX.is_match(lyrics_id.as_ref())
}
