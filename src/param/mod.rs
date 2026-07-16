//! # Query parameters
//!
//! This module contains structs and enums used as input parameters
//! for the functions in RustyPipe.

mod locale;
mod stream_filter;

pub mod search_filter;

pub use locale::{Country, Language, COUNTRIES, LANGUAGES};
pub(crate) use stream_filter::cmp_bitrate;
pub use stream_filter::StreamFilter;

/// Channel content selection
///
/// Selects which tab (or search query) of a channel to fetch via
/// [`crate::client::RustyPipeQuery::channel_content`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelContent<'a> {
    /// Regular videos
    Videos,
    /// Short videos
    Shorts,
    /// Livestreams
    Live,
    /// Playlists created by the channel
    Playlists,
    /// Search the videos of a channel
    Search(&'a str),
}

/// Internal: video tab subset used for ordered continuations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelVideoTab {
    Videos,
    Shorts,
    Live,
}

impl<'a> ChannelContent<'a> {
    /// Returns the matching private video tab, or `None` for non-video variants
    /// (Playlists, Search).
    pub(crate) const fn as_video_tab(self) -> Option<ChannelVideoTab> {
        match self {
            Self::Videos => Some(ChannelVideoTab::Videos),
            Self::Shorts => Some(ChannelVideoTab::Shorts),
            Self::Live => Some(ChannelVideoTab::Live),
            Self::Playlists | Self::Search(_) => None,
        }
    }

    /// The browse `params` value for this content type. Used by the regular
    /// `browse` call path. Search uses the same params as a normal search
    /// (the query is passed separately).
    pub(crate) const fn browse_params(self) -> &'static str {
        match self {
            Self::Videos => "EgZ2aWRlb3PyBgQKAjoA",
            Self::Shorts => "EgZzaG9ydHPyBgUKA5oBAA%3D%3D",
            Self::Live => "EgdzdHJlYW1z8gYECgJ6AA%3D%3D",
            Self::Playlists => "EglwbGF5bGlzdHMgAQ%3D%3D",
            Self::Search(_) => "EgZzZWFyY2jyBgQKAloA",
        }
    }
}

/// Sort order for channel videos
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelOrder {
    /// Order videos with the latest upload date first (default)
    #[default]
    Latest, // video 3=1,4=4; shorts 4=4; live 5=12
    /// Order videos with the highest number of views first
    Popular, // video 3=2,4=2; shorts 4=2; live 5=14
    /// Order videos with the earliest upload date first
    Oldest, // video 3=4,4=5; shorts 4=5; live 5=13
}

/// Explore / trending tab
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendingTab {
    /// YouTube News explore tab
    #[default]
    News,
    /// YouTube Music explore tab
    Music,
    /// YouTube Gaming explore tab
    Gaming,
    /// YouTube Sports explore tab
    Sports,
    /// YouTube Live explore tab
    Live,
    /// YouTube Shopping explore tab
    Shopping,
}

impl TrendingTab {
    /// Browse ID used by the current explore category.
    pub const fn browse_id(self) -> &'static str {
        match self {
            TrendingTab::News => "UCYfdidRxbB8Qhf0Nx7ioOYw",
            TrendingTab::Music => "UC-9-kyTW8ZkZNDHQJ6FgpwQ",
            TrendingTab::Gaming => "UCOpNcN46UbXVtpKMrmU4Abg",
            TrendingTab::Sports => "UCEgdi0XIXXZ-qJOFPf4JSKw",
            TrendingTab::Live => "UC4R8DWoMoI7CAwX8_LjQHig",
            TrendingTab::Shopping => "UCkYQyvc_i9hXEo4xic9Hh2g",
        }
    }
}

impl ChannelVideoTab {
    /// Get the tab ID used to create ordered continuation tokens
    pub(crate) const fn order_ctoken_id(self) -> u32 {
        match self {
            ChannelVideoTab::Videos => 15,
            ChannelVideoTab::Shorts => 10,
            ChannelVideoTab::Live => 14,
        }
    }
}

/// Sort order for YTM artist albums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumOrder {
    /// Sort albums by release date
    Recency = 1,
    /// Sort albums by popularity
    Popularity = 2,
    /// Sort albums by their name
    Alphabetical = 3,
}

/// Filter for YTM artist albums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumFilter {
    /// Only show albums
    Albums = 1,
    /// Only show singles
    Singles = 2,
}

/// Whether to fetch the albums behind the "More" buttons on a YTM artist page
///
/// Used by [`crate::client::RustyPipeQuery::music_artist`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MusicArtistAlbums {
    /// Skip the albums behind the "More" buttons (default)
    #[default]
    Exclude,
    /// Fetch the albums behind the "More" buttons too
    Include,
}

/// Whether to resolve YTM album URLs to their short album ids
///
/// Used by [`crate::client::RustyPipeQuery::resolve_url`] and
/// [`crate::client::RustyPipeQuery::resolve_string`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AlbumResolution {
    /// Don't resolve YTM album URLs
    #[default]
    No,
    /// Resolve YTM album URLs to their short album ids (e.g. `OLAK5uy_...` to
    /// `MPREb_...`)
    Yes,
}

/// Selection of the user's YouTube Music library ("saved X") feed
///
/// Selects which kind of items to fetch via
/// [`crate::client::RustyPipeQuery::music_saved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicSavedKind {
    /// Artists the user subscribed to
    Artists,
    /// Albums in the user's collection
    Albums,
    /// Tracks in the user's collection (liked + tracks from saved albums)
    Tracks,
    /// Playlists in the user's collection
    Playlists,
}

/// Selection of a built-in user playlist
///
/// Selects which built-in playlist to fetch via
/// [`crate::client::RustyPipeQuery::user_playlist`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserPlaylistKind {
    /// The "Liked videos" playlist (`LL`)
    LikedVideos,
    /// The "Watch later" playlist (`WL`)
    WatchLater,
    /// The "Liked music tracks" playlist (`LM`)
    MusicLikedTracks,
}

/// Selection of a "new releases" feed on YouTube Music
///
/// Selects which kind of new release to fetch via
/// [`crate::client::RustyPipeQuery::music_new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicNewKind {
    /// New albums released on YouTube Music
    Albums,
    /// New music videos released on YouTube Music
    Videos,
}
