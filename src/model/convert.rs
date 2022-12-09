use super::{
    AlbumItem, ArtistItem, Channel, ChannelId, ChannelItem, ChannelTag, MusicItem,
    MusicPlaylistItem, PlaylistItem, TrackItem, VideoItem, YouTubeItem,
};

/// Trait for casting generic YouTube/YouTube music items to a specific kind.
///
/// Returns [`None`] if the item does not match.
pub trait FromYtItem: Sized {
    /// Casting from a generic YouTube item to a specific kind
    ///
    /// Returns [`None`] if the item does not match.
    fn from_yt_item(_item: YouTubeItem) -> Option<Self> {
        None
    }
    /// Casting from a generic YouTube Music item to a specific kind
    ///
    /// Returns [`None`] if the item does not match.
    fn from_ytm_item(_item: MusicItem) -> Option<Self> {
        None
    }
}

impl FromYtItem for YouTubeItem {
    fn from_yt_item(item: YouTubeItem) -> Option<Self> {
        Some(item)
    }
}

impl FromYtItem for VideoItem {
    fn from_yt_item(item: YouTubeItem) -> Option<Self> {
        match item {
            YouTubeItem::Video(video) => Some(video),
            _ => None,
        }
    }
}

impl FromYtItem for PlaylistItem {
    fn from_yt_item(item: YouTubeItem) -> Option<Self> {
        match item {
            YouTubeItem::Playlist(playlist) => Some(playlist),
            _ => None,
        }
    }
}

impl FromYtItem for ChannelItem {
    fn from_yt_item(item: YouTubeItem) -> Option<Self> {
        match item {
            YouTubeItem::Channel(channel) => Some(channel),
            _ => None,
        }
    }
}

impl FromYtItem for MusicItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        Some(item)
    }
}

impl FromYtItem for TrackItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        match item {
            MusicItem::Track(track) => Some(track),
            _ => None,
        }
    }
}

impl FromYtItem for AlbumItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        match item {
            MusicItem::Album(album) => Some(album),
            _ => None,
        }
    }
}

impl FromYtItem for ArtistItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        match item {
            MusicItem::Artist(artist) => Some(artist),
            _ => None,
        }
    }
}

impl FromYtItem for MusicPlaylistItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        match item {
            MusicItem::Playlist(playlist) => Some(playlist),
            _ => None,
        }
    }
}

impl<T> From<Channel<T>> for ChannelTag {
    fn from(channel: Channel<T>) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
            avatar: channel.avatar,
            verification: channel.verification,
            subscriber_count: channel.subscriber_count,
        }
    }
}

impl From<ChannelTag> for ChannelId {
    fn from(channel: ChannelTag) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
        }
    }
}

impl<T> From<Channel<T>> for ChannelId {
    fn from(channel: Channel<T>) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
        }
    }
}
