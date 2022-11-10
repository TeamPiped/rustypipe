mod stream_filter;

pub mod locale;
pub mod search_filter;

pub use locale::{Country, Language};
use serde::{Deserialize, Serialize};
pub use stream_filter::StreamFilter;

/// YouTube API endpoint to fetch continuations from
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContinuationEndpoint {
    Browse,
    Search,
    Next,
    MusicBrowse,
    MusicSearch,
    MusicNext,
}

impl ContinuationEndpoint {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ContinuationEndpoint::Browse | ContinuationEndpoint::MusicBrowse => "browse",
            ContinuationEndpoint::Search | ContinuationEndpoint::MusicSearch => "search",
            ContinuationEndpoint::Next | ContinuationEndpoint::MusicNext => "next",
        }
    }

    pub(crate) fn is_music(self) -> bool {
        matches!(
            self,
            ContinuationEndpoint::MusicBrowse
                | ContinuationEndpoint::MusicSearch
                | ContinuationEndpoint::MusicNext
        )
    }
}
