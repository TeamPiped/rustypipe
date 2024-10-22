//! # Query parameters
//!
//! This module contains structs and enums used as input parameters
//! for the functions in RustyPipe.

mod locale;
mod stream_filter;

pub mod search_filter;

pub use locale::{Country, Language, COUNTRIES, LANGUAGES};
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
    Latest = 1,
    /// Order videos with the highest number of views first
    Popular = 2,
    /// Order videos with the earliest upload date first
    Oldest = 4,
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
