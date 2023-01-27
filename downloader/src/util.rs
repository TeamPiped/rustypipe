use std::{borrow::Cow, collections::BTreeMap};

use reqwest::Url;

/// Error from the video downloader
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DownloadError {
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// File IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("FFmpeg error: {0}")]
    Ffmpeg(Cow<'static, str>),
    #[error("Progressive download error: {0}")]
    Progressive(Cow<'static, str>),
    #[error("input error: {0}")]
    Input(Cow<'static, str>),
    #[error("error: {0}")]
    Other(Cow<'static, str>),
}

/// Split an URL into its base string and parameter map
///
/// Example:
///
/// `example.com/api?k1=v1&k2=v2 => example.com/api; {k1: v1, k2: v2}`
pub fn url_to_params(url: &str) -> Result<(Url, BTreeMap<String, String>), DownloadError> {
    let mut parsed_url = Url::parse(url).map_err(|e| {
        DownloadError::Other(format!("could not parse url `{url}` err: {e}").into())
    })?;
    let url_params: BTreeMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    parsed_url.set_query(None);

    Ok((parsed_url, url_params))
}
