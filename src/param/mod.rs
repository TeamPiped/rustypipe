mod stream_filter;

pub mod locale;
pub mod search_filter;

pub use locale::{Country, Language};
use serde::{Deserialize, Serialize};
pub use stream_filter::StreamFilter;

/// Channel video sort order
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChannelOrder {
    /// Output the latest videos first
    #[default]
    Latest,
    /// Output the oldest videos first
    Oldest,
    /// Output the most viewed videos first
    Popular,
}

/// YouTube API endpoint to fetch continuations from
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContinuationEndpoint {
    Browse,
    Search,
    Next,
}

impl ContinuationEndpoint {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ContinuationEndpoint::Browse => "browse",
            ContinuationEndpoint::Search => "search",
            ContinuationEndpoint::Next => "next",
        }
    }
}
