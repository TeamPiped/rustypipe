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

/// Channel video tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelVideoTab {
    /// Regular videos
    Videos,
    /// Short videos
    Shorts,
    /// Livestreams
    Live,
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
