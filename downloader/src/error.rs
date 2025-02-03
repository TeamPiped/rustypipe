use std::{borrow::Cow, path::PathBuf};

use rustypipe::client::ClientType;

/// Error from the video downloader
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DownloadError {
    /// RustyPipe error
    #[error("{0}")]
    RustyPipe(#[from] rustypipe::error::Error),
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// 403 error trying to download video
    #[error("YouTube returned 403 error; visitor_data={}", .1.as_deref().unwrap_or_default())]
    Forbidden(ClientType, Option<String>),
    /// File IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// FFmpeg returned an error
    #[error("FFmpeg error: {0}")]
    Ffmpeg(Cow<'static, str>),
    /// Error parsing ranges for progressive download
    #[error("Progressive download error: {0}")]
    Progressive(Cow<'static, str>),
    /// Video could not be downloaded because of invalid player data
    #[error("source error: {0}")]
    Source(Cow<'static, str>),
    /// Download target already exists
    #[error("file {0} already exists")]
    Exists(PathBuf),
    #[cfg(feature = "audiotag")]
    /// Audio tagging error
    #[error("Audio tag error: {0}")]
    AudioTag(Cow<'static, str>),
    /// Other error
    #[error("error: {0}")]
    Other(Cow<'static, str>),
}

#[cfg(feature = "audiotag")]
impl From<lofty::error::LoftyError> for DownloadError {
    fn from(value: lofty::error::LoftyError) -> Self {
        Self::AudioTag(value.to_string().into())
    }
}

#[cfg(feature = "audiotag")]
impl From<image::ImageError> for DownloadError {
    fn from(value: image::ImageError) -> Self {
        Self::AudioTag(value.to_string().into())
    }
}
