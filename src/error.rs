//! RustyPipe error types

use std::{borrow::Cow, fmt::Display};

use reqwest::StatusCode;

/// Error type for the RustyPipe library
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error extracting content from YouTube
    #[error("extraction error: {0}")]
    Extraction(#[from] ExtractionError),
    /// Error from the HTTP client
    #[error("http error: {0}")]
    Http(Cow<'static, str>),
    /// Erroneous HTTP status code received
    #[error("http status code: {0} message: {1}")]
    HttpStatus(u16, Cow<'static, str>),
    /// Authentication error
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    /// Unspecified error
    #[error("error: {0}")]
    Other(Cow<'static, str>),
}

/// Error extracting content from YouTube
#[derive(thiserror::Error, Debug)]
pub enum ExtractionError {
    /// Content cannot be extracted with RustyPipe
    ///
    /// Reasons include:
    /// - Deletion/Censorship
    /// - Age restriction
    /// - Private video
    /// - DRM (Movies and TV shows)
    #[error("content unavailable ({reason}). Reason (from YT): {msg}")]
    Unavailable {
        /// Reason why the video could not be extracted
        reason: UnavailabilityReason,
        /// The error message as returned from YouTube
        msg: String,
    },
    /// Content with the given ID does not exist
    #[error("content `{id}` was not found ({msg})")]
    NotFound {
        /// ID of the requested content
        id: String,
        /// Error message
        msg: Cow<'static, str>,
    },
    /// Bad request (Error 400 from YouTube), probably invalid input parameters
    #[error("bad request ({0})")]
    BadRequest(Cow<'static, str>),
    /// YouTube returned data that could not be deserialized or parsed
    #[error("invalid data from YT: {0}")]
    InvalidData(Cow<'static, str>),
    /// Error deobfuscating YouTube's URL signatures
    #[error("deobfuscation error: {0}")]
    Deobfuscation(Cow<'static, str>),
    /// Error generating Botguard tokens
    #[error("botguard error: {0}")]
    Botguard(Cow<'static, str>),
    /// YouTube returned data that does not match the queried ID
    ///
    /// Specifically YouTube may return this video <https://www.youtube.com/watch?v=aQvGIIdgFDM>,
    /// which is a 5 minute error message, instead of the requested video when using an outdated
    /// Android client.
    #[error("wrong result from YT: {0}")]
    WrongResult(String),
    /// YouTube redirects you to another content ID
    ///
    /// This is used internally for YouTube Music channels that link to a main channel.
    #[error("redirecting to: {0}")]
    Redirect(String),
    /// Warnings occurred during deserialization/mapping
    ///
    /// This error is only returned in strict mode.
    #[error("warnings during deserialization/mapping")]
    DeserializationWarnings,
}

/// Reason why a video cannot be extracted
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnavailabilityReason {
    /// Video/Channel is age restricted.
    AgeRestricted,
    /// Video was deleted or censored
    Deleted,
    /// Video is not available in your country
    Geoblocked,
    /// Video cannot be extracted with the specified client
    UnsupportedClient,
    /// Video is private
    Private,
    /// Video needs to be purchased and is protected by digital restrictions management
    /// (e.g. movies and TV shows)
    Paid,
    /// Video is only available to YouTube Premium users
    Premium,
    /// Video is only available to channel members
    MembersOnly,
    /// Livestream has gone offline
    OfflineLivestream,
    /// YouTube banned your IP address from accessing the platform without an account
    IpBan,
    /// YouTube bans IP addresses from certain VPN providers from accessing certain geo-restricted
    /// videos.
    ///
    /// If this happens to you, you can try another server / VPN provider or disable your VPN.
    VpnBan,
    /// YouTube requires the user to solve a ReCaptcha
    Captcha,
    /// Video temporarily unavailable (rate limit)
    TryAgain,
    /// Video cant be played for other reasons
    #[default]
    Unplayable,
}

impl Display for UnavailabilityReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnavailabilityReason::AgeRestricted => f.write_str("age-restricted"),
            UnavailabilityReason::Deleted => f.write_str("deleted"),
            UnavailabilityReason::Geoblocked => f.write_str("geoblocked"),
            UnavailabilityReason::UnsupportedClient => f.write_str("unsupported by client"),
            UnavailabilityReason::Private => f.write_str("private"),
            UnavailabilityReason::Paid => f.write_str("paid"),
            UnavailabilityReason::Premium => f.write_str("premium-only"),
            UnavailabilityReason::MembersOnly => f.write_str("members-only"),
            UnavailabilityReason::OfflineLivestream => f.write_str("offline stream"),
            UnavailabilityReason::IpBan => f.write_str("ip-ban"),
            UnavailabilityReason::VpnBan => f.write_str("vpn-ban"),
            UnavailabilityReason::Captcha => f.write_str("captcha"),
            UnavailabilityReason::TryAgain => f.write_str("try again"),
            UnavailabilityReason::Unplayable => f.write_str("unplayable"),
        }
    }
}

/// Error authenticating a YouTube user
#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    /// No user is logged in
    #[error("you are not logged in")]
    NoLogin,
    /// The device code for user login has expired.
    ///
    /// Generate a new device code and try again
    #[error("device code expired; try again")]
    DeviceCodeExpired,
    /// The access token could not be refreshed
    #[error("error refreshing token: {0}; log in again")]
    Refresh(String),
    /// Unhandled OAuth error
    #[error("unhandled OAuth error: {0}")]
    Other(String),
}

pub(crate) mod internal {
    use std::borrow::Cow;

    use super::{Error, ExtractionError};

    /// Error that occurred during the initialization
    /// or use of the YouTube URL signature deobfuscator.
    #[derive(thiserror::Error, Debug)]
    pub enum DeobfError {
        /// Error during JavaScript execution
        #[error("js execution error: {0}")]
        JavaScript(#[from] rquickjs::Error),
        /// Error during JavaScript parsing
        #[error("js parsing: {0}")]
        JsParser(#[from] ress::error::Error),
        /// Could not extract certain data
        #[error("could not extract {0}")]
        Extraction(&'static str),
        /// Unspecified error
        #[error("error: {0}")]
        Other(Cow<'static, str>),
    }

    impl From<DeobfError> for Error {
        fn from(value: DeobfError) -> Self {
            Self::Extraction(value.into())
        }
    }

    impl From<DeobfError> for ExtractionError {
        fn from(value: DeobfError) -> Self {
            Self::Deobfuscation(value.to_string().into())
        }
    }
}

impl From<serde_json::Error> for ExtractionError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidData(value.to_string().into())
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        if value.is_status() {
            if let Some(status) = value.status() {
                return Self::HttpStatus(status.as_u16(), Cow::default());
            }
        }
        Self::Http(value.to_string().into())
    }
}

impl From<serde_plain::Error> for Error {
    fn from(value: serde_plain::Error) -> Self {
        Self::Other(value.to_string().into())
    }
}

impl Error {
    /// Return true if a report should be generated
    pub(crate) fn should_report(&self) -> bool {
        matches!(
            self,
            Self::HttpStatus(_, _)
                | Self::Extraction(
                    ExtractionError::InvalidData(_) | ExtractionError::WrongResult(_)
                )
        )
    }

    /// Return true if the request should be retried
    pub(crate) fn should_retry(&self) -> bool {
        match self {
            Self::HttpStatus(code, _) => match StatusCode::try_from(*code) {
                Ok(status) => status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS,
                Err(_) => false,
            },
            Self::Extraction(
                ExtractionError::InvalidData(_)
                | ExtractionError::Unavailable {
                    reason: UnavailabilityReason::TryAgain,
                    ..
                },
            ) => true,
            _ => false,
        }
    }
}

impl ExtractionError {
    /// Return true if the video should be fetched with a different client
    pub(crate) fn switch_client(&self) -> bool {
        matches!(
            self,
            ExtractionError::Unavailable {
                reason: UnavailabilityReason::UnsupportedClient | UnavailabilityReason::TryAgain,
                ..
            } | ExtractionError::WrongResult(_)
                | ExtractionError::Botguard(_)
        )
    }

    /// Return true if the video should be fetched as a logged in user
    pub(crate) fn use_login(&self) -> bool {
        matches!(
            self,
            ExtractionError::Unavailable {
                reason: UnavailabilityReason::AgeRestricted | UnavailabilityReason::IpBan,
                ..
            }
        )
    }
}
