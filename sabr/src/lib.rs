mod bytes;
mod error;
mod stream;
mod ump;

pub mod proto {
    pub mod misc {
        include!(concat!(env!("OUT_DIR"), "/misc.rs"));
    }
    pub mod video_streaming {
        include!(concat!(env!("OUT_DIR"), "/video_streaming.rs"));
    }
}

pub use bytes::Bytes;
pub use error::Error;
pub use proto::misc::FormatId;
pub use stream::{Segment, Stream};

/// A [`Result`] alias where the `Err` case is [`crate::Error`].
pub type Result<T> = core::result::Result<T, Error>;
