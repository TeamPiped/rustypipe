use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
pub(crate) struct ChannelRss {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub title: String,
    pub author: Author,
    #[serde(rename = "published", with = "time::serde::rfc3339")]
    pub create_date: OffsetDateTime,
    #[serde(default)]
    pub entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Entry {
    #[serde(rename = "videoId")]
    pub video_id: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub title: String,
    #[serde(with = "time::serde::rfc3339")]
    pub published: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    #[serde(rename = "group")]
    pub media_group: MediaGroup,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MediaGroup {
    pub thumbnail: Thumbnail,
    pub description: String,
    pub community: Community,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Thumbnail {
    #[serde(rename = "@url")]
    pub url: String,
    #[serde(rename = "@width")]
    pub width: u32,
    #[serde(rename = "@height")]
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Community {
    #[serde(rename = "starRating")]
    pub rating: Rating,
    pub statistics: Statistics,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Rating {
    #[serde(rename = "@count")]
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Statistics {
    #[serde(rename = "@views")]
    pub views: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Author {
    pub uri: String,
}

impl From<Thumbnail> for crate::model::Thumbnail {
    fn from(tn: Thumbnail) -> Self {
        crate::model::Thumbnail {
            url: tn.url,
            width: tn.width,
            height: tn.height,
        }
    }
}
