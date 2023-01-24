//! RustyPipe error types

use std::borrow::Cow;

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
    /// File IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Erroneous HTTP status code received
    #[error("http status code: {0} message: {1}")]
    HttpStatus(u16, Cow<'static, str>),
    /// Unspecified error
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
    /// Error during JavaScript parsing
    #[error("js parsing: {0}")]
    JsParser(#[from] ress::error::Error),
    /// Could not extract certain data
    #[error("could not extract {0}")]
    Extraction(&'static str),
    /// Unspecified error
    #[error("error: {0}")]
    Other(&'static str),
}

/// Error extracting content from YouTube
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum ExtractionError {
    /// Video cannot be extracted with RustyPipe
    ///
    /// Reasons include:
    /// - Deletion/Censorship
    /// - Private video that requires a Google account
    /// - DRM (Movies and TV shows)
    #[error("Video cant be played because of {0}. Reason (from YT): {1}")]
    VideoUnavailable(&'static str, String),
    /// Video cannot be extracted because it is age restricted.
    ///
    /// Age restriction may be circumvented with the [`crate::client::ClientType::TvHtml5Embed`] client.
    #[error("Video is age restricted")]
    VideoAgeRestricted,
    /// Video cannot be extracted because it is not available in your country
    #[error("Video is not available in your country")]
    VideoGeoblocked,
    /// Video cannot be extracted with the specified client
    #[error("Video cant be played with this client. Reason (from YT): {0}")]
    VideoClientUnsupported(String),
    /// Content is not available / does not exist
    #[error("Content is not available. Reason: {0}")]
    ContentUnavailable(Cow<'static, str>),
    /// Bad request (Error 400 from YouTube), probably invalid input parameters
    #[error("Bad request. Reason: {0}")]
    BadRequest(Cow<'static, str>),
    /// Error deserializing YouTube's response JSON
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    /// YouTube returned invalid data
    #[error("got invalid data from YT: {0}")]
    InvalidData(Cow<'static, str>),
    /// YouTube returned data that does not match the queried ID
    ///
    /// Specifically YouTube may return this video <https://www.youtube.com/watch?v=aQvGIIdgFDM>,
    /// which is a 5 minute error message, instead of the requested video when using an outdated
    /// Android client.
    #[error("got wrong result from YT: {0}")]
    WrongResult(String),
    /// YouTube redirects you to another content ID
    ///
    /// This is used internally for YouTube Music channels that link to a main channel.
    #[error("redirecting to: {0}")]
    Redirect(String),
    /// Warnings occurred during deserialization/mapping
    ///
    /// This error is only returned in strict mode.
    #[error("Warnings during deserialization/mapping")]
    DeserializationWarnings,
}

impl ExtractionError {
    pub(crate) fn should_report(&self) -> bool {
        matches!(
            self,
            ExtractionError::Deserialization(_)
                | ExtractionError::InvalidData(_)
                | ExtractionError::WrongResult(_)
        )
    }

    pub(crate) fn switch_client(&self) -> bool {
        matches!(
            self,
            ExtractionError::VideoClientUnsupported(_)
                | ExtractionError::VideoAgeRestricted
                | ExtractionError::WrongResult(_)
        )
    }
}
