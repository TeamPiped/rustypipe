use super::{
    AlbumItem, ArtistId, ArtistItem, Channel, ChannelId, ChannelItem, ChannelRssVideo, ChannelTag,
    MusicArtist, MusicItem, MusicPlaylistItem, PlaylistItem, TrackItem, UserItem, VideoId,
    VideoItem, YouTubeItem,
};

/// Trait for casting generic YouTube/YouTube Music items to a specific kind.
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

impl From<VideoItem> for YouTubeItem {
    fn from(value: VideoItem) -> Self {
        Self::Video(value)
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

impl From<PlaylistItem> for YouTubeItem {
    fn from(value: PlaylistItem) -> Self {
        Self::Playlist(value)
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

impl From<ChannelItem> for YouTubeItem {
    fn from(value: ChannelItem) -> Self {
        Self::Channel(value)
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

impl From<TrackItem> for MusicItem {
    fn from(value: TrackItem) -> Self {
        Self::Track(value)
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

impl From<AlbumItem> for MusicItem {
    fn from(value: AlbumItem) -> Self {
        Self::Album(value)
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

impl From<ArtistItem> for MusicItem {
    fn from(value: ArtistItem) -> Self {
        Self::Artist(value)
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

impl From<MusicPlaylistItem> for MusicItem {
    fn from(value: MusicPlaylistItem) -> Self {
        Self::Playlist(value)
    }
}

impl FromYtItem for UserItem {
    fn from_ytm_item(item: MusicItem) -> Option<Self> {
        match item {
            MusicItem::User(user) => Some(user),
            _ => None,
        }
    }
}

impl From<UserItem> for MusicItem {
    fn from(value: UserItem) -> Self {
        Self::User(value)
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

impl From<MusicArtist> for ChannelId {
    fn from(artist: MusicArtist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
        }
    }
}

impl TryFrom<ArtistId> for ChannelId {
    type Error = ();

    fn try_from(artist: ArtistId) -> Result<Self, Self::Error> {
        match artist.id {
            Some(id) => Ok(Self {
                id,
                name: artist.name,
            }),
            None => Err(()),
        }
    }
}

impl From<VideoItem> for VideoId {
    fn from(video: VideoItem) -> Self {
        Self {
            id: video.id,
            name: video.name,
        }
    }
}

impl From<ChannelRssVideo> for VideoId {
    fn from(video: ChannelRssVideo) -> Self {
        Self {
            id: video.id,
            name: video.name,
        }
    }
}

impl From<TrackItem> for VideoId {
    fn from(track: TrackItem) -> Self {
        Self {
            id: track.id,
            name: track.name,
        }
    }
}
