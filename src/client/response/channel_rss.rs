use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::Thumbnail;

#[derive(Clone, Debug, Deserialize)]
pub struct ChannelRss {
    #[serde(rename = "$unflatten=yt:channelId")]
    pub channel_id: String,
    #[serde(rename = "$unflatten=title")]
    pub title: String,
    #[serde(rename = "$unflatten=published")]
    pub create_date: DateTime<Utc>,
    pub entry: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    #[serde(rename = "$unflatten=yt:videoId")]
    pub video_id: String,
    #[serde(rename = "$unflatten=title")]
    pub title: String,
    #[serde(rename = "$unflatten=published")]
    pub published: DateTime<Utc>,
    #[serde(rename = "$unflatten=updated")]
    pub updated: DateTime<Utc>,
    #[serde(rename = "$unflatten=media:group")]
    pub media_group: MediaGroup,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MediaGroup {
    #[serde(rename = "$unflatten=media:thumbnail")]
    pub thumbnail: Thumbnail,
    #[serde(rename = "$unflatten=media:description")]
    pub description: String,
    #[serde(rename = "$unflatten=media:community")]
    pub community: Community,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Community {
    #[serde(rename = "$unflatten=media:starRating")]
    pub rating: Rating,
    #[serde(rename = "$unflatten=media:statistics")]
    pub statistics: Statistics,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rating {
    pub count: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Statistics {
    pub views: u64,
}

impl From<ChannelRss> for crate::model::ChannelRss {
    fn from(feed: ChannelRss) -> Self {
        Self {
            id: feed.channel_id,
            name: feed.title,
            videos: feed
                .entry
                .into_iter()
                .map(|item| crate::model::ChannelRssVideo {
                    id: item.video_id,
                    title: item.title,
                    description: item.media_group.description,
                    thumbnail: item.media_group.thumbnail.into(),
                    publish_date: item.published,
                    update_date: item.updated,
                    view_count: item.media_group.community.statistics.views,
                    like_count: item.media_group.community.rating.count,
                })
                .collect(),
            create_date: feed.create_date,
        }
    }
}
