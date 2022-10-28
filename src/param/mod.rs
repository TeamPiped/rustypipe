mod stream_filter;

pub mod locale;
pub mod search_filter;

pub use locale::{Country, Language};
use serde::{Deserialize, Serialize};
pub use stream_filter::StreamFilter;

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
