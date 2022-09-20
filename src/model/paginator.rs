use serde::{Deserialize, Serialize};


/// The paginator is a wrapper around a list of items that are fetched
/// in pages from the YouTube API (e.g. playlist items,
/// video recommendations or comments).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub count: Option<u32>,
    /// Content of the paginator
    pub items: Vec<T>,
    /// The continuation token is passed to the YouTube API to fetch
    /// more items.
    ///
    /// If it is None, it means that no more items can be fetched.
    pub ctoken: Option<String>,
}

impl<T> Default for Paginator<T> {
    fn default() -> Self {
        Self {
            count: None,
            items: Vec::new(),
            ctoken: None,
        }
    }
}

impl<T> Paginator<T> {
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
