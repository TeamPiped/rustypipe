use std::fmt::{Debug, Display};

use crate::proto::video_streaming::SabrError;

/// Errors that can occur while using the SABR client.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An error was returned by the SABR server.
    SabrError {
        /// Numeric error code from the server.
        code: i32,
        /// Human-readable error type from the server.
        type_: String,
    },
    /// The stream requires the client to provide a fresh PO token
    /// (attestation) before continuing.
    AttestationRequired,
    /// The UMP data was malformed.
    InvalidData,
    /// A media segment's content length did not match the header.
    ContentLengthMismatch,
    /// Received a media header for a different video.
    HeaderMismatch,
    /// An HTTP request failed.
    Http(wreq::Error),
    /// An HTTP request returned a non-success status.
    Status {
        /// The HTTP status code returned by the server.
        code: u16,
        /// The body of the error response.
        body: String,
    },
    /// Protobuf (de)serialization failed.
    Protobuf(prost::DecodeError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SabrError { code, type_ } => write!(f, "SABR error ({code}): {type_}"),
            Error::AttestationRequired => f.write_str("stream requires attestation"),
            Error::InvalidData => f.write_str("received invalid UMP data"),
            Error::ContentLengthMismatch => f.write_str(
                "expected content length does not match actual content length",
            ),
            Error::HeaderMismatch => f.write_str("unexpected media header"),
            Error::Http(e) => Debug::fmt(e, f),
            Error::Status { code, body } => {
                if body.is_empty() {
                    write!(f, "HTTP {code}")
                } else {
                    write!(f, "HTTP {code}: {body}")
                }
            }
            Error::Protobuf(e) => Debug::fmt(e, f),
        }
    }
}

impl std::error::Error for Error {}

impl From<SabrError> for Error {
    fn from(mut value: SabrError) -> Self {
        Self::SabrError {
            code: value.code.unwrap_or_default(),
            type_: value.r#type.take().unwrap_or_default(),
        }
    }
}

impl From<prost::DecodeError> for Error {
    fn from(value: prost::DecodeError) -> Self {
        Self::Protobuf(value)
    }
}

impl From<wreq::Error> for Error {
    fn from(value: wreq::Error) -> Self {
        Self::Http(value)
    }
}
