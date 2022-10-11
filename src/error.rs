use std::borrow::Cow;

pub(crate) type Result<T> = core::result::Result<T, Error>;

/// Custom error type for the RustyPipe library
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error extracting content from YouTube
    #[error("extraction error: {0}")]
    Extraction(#[from] ExtractionError),
    /// Error from the deobfuscater
    #[error("deobfuscator error: {0}")]
    Deobfuscation(#[from] DeobfError),
    /// Error from the video downloader
    #[error("download error: {0}")]
    Download(#[from] DownloadError),
    /// File IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("http status code: {0}")]
    HttpStatus(u16),
    #[error("error: {0}")]
    Other(Cow<'static, str>),
}

/// Error that occurred during the initialization
/// or use of the YouTube URL signature deobfuscator.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DeobfError {
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Error during JavaScript execution
    #[error("js execution error: {0}")]
    JavaScript(#[from] quick_js::ExecutionError),
    #[error("js parsing: {0}")]
    JsParser(#[from] ress::error::Error),
    /// Could not extract certain data
    #[error("could not extract {0}")]
    Extraction(&'static str),
    #[error("error: {0}")]
    Other(&'static str),
}

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

/// Error extracting content from YouTube
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum ExtractionError {
    #[error("Video cant be played because of {0}. Reason (from YT): {1}")]
    VideoUnavailable(&'static str, String),
    #[error("Video is age restricted")]
    VideoAgeRestricted,
    #[error("Content is not available. Reason (from YT): {0}")]
    ContentUnavailable(String),
    #[error("Got no data from YouTube")]
    NoData,
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("got invalid data from YT: {0}")]
    InvalidData(Cow<'static, str>),
    #[error("got wrong result from YT: {0}")]
    WrongResult(String),
    #[error("Warnings during deserialization/mapping")]
    DeserializationWarnings,
    #[error("Got no data from YouTube, attempt retry")]
    Retry,
}
