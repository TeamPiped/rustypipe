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
    /// YTM item type
    fn music_item_type(&self) -> Option<MusicItemType>;
}

macro_rules! yt_entity {
    ($entity_type:ty, $music_item_type:expr) => {
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

            fn music_item_type(&self) -> Option<MusicItemType> {
                $music_item_type
            }
        }
    };
}

macro_rules! yt_entity_owner {
    ($entity_type:ty, $music_item_type:expr) => {
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

            fn music_item_type(&self) -> Option<MusicItemType> {
                Some($music_item_type)
            }
        }
    };
}

macro_rules! yt_entity_owner_opt {
    ($entity_type:ty, $music_item_type:expr) => {
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

            fn music_item_type(&self) -> Option<MusicItemType> {
                Some($music_item_type)
            }
        }
    };
}

macro_rules! yt_entity_owner_music {
    ($entity_type:ty, $music_item_type:expr) => {
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
                if self.by_va {
                    Some(crate::util::VARIOUS_ARTISTS)
                } else {
                    self.artists.first().map(|a| a.name.as_str())
                }
            }

            fn music_item_type(&self) -> Option<MusicItemType> {
                Some($music_item_type)
            }
        }
    };
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

    fn music_item_type(&self) -> Option<MusicItemType> {
        Some(MusicItemType::User)
    }
}

impl YtEntity for YouTubeItem {
    fn id(&self) -> &str {
        match self {
            YouTubeItem::Video(v) => &v.id,
            YouTubeItem::Playlist(p) => &p.id,
            YouTubeItem::Channel(c) => &c.id,
        }
    }

    fn name(&self) -> &str {
        match self {
            YouTubeItem::Video(v) => &v.name,
            YouTubeItem::Playlist(p) => &p.name,
            YouTubeItem::Channel(c) => &c.name,
        }
    }

    fn channel_id(&self) -> Option<&str> {
        match self {
            YouTubeItem::Video(v) => v.channel_id(),
            YouTubeItem::Playlist(p) => p.channel_id(),
            YouTubeItem::Channel(_) => None,
        }
    }

    fn channel_name(&self) -> Option<&str> {
        match self {
            YouTubeItem::Video(v) => v.channel_name(),
            YouTubeItem::Playlist(p) => p.channel_name(),
            YouTubeItem::Channel(_) => None,
        }
    }

    fn music_item_type(&self) -> Option<MusicItemType> {
        Some(match self {
            YouTubeItem::Video(_) => MusicItemType::Track,
            YouTubeItem::Playlist(_) => MusicItemType::Playlist,
            YouTubeItem::Channel(_) => MusicItemType::User,
        })
    }
}

impl YtEntity for MusicItem {
    fn id(&self) -> &str {
        match self {
            MusicItem::Track(t) => &t.id,
            MusicItem::Album(b) => &b.id,
            MusicItem::Artist(a) => &a.id,
            MusicItem::Playlist(p) => &p.id,
            MusicItem::User(u) => &u.id,
        }
    }

    fn name(&self) -> &str {
        match self {
            MusicItem::Track(t) => &t.name,
            MusicItem::Album(b) => &b.name,
            MusicItem::Artist(a) => &a.name,
            MusicItem::Playlist(p) => &p.name,
            MusicItem::User(u) => &u.name,
        }
    }

    fn channel_id(&self) -> Option<&str> {
        match self {
            MusicItem::Track(t) => t.channel_id(),
            MusicItem::Album(b) => b.channel_id(),
            MusicItem::Artist(_) | MusicItem::User(_) => None,
            MusicItem::Playlist(p) => p.channel_id(),
        }
    }

    fn channel_name(&self) -> Option<&str> {
        match self {
            MusicItem::Track(t) => t.channel_name(),
            MusicItem::Album(b) => b.channel_name(),
            MusicItem::Artist(_) | MusicItem::User(_) => None,
            MusicItem::Playlist(p) => p.channel_name(),
        }
    }

    fn music_item_type(&self) -> Option<MusicItemType> {
        Some(match self {
            MusicItem::Track(_) => MusicItemType::Track,
            MusicItem::Album(_) => MusicItemType::Album,
            MusicItem::Artist(_) => MusicItemType::Artist,
            MusicItem::Playlist(_) => MusicItemType::Playlist,
            MusicItem::User(_) => MusicItemType::User,
        })
    }
}

yt_entity_owner_opt! {Playlist, MusicItemType::Playlist}
yt_entity! {ChannelId, Some(MusicItemType::User)}
yt_entity_owner! {VideoDetails, MusicItemType::Track}
yt_entity! {ChannelTag, Some(MusicItemType::User)}
yt_entity! {ChannelRss, Some(MusicItemType::User)}
yt_entity! {ChannelRssVideo, Some(MusicItemType::Track)}
yt_entity_owner_opt! {VideoItem, MusicItemType::Track}
yt_entity! {ChannelItem, Some(MusicItemType::User)}
yt_entity_owner_opt! {PlaylistItem, MusicItemType::Playlist}
yt_entity! {VideoId, Some(MusicItemType::Track)}
yt_entity_owner_music! {TrackItem, MusicItemType::Track}
yt_entity! {ArtistItem, Some(MusicItemType::Artist)}
yt_entity_owner_music! {AlbumItem, MusicItemType::Album}
yt_entity_owner_opt! {MusicPlaylistItem, MusicItemType::Playlist}
yt_entity! {AlbumId, Some(MusicItemType::Album)}
yt_entity_owner_opt! {MusicPlaylist, MusicItemType::Playlist}
yt_entity_owner_music! {MusicAlbum, MusicItemType::Album}
yt_entity! {MusicArtist, Some(MusicItemType::Artist)}
yt_entity! {UserItem, Some(MusicItemType::User)}
yt_entity! {MusicGenreItem, None}
yt_entity! {MusicGenre, None}
