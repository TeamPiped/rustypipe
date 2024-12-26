//! Wrapper model for progressively fetched items

use std::ops::Not;

use serde::{Deserialize, Serialize};

/// Wrapper around progressively fetched items
///
/// The paginator is a wrapper around a list of items that are fetched
/// in pages from the YouTube API (e.g. playlist items,
/// video recommendations or comments).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Paginator<T> {
    /// Total number of items if finite and known.
    ///
    /// Note that this number may not be 100% accurate, as this is the
    /// number returned by the YouTube API at the initial fetch.
    ///
    /// It is intended to be shown to the user (e.g. 1261 comments,
    /// 18 Videos) and for progress estimation.
    ///
    /// Don't use this number to check if all items were fetched or for
    /// iterating over the items.
    pub count: Option<u64>,
    /// Content of the paginator
    pub items: Vec<T>,
    /// The continuation token is passed to the YouTube API to fetch
    /// more items.
    ///
    /// If it is None, it means that no more items can be fetched.
    pub ctoken: Option<String>,
    /// YouTube visitor data. Required for fetching the startpage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visitor_data: Option<String>,
    /// YouTube API endpoint to fetch continuations from
    pub endpoint: ContinuationEndpoint,
    /// True if the paginator should be fetched with YouTube credentials
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub authenticated: bool,
}

impl<T> Default for Paginator<T> {
    fn default() -> Self {
        Self {
            count: Some(0),
            items: Vec::new(),
            ctoken: None,
            visitor_data: None,
            endpoint: ContinuationEndpoint::Browse,
            authenticated: false,
        }
    }
}

/// YouTube API endpoint to fetch continuations from
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[allow(missing_docs)]
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

impl<T> Paginator<T> {
    pub(crate) fn new(count: Option<u64>, items: Vec<T>, ctoken: Option<String>) -> Self {
        Self::new_ext(
            count,
            items,
            ctoken,
            None,
            ContinuationEndpoint::Browse,
            false,
        )
    }

    pub(crate) fn new_ext(
        count: Option<u64>,
        items: Vec<T>,
        ctoken: Option<String>,
        visitor_data: Option<String>,
        endpoint: ContinuationEndpoint,
        authenticated: bool,
    ) -> Self {
        Self {
            count: match ctoken {
                Some(_) => count,
                None => items.len().try_into().ok(),
            },
            items,
            ctoken,
            visitor_data,
            endpoint,
            authenticated,
        }
    }

    /// Check if the paginator is exhausted, meaning that no more
    /// items can be fetched.
    ///
    /// Equivalent to `paginator.ctoken.is_none()`.
    pub fn is_exhausted(&self) -> bool {
        self.ctoken.is_none()
    }

    /// Check if the paginator does not contain any data, meaning that it
    /// is exhausted and does not contain any items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.is_exhausted()
    }
}
