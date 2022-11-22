use serde::Deserialize;
use time::OffsetDateTime;

use crate::util;

use super::Thumbnail;

#[derive(Debug, Deserialize)]
pub(crate) struct ChannelRss {
    #[serde(rename = "$unflatten=yt:channelId")]
    pub channel_id: String,
    #[serde(rename = "$unflatten=title")]
    pub title: String,
    pub author: Author,
    #[serde(rename = "$unflatten=published", with = "time::serde::rfc3339")]
    pub create_date: OffsetDateTime,
    pub entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Entry {
    #[serde(rename = "$unflatten=yt:videoId")]
    pub video_id: String,
    #[serde(rename = "$unflatten=yt:channelId")]
    pub channel_id: String,
    #[serde(rename = "$unflatten=title")]
    pub title: String,
    #[serde(rename = "$unflatten=published", with = "time::serde::rfc3339")]
    pub published: OffsetDateTime,
    #[serde(rename = "$unflatten=updated", with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    #[serde(rename = "$unflatten=media:group")]
    pub media_group: MediaGroup,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MediaGroup {
    #[serde(rename = "$unflatten=media:thumbnail")]
    pub thumbnail: Thumbnail,
    #[serde(rename = "$unflatten=media:description")]
    pub description: String,
    #[serde(rename = "$unflatten=media:community")]
    pub community: Community,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Community {
    #[serde(rename = "$unflatten=media:starRating")]
    pub rating: Rating,
    #[serde(rename = "$unflatten=media:statistics")]
    pub statistics: Statistics,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Rating {
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Statistics {
    pub views: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Author {
    pub uri: String,
}

impl From<ChannelRss> for crate::model::ChannelRss {
    fn from(feed: ChannelRss) -> Self {
        let id = if feed.channel_id.is_empty() {
            feed.entry
                .iter()
                .find_map(|entry| {
                    if !entry.channel_id.is_empty() {
                        Some(entry.channel_id.to_owned())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    feed.author
                        .uri
                        .strip_prefix("https://www.youtube.com/channel/")
                        .and_then(|id| {
                            if util::CHANNEL_ID_REGEX.is_match(id).unwrap_or_default() {
                                Some(id.to_owned())
                            } else {
                                None
                            }
                        })
                })
                .unwrap_or_default()
        } else {
            feed.channel_id
        };

        Self {
            id,
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
