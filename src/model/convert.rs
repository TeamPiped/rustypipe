use super::{Channel, ChannelId, ChannelItem, ChannelTag, PlaylistItem, VideoItem, YouTubeItem};

impl TryFrom<YouTubeItem> for VideoItem {
    type Error = ();

    fn try_from(value: YouTubeItem) -> Result<Self, Self::Error> {
        match value {
            YouTubeItem::Video(video) => Ok(video),
            _ => Err(()),
        }
    }
}

impl TryFrom<YouTubeItem> for PlaylistItem {
    type Error = ();

    fn try_from(value: YouTubeItem) -> Result<Self, Self::Error> {
        match value {
            YouTubeItem::Playlist(playlist) => Ok(playlist),
            _ => Err(()),
        }
    }
}

impl TryFrom<YouTubeItem> for ChannelItem {
    type Error = ();

    fn try_from(value: YouTubeItem) -> Result<Self, Self::Error> {
        match value {
            YouTubeItem::Channel(channel) => Ok(channel),
            _ => Err(()),
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
