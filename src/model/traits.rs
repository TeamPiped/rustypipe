//! Traits for working with response models

use std::ops::Range;

pub use super::{convert::FromYtItem, ordering::QualityOrd};

use super::*;

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

/// Trait for YouTube entities (Videos, Channels, Playlists)
pub trait YtEntity {
    /// ID
    fn id(&self) -> &str;
    /// Name
    fn name(&self) -> &str;
    /// Channel id
    ///
    /// `None` if the entity does not belong to a channel
    fn channel_id(&self) -> Option<&str>;
    /// Channel name
    ///
    /// `None` if the entity does not belong to a channel
    fn channel_name(&self) -> Option<&str>;
}

macro_rules! yt_entity {
    ($entity_type:ty) => {
        impl YtEntity for $entity_type {
            fn id(&self) -> &str {
                &self.id
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn channel_id(&self) -> Option<&str> {
                None
            }

            fn channel_name(&self) -> Option<&str> {
                None
            }
        }
    };
}

macro_rules! yt_entity_owner {
    ($entity_type:ty) => {
        impl YtEntity for $entity_type {
            fn id(&self) -> &str {
                &self.id
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn channel_id(&self) -> Option<&str> {
                Some(&self.channel.id)
            }

            fn channel_name(&self) -> Option<&str> {
                Some(&self.channel.name)
            }
        }
    };
}

macro_rules! yt_entity_owner_opt {
    ($entity_type:ty) => {
        impl YtEntity for $entity_type {
            fn id(&self) -> &str {
                &self.id
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn channel_id(&self) -> Option<&str> {
                self.channel.as_ref().map(|c| c.id.as_str())
            }

            fn channel_name(&self) -> Option<&str> {
                self.channel.as_ref().map(|c| c.name.as_str())
            }
        }
    };
}

macro_rules! yt_entity_owner_music {
    ($entity_type:ty) => {
        impl YtEntity for $entity_type {
            fn id(&self) -> &str {
                &self.id
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn channel_id(&self) -> Option<&str> {
                self.artists.first().and_then(|a| a.id.as_deref())
            }

            fn channel_name(&self) -> Option<&str> {
                self.artists.first().map(|a| a.name.as_str())
            }
        }
    };
}

impl YtEntity for VideoPlayer {
    fn id(&self) -> &str {
        &self.details.id
    }

    fn name(&self) -> &str {
        &self.details.name
    }

    fn channel_id(&self) -> Option<&str> {
        Some(&self.details.channel.id)
    }

    fn channel_name(&self) -> Option<&str> {
        Some(&self.details.channel.name)
    }
}

impl<T> YtEntity for Channel<T> {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> Option<&str> {
        None
    }

    fn channel_name(&self) -> Option<&str> {
        None
    }
}

yt_entity_owner! {VideoPlayerDetails}
yt_entity_owner_opt! {Playlist}
yt_entity! {ChannelId}
yt_entity_owner! {VideoDetails}
yt_entity! {ChannelTag}
yt_entity! {ChannelRss}
yt_entity! {ChannelRssVideo}
yt_entity_owner_opt! {VideoItem}
yt_entity! {ChannelItem}
yt_entity_owner_opt! {PlaylistItem}
yt_entity! {VideoId}
yt_entity_owner_music! {TrackItem}
yt_entity! {ArtistItem}
yt_entity_owner_music! {AlbumItem}
yt_entity_owner_opt! {MusicPlaylistItem}
yt_entity! {AlbumId}
yt_entity_owner_opt! {MusicPlaylist}
yt_entity_owner_music! {MusicAlbum}
yt_entity! {MusicArtist}
yt_entity! {MusicGenreItem}
yt_entity! {MusicGenre}
