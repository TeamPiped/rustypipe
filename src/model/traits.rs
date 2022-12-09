//! Traits for working with response models

use std::ops::Range;

pub use super::{convert::FromYtItem, ordering::QualityOrd};

use super::{AudioFormat, AudioStream, VideoFormat, VideoStream};

/// Trait for YouTube streams (video and audio)
pub trait YtStream {
    /// Stream URL
    fn url(&self) -> &str;
    /// YouTube stream format identifier
    fn itag(&self) -> u32;
    /// Stream bitrate (in bits/second)
    fn bitrate(&self) -> u32;
    /// Average stream bitrate (in bits/second)
    fn averate_bitrate(&self) -> u32;
    /// File size in bytes
    fn size(&self) -> Option<u64>;
    /// Index range (used for DASH streaming)
    fn index_range(&self) -> Option<Range<u32>>;
    /// Init range (used for DASH streaming)
    fn init_range(&self) -> Option<Range<u32>>;
    /// Stream duration in milliseconds
    fn duration_ms(&self) -> Option<u32>;
    /// MIME file type
    fn mime(&self) -> &str;
}

impl YtStream for VideoStream {
    fn url(&self) -> &str {
        &self.url
    }

    fn itag(&self) -> u32 {
        self.itag
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn averate_bitrate(&self) -> u32 {
        self.average_bitrate
    }

    fn size(&self) -> Option<u64> {
        self.size
    }

    fn index_range(&self) -> Option<Range<u32>> {
        self.index_range.clone()
    }

    fn init_range(&self) -> Option<Range<u32>> {
        self.init_range.clone()
    }

    fn duration_ms(&self) -> Option<u32> {
        self.duration_ms
    }

    fn mime(&self) -> &str {
        &self.mime
    }
}

impl YtStream for AudioStream {
    fn url(&self) -> &str {
        &self.url
    }

    fn itag(&self) -> u32 {
        self.itag
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn averate_bitrate(&self) -> u32 {
        self.average_bitrate
    }

    fn size(&self) -> Option<u64> {
        Some(self.size)
    }

    fn index_range(&self) -> Option<Range<u32>> {
        self.index_range.clone()
    }

    fn init_range(&self) -> Option<Range<u32>> {
        self.init_range.clone()
    }

    fn duration_ms(&self) -> Option<u32> {
        self.duration_ms
    }

    fn mime(&self) -> &str {
        &self.mime
    }
}

/// Trait for file types
pub trait FileFormat {
    /// Get the file extension (".xyz") of the file format
    fn extension(&self) -> &str;
}

impl FileFormat for VideoFormat {
    fn extension(&self) -> &str {
        match self {
            VideoFormat::ThreeGp => ".3gp",
            VideoFormat::Mp4 => ".mp4",
            VideoFormat::Webm => ".webm",
        }
    }
}

impl FileFormat for AudioFormat {
    fn extension(&self) -> &str {
        match self {
            AudioFormat::M4a => ".m4a",
            AudioFormat::Webm => ".webm",
        }
    }
}
